"""One-shot local installation continuation, with no recurring schedule or remote actions."""
from pathlib import Path
import datetime,json,subprocess,sys,time
ROOT=Path(__file__).resolve().parents[2];E=ROOT/'evidence/setup-20261102';PYTHON=ROOT/'shared/runtime/venv/bin/python'
state={'started_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'phase':'waiting_for_downloads_and_dependencies','steps':{},'remaining':['selected-model tests','image and video generation and visual inspection','native desktop acceptance','final qualified package']}
if (E/'continuation-status.json').exists():
 previous_state=json.loads((E/'continuation-status.json').read_text())
 if any(item.get('status')=='running' for item in previous_state.get('steps',{}).values()):raise RuntimeError('A previous step may still be running; inspect its process before resuming.')
 state=previous_state;state['resumed_utc']=datetime.datetime.now(datetime.timezone.utc).isoformat()
def save():
 state['updated_utc']=datetime.datetime.now(datetime.timezone.utc).isoformat();p=E/'continuation-status.json';temp=p.with_suffix('.tmp');temp.write_text(json.dumps(state,indent=2));temp.replace(p)
def run(name,argv,timeout=1800):
 state['steps'][name]={'status':'running'};save()
 try:
  with (E/(name+'.log')).open('w') as log:r=subprocess.run(argv,cwd=ROOT,stdout=log,stderr=subprocess.STDOUT,timeout=timeout)
  state['steps'][name]={'status':'command_completed' if r.returncode==0 else 'failed','exit_code':r.returncode}
 except Exception as error:state['steps'][name]={'status':'failed','error':str(error)}
 save();return state['steps'][name]['status']=='command_completed'
def report():subprocess.run([sys.executable,str(ROOT/'shared/bin/build_status_report.py')],cwd=ROOT,stdout=subprocess.DEVNULL,timeout=20)
def main():
 save();deadline=time.monotonic()+12*3600;deps=False;last_report=0
 while time.monotonic()<deadline:
  if not deps and (ROOT/'shared/runtime/venv/bin/mflux-generate-flux2').exists():
   test=subprocess.run([str(PYTHON),'-c','import mflux,playwright,jsonschema,psutil,Quartz,ApplicationServices'],capture_output=True,timeout=60)
   if test.returncode==0:
    deps=True;state['steps']['dependencies']={'status':'imports_passed'}
    if 'video-dependencies' not in state['steps']:run('video-dependencies',['uv','pip','install','--python',str(PYTHON),'einops>=0.8.2'],600)
    freeze=subprocess.run(['uv','pip','freeze','--python',str(PYTHON)],capture_output=True,text=True,timeout=30)
    if freeze.returncode==0:(ROOT/'shared/config/requirements.lock.txt').write_text(freeze.stdout)
    if 'installed-tool-tests' not in state['steps']:run('installed-tool-tests',['cargo','test','--release','--manifest-path',str(ROOT/'shared/rust-harness/Cargo.toml'),'tools::'],600)
    if 'installed-runtime-tests' not in state['steps']:run('installed-runtime-tests',[str(PYTHON),'shared/harness/test_runtime.py'],60)
    check=subprocess.run([str(PYTHON),'-c','import json,Quartz,ApplicationServices; print(json.dumps({"accessibility":bool(ApplicationServices.AXIsProcessTrusted()),"screen_recording":bool(Quartz.CGPreflightScreenCaptureAccess()),"native_task_test":"not_executed"}))'],capture_output=True,text=True,timeout=20)
    if check.returncode==0:(E/'desktop-permissions.json').write_text(check.stdout)
    save()
  if deps and (E/'qwen-download.json').exists() and 'cyber-acceptance' not in state['steps']:
   state['phase']='testing_selected_language_model';save()
   run('cyber-acceptance',[str(PYTHON),'shared/harness/acceptance.py','cyber'],1000)
   run('browser-acceptance',[str(PYTHON),'shared/harness/acceptance.py','browser'],1000)
   run('vision-acceptance',[str(PYTHON),'shared/harness/acceptance_extended.py','vision'],600)
   run('lifecycle-acceptance',[str(PYTHON),'shared/harness/acceptance_extended.py','lifecycle'],700)
   run('communication-acceptance',[str(PYTHON),'shared/harness/acceptance_extended.py','communication'],1400)
   run('desktop-acceptance',[str(PYTHON),'shared/harness/acceptance_desktop.py'],1100)
   report()
  if deps and (E/'qwen-download.json').exists() and (E/'local-agent-memory-skills.json').exists() and 'memory-agent-acceptance' not in state['steps']:
   memory=json.loads((E/'local-agent-memory-skills.json').read_text())
   if memory.get('live_identity_verified') and memory.get('live_memory_roundtrip_verified'):
    run('memory-agent-acceptance',[str(PYTHON),'shared/harness/acceptance_memory.py'],1000)
    report()
  if deps and (E/'image-base-download.json').exists() and (E/'image-encoder-download.json').exists() and 'encoder-conversion' not in state['steps']:
   state['phase']='qualifying_image_pipeline';save()
   if run('encoder-conversion',[str(PYTHON),'shared/harness/convert_image_encoder.py'],1800):
    run('local-image-generation',[str(ROOT/'shared/bin/run-image.sh'),'A small red robot reading a book beside a blue lamp, detailed studio photograph, clean composition.','--output',str(ROOT/'agents/image-studio/outputs/local-smoke.png')],1800)
   report()
  if deps and (E/'video-download.json').exists() and 'local-video-generation' not in state['steps']:
   state['phase']='qualifying_video_pipeline';save()
   run('local-video-generation',[str(ROOT/'shared/bin/run-video.sh'),'A yellow toy car rolling smoothly from left to right on a plain wooden table, natural daylight, fixed camera, steady motion.','--output',str(ROOT/'agents/video-studio/outputs/local-smoke.mp4')],1800)
   report()
  if deps and (E/'qwen-download.json').exists() and (E/'image-encoder-conversion.json').exists() and 'media-routing-acceptance' not in state['steps']:
   run('media-routing-acceptance',[str(PYTHON),'shared/harness/acceptance_media_routing.py'],1900)
   report()
  if 'desktop-acceptance' in state['steps'] and 'encoder-conversion' in state['steps'] and 'local-video-generation' in state['steps']:
   state['phase']='requires_review_of_results';state['remaining']=['inspect generated image and video','review task evidence and repair failures','resolve any native desktop permission blocker','final qualified package'];save();report();return
  if time.monotonic()-last_report>60:save();report();last_report=time.monotonic()
  time.sleep(5)
 state['phase']='download_or_dependency_timeout';save();report()
if __name__=='__main__':main()
