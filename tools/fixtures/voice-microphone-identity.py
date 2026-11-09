"""Exercise the exact capture scripts with synthetic media inside private Podman."""
import os,subprocess,json,pathlib,time,socket,fcntl,struct,hashlib,math,uuid,signal
r,w=os.pipe();x=subprocess.Popen(['/usr/bin/Xvfb','-displayfd',str(w),'-nolisten','tcp','-screen','0','640x480x24','-extension','GLX'],pass_fds=(w,),stdout=subprocess.DEVNULL,stderr=subprocess.PIPE);os.close(w)
root=pathlib.Path('/state');profile=root/'capture-profile';profile.mkdir(mode=0o700)
rows=[]
try:
 display=os.read(r,32).decode().strip();os.close(r);assert display.isdigit(),x.stderr.read().decode()
 def run(selected,index,expected=True,allow=True,cancel=False,capture_profile=profile):
  directory=root/('capture-'+uuid.uuid4().hex);directory.mkdir(mode=0o700)
  reader,child=socket.socketpair();fd=fcntl.fcntl(child.fileno(),fcntl.F_DUPFD_CLOEXEC,64);os.set_inheritable(fd,True)
  capture=os.urandom(24).hex();payload={'fd':fd,'peerPid':os.getpid(),'captureId':capture,'directory':str(directory),'profile':str(capture_profile)}
  env={'PATH':'/usr/bin:/bin','LANG':'C.UTF-8','HOME':str(directory),'XDG_CONFIG_HOME':str(directory),'XDG_DATA_HOME':str(directory),'XDG_CACHE_HOME':str(directory),'DISPLAY':':'+display,'O3_NATIVE_VOICE_DATA':json.dumps(payload),'LIBGL_ALWAYS_SOFTWARE':'1','__EGL_VENDOR_LIBRARY_FILENAMES':'/usr/share/glvnd/egl_vendor.d/50_mesa.json'}
  log=(root/('capture-%s.log'%index)).open('w');p=subprocess.Popen(['/electron/electron','--disable-setuid-sandbox','--use-fake-device-for-media-stream','/source/electron-capture-main.cjs'],env=env,pass_fds=(fd,),stdout=subprocess.DEVNULL,stderr=log,start_new_session=True);os.close(fd);child.close();reader.settimeout(15)
  row={'selected':selected,'permissionRequests':0,'pcmBytes':0,'devices':None,'events':[]}
  def command(id,type,**fields):
   body=json.dumps({'id':id,'type':type,'captureId':capture,**fields}).encode();reader.sendall(b'OVC1'+struct.pack('<II',0,len(body))+body)
  def exact(size):
   value=b''
   while len(value)<size:
    block=reader.recv(size-len(value));assert block,'Unexpected capture EOF';value+=block
   return value
  def packet():
   header=exact(12);assert header[:4]==b'OVC1';kind,length=struct.unpack('<II',header[4:]);assert length<=65536;body=exact(length)
   if kind:
    assert kind==1 and length%4==0;assert all(math.isfinite(v[0])for v in struct.iter_unpack('<f',body));row['pcmBytes']+=length;return None
   value=json.loads(body);assert value['captureId']==capture;row['events'].append(value['type'])
   if value['type']=='permission-request':
    row['permissionRequests']+=1;assert row['permissionRequests']==1
    if cancel:command(2,'close')
    else:command(2,'permission',requestId=value['requestId'],allow=allow)
   if value['type']=='microphones':row['devices']=value['devices']
   return value
  try:
   ready=packet();assert ready['type']=='ready' and ready['runtime']=='44.10.1';command(1,'prepare',mode='streaming',microphoneId=selected)
   if cancel:
    while not((event:=packet())and event['type']=='result'and event['id']==2):pass
    assert event['ok']and event['value']=={'cleanup':'confirmed'};reader.close();p.wait(timeout=10);row['exit']=p.returncode;row['cancelled']=True;assert p.returncode==0 and row['pcmBytes']==0;return row
   while True:
    event=packet()
    if event and event['type']=='result'and event['id']==1:
     row['prepare']=event;assert event['ok']==expected;break
   if expected:
    assert row['permissionRequests']==1 and row['devices'] is not None;command(3,'record')
    while not((event:=packet())and event['type']=='result'and event['id']==3):pass
    until=time.monotonic()+.25
    while time.monotonic()<until:packet()
    command(4,'stop')
    while not((event:=packet())and event['type']=='result'and event['id']==4):pass
    assert event['ok'] and row['pcmBytes']>0
   command(5,'close')
   while not((event:=packet())and event['type']=='result'and event['id']==5):pass
   assert event['ok']and event['value']=={'cleanup':'confirmed'};reader.close();p.wait(timeout=10);row['exit']=p.returncode;assert p.returncode==0
   return row
  finally:
   if p.poll()is None:os.killpg(p.pid,signal.SIGKILL);p.wait(timeout=5)
   reader.close();log.close()
 first=run('',0);rows.append(first);selected=[d['deviceId']for d in first['devices']if d['deviceId']not in['','default','communications']][-1]
 second=run(selected,1);rows.append(second)
 assert first['devices']==second['devices'];assert len(first['devices'])==3
 third=run('no-such-device',2,False);rows.append(third);assert third['pcmBytes']==0 and third['devices']is None
 denied=run('',3,False,allow=False);rows.append(denied);assert denied['prepare']['value']['error']=='capture_permission_denied'and denied['pcmBytes']==0 and denied['permissionRequests']==1
 cancelled=run('',4,cancel=True);rows.append(cancelled)
 fresh=root/'fresh-profile';fresh.mkdir(mode=0o700);foreign=run(selected,5,False,capture_profile=fresh);rows.append(foreign);assert foreign['pcmBytes']==0
 result={'complete':True,'rows':rows,'scope':'Exact production capture JS/addon/DSP with synthetic media in private Podman; Rust profile/discovery/host/UI changes uncompiled and unaccepted'}
 (root/'result.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
except Exception as error:
 (root/'result.json').write_text(json.dumps({'complete':False,'rows':rows,'failure':repr(error)},indent=2)+'\n');raise
finally:
 x.terminate();x.wait(timeout=5)
