import os,signal,subprocess,sys,tempfile,unittest
from pathlib import Path
from runtime import ROOT,heavy_lock,terminate
class RuntimeTests(unittest.TestCase):
 def test_lock_blocks_a_second_worker(self):
  code='from runtime import heavy_lock\nwith heavy_lock(): pass\n'
  with heavy_lock():
   result=subprocess.run([sys.executable,'-c',code],cwd=Path(__file__).parent,capture_output=True,text=True,timeout=5)
  self.assertNotEqual(result.returncode,0)
  self.assertIn('Another local heavy-model worker is active',result.stderr)
  with heavy_lock():pass
 def test_owned_process_terminates(self):
  process=subprocess.Popen([sys.executable,'-c','import time; time.sleep(60)'],start_new_session=True)
  terminate(process)
  self.assertIsNotNone(process.poll())
 def test_model_server_routes_to_private_gateway_and_respects_local_mode(self):
  from unittest.mock import patch
  import json, runtime
  fake_out=json.dumps({'result':{'choices':[{'message':{'content':'ok-runpod'}}]},'receipt':{'source':'private','route':'chat-primary'}})
  completed=subprocess.CompletedProcess(['node'],0,stdout=fake_out,stderr='')
  with tempfile.TemporaryDirectory() as tmp:
   run_dir=Path(tmp)/'run'
   env={'PRIVATE_INFERENCE_URL':'http://127.0.0.1:8790/v1/infer','PRIVATE_INFERENCE_TOKEN':'t'*48}
   with patch.dict(os.environ,env,clear=False), patch('runtime.subprocess.run',return_value=completed) as m_run:
    with runtime.model_server(run_dir,vision=False):
     tok=runtime.request_json('/tokenize',{'content':'hello world'})
     self.assertTrue(len(tok['tokens'])>=1)
     res=runtime.request_json('/v1/chat/completions',{'messages':[{'role':'user','content':'hi'}]})
     self.assertEqual(res['choices'][0]['message']['content'],'ok-runpod')
     self.assertEqual(m_run.call_count,1)
   self.assertTrue((run_dir/'inference-routing.jsonl').is_file())
   with patch.dict(os.environ,{**env,'PRAECO_INFERENCE_MODE':'local'},clear=False):
    self.assertFalse(runtime._use_private_gateway())
if __name__=='__main__':unittest.main()
