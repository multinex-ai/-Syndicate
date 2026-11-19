export function plainText(text:string):string {
  return text.replace(/\x1b\]8;;[^\x1b]*\x1b\\/g,'').replace(/\x1b\[[0-9;?]*[A-Za-z]/g,'');
}
function cellWidth(text:string):number {
  if(/\p{Extended_Pictographic}/u.test(text))return 2;
  const point=text.codePointAt(0)??0;
  return point>=0x1100&&(point<=0x115f||point>=0x2e80&&point<=0xa4cf||point>=0xac00&&point<=0xd7a3||point>=0xf900&&point<=0xfaff||point>=0xff01&&point<=0xff60)?2:1;
}
export function displayWidth(text:string):number {
  return [...new Intl.Segmenter(undefined,{granularity:'grapheme'}).segment(plainText(text))].reduce((total,part)=>total+cellWidth(part.segment),0);
}
export function wrapText(text:string,width:number):string[] {
  const columns=Math.max(2,Math.floor(width)), lines:string[]=[];
  for(const line of plainText(text).split('\n')) {
    let current='',used=0;
    for(const {segment} of new Intl.Segmenter(undefined,{granularity:'grapheme'}).segment(line)) {
      const size=cellWidth(segment);
      if(used+size>columns&&current){lines.push(current);current='';used=0;}
      current+=segment;used+=size;
    }
    lines.push(current);
  }
  return lines;
}
export function viewport(total:number,height:number,offset:number):{offset:number;start:number;max:number} {
  const max=Math.max(0,total-height), bounded=Math.min(max,Math.max(0,offset));
  return {offset:bounded,start:Math.max(0,total-height-bounded),max};
}
export interface PaneState {mode:string;activePane:string;splitRatio:number;traceExpanded:boolean;traceScroll:number;chatScroll:number;traceMaxScroll:number;chatMaxScroll:number;pageSize:number}
export function paneKey(state:PaneState,text:string,key:{name?:string;ctrl?:boolean}):boolean {
  const normal=state.mode==='NORMAL';
  if((normal&&(text==='['||text===']'))||(key.ctrl&&(key.name==='left'||key.name==='right'))) {
    const direction=text==='['||key.name==='left'?-1:1;
    state.splitRatio=Math.max(.2,Math.min(.8,state.splitRatio+direction*.05));return true;
  }
  if(normal&&text==='f'){state.traceExpanded=!state.traceExpanded;state.activePane='trace';return true;}
  if(!normal&&state.activePane!=='trace')return false;
  const field=state.activePane==='trace'?'traceScroll':'chatScroll';
  const maximum=state.activePane==='trace'?state.traceMaxScroll:state.chatMaxScroll;
  const name=key.name;
  if(name==='home')state[field]=maximum;
  else if(name==='end')state[field]=0;
  else if(name==='pageup')state[field]+=state.pageSize;
  else if(name==='pagedown')state[field]-=state.pageSize;
  else if(name==='up'||normal&&name==='k')state[field]++;
  else if(name==='down'||normal&&name==='j')state[field]--;
  else return false;
  state[field]=Math.max(0,Math.min(maximum,state[field]));return true;
}
export interface MouseEvent {button:number;x:number;y:number;released:boolean}
export function mouseInput(input:string):{text:string;pending:string;events:MouseEvent[]} {
  const events:MouseEvent[]=[];
  let text=input.replace(/\x1b\[<(\d+);(\d+);(\d+)([Mm])/g,(_,b:string,x:string,y:string,end:string)=>{
    events.push({button:Number(b),x:Number(x),y:Number(y),released:end==='m'});return '';
  });
  let pending='';const start=text.lastIndexOf('\x1b[<');
  if(start>=0&&/^\x1b\[<[\d;]*$/.test(text.slice(start))){pending=text.slice(start);text=text.slice(0,start);}
  return {text,pending,events};
}
