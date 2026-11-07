"""Actual NativeMain control; no GUI, real account, shell, or tree cleanup."""
import fcntl, hashlib, http.server, json, os, pathlib, queue, resource, selectors
import signal, socket, struct, subprocess, threading, time, uuid

assert resource.getrlimit(resource.RLIMIT_CORE) == (0,0)
manifest = json.loads(pathlib.Path('/proof/runtime.json').read_text())
for row in manifest['files']:
    file = pathlib.Path('/electron')/row['path']
    assert file.is_file() and not file.is_symlink()
    with file.open('rb') as source:
        assert hashlib.file_digest(source,'sha256').hexdigest() == row['sha256']
base = pathlib.Path('/tmp/entrypoint')
base.mkdir(mode=0o700)
for name in ['home','runtime','frames','electron','native','cache','config','data','state']:
    (base/name).mkdir(mode=0o700)
pathlib.Path('/tmp/.X11-unix').mkdir(mode=0o1777)
reports = queue.Queue()
page = pathlib.Path('/proof/native-ime-page.html').read_bytes()
class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self,*args): pass
    def do_GET(self):
        body=page if self.path=='/read-6' else pathlib.Path('/proof/'+('write-custom.html' if self.path=='/read-4' else 'read-custom.html' if self.path=='/read-5' else 'read-text.html')).read_bytes()
        self.send_response(200)
        self.send_header('Content-Type','text/html')
        self.send_header('Content-Length',str(len(body)))
        self.end_headers()
        self.wfile.write(body)
    def do_POST(self):
        size = int(self.headers.get('Content-Length','0'))
        assert 0 <= size <= 32768
        report = json.loads(self.rfile.read(size))
        assert report['outcome'] in ['ime-page-ready','ime-observation','allowed','denied']
        reports.put(report)
        self.send_response(200)
        self.send_header('Content-Length','2')
        self.end_headers()
        self.wfile.write(b'ok')
