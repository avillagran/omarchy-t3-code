"""Controlled private HTTP/RPC peers for the real canonical application only."""
import base64,hashlib,http.server,json,os,signal,socket,struct,sys,threading,time
from pathlib import Path
assert Path('/run/.containerenv').is_file()
ROOT=Path('/tmp/canonical-lossless');ROOT.mkdir(mode=0o700)
QUERIES=['a'*199+'\ud83d','ab\udc00','ab\U0001f600','ab\ud800'];PICKS=['a'*32,'ab\udc00','ab\U0001f600','ab'];LOCK=threading.Lock();STOP=threading.Event();SEARCHES={};VISITS=[];ERRORS=[];SERVERS=[]
def units(s):return list(struct.unpack('<'+'H'*(len(s.encode('utf-16-le','surrogatepass'))//2),s.encode('utf-16-le','surrogatepass')))
def data(value):return json.dumps(value,ensure_ascii=True,separators=(',',':')).encode()
def saved(name,value):
 p=ROOT/name
 with p.open('x')as f:json.dump(value,f,ensure_ascii=True);f.write('\n');f.flush();os.fsync(f.fileno())
def frame(stream,opcode,payload):
 size=len(payload);head=bytes([0x80|opcode])+ (bytes([size])if size<126 else b'\x7e'+struct.pack('!H',size)if size<65536 else b'\x7f'+struct.pack('!Q',size));stream.write(head+payload);stream.flush()
def exact(stream,n):
 result=stream.read(n)
 if len(result)!=n:raise EOFError()
 return result
def read_frame(stream):
 a,b=exact(stream,2);assert a&128 and b&128,'Only actual masked, final client frames admitted';size=b&127
 if size==126:size=struct.unpack('!H',exact(stream,2))[0]
 elif size==127:size=struct.unpack('!Q',exact(stream,8))[0]
 assert size<=2*1024*1024
 mask=exact(stream,4);payload=exact(stream,size);return a&15,bytes(v^mask[i%4]for i,v in enumerate(payload))
SHELL={'snapshotSequence':10,'threads':[],'projects':[],'olderSettled':{'before':'2026-10-01T00:00:00.000Z','asOfSequence':10}}
class Peer(http.server.BaseHTTPRequestHandler):
 protocol_version='HTTP/1.1'
 def log_message(self,*args):pass
 def do_GET(self):
  try:
   assert self.headers.get('Authorization')=='Bearer private-lossless-token'
   if self.headers.get('Upgrade','').lower()=='websocket':return self.websocket()
   path=self.path.split('?')[0]
   if path=='/api/auth/session':value={'authenticated':True,'permissions':['orchestration:operate']}
   elif path=='/api/orchestration/shell':value=SHELL
   elif path.startswith('/api/orchestration/threads/')or path.startswith('/api/threads/'):value={'projection':{'messages':[],'activities':[],'plans':[],'thread':{'id':'older'}},'snapshotSequence':10}
   else:value={}
   body=data(value);self.send_response(200);self.send_header('Content-Type','application/json');self.send_header('Content-Length',str(len(body)));self.end_headers();self.wfile.write(body)
  except (BrokenPipeError,ConnectionResetError,EOFError,socket.timeout):pass
  except BaseException as e:
   with LOCK:ERRORS.append(type(e).__name__+':'+str(e))
 def websocket(self):
  key=self.headers['Sec-WebSocket-Key'];accept=base64.b64encode(hashlib.sha1((key+'258EAFA5-E914-47DA-95CA-C5AB0DC85B11').encode()).digest()).decode();self.send_response(101);self.send_header('Upgrade','websocket');self.send_header('Connection','Upgrade');self.send_header('Sec-WebSocket-Accept',accept);self.end_headers();self.connection.settimeout(10)
  while not STOP.is_set():
   opcode,payload=read_frame(self.rfile)
   if opcode==9:frame(self.wfile,10,payload);continue
   if opcode==8:return
   assert opcode==1;packet=json.loads(payload)
   if packet.get('_tag')=='Ping':frame(self.wfile,1,data({'_tag':'Pong'}));continue
   if packet.get('_tag')in['Ack','Interrupt']:continue
   tag=packet.get('tag');body=packet.get('payload',{});value={};failed=False
   if tag=='server.getConfig':value={'shellOlderSettled':True,'providers':[],'settings':{},'environment':{}}
   elif tag=='orchestration.searchThreads':
    query=body['query'];assert query in QUERIES and 2<=len(units(query))<=200
    with LOCK:SEARCHES.setdefault(query,set()).add(self.server.role)
    if self.server.role=='bad':failed=True;value={'_tag':'Fail','query':query}
    else:
     last=chr(units(query)[-1]);value={'matches':[{'threadId':'older','environmentId':'forged','snippet':'s'*239+'\ud83d','query':query}],'snapshotSequence':10,'threads':[{'id':'older','environmentId':'forged','title':'ab\ud800title'+last,'branch':'b\udc00','createdAt':'2026-10-01T00:00:00.000Z','updatedAt':'2026-10-02T00:00:00.000Z','lastVisitedAt':'2026-10-01T00:00:00.000Z'}]}
   elif tag=='orchestration.dispatchCommand':
    assert body['type']=='thread.visit' and body['threadId']=='older' and self.server.role=='good'
    assert isinstance(body['visitedAt'],str)and body['visitedAt']
    if body.get('opened'):
     with LOCK:
      index=len(VISITS);assert index<4 and units(body['searchQuery'])==units(PICKS[index]);VISITS.append({'searchQueryUnits':units(body['searchQuery']),'visitedAt':body['visitedAt'],'opened':True})
    value={'sequence':11,'picked':body.get('searchQuery')}
   elif tag=='orchestration.subscribeShell':
    frame(self.wfile,1,data({'_tag':'Chunk','requestId':packet['id'],'values':[{'kind':'snapshot','snapshot':SHELL}]}));continue
   elif tag=='orchestration.subscribeThread':
    frame(self.wfile,1,data({'_tag':'Chunk','requestId':packet['id'],'values':[]}));continue
   else:failed=True;value={'_tag':'Fail','error':'private_unsupported_background_read','tag':tag}
   frame(self.wfile,1,data({'_tag':'Exit','requestId':packet['id'],'exit':({'_tag':'Failure','cause':value}if failed else{'_tag':'Success','value':value})}))
class Server(http.server.ThreadingHTTPServer):
 daemon_threads=False;block_on_close=True
 def handle_error(self,*args):
  with LOCK:ERRORS.append('peer_handler_error')
try:
 environments=[]
 for role in ['good','bad']:
  server=Server(('127.0.0.1',0),Peer);server.role=role;thread=threading.Thread(target=server.serve_forever,kwargs={'poll_interval':.1});thread.start();SERVERS.append((server,thread));environments.append({'id':role,'name':role,'base_url':'http://127.0.0.1:'+str(server.server_port),'token_file':str(ROOT/'token')})
 saved('environments.json',{'environments':environments});(ROOT/'token').write_bytes(b'private-lossless-token');(ROOT/'token').chmod(0o600);saved('ready.json',{'environmentFile':str(ROOT/'environments.json'),'privatePeers':2,'actualCanonicalApplicationRequired':True});print('CANONICAL_PRIVATE_PEERS_READY',flush=True)
 assert sys.stdin.readline().strip()=='stop','Actual supervisor close required'
finally:
 STOP.set()
 for server,thread in SERVERS:server.shutdown();server.server_close();thread.join()
 with LOCK:
  result={'searchQueryUnits':[units(q)for q in SEARCHES],'allFourQueriesBothRealPeers':all(SEARCHES.get(q)=={'good','bad'}for q in QUERIES),'visits':VISITS,'errors':ERRORS,'allPeerThreadsJoined':True}
 saved('terminal.json',result);print('CANONICAL_PRIVATE_PEERS_TERMINAL',flush=True)
