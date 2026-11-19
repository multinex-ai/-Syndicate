import fs from 'node:fs';
export function writeControls(state) {
  if(state.controlFile)fs.writeFileSync(state.controlFile,JSON.stringify({reasoningEnabled:state.reasoningEnabled,localFallbackEnabled:state.localFallbackEnabled===true}),{mode:0o600});
}
export function toggleFallback(state) {
  state.localFallbackEnabled=!state.localFallbackEnabled;
  writeControls(state);
  state.statusMessage=`Local fallback ${state.localFallbackEnabled?'ON':'OFF'} for subsequent routing attempts.`;
}
export function toggleReasoning(state) {
  state.reasoningEnabled=!state.reasoningEnabled;
  writeControls(state);
  state.statusMessage=`Reasoning ${state.reasoningEnabled?'ON':'OFF'}; display changes now, generation preference applies to the next inference call.`;
}
export function inferenceLines(state,render) {
  let pending='';
  function consume(line) {
    if(line.startsWith('INFERENCE_EVENT ')) {
      try {const event=JSON.parse(line.slice(16));
        if(!['status','reasoning','route'].includes(event.type)||typeof event.text!=='string')return;
        if(event.type==='route')state.modelName=event.text;
        state.traceLogs.push({type:event.type==='reasoning'?'REASONING':event.type==='route'?'ROUTE':'INFERENCE',text:event.text,time:new Date().toLocaleTimeString()});render();
      }catch{/* Malformed provider progress never becomes an instruction or a fatal UI error. */}
    } else if(line.trim()) {state.traceLogs.push({type:'RUN',text:line,time:new Date().toLocaleTimeString()});render();}
  }
  return {push(chunk){pending+=chunk;let n;while((n=pending.indexOf('\n'))>=0){consume(pending.slice(0,n));pending=pending.slice(n+1);}if(pending.length>64000){consume(pending.slice(0,64000));pending='';}},finish(){if(pending)consume(pending);pending='';}};
}
