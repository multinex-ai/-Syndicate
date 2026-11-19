import {budgetStatus,infer} from '../private-inference/client.ts';
import {classifyRequest} from './tui-routing.mjs';

const TARGETS=new Set(['chat','cyber','creative','image','video','video-edit','speech','effects','music']);
const routeEnv={chat:'PRIVATE_INFERENCE_CHAT_ROUTE',cyber:'SYNDICATE_ROUTE',creative:'SYNDICATE_ROUTE',image:'PRIVATE_INFERENCE_IMAGE_ROUTE',video:'PRIVATE_INFERENCE_VIDEO_ROUTE','video-edit':'PRIVATE_INFERENCE_VIDEO_EDIT_ROUTE',speech:'PRIVATE_INFERENCE_SPEECH_ROUTE',effects:'PRIVATE_INFERENCE_EFFECTS_ROUTE',music:'PRIVATE_INFERENCE_MUSIC_ROUTE'};

function stagesFor(target,attached){
  if(target==='image')return [{target:'creative',purpose:'Rewrite the request into a precise positive and negative prompt'},{target:'image',purpose:attached?'Edit the attached image and return an image artifact':'Generate and return an image artifact'}];
  if(['video','video-edit','speech','effects','music'].includes(target))return [{target:'creative',purpose:`Rewrite the request for ${target}`},{target,purpose:`Generate and return the ${target} artifact`}];
  return [{target,purpose:target==='cyber'?'Analyze with the authorized security specialist':'Handle the request with the primary agent'}];
}

function deterministicPlan(prompt,role,attached){
  const target=attached?'image':classifyRequest(prompt);
  if(target)return {version:1,source:'deterministic',intent:target,stages:stagesFor(target,attached),terminalTarget:target};
  return {version:1,source:'role-default',intent:'general',stages:stagesFor(role==='cyber-lab'?'cyber':'chat',false),terminalTarget:role==='cyber-lab'?'cyber':'chat'};
}

function needsModelPlan(prompt){
  const text=String(prompt).toLowerCase();
  const families=[/\b(image|photo|picture|artwork)\b/,/\b(video|animation|footage|clip)\b/,/\b(song|music|lyrics|soundtrack)\b/,/\b(speech|voiceover|narration)\b/,/\b(sound effect|soundscape|foley|ambience)\b/].filter(rule=>rule.test(text)).length;
  return families>1||/\b(and then|then turn|after that|use the result|from that|convert .* into|transform .* into)\b/.test(text);
}

function mentionedTargets(prompt){
  const text=String(prompt).toLowerCase(),targets=[];
  if(/\b(image|photo|picture|artwork)\b/.test(text))targets.push('image');
  if(/\b(video|animation|footage|clip)\b/.test(text))targets.push(/\b(edit|modify|transform|restyle)\b/.test(text)?'video-edit':'video');
  if(/\b(song|music|lyrics|soundtrack)\b/.test(text))targets.push('music');
  if(/\b(speech|voiceover|narration)\b/.test(text))targets.push('speech');
  if(/\b(sound effect|soundscape|foley|ambience)\b/.test(text))targets.push('effects');
  return targets;
}

export function validateRoutePlan(value){
  if(!value||typeof value!=='object'||Array.isArray(value))throw new Error('Router returned no object');
  const stages=value.stages;
  if(!Array.isArray(stages)||stages.length<1||stages.length>5)throw new Error('Router stages are invalid');
  const clean=stages.map((stage,index)=>{
    if(!stage||typeof stage!=='object'||Array.isArray(stage)||!TARGETS.has(stage.target))throw new Error('Router selected an unknown target');
    const purpose=typeof stage.purpose==='string'?stage.purpose.trim():'';
    if(!purpose||purpose.length>240)throw new Error('Router purpose is invalid');
    return {target:stage.target,purpose,order:index+1};
  });
  const terminalTarget=clean.at(-1).target;
  if(terminalTarget==='creative')throw new Error('Creative rewriting cannot be a terminal artifact route');
  return {version:1,source:'ai',intent:typeof value.intent==='string'?value.intent.slice(0,80):terminalTarget,stages:clean,terminalTarget};
}

