import {wrapText} from './terminal-layout.ts';
export function mediaView(state,cols,rows) {
  const lines=['MEDIA · ↑/↓ select · Enter open · Esc back','Files open in your system image, video or audio viewer.',''];
  if(!state.mediaFiles.length)lines.push('No media discovered yet. Generate an image, or use :media path/to/file.png');
  else {
    const item=state.mediaFiles[state.mediaIndex];
    lines.push(`Selected ${state.mediaIndex+1} of ${state.mediaFiles.length} · ${item.kind} · ${item.bytes.toLocaleString()} bytes`);
    lines.push(...wrapText(item.path,cols-2),'');
    const count=Math.max(1,rows-lines.length-4),start=Math.max(0,state.mediaIndex-Math.floor(count/2));
    for(let i=start;i<Math.min(state.mediaFiles.length,start+count);i++)lines.push(`${i===state.mediaIndex?'>':' '} ${i+1}. ${state.mediaFiles[i].path.split('/').at(-1)}`);
  }
  lines.push('',state.statusMessage);
  return '\x1b[H\x1b[2J'+lines.slice(0,rows-1).map(line=>wrapText(line,cols-2)[0]).join('\n');
}
