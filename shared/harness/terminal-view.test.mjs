import {test} from 'node:test';
import assert from 'node:assert/strict';
import {createView} from './tui-view.mjs';
import {displayWidth,plainText} from './terminal-layout.ts';

test('rendered logs preserve full paths, stay within terminal width, and anchor history',()=>{
  const originalWrite=process.stdout.write, columns=process.stdout.columns, rows=process.stdout.rows;
  let frame='';
  process.stdout.write=(chunk)=>{frame+=chunk;return true;};
  const state={mode:'NORMAL',modelName:'Fixture model',role:'coordinator',contextSize:8192,
    budgetLine:'Fixture budget',splitRatio:.55,traceExpanded:false,activePane:'trace',
    chatScroll:0,traceScroll:0,inputBuffer:'',statusMessage:'Fixture',chatHistory:[],
    telemetry:{totalGiB:18,availableGiB:6,rssGiB:0,pressureLevel:1,lockState:'IDLE',tokensIn:0,tokensOut:0,contextUsed:0},
    traceLogs:[]};
  const path='/workspace/full/path/with/many/directories/and/a-very-long-filename.json';
  const {render}=createView(state,'/workspace');
  try {
    for(const width of [60,80,120,180]) {
      process.stdout.columns=width;process.stdout.rows=32;
      state.traceLogs=[{time:'12:00',type:'FILE',text:path}];state.traceScroll=0;frame='';render();
      const lines=plainText(frame).split('\n');
      assert(lines.every(line=>displayWidth(line)<=width),`overflow at ${width}`);
      const divider=Math.floor(width*state.splitRatio);
      const logText=lines.slice(8,28).map(line=>line.slice(divider+1,-1).trimEnd()).join('');
      assert(logText.includes(path),`missing path at ${width}`);
    }
    state.traceLogs=Array.from({length:50},(_,i)=>({time:'12:00',type:'LOG',text:`entry ${i}`}));
    render();state.traceScroll=10;render();const before=state.traceScroll;
    state.traceLogs.push({time:'12:00',type:'LOG',text:'new entry'});render();assert.equal(state.traceScroll,before+1);
    state.traceExpanded=true;state.traceScroll=0;state.traceLogs=[{time:'12:00',type:'FILE',text:path}];frame='';render();
    assert(plainText(frame).includes(path));
  } finally {process.stdout.write=originalWrite;process.stdout.columns=columns;process.stdout.rows=rows;}
});

test('reasoning toggle and fragmented inference events preserve routing and display state',async()=>{
 const {toggleReasoning,inferenceLines}=await import('./tui-reasoning.mjs');
 const state={reasoningEnabled:true,controlFile:null,traceLogs:[],modelName:'pending'};let renders=0;
 const lines=inferenceLines(state,()=>renders++);
 lines.push('INFERENCE_EVENT {"type":"rou');lines.push('te","text":"runpod · chat · fixture-model"}\n');
 lines.push('INFERENCE_EVENT {"type":"reasoning","text":"Model returned explanation."}\n');
 assert.equal(state.modelName,'runpod · chat · fixture-model');assert.equal(state.traceLogs.length,2);
 toggleReasoning(state);assert.equal(state.reasoningEnabled,false);assert(state.statusMessage.includes('next inference call'));
 toggleReasoning(state);assert.equal(state.reasoningEnabled,true);assert.equal(renders,2);
});

test('media screen distinguishes no artifacts and wraps long selected paths',async()=>{
 const {mediaView}=await import('./tui-media-view.mjs');
 assert(mediaView({mediaFiles:[],statusMessage:''},80,30).includes('No media discovered'));
 const filepath='/workspace/'+ 'long-directory/'.repeat(7)+'image.png';
 const output=plainText(mediaView({mediaFiles:[{path:filepath,bytes:123,kind:'image'}],mediaIndex:0,statusMessage:''},80,30));
 assert(output.includes('Selected 1 of 1'));assert(output.split('\n').every(line=>displayWidth(line)<=78));
 assert(output.replaceAll('\n','').includes(filepath));
});

test('fallback setting is opt-in and toggles independently of reasoning',async()=>{
 const {toggleFallback,toggleReasoning}=await import('./tui-reasoning.mjs');
 const state={localFallbackEnabled:false,reasoningEnabled:true,controlFile:null};
 toggleFallback(state);assert.equal(state.localFallbackEnabled,true);assert.equal(state.reasoningEnabled,true);
 toggleReasoning(state);assert.equal(state.localFallbackEnabled,true);
 toggleFallback(state);assert.equal(state.localFallbackEnabled,false);
});

test('explicit image generation bypasses the active agent role and selects the media chain',async()=>{
 const {classifyRequest,executionPlan}=await import('./tui-routing.mjs');
 assert.equal(classifyRequest('Generate an image of a red robot.'),'image');
 assert.equal(classifyRequest('generaete an image of a red robot.'),'image');
 assert.equal(classifyRequest('Can you genreate a picture of a red robot?'),'image');
 assert.equal(classifyRequest('Create a hyper realistic scene of a lunar city.'),'image');
 assert.equal(classifyRequest('Review the images in this folder.'),null);
 assert.equal(classifyRequest('Review this created image for defects.'),null);
 assert.equal(classifyRequest('Write a scene for the second act.'),null);
 const plan=executionPlan('Create a poster of a lunar city.','cyber-lab','/tmp/run');
 assert.equal(plan.label,'creative-director → image-studio');
 assert.deepEqual(plan.args,['image','Create a poster of a lunar city.','--output','/tmp/run/generated.png','--preset','quality']);
 const edit=executionPlan('Make the sky violet.','cyber-lab','/tmp/run','/workspace/source.png');
 assert.equal(edit.label,'creative-director → image-edit');
 assert.deepEqual(edit.args,['image','Make the sky violet.','--output','/tmp/run/generated.png','--preset','quality','--reference-image','/workspace/source.png']);
});

test('audio and video generation requests select their private media routes',async()=>{
 const {classifyRequest,executionPlan}=await import('./tui-routing.mjs');
 assert.equal(classifyRequest('Compose a song with these lyrics.'),'music');
 assert.equal(classifyRequest('Generate a cinematic video clip.'),'video');
 assert.equal(classifyRequest('Edit this video into a night scene.'),'video-edit');
 assert.equal(classifyRequest('Create a voiceover narration.'),'speech');
 assert.equal(classifyRequest('Create a rain soundscape.'),'effects');
 const plan=executionPlan('Compose a song with these lyrics.','cyber-lab','/tmp/run');
 assert.equal(plan.label,'creative-director → music');
 assert.equal(plan.args.at(-1),'/tmp/run/generated.wav');
});