server = http.server.ThreadingHTTPServer(('127.0.0.1',0),Handler)
server.daemon_threads = True
http_task = threading.Thread(target=server.serve_forever,daemon=True)
http_task.start()
auth = os.memfd_create('entrypoint-display-auth',os.MFD_CLOEXEC|os.MFD_ALLOW_SEALING)
fields = [b'',b'',b'MIT-MAGIC-COOKIE-1',os.urandom(16)]
record = struct.pack('>H',65535)+b''.join(struct.pack('>H',len(field))+field for field in fields)
assert os.write(auth,record) == len(record)
fcntl.fcntl(auth,fcntl.F_ADD_SEALS,fcntl.F_SEAL_SEAL|fcntl.F_SEAL_SHRINK|fcntl.F_SEAL_GROW|fcntl.F_SEAL_WRITE)
auth_path = f'/proc/{os.getpid()}/fd/{auth}'
display = subprocess.Popen(['Xvfb',':9','-screen','0','1280x1024x24','-nolisten','tcp','-extension','GLX','-auth',auth_path],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
child = None
clipboard = None
permission_count = 0
parent, inherited = socket.socketpair()
source = fcntl.fcntl(inherited.fileno(),fcntl.F_DUPFD_CLOEXEC,198)
inherited.close()
trace = base/'home/native-electron.log'
events = []
transport = []
buffers = {'stdout':bytearray(),'private':bytearray()}
selector = selectors.DefaultSelector()
instance, lease = str(uuid.uuid4()), str(uuid.uuid4())
key = {'threadId':'entrypoint-thread','tabId':'entrypoint-tab'}
serial = 0
def stdin(row):
    child.stdin.write((json.dumps(row,ensure_ascii=False)+'\n').encode())
    child.stdin.flush()
def private(row):
    parent.sendall((json.dumps({**key,'instance':instance,**row},ensure_ascii=False)+'\n').encode())
def pump(predicate,seconds=15):
    global permission_count
    deadline = time.monotonic()+seconds
    while not predicate():
        assert time.monotonic()<deadline,'actual entrypoint deadline'
        assert child.poll() is None,'actual native entrypoint exited'
        for selected,_ in selector.select(.05):
            data = os.read(selected.fd,65536)
            assert data,'actual native stream ended'
            buffer = buffers[selected.data]
            buffer.extend(data)
            assert len(buffer)<=2*1024*1024
            while b'\n' in buffer:
                line,_,rest = buffer.partition(b'\n')
                buffer[:] = rest
                row = json.loads(line)
                if selected.data=='stdout' and row.get('type')=='native-dialog-request':
                    permission_count += 1
                    assert permission_count <= 2
                    assert row['origin'] == f'http://'+('127.0.0.1' if permission_count==1 else 'localhost')+f':{server.server_port}'
                    assert row['options']['defaultId'] == 1
                    stdin({'type':'permission-decision','requestId':row['requestId'],'allow':permission_count==2,'remember':True})
                (events if selected.data=='stdout' else transport).append(row)
                assert len(events)<4096 and len(transport)<4096
def command(method,params=None,session=None):
    global serial
    serial += 1
    request = serial
    packet = {'id':request,'method':method,'params':params or {}}
    if session is not None: packet['sessionId'] = session
    private({'kind':'command','lease':lease,'command':{'type':'cdp',**key,'message':json.dumps(packet)}})
    def reply():
        for row in transport:
            event = row.get('event',{})
            if event.get('type')=='cdp':
                message = json.loads(event['message'])
                if message.get('id')==request:return message
    pump(lambda:reply() is not None)
    value = reply()
    assert 'error' not in value,value.get('error')
    return value.get('result',{})
try:
    for _ in range(200):
        assert display.poll() is None
        if pathlib.Path('/tmp/.X11-unix/X9').is_socket():break
        time.sleep(.01)
    else:raise AssertionError('owned X display unavailable')
    env = dict(os.environ,HOME=str(base/'home'),LANG='C.UTF-8',DISPLAY=':9',XAUTHORITY=auth_path,XDG_RUNTIME_DIR=str(base/'runtime'),XDG_CACHE_HOME=str(base/'cache'),XDG_CONFIG_HOME=str(base/'config'),XDG_DATA_HOME=str(base/'data'),XDG_STATE_HOME=str(base/'state'),T3CODE_HOME=str(base/'native'),DBUS_SESSION_BUS_ADDRESS='unix:path=/tmp/no-entrypoint-bus',QT_QPA_PLATFORM='offscreen',QT_QUICK_BACKEND='software',QT_SCALE_FACTOR='1',QT_SCREEN_SCALE_FACTORS='1',QTWEBENGINE_CHROMIUM_FLAGS='--disable-gpu --password-store=basic',O3_NATIVE_ELECTRON_DIAGNOSTICS=str(trace))
    clipboard = subprocess.Popen(['/proof/clipboard-owner'],env=dict(env,QT_QPA_PLATFORM='xcb'),stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL)
    assert json.loads(clipboard.stdout.readline())['ready']
    def offer(operation, text=None):
        row={'op':operation,'id':1}
        if text is not None:row['text']=text
        clipboard.stdin.write((json.dumps(row)+'\n').encode());clipboard.stdin.flush()
        assert json.loads(clipboard.stdout.readline())['ready']
    payload = {'privateBrowserFd':source,'privateBrowserPeerPid':os.getpid(),'frameDirectory':str(base/'frames'),'electronDirectory':str(base/'electron'),'environmentId':'entrypoint','profileId':'default','persistent':True,'ownedNativeHost':True,'width':1280,'height':800}
    env['O3_NATIVE_BROWSER_DATA'] = json.dumps(payload)
    child = subprocess.Popen(['/electron/electron','--disable-setuid-sandbox','/fixture/electron/native-electron-main.cjs'],env=env,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,pass_fds=(source,),start_new_session=True)
    os.close(source)
    os.set_blocking(child.stdout.fileno(),False)
    parent.setblocking(False)
    selector.register(child.stdout,selectors.EVENT_READ,'stdout')
    selector.register(parent,selectors.EVENT_READ,'private')
    pump(lambda:any(row.get('type')=='ready' for row in events))
    stdin({'type':'open',**key,'background':True,'automation':True,'automationInstance':instance})
    pump(lambda:any(row.get('kind')=='proof' for row in transport))
    private({'kind':'admitted','lease':lease,'agentTurn':False})
    pump(lambda:any(row.get('event',{}).get('type')=='attached' for row in transport))
    targets = command('Target.getTargets')['targetInfos']
    session = command('Target.attachToTarget',{'targetId':targets[0]['targetId'],'flatten':True})['sessionId']
    for method,params,owner in [('Page.enable',{},session),('Target.setDiscoverTargets',{'discover':True},None),('Target.setAutoAttach',{'autoAttach':True,'waitForDebuggerOnStart':False,'flatten':True},None),('Page.getFrameTree',{},session),('Network.enable',{},session)]:
        command(method,params,owner)
    metadata='({url:location.href,title:document.title,readyState:document.readyState})'
    assert hashlib.sha256(metadata.encode()).hexdigest()=='41dc123e051c0e7f367837b4eb43d0ee4026318823d2bafe4a3ae660fe847ff4'
    for _ in range(2):
        command('Page.getNavigationHistory',{},session)
        command('Runtime.evaluate',{'expression':metadata,'returnByValue':True,'timeout':1000},session)
    stdin({'type':'resize','width':640,'height':480})
    stdin({'type':'activate',**key,'surfaceId':'entrypoint-view','automation':True,'automationInstance':instance})
    command('Page.getNavigationHistory',{},session)
    targets = command('Target.getTargets')['targetInfos']
    session = command('Target.attachToTarget',{'targetId':targets[0]['targetId'],'flatten':True})['sessionId']
    for method,params,owner in [('Page.enable',{},session),('Target.setDiscoverTargets',{'discover':True},None),('Target.setAutoAttach',{'autoAttach':True,'waitForDebuggerOnStart':False,'flatten':True},None),('Page.getFrameTree',{},session),('Network.enable',{},session)]:
        command(method,params,owner)
    command('Page.getNavigationHistory',{},session)
    frame_counts=[]
    for index in range(7):
        host='127.0.0.1' if index<2 else 'localhost'
        if index in [0,2,3]:offer('instrumented-text', {0:'native denied offer',2:'native permission allowed one',3:'native permission allowed two'}[index])
        if index==5:offer('custom-binary')
        command('Page.navigate',{'url':f'http://{host}:{server.server_port}/read-{index}'},session)
        if index==0:command('Runtime.evaluate',{'expression':metadata,'returnByValue':True,'timeout':1000},session)
        pump(lambda:not reports.empty())
        ready = reports.get_nowait()
        if index<2:assert ready['outcome']=='denied',ready
        elif index<6:
            expected={2:'native permission allowed one',3:'native permission allowed two',4:'custom-written',5:'200000:1bd168fa4e29a8a1af90173db63749b296c7f417c23487dd03ebf21d6ed663a6'}[index]
            assert ready=={'outcome':'allowed','text':expected},ready
        else:assert ready['outcome']=='ime-page-ready'
        frame_counts.append(sum(row.get('type')=='frame' for row in events))
    assert permission_count==2
    print('ACTUAL_NATIVE_ENTRYPOINT_DOUBLE_INIT_METADATA_REPLAY 3',flush=True)
    print('ACTUAL_NATIVE_ENTRYPOINT_CLIPBOARD_REPLAY 6',flush=True)
    print('ACTUAL_NATIVE_ENTRYPOINT_NAVIGATION_REPLAY 7',flush=True)
    pump(lambda:any(row.get('type')=='frame' for row in events))
    for kind in ['press','release','move']:
        stdin({'type':kind,'x':50,'y':40,'button':1 if kind!='move' else 0,'buttons':1 if kind=='press' else 0,'modifiers':0})
    pump(lambda:any(row.get('type')=='frame' and row.get('nativeInput') is True for row in events))
    stdin({'type':'composition','preedit':'にほん','selectionStart':3,'selectionEnd':3})
    pump(lambda:trace.exists() and 'human-complete-composition' in trace.read_text())
    stdin({'type':'text','text':'日本'})
    final = None
    def finished():
        global final
        while not reports.empty():
            report = reports.get_nowait()
            if report['outcome']=='allowed':final=report
        return final is not None
    pump(finished)
    assert final['text']=='日本'
    assert any(event['type']=='compositionstart' and event['trusted'] for event in final['events'])
    print('ACTUAL_NATIVE_ENTRYPOINT_IME_PASS '+json.dumps({'actualMain':True,'finalTextMatches':True,'frameCountsAfterNavigations':frame_counts,'finalFrameCount':sum(row.get('type')=='frame' for row in events),'events':[event['type'] for event in final['events']]}),flush=True)
finally:
    if child is not None and child.poll() is None:
        try:
            stdin({'type':'close'})
            child.wait(timeout=5)
        except (OSError,subprocess.TimeoutExpired):
            os.killpg(child.pid,signal.SIGTERM)
            try:child.wait(timeout=3)
            except subprocess.TimeoutExpired:
                os.killpg(child.pid,signal.SIGKILL)
                child.wait(timeout=3)
    if trace.exists():
        with trace.open('rb') as source:print(source.read(131072).decode('utf-8',errors='replace'),flush=True)
    if clipboard is not None:
        clipboard.stdin.close()
        try:clipboard.wait(timeout=3)
        except subprocess.TimeoutExpired:
            clipboard.terminate();clipboard.wait(timeout=3)
    display.terminate()
    display.wait(timeout=5)
    server.shutdown()
    server.server_close()
    parent.close()
    os.close(auth)
