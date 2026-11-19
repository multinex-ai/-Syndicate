"""Verify byte-range recovery against a deliberately truncated local response."""
import hashlib
from http.server import BaseHTTPRequestHandler,ThreadingHTTPServer
import os
from pathlib import Path
import tempfile
import threading
import unittest
from download_parallel import fetch

class DownloadResumeTest(unittest.TestCase):
 def test_truncated_range_resumes_prefix_and_verifies_full_hash(self):
  payload=bytes(range(251))*3000
  requests=[]
  class Handler(BaseHTTPRequestHandler):
   def do_GET(self):
    lo,hi=map(int,self.headers['Range'][6:].split('-'));requests.append((lo,hi))
    self.send_response(206);self.send_header('Content-Range',f'bytes {lo}-{hi}/{len(payload)}');self.send_header('Content-Length',str(hi-lo+1));self.end_headers()
    body=payload[lo:hi+1]
    self.wfile.write(body[:len(body)//2] if len(requests)==1 else body)
    self.close_connection=True
   def log_message(self,*args):pass
  server=ThreadingHTTPServer(('127.0.0.1',0),Handler);thread=threading.Thread(target=server.serve_forever,daemon=True);thread.start()
  try:
   with tempfile.TemporaryDirectory() as temp:
    target=Path(temp)/'model.bin'
    result=fetch('fixture','pinned','model.bin',len(payload),hashlib.sha256(payload).hexdigest(),target,workers=1,url=f'http://127.0.0.1:{server.server_port}/fixture',chunk_bytes=len(payload))
    self.assertTrue(result['verified']);self.assertEqual(target.read_bytes(),payload)
    self.assertEqual(requests[1][0],len(payload)//2)
    self.assertFalse(target.with_suffix('.bin.chunks.json').exists())
  finally:server.shutdown();server.server_close();thread.join()

if __name__=='__main__':unittest.main()