async function modelPlan(prompt,callInfer){
  const response=await callInfer('chat',{messages:[
    {role:'system',content:'You are a routing planner. Break the request into the smallest ordered model chain. Select only from chat, cyber, creative, image, video, video-edit, speech, effects, music. Creative rewrites prompts but never produces the final artifact. Return strict JSON only: {"intent":"short label","stages":[{"target":"allowed target","purpose":"short purpose"}]}. Use at most 5 stages. Do not include endpoint names, model names, policy, commentary, markdown, or executable commands.'},
    {role:'user',content:prompt}],temperature:0,max_tokens:500,response_format:{type:'json_object'}});
  const content=response?.result?.choices?.[0]?.message?.content;
  if(typeof content!=='string')throw new Error('Router returned no JSON');
  return validateRoutePlan(JSON.parse(content));
}

function qualification(plan,status,env){
  let estimatedReserveMicros=0;
  const allowFallback=env.LOCAL_AGENTS_ALLOW_LOCAL_FALLBACK!=='0';
  let localFallback=false;
  for(const stage of plan.stages){
    const routeKey=stage.target==='image'&&plan.stages.some(s=>s.purpose.includes('Edit the attached'))&&env.PRIVATE_INFERENCE_IMAGE_EDIT_ROUTE
      ? 'PRIVATE_INFERENCE_IMAGE_EDIT_ROUTE'
      : routeEnv[stage.target];
    const routeId=env[routeKey];
    const route=Array.isArray(status?.routes)?status.routes.find(item=>item?.id===routeId):null;
    const unavailable=!routeId||route?.qualified===false||route?.admissible===false;
    if(unavailable){
      if(stage.target==='creative'){
        continue;
      }
      if(stage.target==='image'&&allowFallback){
        localFallback=true;
        continue;
      }
      return {...plan,estimatedReserveMicros,executable:false,blockedReason:!routeId?`No private ${stage.target} route is configured`:route?.qualified===false?`The ${stage.target} route is deployed but not qualified`:`The ${stage.target} route is unavailable under the current budget or provider state`};
    }
    if(route&&Number.isSafeInteger(route.maxCostMicros))estimatedReserveMicros+=route.maxCostMicros;
  }
  const remaining=status?.local&&Number.isSafeInteger(status.local.remainingMicros)?status.local.remainingMicros:null;
  if(remaining!==null&&estimatedReserveMicros>remaining){
    if(plan.terminalTarget==='image'&&allowFallback){
      return {...plan,estimatedReserveMicros,executable:true,localFallback:true,forceLocal:true,blockedReason:null};
    }
    return {...plan,estimatedReserveMicros,executable:false,localFallback:false,forceLocal:false,blockedReason:'Multi-stage reservation exceeds remaining local budget allowance'};
  }
  return {...plan,estimatedReserveMicros,executable:true,localFallback,forceLocal:localFallback,blockedReason:null};
}

export async function planRoute(prompt,role,attached,env=process.env,deps={infer,budgetStatus}){
  let status=null;try{status=await deps.budgetStatus(env);}catch{/* Gateway remains the final enforcement point. */}
  let plan=deterministicPlan(prompt,role,attached);
  if(!attached&&needsModelPlan(prompt)){
    const plannerRoute=env.PRIVATE_INFERENCE_CHAT_ROUTE;
    const planner=Array.isArray(status?.routes)?status.routes.find(item=>item?.id===plannerRoute):null;
    const candidatesReady=mentionedTargets(prompt).every(target=>{
      const routeId=env[routeEnv[target]],route=Array.isArray(status?.routes)?status.routes.find(item=>item?.id===routeId):null;
      return !route||route.qualified!==false&&route.admissible!==false;
    });
    if(candidatesReady&&(!planner||planner.admissible!==false)){try{plan=await modelPlan(prompt,(target,input)=>deps.infer(target,input,env));}catch{/* Use the bounded deterministic plan. */}}
  }
  return qualification(plan,status,env);
}
