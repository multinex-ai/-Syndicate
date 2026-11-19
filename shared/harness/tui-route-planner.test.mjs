import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {executionPlan} from './tui-routing.mjs';
import {planRoute,validateRoutePlan} from './tui-route-planner.mjs';

const env={PRIVATE_INFERENCE_CHAT_ROUTE:'chat-primary',SYNDICATE_ROUTE:'cyber-specialist',SYNDICATE_ROUTE:'chat-primary',PRIVATE_INFERENCE_IMAGE_ROUTE:'image-quality',PRIVATE_INFERENCE_VIDEO_ROUTE:'video-generation',PRIVATE_INFERENCE_VIDEO_EDIT_ROUTE:'video-edit',PRIVATE_INFERENCE_SPEECH_ROUTE:'speech-quality',PRIVATE_INFERENCE_EFFECTS_ROUTE:'sound-effects',PRIVATE_INFERENCE_MUSIC_ROUTE:'music-generation'};
const status={routes:[{id:'chat-primary',qualified:true,admissible:true},{id:'image-quality',qualified:true,admissible:true},{id:'video-generation',qualified:false,admissible:false},{id:'cyber-specialist',qualified:true,admissible:true}]};
const readyStatus={routes:status.routes.map(route=>route.id==='video-generation'?{...route,qualified:true,admissible:true}:route)};

test('typo-tolerant explicit image requests bypass the model planner',async()=>{
 let calls=0;const plan=await planRoute('generaete an image of a red robot','cyber-lab',null,env,{budgetStatus:async()=>status,infer:async()=>{calls++;throw new Error('unused');}});
 assert.equal(calls,0);assert.equal(plan.terminalTarget,'image');assert.deepEqual(plan.stages.map(stage=>stage.target),['creative','image']);assert.equal(plan.executable,true);
});

test('compound prompts accept a bounded structured model plan',async()=>{
 const plan=await planRoute('Create an image and then turn it into a video','coordinator',null,env,{budgetStatus:async()=>readyStatus,infer:async()=>({result:{choices:[{message:{content:JSON.stringify({intent:'image to video',stages:[{target:'creative',purpose:'Prepare both prompts'},{target:'image',purpose:'Create the source frame'},{target:'video',purpose:'Animate the source frame'}]})}}]}})});
 assert.deepEqual(plan.stages.map(stage=>stage.target),['creative','image','video']);assert.equal(plan.terminalTarget,'video');assert.equal(plan.executable,true);
});

test('compound planning does not spend on a planner when a named route is unqualified',async()=>{
 let calls=0;const plan=await planRoute('Create an image and then turn it into a video','coordinator',null,env,{budgetStatus:async()=>status,infer:async()=>{calls++;throw new Error('unused');}});
 assert.equal(calls,0);assert.equal(plan.executable,false);assert.match(plan.blockedReason,/not qualified/);
});

test('model plans cannot invent routes or terminate at the rewrite stage',()=>{
 assert.throws(()=>validateRoutePlan({stages:[{target:'shell',purpose:'escape'}]}),/unknown target/);
 assert.throws(()=>validateRoutePlan({stages:[{target:'creative',purpose:'rewrite'}]}),/cannot be a terminal/);
});

test('explicit security requests select cyber from coordinator, while general prompts select chat',async()=>{
 const chatPlan=await planRoute('Explain Rust async iterators','coordinator',null,env,{budgetStatus:async()=>status,infer:async()=>{throw new Error('unused');}});
 assert.equal(chatPlan.terminalTarget,'chat');
 const cyberPlan=await planRoute('Run a vulnerability audit on CVE-2026-1042 attack surface','coordinator',null,env,{budgetStatus:async()=>status,infer:async()=>{throw new Error('unused');}});
 assert.equal(cyberPlan.terminalTarget,'cyber');
});

test('cumulative multi-stage reserve exceeding remaining allowance steers image to local fallback instead of mid-pipeline 402',async()=>{
 const tightStatus={local:{remainingMicros:1200000},routes:[
   {id:'chat-primary',maxCostMicros:1000000,qualified:true,admissible:true},
   {id:'image-quality',maxCostMicros:500000,qualified:true,admissible:true},
 ]};
 const plan=await planRoute('Generate an image of a brass compass','coordinator',null,env,{budgetStatus:async()=>tightStatus,infer:async()=>{throw new Error('unused');}});
 assert.equal(plan.executable,true);
 assert.equal(plan.forceLocal,true);
});

test('media and TUI paths decode spaced and Unicode filesystem directories without percent-encoding',()=>{
 const plan=executionPlan('compose a synthwave soundtrack','coordinator','/tmp/run dir');
 assert.ok(!plan.args[0].includes('%20'),'media-client path must not contain %20');
 assert.ok(!plan.args[0].includes('%E2'),'media-client path must not contain %E2');
 assert.ok(fs.existsSync(plan.args[0]),`expected ${plan.args[0]} to exist on disk`);
 const tuiSource=fs.readFileSync(new URL('./tui.mjs',import.meta.url),'utf8');
 assert.ok(!tuiSource.includes('import.meta.url).pathname'),'tui.mjs must decode file URLs via fileURLToPath');
});
