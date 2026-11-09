"""Actual NativeMain control; no GUI, real account, shell, or tree cleanup."""
import fcntl, hashlib, http.server, json, os, pathlib, queue, resource, selectors
import signal, socket, struct, subprocess, threading, time, uuid, sys, re, ctypes

gui = os.environ.get("O3_NATIVE_ENTRYPOINT_GUI") == "1"
sustain = int(os.environ.get("O3_NATIVE_ENTRYPOINT_SUSTAIN","0"))
assert sustain in [0,15]
stack_diagnostics = gui or sustain > 0
def diagnostic(*args, **kwargs):
    print(*args, file=sys.stderr if gui else sys.stdout, **kwargs)
def gui_event(row):
    if gui:print(json.dumps(row,ensure_ascii=False),flush=True)

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
xvfb_args=['Xvfb',':9','-screen','0','1280x1024x24','-nolisten','tcp','-auth',auth_path]
if os.environ.get('O3_NATIVE_ENTRYPOINT_GLX')!='1':xvfb_args+=['-extension','GLX']
display = subprocess.Popen(xvfb_args,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
child = None
clipboard = None
permission_count = 0
stack_task = None
stack_records = []
resource_state={'samples':0,'minimumShmFreeBytes':None,'maximumRootFdCount':0,'maximumRootShmMappingBytes':0,'maximumRootShmMappings':0,'privateFdCloexecAfterReady':True}
last_resource_sample=0
module_paths=set()
renderer_module_paths=set()
renderer_map_status='not-observed'
auxiliary_paths={}
auxiliary_status={}
def owned_descendant(pid):
    ancestor=pid
    for _ in range(8):
        if ancestor==child.pid:return True
        status=pathlib.Path(f'/proc/{ancestor}/status').read_text().splitlines()
        parents=[int(line.split()[1]) for line in status if line.startswith('PPid:')]
        if len(parents)!=1 or parents[0]<=0:return False
        ancestor=parents[0]
    return False
def auxiliary_maps():
    if not trace.exists():return
    rows=re.findall(r'^native-process-metrics-(.*)$',trace.read_text(),re.M)
    if not rows:return
    processes=json.loads(rows[-1]);assert len(processes)<=64
    for process in processes:
        if process['type'] not in ['GPU','Utility']:continue
        label=process['type']+':'+process['service'];pid=process['pid']
        try:
            if not isinstance(pid,int) or pid<=0 or not owned_descendant(pid):auxiliary_status[label]='not-owned';continue
            lines=pathlib.Path(f'/proc/{pid}/maps').read_text().splitlines()
            paths=auxiliary_paths.setdefault(label,set())
            for line in lines:
                parts=line.split(maxsplit=5)
                if len(parts)!=6:continue
                path=parts[5]
                if path.endswith(' (deleted)') or not path.startswith(('/usr/','/opt/','/electron/','/lib','/var/cache/fontconfig/')):continue
                if '.so' in path or path=='/electron/electron' or path.endswith(('.ttf','.ttc','.otf')) or '/fontconfig/' in path:paths.add(path)
            assert len(paths)<=256
            auxiliary_status[label]='observed'
        except PermissionError:auxiliary_status[label]='permission-denied'
        except (FileNotFoundError,ProcessLookupError):auxiliary_status[label]='exited'

def renderer_maps():
    global renderer_map_status
    if not trace.exists():return
    matches=re.findall(r'^native-renderer-pid-([0-9]+)$',trace.read_text(),re.M)
    if not matches:return
    pid=int(matches[-1]);ancestor=pid
    for _ in range(8):
        if ancestor==child.pid:break
        status=pathlib.Path(f'/proc/{ancestor}/status').read_text().splitlines()
        parents=[int(line.split()[1]) for line in status if line.startswith('PPid:')]
        if len(parents)!=1 or parents[0]<=0:renderer_map_status='not-owned';return
        ancestor=parents[0]
    else:renderer_map_status='not-owned';return
    lines=pathlib.Path(f'/proc/{pid}/maps').read_text().splitlines()
    for line in lines:
        parts=line.split(maxsplit=5)
        if len(parts)!=6:continue
        path=parts[5]
        if path.endswith(' (deleted)') or not path.startswith(('/usr/','/opt/','/electron/','/lib','/var/cache/fontconfig/')):continue
        if '.so' in path or path=='/electron/electron' or path.endswith(('.ttf','.ttc','.otf')) or '/fontconfig/' in path:renderer_module_paths.add(path)
    assert len(renderer_module_paths)<=256
    renderer_map_status='observed'

def sample_resources(force=False):
    global last_resource_sample,renderer_map_status
    if not force and time.monotonic()-last_resource_sample<.5:return
    last_resource_sample=time.monotonic()
    shared=os.statvfs('/dev/shm')
    free=shared.f_bavail*shared.f_frsize
    resource_state['minimumShmFreeBytes']=free if resource_state['minimumShmFreeBytes'] is None else min(resource_state['minimumShmFreeBytes'],free)
    resource_state['shmTotalBytes']=shared.f_blocks*shared.f_frsize
    if child is None:return
    try:
        descriptors=sum(1 for item in pathlib.Path(f'/proc/{child.pid}/fd').iterdir() if item.name.isdecimal())
        mappings=pathlib.Path(f'/proc/{child.pid}/maps').read_text().splitlines()
        for line in mappings:
            parts=line.split(maxsplit=5)
            if len(parts)==6 and parts[5].startswith(('/usr/','/opt/','/electron/','/lib')) and (' (deleted)' not in parts[5]) and ('.so' in parts[5] or parts[5]=='/electron/electron'):module_paths.add(parts[5])
        assert len(module_paths)<=256
        resource_state['fcitxLoadedNow']=any('/im-fcitx5.so' in line for line in mappings)
        ranges=[line.split()[0] for line in mappings if '/dev/shm/' in line or '/memfd:' in line]
        mapping_bytes=sum(int(bounds.split('-')[1],16)-int(bounds.split('-')[0],16) for bounds in ranges)
        resource_state['maximumRootFdCount']=max(resource_state['maximumRootFdCount'],descriptors)
        resource_state['maximumRootShmMappingBytes']=max(resource_state['maximumRootShmMappingBytes'],mapping_bytes)
        resource_state['maximumRootShmMappings']=max(resource_state['maximumRootShmMappings'],len(ranges))
        if any(row.get('type')=='ready' for row in events):
            info=pathlib.Path(f'/proc/{child.pid}/fdinfo/198').read_text().splitlines()
            flags=[int(line.split()[1],8) for line in info if line.startswith('flags:')]
            resource_state['privateFdCloexecAfterReady'] &= len(flags)==1 and bool(flags[0]&os.O_CLOEXEC)
        resource_state['samples']+=1
        if os.environ.get('O3_NATIVE_ENTRYPOINT_MODULE_MANIFEST')=='1':
            try:renderer_maps();auxiliary_maps()
            except PermissionError:renderer_map_status='permission-denied'
            except (FileNotFoundError,ProcessLookupError):renderer_map_status='exited'
    except (OSError,ValueError):pass

def stack_drain(stream):
    line=bytearray();overflow=False;retained=0
    while chunk:=stream.read(4096):
        for byte in chunk:
            if byte==10:
                frame=re.match(rb"^#([0-9]+) (0x[0-9a-fA-F]+)(?:\s|$)",line)
                fatal=re.search(rb"(?:FATAL|ERROR):([A-Za-z0-9_./-]+\.(?:cc|h)):([0-9]+)\]",line)
                interface=re.search(rb"Message ([0-9]+) rejected by interface ([A-Za-z0-9_.]+)",line)
                validation=re.search(rb"VALIDATION_ERROR_[A-Z_]+",line)
                egl=re.search(rb"EGL_(?:NOT_INITIALIZED|BAD_[A-Z_]+|SUCCESS)",line)
                auth_failed=any(text in line for text in [b"Authorization required, but no authorization protocol specified",b"Invalid MIT-MAGIC-COOKIE-1 key",b"Could not open the default X display",b"Failed to open X11 display"])
                gpu_exit=re.search(rb"GPU process exited unexpectedly: exit_code=([0-9-]+)",line)
                value=("native-mojo-interface-"+interface[2].decode()+"-message-"+interface[1].decode() if interface else None) or ("native-x11-authorization-error" if auth_failed else None) or ("native-validation-"+validation[0].decode() if validation else None) or ("native-egl-"+egl[0].decode() if egl else None) or (("native-gpu-exit-"+gpu_exit[1].decode()) if gpu_exit else ("native-main-pc-"+frame[2].decode()) if frame else ("native-log-location-"+fatal[1].decode()+":"+fatal[2].decode()) if fatal else None)
                if value and retained+len(value)<65536:stack_records.append(value);retained+=len(value)
                line.clear();overflow=False
            elif len(line)<256 and not overflow:line.append(byte)
            else:overflow=True

parent, inherited = socket.socketpair()
source = fcntl.fcntl(inherited.fileno(),fcntl.F_DUPFD_CLOEXEC,198)
inherited.close()
trace = base/'home/native-electron.log'
events = []
transport = []
buffers = {'stdout':bytearray(),'private':bytearray(),'gui':bytearray()}
selector = selectors.DefaultSelector()
instance, lease = str(uuid.uuid4()), str(uuid.uuid4())
key = {'threadId':'entrypoint-thread','tabId':'entrypoint-tab'}
serial = 0
ime_ready_time = None
input_trace = []
selection_script="""(()=>{const f=document.getElementById('field'),s=document.getSelection(),a=s?.anchorNode,r=s?.rangeCount?s.getRangeAt(0):null;const path=[];for(let n=a;n&&path.length<6;n=n.parentNode)path.push({type:n.nodeType,tag:n.nodeType===1?n.tagName:'',editable:n.nodeType===1&&n.isContentEditable});return {focused:document.hasFocus(),activeIsField:document.activeElement===f,activeTag:document.activeElement?.tagName||'',fieldConnected:f?.isConnected===true,readOnly:f?.readOnly,disabled:f?.disabled,valueUnits:f?.value?.length,selectionStart:f?.selectionStart,selectionEnd:f?.selectionEnd,selectionDirection:f?.selectionDirection,selectionType:s?.type,rangeCount:s?.rangeCount,anchorOffset:s?.anchorOffset,focusOffset:s?.focusOffset,anchorPath:path,rangeStartType:r?.startContainer?.nodeType,rangeStartOffset:r?.startOffset,rangeEndType:r?.endContainer?.nodeType,rangeEndOffset:r?.endOffset}})()"""
def inspect_x11_focus():
    handles=re.findall(r'^native-window-xid-([0-9]+)$',trace.read_text(),re.M)
    assert handles
    expected=int(handles[-1])
    endpoint=socket.socket(socket.AF_UNIX,socket.SOCK_STREAM)
    endpoint.connect('/tmp/.X11-unix/X9')
    peer=struct.unpack('3i',endpoint.getsockopt(socket.SOL_SOCKET,socket.SO_PEERCRED,12))
    assert peer[0]==display.pid and peer[1]==os.getuid()
    class Auth(ctypes.Structure):_fields_=[('namelen',ctypes.c_int),('name',ctypes.c_char_p),('datalen',ctypes.c_int),('data',ctypes.c_char_p)]
    class Cookie(ctypes.Structure):_fields_=[('sequence',ctypes.c_uint)]
    class Reply(ctypes.Structure):_fields_=[('response',ctypes.c_uint8),('revert',ctypes.c_uint8),('sequence',ctypes.c_uint16),('length',ctypes.c_uint32),('focus',ctypes.c_uint32)]
    library=ctypes.CDLL('/usr/lib/libxcb.so.1');libc=ctypes.CDLL(None)
    library.xcb_connect_to_fd.argtypes=[ctypes.c_int,ctypes.POINTER(Auth)];library.xcb_connect_to_fd.restype=ctypes.c_void_p
    library.xcb_get_input_focus.argtypes=[ctypes.c_void_p];library.xcb_get_input_focus.restype=Cookie
    library.xcb_get_input_focus_reply.argtypes=[ctypes.c_void_p,Cookie,ctypes.c_void_p];library.xcb_get_input_focus_reply.restype=ctypes.c_void_p
    library.xcb_disconnect.argtypes=[ctypes.c_void_p];libc.free.argtypes=[ctypes.c_void_p]
    credential=Auth(len(fields[2]),fields[2],len(fields[3]),fields[3])
    connection=library.xcb_connect_to_fd(endpoint.detach(),ctypes.byref(credential));assert connection
    pointer=None
    try:
        pointer=library.xcb_get_input_focus_reply(connection,library.xcb_get_input_focus(connection),None)
        assert pointer
        focus=Reply.from_address(pointer).focus
        diagnostic('ACTUAL_NATIVE_X11_FOCUS '+json.dumps({'expectedWindow':expected,'focusWindow':focus,'matchesNativeWindow':focus==expected}),flush=True)
    finally:
        if pointer:libc.free(pointer)
        library.xcb_disconnect(connection)
def stdin(row):
    if os.environ.get('O3_NATIVE_ENTRYPOINT_INSPECT_FOCUS')=='1' and row.get('type')=='composition' and row.get('preedit'):inspect_x11_focus()
    if os.environ.get('O3_NATIVE_ENTRYPOINT_MODULE_MANIFEST')=='1' and row.get('type')=='composition' and row.get('preedit'):
        sample_resources(force=True)
        diagnostic('ACTUAL_NATIVE_PRECOMPOSITION_MODULE_STATE '+json.dumps({'fcitxLoaded':resource_state.get('fcitxLoadedNow')}),flush=True)
    if os.environ.get('O3_NATIVE_ENTRYPOINT_INSPECT_SELECTION')=='1' and row.get('type')=='composition' and row.get('preedit'):
        inspect={'type':'evaluate',**key,'requestId':'diagnostic-before-preedit','script':selection_script}
        child.stdin.write((json.dumps(inspect)+'\n').encode())
    child.stdin.write((json.dumps(row,ensure_ascii=False)+'\n').encode())
    child.stdin.flush()
def private(row):
    parent.sendall((json.dumps({**key,'instance':instance,**row},ensure_ascii=False)+'\n').encode())
def pump(predicate,seconds=15,target_ns=None):
    global permission_count
    deadline = time.monotonic()+seconds
    while not predicate():
        sample_resources()
        assert time.monotonic()<deadline,'actual entrypoint deadline'
        assert child.poll() is None,'actual native entrypoint exited'
        for selected,_ in selector.select(.05 if target_ns is None else min(.05,max(0,(target_ns-time.monotonic_ns())/1e9))):
            data = os.read(selected.fd,65536)
            assert data,'actual native stream ended: '+selected.data+' child-exit='+str(child.poll())
            buffer = buffers[selected.data]
            buffer.extend(data)
            assert len(buffer)<=2*1024*1024
            while b'\n' in buffer:
                line,_,rest = buffer.partition(b'\n')
                buffer[:] = rest
                row = json.loads(line)
                if selected.data=='gui':
                    if row=={'op':'ready'}:continue
                    assert row.get('op')=='input' and row['event'].get('type') in ['press','release','move','double','wheel','key','keyup','text','composition']
                    assert ime_ready_time is not None
                    assert set(row['event']) <= {'type','x','y','button','buttons','modifiers','key','text','repeat','preedit','selectionStart','selectionEnd','replacementStart','replacementEnd','ax','ay','px','py'}
                    for name in ['text','preedit']:
                        if name in row['event']:assert row['event'][name] in ['', 'にほん', '日本']
                    assert len(input_trace)<256
                    input_trace.append({'microseconds':(time.monotonic_ns()-ime_ready_time)//1000,'event':row['event']})
                    stdin(row['event'])
                    continue
                if selected.data=='stdout' and row.get('type')=='evaluation' and row.get('requestId')=='diagnostic-before-preedit':diagnostic('ACTUAL_NATIVE_PRECOMPOSITION_SELECTION '+json.dumps(row.get('value')),flush=True)
                if selected.data=='stdout' and row.get('type')=='error':diagnostic('native-ui-error-'+(row['error'] if row.get('error') in ['browser_unavailable','browser_native_input_focus_required'] else 'other'),flush=True)
                if selected.data=='stdout' and row.get('type')=='frame':
                    assert row['file'] in ['frame-0.png','frame-1.png']
                    gui_event({'type':'frame','page':{**row,'requestId':'native-entrypoint-view','url':(base/'frames'/row['file']).as_uri()}})
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
    env = dict({name:os.environ[name] for name in ['LIBGL_ALWAYS_SOFTWARE','__EGL_VENDOR_LIBRARY_FILENAMES'] if name in os.environ},PATH='/usr/bin:/bin',HOME=str(base/'home'),LANG='C.UTF-8',DISPLAY=':9',XAUTHORITY=auth_path,XDG_RUNTIME_DIR=str(base/'runtime'),XDG_CACHE_HOME=str(base/'cache'),XDG_CONFIG_HOME=str(base/'config'),XDG_DATA_HOME=str(base/'data'),XDG_STATE_HOME=str(base/'state'),T3CODE_HOME=str(base/'native'),DBUS_SESSION_BUS_ADDRESS='unix:path=/tmp/no-entrypoint-bus',QT_QPA_PLATFORM='offscreen',QT_QUICK_BACKEND='software',QT_SCALE_FACTOR='1',QT_SCREEN_SCALE_FACTORS='1',QTWEBENGINE_CHROMIUM_FLAGS='--disable-gpu --password-store=basic',O3_NATIVE_ELECTRON_DIAGNOSTICS=str(trace))
    env.pop('WAYLAND_DISPLAY',None)
    clipboard = subprocess.Popen(['/proof/clipboard-owner'],env=dict(env,QT_QPA_PLATFORM='xcb'),stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL)
    assert json.loads(clipboard.stdout.readline())['ready']
    def offer(operation, text=None):
        row={'op':operation,'id':1}
        if text is not None:row['text']=text
        clipboard.stdin.write((json.dumps(row)+'\n').encode());clipboard.stdin.flush()
        assert json.loads(clipboard.stdout.readline())['ready']
    payload = {'privateBrowserFd':source,'privateBrowserPeerPid':os.getpid(),'frameDirectory':str(base/'frames'),'electronDirectory':str(base/'electron'),'environmentId':'entrypoint','profileId':'default','persistent':True,'ownedNativeHost':True,'width':1280,'height':800}
    env['O3_NATIVE_BROWSER_DATA'] = json.dumps(payload)
    if os.environ.get('O3_NATIVE_ENTRYPOINT_MODULE_MANIFEST')=='1':env['O3_NATIVE_MODULE_METADATA']='1'
    if stack_diagnostics:env['ELECTRON_ENABLE_STACK_DUMPING']='1'
    child = subprocess.Popen(['/electron/electron','--disable-setuid-sandbox','/fixture/electron/native-electron-main.cjs'],env=env,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE if stack_diagnostics else subprocess.DEVNULL,pass_fds=(source,),start_new_session=True)
    if stack_diagnostics:
        stack_task=threading.Thread(target=stack_drain,args=(child.stderr,),daemon=True);stack_task.start()
        for _ in range(100):
            mappings=pathlib.Path(f"/proc/{child.pid}/maps").read_text().splitlines()
            own=[line for line in mappings if line.endswith(" /electron/electron") and line.split()[2]=="00000000"]
            if own:diagnostic("native-main-load-base-0x"+own[0].split("-")[0],flush=True);break
            time.sleep(.01)
    os.close(source)
    os.set_blocking(child.stdout.fileno(),False)
    parent.setblocking(False)
    selector.register(child.stdout,selectors.EVENT_READ,'stdout')
    selector.register(parent,selectors.EVENT_READ,'private')
    if gui:
        os.set_blocking(0,False)
        selector.register(0,selectors.EVENT_READ,'gui')
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
    diagnostic('ACTUAL_NATIVE_ENTRYPOINT_DOUBLE_INIT_METADATA_REPLAY 3',flush=True)
    diagnostic('ACTUAL_NATIVE_ENTRYPOINT_CLIPBOARD_REPLAY 6',flush=True)
    diagnostic('ACTUAL_NATIVE_ENTRYPOINT_NAVIGATION_REPLAY 7',flush=True)
    pump(lambda:any(row.get('type')=='frame' for row in events))
    if sustain:
        before=sum(row.get('type')=='frame' for row in events)
        until=time.monotonic()+sustain
        pump(lambda:time.monotonic()>=until,seconds=sustain+3)
        diagnostic('ACTUAL_NATIVE_ENTRYPOINT_SUSTAIN '+json.dumps({'seconds':sustain,'newFrames':sum(row.get('type')=='frame' for row in events)-before}),flush=True)
    ime_ready_time=time.monotonic_ns()
    if gui:
        gui_event({'type':'ime-ready'})
    if os.environ.get('O3_NATIVE_ENTRYPOINT_REPLAY')=='1':
        replay=json.loads(pathlib.Path('/proof/input-replay.json').read_text())
        assert replay['timebase']=='actual-ime-ready' and 0<len(replay['rows'])<=256
        previous=0;maximum_late=0
        for row in replay['rows']:
            delay=row['microseconds'];assert isinstance(delay,int) and previous<=delay<=10000000;previous=delay
            event=row['event'];assert event['type'] in ['press','release','move','double','wheel','key','keyup','text','composition']
            for name in ['text','preedit']:
                if name in event:assert event[name] in ['', 'にほん', '日本']
            target=ime_ready_time+delay*1000
            pump(lambda:time.monotonic_ns()>=target,seconds=11,target_ns=target)
            maximum_late=max(maximum_late,(time.monotonic_ns()-target)//1000)
            stdin(event)
        diagnostic('ACTUAL_NATIVE_ENTRYPOINT_TIMELINE_REPLAY '+json.dumps({'rows':len(replay['rows']),'maximumLatenessMicroseconds':maximum_late}),flush=True)
    elif not gui:
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
    gui_event({'type':'complete','textMatches':True,'events':final['events']})
    assert any(event['type']=='compositionstart' and event['trusted'] for event in final['events'])
    diagnostic('ACTUAL_NATIVE_ENTRYPOINT_IME_PASS '+json.dumps({'actualMain':True,'actualGuiInput':gui and os.environ.get('O3_NATIVE_ENTRYPOINT_REPLAY')!='1','finalTextMatches':True,'frameCountsAfterNavigations':frame_counts,'finalFrameCount':sum(row.get('type')=='frame' for row in events),'events':[event['type'] for event in final['events']]}),flush=True)
finally:
    if input_trace:diagnostic('ACTUAL_NATIVE_ENTRYPOINT_INPUT_TRACE '+json.dumps({'timebase':'actual-ime-ready','rows':input_trace},ensure_ascii=False),flush=True)
    if os.environ.get('O3_NATIVE_ENTRYPOINT_MODULE_MANIFEST')=='1':
        modules=[]
        for path in sorted(module_paths):
            file=pathlib.Path(path);size=file.stat().st_size;assert size<256*1024*1024
            with file.open('rb') as stream:digest=hashlib.file_digest(stream,'sha256').hexdigest()
            modules.append({'path':path,'bytes':size,'sha256':digest})
        diagnostic('ACTUAL_NATIVE_ENTRYPOINT_MODULE_MANIFEST '+json.dumps(modules),flush=True)
        renderer_modules=[]
        for path in sorted(renderer_module_paths):
            file=pathlib.Path(path);size=file.stat().st_size;assert size<256*1024*1024
            with file.open('rb') as stream:digest=hashlib.file_digest(stream,'sha256').hexdigest()
            renderer_modules.append({'path':path,'bytes':size,'sha256':digest})
        diagnostic('ACTUAL_NATIVE_ENTRYPOINT_RENDERER_MANIFEST '+json.dumps({'status':renderer_map_status,'files':renderer_modules}),flush=True)
        auxiliary={}
        for label,status in sorted(auxiliary_status.items()):
            records=[]
            for path in sorted(auxiliary_paths.get(label,set())):
                file=pathlib.Path(path);size=file.stat().st_size;assert size<256*1024*1024
                with file.open('rb') as stream:digest=hashlib.file_digest(stream,'sha256').hexdigest()
                records.append({'path':path,'bytes':size,'sha256':digest})
            auxiliary[label]={'status':status,'files':records}
        diagnostic('ACTUAL_NATIVE_ENTRYPOINT_AUXILIARY_MANIFEST '+json.dumps(auxiliary),flush=True)
    diagnostic('ACTUAL_NATIVE_ENTRYPOINT_RESOURCES '+json.dumps(resource_state),flush=True)
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
    if stack_task is not None:
        stack_task.join(timeout=3)
        for entry in stack_records:diagnostic(entry,flush=True)
    if trace.exists():
        with trace.open('rb') as source:diagnostic(source.read(131072).decode('utf-8',errors='replace'),flush=True)
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
