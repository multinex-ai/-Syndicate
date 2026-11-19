import {discoverMedia,mediaFile,openMedia} from './media-viewer.ts';
export function createMediaBrowser(state,roots,render) {
  function collect(result,runDirectory) {
    const merged=new Map(state.mediaFiles.map(item=>[item.path,item]));
    const discovered=discoverMedia(result,roots,runDirectory);
    state.currentRunMediaCount=discovered.length;
    for(const item of discovered)merged.set(item.path,item);
    state.mediaFiles=[...merged.values()].slice(-100);
    state.mediaIndex=Math.max(0,state.mediaFiles.length-1);
    return discovered;
  }
  function show(candidate) {
    if(candidate){const item=mediaFile(candidate,roots);if(!item){state.statusMessage='Media file is missing, unsupported, or outside this workspace.';render();return;}collect({path:item.path});}
    state.mode='MEDIA';render();
  }
  function open(item) {
    state.statusMessage='Opening selected media…';
    return openMedia(item,roots).then(()=>{state.statusMessage=`Opened in system viewer: ${item.path}`;render();}).catch(error=>{state.statusMessage=error.message;render();});
  }
  function key(key) {
    if(key.name==='escape'||key.name==='q'){state.mode='NORMAL';render();return;}
    if(key.name==='up'||key.name==='k')state.mediaIndex=Math.max(0,state.mediaIndex-1);
    if(key.name==='down'||key.name==='j')state.mediaIndex=Math.min(Math.max(0,state.mediaFiles.length-1),state.mediaIndex+1);
    if(key.name==='return'&&state.mediaFiles[state.mediaIndex]) {
      void open(state.mediaFiles[state.mediaIndex]);
    }
    render();
  }
  return {collect,show,key,open};
}
