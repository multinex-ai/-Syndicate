"""Local fixture acceptance tests; evidence distinguishes agent text from actual state."""
from pathlib import Path
from http.server import BaseHTTPRequestHandler,ThreadingHTTPServer
import datetime,json,subprocess,sys,threading
from runtime import ROOT
from rust_agent import run_agent
EVIDENCE=ROOT/'evidence/setup-20261102'

def cyber():
 workspace=ROOT/'agents/cyber-lab/workspace/acceptance';workspace.mkdir(parents=True,exist_ok=True)
 (workspace/'lookup.py').write_text('def lookup(connection, username):\n    return connection.execute("SELECT role FROM users WHERE username = \'" + username + "\'").fetchall()\n')
 test='''import sqlite3
from lookup import lookup
c=sqlite3.connect(':memory:')
c.execute('CREATE TABLE users(username TEXT,role TEXT)')
c.executemany('INSERT INTO users VALUES (?,?)',[('alex','reader'),('root','admin')])
assert lookup(c,'alex')==[('reader',)]
assert lookup(c,"' OR 1=1 --")==[]
print('SQL_PARAMETERIZATION_PASS')
'''
 verifier=EVIDENCE/'verify_cyber_fixture.py';verifier.write_text(test)
 result=run_agent('cyber-lab','Review lookup.py for SQL injection. Fix the function while preserving its signature and behavior for valid usernames. Use parameterized SQL. Run a local regression probe showing that a quoted SQL-injection username does not retrieve other rows. Do not change files outside this workspace.',workspace,EVIDENCE/'cyber-run',max_turns=12)
 validation=subprocess.run([sys.executable,'-c',test],cwd=workspace,capture_output=True,text=True,timeout=20)
 output={'agent_status':result['status'],'seconds':result['seconds'],'independent_verifier_exit':validation.returncode,'stdout':validation.stdout,'stderr':validation.stderr,'passed':result['status']=='model_finished' and validation.returncode==0,'tool_calls':len(result['tools'])}
 (EVIDENCE/'cyber-acceptance.json').write_text(json.dumps(output,indent=2));return output

def browser():
 received=[]
 class Handler(BaseHTTPRequestHandler):
  def log_message(self,*args):pass
  def do_GET(self):
   body=b'''<!doctype html><html><head><title>Local agent fixture</title></head><body><h1>Local test registration</h1><form action="/submit" method="post"><label for="name">Name</label><input id="name" name="name" required><label for="code">Code</label><input id="code" name="code" required><button id="submit" type="submit">Register</button></form></body></html>'''
   self.send_response(200);self.send_header('Content-Type','text/html');self.end_headers();self.wfile.write(body)
  def do_POST(self):
   from urllib.parse import parse_qs
   data=parse_qs(self.rfile.read(int(self.headers['Content-Length'])).decode());received.append(data)
   self.send_response(200);self.send_header('Content-Type','text/html');self.end_headers();self.wfile.write(b'<html><body><h1>Registration saved</h1></body></html>')
 server=ThreadingHTTPServer(('127.0.0.1',0),Handler);thread=threading.Thread(target=server.serve_forever,daemon=True);thread.start()
 try:result=run_agent('browser',f'Open http://127.0.0.1:{server.server_port}/. Register with name Alex Example and code LOCAL-729. Submit and observe the confirmation. This is a local test form.',ROOT/'agents/browser/workspace',EVIDENCE/'browser-run',max_turns=12)
 finally:server.shutdown();server.server_close()
 passed=any(x=={'name':['Alex Example'],'code':['LOCAL-729']} for x in received)
 output={'agent_status':result['status'],'seconds':result['seconds'],'server_received':received,'passed':passed and result['status']=='model_finished','kind':'DOM-assisted browser; no native desktop or screenshot grounding claim','tool_calls':len(result['tools'])}
 (EVIDENCE/'browser-acceptance.json').write_text(json.dumps(output,indent=2));return output
if __name__=='__main__':
 choice=sys.argv[1] if len(sys.argv)>1 else 'all'
 for name,fn in [('cyber',cyber),('browser',browser)]:
  if choice in (name,'all'):print(name,json.dumps(fn()),flush=True)
