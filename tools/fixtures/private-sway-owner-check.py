import os,pathlib,socket,struct,subprocess,time,resource,json
assert resource.getrlimit(resource.RLIMIT_CORE)==(0,0)
base=pathlib.Path('/tmp/owned');base.mkdir(mode=0o700);runtime=base/'runtime';runtime.mkdir(mode=0o700);home=base/'home';home.mkdir(mode=0o700)
config=base/'sway.conf';config.write_text('xwayland disable\noutput * mode 640x480\nseat seat0 fallback true\ndefault_border none\n')
env={'PATH':'/usr/bin','HOME':str(home),'XDG_RUNTIME_DIR':str(runtime),'WLR_BACKENDS':'headless','WLR_RENDERER':'pixman','WLR_HEADLESS_OUTPUTS':'1','WLR_LIBINPUT_NO_DEVICES':'1','LD_LIBRARY_PATH':'/fixture/prefix/usr/lib','SWAYSOCK':str(runtime/'ipc.sock')}
def connect(path):
 s=socket.socket(socket.AF_UNIX);s.settimeout(2);s.connect(str(path));return s
def wordstring(text):
 b=text.encode()+b'\0';return struct.pack('I',len(b))+b+b'\0'*((-len(b))%4)
def query(s,guess=None):
 # wl_display.get_registry(newid=2), then sync(newid=3).
 s.sendall(struct.pack('III',1,(12<<16)|1,2))
 if guess is not None:
  body=struct.pack('I',guess)+wordstring('wl_compositor')+struct.pack('II',1,4)
  s.sendall(struct.pack('II',2,((len(body)+8)<<16)|0)+body)
 s.sendall(struct.pack('III',1,12<<16,3));pending=b'';globals=[]
 while True:
  b=s.recv(65536)
  if not b:return ('closed',globals)
  pending+=b
  assert len(pending)<1024*1024
  while len(pending)>=8:
   obj,head=struct.unpack_from('II',pending);length=head>>16;op=head&65535
   assert length>=8 and length%4==0
   if len(pending)<length:break
   data=pending[8:length];pending=pending[length:]
   if obj==1 and op==0:return ('denied',globals)
   if obj==2 and op==0:
    name=struct.unpack_from('I',data)[0];n=struct.unpack_from('I',data,4)[0];interface=data[8:8+n-1].decode();globals.append((name,interface))
   if obj==3:return ('done',globals)
def ipc(s,kind,body=b''):
 s.sendall(b'i3-ipc'+struct.pack('II',len(body),kind)+body)
 def exact(n):
  b=b''
  while len(b)<n:
   part=s.recv(n-len(b));assert part;b+=part
  return b
 header=exact(14);assert header[:6]==b'i3-ipc';n,reply=struct.unpack_from('II',header,6);assert n<1024*1024 and reply==kind
 return json.loads(exact(n))
log=open('/result/runtime-sway.log','wb');child=subprocess.Popen(['/fixture/sway-owned','--config',str(config)],env=env,stdout=subprocess.DEVNULL,stderr=log)
try:
 deadline=time.monotonic()+5
 while time.monotonic()<deadline:
  assert child.poll() is None
  paths=list(runtime.glob('wayland-*'));paths=[p for p in paths if p.is_socket()]
  if paths and (runtime/'ipc.sock').is_socket():break
  time.sleep(.01)
 assert paths and (runtime/'ipc.sock').is_socket()
 display=connect(paths[0]);status,globals=query(display);display.close();assert status=='done' and len(globals)>5
 name=next(n for n,i in globals if i=='wl_compositor')
 # Real supervisor-created and connected sockets, delegated only after connect.
 delegated=connect(paths[0]);control=connect(runtime/'ipc.sock');consumer=os.fork()
 if consumer==0:
  try:
   status,observed=query(delegated);assert status=='done' and len(observed)==len(globals)
   assert ipc(control,7)['major']==1
   os._exit(0)
  except BaseException:os._exit(1)
 delegated.close();control.close();assert os.waitpid(consumer,0)[1]==0
 foreign=os.fork()
 if foreign==0:
  try:
   s=connect(paths[0])
   try:
    state,seen=query(s,name);assert state in ['denied','closed'] and seen==[]
   except (BrokenPipeError,ConnectionResetError):pass
   s.close()
   s=connect(runtime/'ipc.sock')
   try:
    s.sendall(b'i3-ipc'+struct.pack('II',0,7));assert s.recv(16)==b''
   except (BrokenPipeError,ConnectionResetError):pass
   s.close();os._exit(0)
  except BaseException:os._exit(1)
 assert os.waitpid(foreign,0)[1]==0
 control=connect(runtime/'ipc.sock');control.sendall(b'i3-ipc'+struct.pack('II',4,0)+b'exit')
 assert child.wait(timeout=3)==0
 control.close()
 print('PRIVATE_SWAY_OWNER_PASS headless-pixman connected-delegation foreign-registry-guessedbind IPC-preparse graceful-exit no-engine no-host-display')
finally:
 if child.poll() is None:child.terminate();child.wait(timeout=3)
 log.close()
