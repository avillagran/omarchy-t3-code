"""Private capture process protocol, exact native renderer and synthetic audio only."""
import fcntl,hashlib,json,os,pathlib,resource,signal,struct,subprocess,time
assert resource.getrlimit(resource.RLIMIT_CORE)==(0,0)
manifest=json.loads(pathlib.Path('/source/runtime.json').read_text())
for row in manifest['files']:
 path=pathlib.Path('/electron')/row['path'];assert path.is_file()and not path.is_symlink()
 with path.open('rb')as file:assert hashlib.file_digest(file,'sha256').hexdigest()==row['sha256']
base=pathlib.Path('/state')
for name in ['home','runtime','profile','cache','config','data']:(base/name).mkdir(mode=0o700)
pathlib.Path('/tmp/.X11-unix').mkdir(mode=0o1777)
auth=os.memfd_create('pcm-probe-display',os.MFD_CLOEXEC|os.MFD_ALLOW_SEALING)
fields=[b'',b'',b'MIT-MAGIC-COOKIE-1',os.urandom(16)]
record=struct.pack('>H',65535)+b''.join(struct.pack('>H',len(field))+field for field in fields)
assert os.write(auth,record)==len(record)
fcntl.fcntl(auth,fcntl.F_ADD_SEALS,fcntl.F_SEAL_SEAL|fcntl.F_SEAL_GROW|fcntl.F_SEAL_SHRINK|fcntl.F_SEAL_WRITE)
auth_path=f'/proc/{os.getpid()}/fd/{auth}'
display=subprocess.Popen(['Xvfb',':9','-screen','0','1280x1024x24','-nolisten','tcp','-extension','GLX','-auth',auth_path],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
child=None
try:
 for _ in range(200):
  assert display.poll()is None,'Private display exited'
  if pathlib.Path('/tmp/.X11-unix/X9').is_socket():break
  time.sleep(.01)
 else:raise RuntimeError('Private display unavailable')
 env={'PATH':'/usr/bin','HOME':str(base/'home'),'LANG':'C.UTF-8','DISPLAY':':9','XAUTHORITY':auth_path,'XDG_RUNTIME_DIR':str(base/'runtime'),'XDG_CACHE_HOME':str(base/'cache'),'XDG_CONFIG_HOME':str(base/'config'),'XDG_DATA_HOME':str(base/'data'),'DBUS_SESSION_BUS_ADDRESS':'unix:path=/tmp/no-pcm-bus','LIBGL_ALWAYS_SOFTWARE':'1','__EGL_VENDOR_LIBRARY_FILENAMES':'/usr/share/glvnd/egl_vendor.d/50_mesa.json'}
 from importlib.machinery import SourceFileLoader
 parent=SourceFileLoader('owned_pcm_parent','/source/pcm-host-parent.py').load_module()
 rows=parent.exercise(pathlib.Path('/electron'),pathlib.Path('/source'),base,env)
 (base/'host.json').write_text(json.dumps({'exit':0,'cases':rows,'scope':'Actual private main/preload/renderer, inherited peer-admitted binary FD and real synthetic audio; no SDK or consent activation'})+'\n')
 raise SystemExit(0)

finally:
 if child and child.poll()is None:
  os.killpg(child.pid,signal.SIGTERM)
  try:child.wait(timeout=2)
  except subprocess.TimeoutExpired:os.killpg(child.pid,signal.SIGKILL);child.wait(timeout=2)
 display.terminate()
 try:display.wait(timeout=2)
 except subprocess.TimeoutExpired:display.kill();display.wait(timeout=2)
 os.close(auth)
