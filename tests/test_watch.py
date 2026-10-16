"""Production Rust worker integration: visibility, filtering and transcript cache."""
import json, os, queue, subprocess, tempfile, threading, unittest
from pathlib import Path
from http.server import BaseHTTPRequestHandler, HTTPServer

class WatchTest(unittest.TestCase):
    def test_visible_reads_search_and_cache(self):
        binary=Path(__file__).parents[1]/'bin/omarchy-t3-client'
        self.assertTrue(binary.is_file(),'Build the Rust client before testing')
        calls=[]; polled=threading.Event()
        class Handler(BaseHTTPRequestHandler):
            def log_message(self,*args): pass
            def do_GET(self):
                calls.append(self.path)
                if self.path.endswith('/shell'):
                    data={'threads':[{'id':'thread-0001','title':'Fixture work','status':'idle','updatedAt':'2026-10-15T04:00:00Z'}]}
                    polled.set()
                else:data={'projection':{'messages':[{'id':'m1','role':'assistant','text':'**Hello**','createdAt':'2026-10-15T04:00:00Z'}]},'hasMoreHistory':True}
                body=json.dumps(data).encode(); self.send_response(200);self.send_header('Content-Type','application/json');self.send_header('Content-Length',str(len(body)));self.end_headers();self.wfile.write(body)
        server=HTTPServer(('127.0.0.1',0),Handler);thread=threading.Thread(target=server.serve_forever,daemon=True);thread.start()
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);token=root/'token';token.write_text('synthetic-test-token');token.chmod(0o600)
            conf=root/'.config/omarchy-t3-code';conf.mkdir(parents=True)
            (conf/'environments.json').write_text(json.dumps({'environments':[{'id':'fixture','name':'Fixture','base_url':f'http://127.0.0.1:{server.server_port}','token_file':str(token)}]}))
            env=os.environ.copy();env['HOME']=tmp;env['OMARCHY_T3_FIXTURE_ENVIRONMENTS']=str(conf/'environments.json')
            for key in ['XDG_CONFIG_HOME','XDG_STATE_HOME','XDG_DATA_HOME','KIMI_CODING_API_KEY']:env.pop(key,None)
            p=subprocess.Popen([str(binary),'watch'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True,env=env,bufsize=1)
            events=queue.Queue()
            reader=threading.Thread(target=lambda:[events.put(json.loads(line)) for line in p.stdout],daemon=True);reader.start()
            def send(cmd):p.stdin.write(json.dumps(cmd)+'\n');p.stdin.flush()
            def event(kind):
                for _ in range(10):
                    e=events.get(timeout=4)
                    if e['type']==kind:return e['data']
                self.fail('Missing worker event '+kind)
            try:
                self.assertEqual(len(event('snapshot')['threads']),1)
                # Invisible selection is ignored, even with a valid thread ID.
                send({'op':'select','environment':'fixture','thread':'thread-0001','serial':1})
                send({'op':'search','query':'missing'})
                self.assertEqual(event('snapshot')['threads'],[])
                self.assertFalse(any(path.endswith('/bounded') for path in calls))
                send({'op':'visible','value':True})
                send({'op':'select','environment':'fixture','thread':'thread-0001','serial':42})
                result=event('messages')
                self.assertEqual(result['serial'],42)
                self.assertIn('<strong>Hello</strong>',result['result']['messages'][0]['rendered'])
                self.assertTrue(result['result']['hasMoreHistory'])
                polled.clear();send({'op':'refresh'})
                self.assertTrue(polled.wait(4))
                # Metadata did not change: reuse the cached transcript.
                send({'op':'search','query':'Fixture'})
                self.assertEqual(len(event('snapshot')['threads']),1)
                self.assertEqual(sum(path.endswith('/bounded') for path in calls),1)
            finally:
                send({'op':'quit'});p.wait(timeout=5);p.stdin.close();reader.join(timeout=2);p.stdout.close()
        server.shutdown();server.server_close();thread.join()
