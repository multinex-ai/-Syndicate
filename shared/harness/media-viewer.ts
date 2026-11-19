import {realpathSync,statSync,readdirSync} from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {spawn} from 'node:child_process';
export interface MediaFile {path:string;kind:'image'|'video'|'audio';bytes:number}
const kinds:Record<string,MediaFile['kind']>={'.png':'image','.jpg':'image','.jpeg':'image','.webp':'image','.gif':'image','.avif':'image','.mp4':'video','.mov':'video','.webm':'video','.mkv':'video','.mp3':'audio','.wav':'audio','.ogg':'audio','.m4a':'audio','.flac':'audio'};
export function mediaFile(candidate:string,roots:string[]):MediaFile|null {
  try {
    if(candidate.includes('\0'))return null;
    if(candidate.startsWith('file://'))candidate=fileURLToPath(candidate);
    else if(/^[a-z]+:\/\//i.test(candidate))return null;
    const resolved=realpathSync(path.resolve(roots[0],candidate));
    if(!roots.some(root=>{const rel=path.relative(realpathSync(root),resolved);return rel===''||!rel.startsWith('..'+path.sep)&&rel!=='..'&&!path.isAbsolute(rel);}))return null;
    const kind=kinds[path.extname(resolved).toLowerCase()],stat=statSync(resolved);
    return kind&&stat.isFile()?{path:resolved,kind,bytes:stat.size}:null;
  }catch{return null;}
}
export function discoverMedia(value:unknown,roots:string[],runDirectory?:string):MediaFile[] {
  const found=new Map<string,MediaFile>();let visited=0;
  function add(candidate:string):void{const item=mediaFile(candidate,roots);if(item)found.set(item.path,item);}
  function walk(node:unknown,depth=0):void {
    if(++visited>10000||depth>16)return;
    if(typeof node==='string'){
      add(node);
      for(const m of node.matchAll(/!?\[[^\]]*\]\((<[^>]+>|[^)\n]+)\)/g))add(m[1].replace(/^<|>$/g,''));
      for(const m of node.matchAll(/`([^`\n]+)`/g))add(m[1]);
    }else if(Array.isArray(node))node.forEach(v=>walk(v,depth+1));
    else if(node&&typeof node==='object')Object.values(node).forEach(v=>walk(v,depth+1));
  }
  walk(value);
  // Generated run files are authoritative local artifacts; do not follow directory symlinks.
  function scan(dir:string,depth:number):void {
    if(depth>4||visited>10000)return;
    try {for(const entry of readdirSync(dir,{withFileTypes:true})){visited++;const full=path.join(dir,entry.name);if(entry.isFile())add(full);else if(entry.isDirectory())scan(full,depth+1);if(visited>10000)break;}}catch{/* Failed runs may not create a directory. */}
  }
  if(runDirectory)scan(runDirectory,0);
  return [...found.values()].slice(0,100);
}
export function viewerCommand(file:MediaFile,platform=process.platform):{command:string;args:string[]} {
  if(platform==='darwin')return {command:'/usr/bin/open',args:[file.path]};
  if(platform==='linux')return {command:'xdg-open',args:[file.path]};
  throw new Error('System media opener is not configured on this platform');
}
export async function openMedia(file:MediaFile,roots:string[]):Promise<void> {
  const current=mediaFile(file.path,roots);if(!current)throw new Error('Media file is missing or outside this workspace');
  const command=viewerCommand(current);
  await new Promise<void>((resolve,reject)=>{const child=spawn(command.command,command.args,{shell:false,stdio:'ignore'});child.on('error',reject);child.on('exit',code=>code===0?resolve():reject(new Error('System viewer could not open media')));});
}
