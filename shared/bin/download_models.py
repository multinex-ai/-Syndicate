#!/usr/bin/env python3
"""Download pinned public model files and verify publisher SHA-256 digests."""
import hashlib,json,urllib.request,time,concurrent.futures
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
REPO='HauhauCS/Qwen3.5-9B-Local-HauhauCS-Aggressive'
REV='0a41c68809d375475f954be12ba7c40efa56c2a9'
FILES=[('Qwen3.5-9B-Local-HauhauCS-Aggressive-Q4_K_M.gguf',5627044224,'2ca636d9e81d3d23ca9b60c234fe185d30ec082eeba69ce770fdb0c76559a4f5'),('mmproj-Qwen3.5-9B-Local-HauhauCS-Aggressive-BF16.gguf',921704448,'05f662501f8bd45607b079723a3e238a4e888fd085a10a53f4057a0e250f6934')]
def digest(p):
 h=hashlib.sha256()
 with p.open('rb') as f:
  for b in iter(lambda:f.read(8*1024*1024),b''):h.update(b)
 return h.hexdigest()
def get(entry):
 name,size,sha=entry;p=ROOT/'shared/models/qwen35-9b-local'/name
 if p.exists() and p.stat().st_size==size and digest(p)==sha:return {'file':name,'sha256':sha,'verified':True,'cached':True}
 temp=p.with_suffix(p.suffix+'.part');url=f'https://huggingface.co/{REPO}/resolve/{REV}/{name}'
 start=time.monotonic();last=start;total=0
 with urllib.request.urlopen(url,timeout=90) as r,temp.open('wb') as f:
  while b:=r.read(8*1024*1024):
   f.write(b);total+=len(b)
   if time.monotonic()-last>15:print(name,round(total/1e9,2),'/',round(size/1e9,2),'GB',flush=True);last=time.monotonic()
 assert temp.stat().st_size==size,'Size mismatch'
 assert digest(temp)==sha,'SHA mismatch'
 temp.replace(p);print('Verified',name,flush=True)
 return {'file':name,'bytes':size,'sha256':sha,'verified':True,'seconds':round(time.monotonic()-start,2)}
if __name__=='__main__':
 with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:results=list(pool.map(get,FILES))
 (ROOT/'evidence/setup-20261102/qwen-download.json').write_text(json.dumps({'repo':REPO,'revision':REV,'files':results},indent=2)+'\n')
