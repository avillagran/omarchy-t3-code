import json
import os
from pathlib import Path
import tempfile
import threading
import unittest
from http.server import BaseHTTPRequestHandler, HTTPServer
import sys
sys.path.insert(0, str(Path(__file__).parent / "reference"))
from t3client.client import Client, T3Error, epoch_ms, normalize_shell, read_token
from t3client.monitor import changes

class ProtocolTest(unittest.TestCase):
    def test_shared_vectors(self):
        vectors=json.loads((Path(__file__).parents[1]/'protocol/fixtures/shell.json').read_text())
        for v in vectors:
            with self.subTest(v['name']):
                self.assertEqual(normalize_shell(v['shell'],'fixture',v['nowMs']),v['expected'])
        by_name={v['name']:v['expected'][0] for v in vectors}
        self.assertTrue(by_name['working']['working'])
        for key in ['request','waiting','failed','plan','review']:
            self.assertTrue(by_name[key]['pending'])
        for key in ['seen','settled','snoozed']: self.assertFalse(by_name[key]['pending'])
        self.assertTrue(by_name['background']['working'])
        self.assertEqual(by_name['unknown-date']['updatedAt'],0)

    def test_time_units(self):
        self.assertEqual(epoch_ms('2026-10-15T04:00:00Z'),1792036800000)
        self.assertEqual(epoch_ms(1792036800),1792036800000)
        self.assertEqual(epoch_ms(1792036800000),1792036800000)
        for invalid in [None,True,float('nan'),'bad','2026-01-01']:
            self.assertEqual(epoch_ms(invalid),0)

    def test_attention_dedup_and_environment_identity(self):
        t={'key':'pc/thread','pending':True,'revision':'run1'}
        seen,events=changes({},[t]); self.assertEqual(len(events),1)
        self.assertFalse(changes(seen,[t])[1])
        self.assertEqual(len(changes(seen,[t|{'revision':'run2'}])[1]),1)
        self.assertEqual(len(changes(seen,[t|{'key':'node/thread'}])[1]),1)
        seen,_=changes(seen,[t|{'pending':False}])
        self.assertEqual(len(changes(seen,[t])[1]),1)

    def test_malformed_is_not_empty(self):
        for d in [{},{'threads':None},{'threads':[{'id':'../escape'}]}]:
            with self.assertRaises(T3Error): normalize_shell(d,'pc')

class TransportTest(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory()
        self.token=Path(self.tmp.name)/'token'
        self.token.write_text('synthetic-test-token')
        self.token.chmod(0o600)
        test=self
        class Handler(BaseHTTPRequestHandler):
            def do_GET(self):
                test.headers={k.lower():v for k,v in self.headers.items()}
                test.path=self.path
                self.send_response(test.status)
                self.send_header('Content-Type',test.content_type)
                if test.status==302: self.send_header('Location','http://127.0.0.1:1/leak')
                self.end_headers(); self.wfile.write(test.body)
            def log_message(self,*args): pass
        self.status=200; self.content_type='application/json'; self.body=b'{"threads":[]}'
        self.server=HTTPServer(('127.0.0.1',0),Handler)
        self.thread=threading.Thread(target=self.server.serve_forever,daemon=True);self.thread.start()
        self.client=Client('http://127.0.0.1:'+str(self.server.server_port),self.token)
    def tearDown(self):
        self.server.shutdown();self.server.server_close();self.thread.join();self.tmp.cleanup()
    def test_auth_header_protocol_and_compression(self):
        self.assertEqual(self.client.threads('test'),[])
        self.assertEqual(self.headers['authorization'],'Bearer synthetic-test-token')
        self.assertEqual(self.headers['x-t3-orchestration-protocol'],'2')
        self.assertEqual(self.headers['accept-encoding'],'identity')
        self.client.get('/.well-known/t3/environment',protocol=False)
        self.assertNotIn('x-t3-orchestration-protocol',self.headers)
    def test_html_auth_and_redirect_errors(self):
        for status,typ,body,code in [(200,'text/html',b'<html>','not_json'),(401,'application/json',b'{}','unauthorized'),(403,'application/json',b'{}','unauthorized'),(302,'text/html',b'','http_302'),(200,'application/json',b'{','malformed')]:
            self.status=status;self.content_type=typ;self.body=body
            with self.assertRaises(T3Error) as error:self.client.threads('pc')
            self.assertEqual(error.exception.code,code)
    def test_encoded_child_identity_and_partial_transcript(self):
        self.body=json.dumps({'projection':{'messages':[{'id':'m','role':'assistant','text':'<b>Plain</b>','createdAt':'2026-10-15T04:00:00Z'}]},'hasMoreHistory':True}).encode()
        result=self.client.messages('thread:child:command%3Ax')
        self.assertIn('command%253Ax',self.path)
        self.assertEqual(result['messages'][0]['text'],'<b>Plain</b>')
        self.assertTrue(result['hasMoreHistory'])
        with self.assertRaises(T3Error):self.client.messages('../escape')
    def test_token_permissions_and_symlink(self):
        self.token.chmod(0o644)
        with self.assertRaises(T3Error):read_token(self.token)
        self.token.chmod(0o600)
        link=Path(self.tmp.name)/'link';link.symlink_to(self.token)
        with self.assertRaises(T3Error):read_token(link)

if __name__=='__main__': unittest.main()
