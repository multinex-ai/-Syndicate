import base64
from contextlib import contextmanager
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch
from types import SimpleNamespace
import chat_inference
from media_policy import MediaGuard
from runtime import heavy_lock

class ChatInferenceTests(unittest.TestCase):
    def test_resource_error_reports_trigger_not_post_cleanup_memory(self):
        guard=SimpleNamespace(reason='Sustained memory warning',failure_sample={'available_bytes':2*2**30,'pressure_level':2},samples=[{'available_bytes':6*2**30,'pressure_level':1}])
        with tempfile.TemporaryDirectory() as directory:
            output=io.StringIO()
            with patch.object(chat_inference,'ROOT',Path(directory)),patch.object(chat_inference,'MediaGuard',return_value=guard),patch.object(chat_inference,'model_server',side_effect=KeyboardInterrupt()),patch.object(chat_inference.sys,'stdin',io.TextIOWrapper(io.BytesIO(b'{}'))),patch.object(chat_inference.sys,'stdout',output):
                chat_inference.main()
            result=json.loads(output.getvalue())
            message=json.loads(base64.b64decode(result['body']))['error']['message']
            self.assertIn('Sustained memory warning',message)
            self.assertIn('2.00 GiB',message)
            self.assertNotIn('6.00 GiB',message)

    def test_waiting_lock_enters_after_owner_releases(self):
        with tempfile.TemporaryDirectory() as directory:
            marker=Path(directory)/'acquired'
            code='from runtime import heavy_lock\nfrom pathlib import Path\nimport sys\nwith heavy_lock(timeout=4): Path(sys.argv[1]).write_text("acquired")'
            with heavy_lock():
                child=subprocess.Popen([sys.executable,'-c',code,str(marker)])
                time.sleep(.4)
                self.assertFalse(marker.exists())
            try:
                self.assertEqual(child.wait(timeout=6),0)
                self.assertTrue(marker.exists())
            finally:
                if child.poll() is None:child.kill();child.wait()

    def test_resource_accounting_includes_real_child(self):
        with tempfile.TemporaryDirectory() as directory:
            marker=Path(directory)/'ready'
            code='import sys,time; from pathlib import Path; data=bytearray(32*1024**2); Path(sys.argv[1]).touch(); time.sleep(30)'
            child=subprocess.Popen([sys.executable,'-c',code,str(marker)])
            try:
                deadline=time.monotonic()+5
                while not marker.exists() and time.monotonic()<deadline:time.sleep(.05)
                self.assertTrue(marker.exists())
                sample=MediaGuard(Path(directory)/'sample',include_children=True).take_snapshot()
                self.assertGreater(sample['child_rss_bytes'],24*1024**2)
                self.assertGreaterEqual(sample['worker_rss_bytes'],sample['child_rss_bytes'])
            finally:child.terminate();child.wait(timeout=5)

    def test_reply_emitted_only_after_server_context_exits(self):
        stopped=[]
        @contextmanager
        def server(*args,**kwargs):
            self.assertTrue(kwargs['low_memory'])
            yield
            stopped.append(True)
        class Response(io.BytesIO):
            status=200
            headers={'Content-Type':'text/event-stream'}
        class Output(io.StringIO):
            def write(self,data):
                if data.strip(): self_test.assertTrue(stopped)
                return super().write(data)
        self_test=self
        with tempfile.TemporaryDirectory() as directory:
            output=Output()
            with patch.object(chat_inference,'ROOT',Path(directory)),patch.object(chat_inference,'model_server',server),patch.object(chat_inference.sys,'stdin',io.TextIOWrapper(io.BytesIO(b'{"input":"hi"}'))),patch.object(chat_inference.sys,'stdout',output),patch.object(chat_inference.urllib.request,'urlopen',return_value=Response(b'data: [DONE]\n\n')):
                chat_inference.main()
            result=json.loads(output.getvalue())
            self.assertEqual(result['status'],200)
            self.assertEqual(base64.b64decode(result['body']),b'data: [DONE]\n\n')

    def test_admission_error_returned_without_claiming_inference(self):
        with tempfile.TemporaryDirectory() as directory:
            output=io.StringIO()
            with patch.object(chat_inference,'ROOT',Path(directory)),patch.object(chat_inference,'model_server',side_effect=RuntimeError('memory pressure is not normal')),patch.object(chat_inference.sys,'stdin',io.TextIOWrapper(io.BytesIO(b'{}'))),patch.object(chat_inference.sys,'stdout',output):
                chat_inference.main()
            result=json.loads(output.getvalue())
            self.assertEqual(result['status'],503)
            self.assertIn('memory pressure',base64.b64decode(result['body']).decode())
