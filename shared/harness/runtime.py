"""Local Metal server lifecycle, serialized heavy-model admission, and RunPod private-gateway bridge."""
from contextlib import contextmanager, nullcontext
from pathlib import Path
import fcntl,http.client,json,os,signal,socket,subprocess,time,urllib.request
ROOT=Path(__file__).resolve().parents[2]
MODEL=ROOT/'shared/models/qwen35-9b-local/Qwen3.5-9B-Local-HauhauCS-Aggressive-Q4_K_M.gguf'
PROJECTOR=MODEL.parent/'mmproj-Qwen3.5-9B-Local-HauhauCS-Aggressive-BF16.gguf'
ENDPOINT='http://127.0.0.1:8087'
_HTTP_CONN=None
_REMOTE_SESSION=None

def _use_private_gateway():
 if os.environ.get('PRAECO_INFERENCE_MODE')=='local' or os.environ.get('LOCAL_AGENTS_FORCE_LOCAL')=='1':
  return False
 return bool(os.environ.get('PRIVATE_INFERENCE_URL') and os.environ.get('PRIVATE_INFERENCE_TOKEN'))

def _fallback_allowed():
 return os.environ.get('PRAECO_ALLOW_LOCAL_FALLBACK')=='1' or os.environ.get('LOCAL_AGENTS_ALLOW_LOCAL_FALLBACK')=='1'

def _local_request_json(path,payload=None,timeout=180):
 global _HTTP_CONN
 data=json.dumps(payload).encode() if payload is not None else None
 method='POST' if data is not None else 'GET'
 headers={'Content-Type':'application/json','Connection':'keep-alive'} if data is not None else {'Connection':'keep-alive'}
 for attempt in range(2):
  try:
   if _HTTP_CONN is None:
    _HTTP_CONN=http.client.HTTPConnection('127.0.0.1',8087,timeout=timeout)
   else:
    _HTTP_CONN.timeout=timeout
   _HTTP_CONN.request(method,path,body=data,headers=headers)
   resp=_HTTP_CONN.getresponse()
   raw=resp.read()
   if resp.status>=400:
    raise RuntimeError(f'HTTP {resp.status}: {raw.decode(errors="replace")[:400]}')
   return json.loads(raw)
  except (http.client.HTTPException,OSError,ConnectionError):
   if _HTTP_CONN is not None:
    try:_HTTP_CONN.close()
    except Exception:pass
    _HTTP_CONN=None
   if attempt==1:
    request=urllib.request.Request(ENDPOINT+path,data=data,headers={'Content-Type':'application/json'})
    with urllib.request.urlopen(request,timeout=timeout) as response:return json.load(response)

def request_json(path,payload=None,timeout=180):
 global _REMOTE_SESSION
 if _REMOTE_SESSION is not None and _REMOTE_SESSION.get('active'):
  if path=='/health':
   return {'status':'ok','mode':'private_gateway'}
  if path=='/tokenize':
   content=(payload or {}).get('content','') if isinstance(payload,dict) else ''
   approx=max(1,(len(str(content))+3)//4)
   return {'tokens':[0]*approx,'approximate':True}
  if path=='/v1/chat/completions':
   helper=ROOT/'local-inference-gateway'
   try:
    if not helper.is_file():
     raise RuntimeError(f'Private inference client missing: {helper}')
    req_body=dict(payload or {})
    result=subprocess.run(['node',str(helper),'chat'],input=json.dumps(req_body),text=True,capture_output=True,timeout=min(max(int(timeout),360),600),check=True)
    parsed=json.loads(result.stdout)
    value=parsed['result']
    if not isinstance(value.get('choices'),list) or not value['choices']:
     raise ValueError('Invalid private gateway response')
    run_dir=_REMOTE_SESSION.get('run_dir')
    if run_dir is not None:
     with (Path(run_dir)/'inference-routing.jsonl').open('a') as receipt:
      receipt.write(json.dumps(parsed.get('receipt',{'source':'private'}))+'\n')
    return value
   except Exception as exc:
    detail=getattr(exc,'stderr',None) or str(exc)
    detail=str(detail).strip()[:300]
    if not _fallback_allowed():
     raise RuntimeError(f'Private RunPod inference failed ({detail}); local fallback is OFF') from exc
    if not MODEL.is_file():
     raise RuntimeError(f'Private RunPod inference failed ({detail}) and local fallback weights are not installed ({MODEL.name})') from exc
    _REMOTE_SESSION['activate_local']()
    _REMOTE_SESSION['active']=False
    run_dir=_REMOTE_SESSION.get('run_dir')
    if run_dir is not None:
     with (Path(run_dir)/'inference-routing.jsonl').open('a') as receipt:
      receipt.write(json.dumps({'source':'local','reason':'private_unavailable_or_denied'})+'\n')
 return _local_request_json(path,payload=payload,timeout=timeout)

@contextmanager
def heavy_lock(timeout=0):
 lock=ROOT/'shared/runtime/heavy-model.lock';lock.parent.mkdir(parents=True,exist_ok=True)
 with lock.open('a+') as handle:
  deadline=time.monotonic()+timeout
  while True:
   try:
    fcntl.flock(handle,fcntl.LOCK_EX|fcntl.LOCK_NB);break
   except BlockingIOError:
    if time.monotonic()>=deadline:raise RuntimeError('Another local heavy-model worker is active.')
    time.sleep(.25)
  try:yield
  finally:fcntl.flock(handle,fcntl.LOCK_UN)

def terminate(process):
 if process.poll() is None:
  try:os.killpg(process.pid,signal.SIGTERM)
  except ProcessLookupError:pass
  try:process.wait(timeout=8)
  except subprocess.TimeoutExpired:
   try:os.killpg(process.pid,signal.SIGKILL)
   except ProcessLookupError:pass
   process.wait(timeout=8)

@contextmanager
def _local_model_server(run_dir,vision=False,guard=None,lock_timeout=0,context_size=8192,low_memory=False):
 global _HTTP_CONN
 if guard is None:
  from media_policy import MediaGuard
  guard=MediaGuard(run_dir/'model.json',include_children=True,shutdown_grace_seconds=15)
 with heavy_lock(timeout=lock_timeout), (guard if guard is not None else nullcontext()):
  if not MODEL.is_file():raise FileNotFoundError('Model download is incomplete: '+str(MODEL))
  if vision and not PROJECTOR.is_file():raise FileNotFoundError('Vision projector download is incomplete')
  with socket.socket() as probe:
   if probe.connect_ex(('127.0.0.1',8087))==0:raise RuntimeError('Port 8087 is already occupied; refusing to reuse an unverified server.')
  args=['/opt/homebrew/bin/llama','serve','-m',str(MODEL),'--alias','local-agent','--host','127.0.0.1','--port','8087','-c',str(context_size),'-np','1','-ngl','99','-fa','on','--cache-type-k','q8_0','--cache-type-v','q8_0','--jinja','--reasoning','off']
  if low_memory:args+=['--batch-size','256','--ubatch-size','64','--cache-ram','1024']
  if vision:args+=['--mmproj',str(PROJECTOR)]
  run_dir.mkdir(parents=True,exist_ok=True)
  with (run_dir/'server.log').open('w') as log:
   process=subprocess.Popen(args,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
   process_record={'pid':process.pid,'endpoint':ENDPOINT,'vision':vision,'state':'starting','context_size':context_size,'low_memory':low_memory}
   process_path=run_dir/'server-process.json';process_path.write_text(json.dumps(process_record,indent=2))
   try:
    deadline=time.monotonic()+180
    while True:
     if process.poll() is not None:raise RuntimeError('Model server exited; inspect '+str(run_dir/'server.log'))
     try:
      if _local_request_json('/health',timeout=2).get('status')=='ok':break
     except Exception:pass
     if time.monotonic()>deadline:raise TimeoutError('Model loading exceeded 180 seconds')
     time.sleep(.25)
    process_record['state']='healthy';process_path.write_text(json.dumps(process_record,indent=2))
    yield ENDPOINT
   finally:
    if _HTTP_CONN is not None:
     try:_HTTP_CONN.close()
     except Exception:pass
     _HTTP_CONN=None
    terminate(process)
    process_record.update(state='terminated',returncode=process.returncode)
    process_path.write_text(json.dumps(process_record,indent=2))

@contextmanager
def model_server(run_dir,vision=False,guard=None,lock_timeout=0,context_size=8192,low_memory=False):
 global _REMOTE_SESSION
 run_dir=Path(run_dir)
 if not _use_private_gateway() or vision:
  with _local_model_server(run_dir,vision=vision,guard=guard,lock_timeout=lock_timeout,context_size=context_size,low_memory=low_memory) as ep:
   yield ep
  return
 from contextlib import ExitStack
 run_dir.mkdir(parents=True,exist_ok=True)
 process_path=run_dir/'server-process.json'
 process_record={'pid':None,'endpoint':os.environ.get('PRIVATE_INFERENCE_URL'),'vision':False,'state':'healthy','mode':'private_gateway','context_size':context_size}
 process_path.write_text(json.dumps(process_record,indent=2))
 with ExitStack() as stack:
  started_local=False
  def activate_local():
   nonlocal started_local
   if not started_local:
    stack.enter_context(_local_model_server(run_dir,vision=vision,guard=guard,lock_timeout=lock_timeout,context_size=context_size,low_memory=low_memory))
    started_local=True
  prev=_REMOTE_SESSION
  _REMOTE_SESSION={'active':True,'run_dir':run_dir,'activate_local':activate_local}
  try:
   yield os.environ.get('PRIVATE_INFERENCE_URL',ENDPOINT)
  finally:
   _REMOTE_SESSION=prev
   if not started_local:
    process_record.update(state='terminated',returncode=0)
    process_path.write_text(json.dumps(process_record,indent=2))
