"""Drive the actual private capture process, retaining metadata/digests only."""
import fcntl,hashlib,json,math,os,pathlib,select,signal,socket,struct,subprocess,time
def exercise(runtime,source,state,env):
 rows=[]
 for case in ['streaming','buffered','denied','cancel-permission','peer-lost','invalid-wire','foreign-capture']:
  directory=state/case;directory.mkdir(mode=0o700)
  parent,child=socket.socketpair(socket.AF_UNIX,socket.SOCK_STREAM)
  inherited=fcntl.fcntl(child.fileno(),fcntl.F_DUPFD_CLOEXEC,198)
  capture_id=os.urandom(24).hex()
  payload={'fd':inherited,'peerPid':os.getpid(),'captureId':capture_id,'directory':str(directory)}
  child_env={**env,'O3_NATIVE_VOICE_DATA':json.dumps(payload)}
  log=(directory/'electron.log').open('w')
  process=subprocess.Popen([str(runtime/'electron'),'--disable-setuid-sandbox','--use-fake-device-for-media-stream',str(source/'electron-capture-main.cjs')],env=child_env,stdin=subprocess.DEVNULL,stdout=subprocess.PIPE,stderr=log,pass_fds=(inherited,),start_new_session=True)
  os.close(inherited);child.close();pidfd=os.pidfd_open(process.pid);pcm=hashlib.sha256();samples=0;nonzero=0;packets=0;levels=0;pending={};permission=None
  def send(kind,id,**fields):
   body=json.dumps({'type':kind,'id':id,'captureId':capture_id,**fields},separators=(',',':')).encode();assert len(body)<=65536
   # Deliberate sub-header/sub-frame writes exercise the actual stream parser.
   wire=b'OVC1'+struct.pack('<II',0,len(body))+body
   for at in range(0,len(wire),7):parent.sendall(wire[at:at+7])
  def exact(size,deadline):
   data=b''
   while len(data)<size:
    remaining=deadline-time.monotonic();assert remaining>0,'Private capture frame deadline'
    assert select.select([parent],[],[],remaining)[0],'Private capture frame timeout'
    chunk=parent.recv(size-len(data));assert chunk,'Private capture closed before receipt';data+=chunk
   return data
  def receive(timeout=8):
   nonlocal samples,nonzero,packets,levels,permission
   deadline=time.monotonic()+timeout;header=exact(12,deadline);assert header[:4]==b'OVC1';kind,size=struct.unpack('<II',header[4:]);assert kind in [0,1,2]and 0<size<=(65536 if kind==0 else 32000)
   body=exact(size,deadline)
   if kind:
    assert size%4==0 and kind==(2 if case=='buffered'else 1);values=struct.iter_unpack('<f',body)
    for(value,)in values:assert math.isfinite(value);samples+=1;nonzero+=value!=0
    packets+=1;pcm.update(body);return {'type':'pcm'}
   value=json.loads(body);assert value['captureId']==capture_id
   if value['type']=='result':pending[value['id']]=value
   if value['type']=='permission-request':assert value['media']=='audio';permission=value['requestId']
   if value['type']=='level':assert math.isfinite(value['value'])and 0<=value['value']<=1;levels+=1
   assert value['type']in ['ready','result','permission-request','level','duration-limit','capture-error']
   assert value['type']not in ['capture-error','duration-limit'],'Unexpected capture failure/limit'
   return value
  def result(id,ok=True):
   while id not in pending:receive()
   value=pending.pop(id);assert value['ok']is ok,value;return value.get('value')
  try:
   assert receive()['type']=='ready'
   flags=next(line.split()[1]for line in pathlib.Path(f'/proc/{process.pid}/fdinfo/{payload["fd"]}').read_text().splitlines()if line.startswith('flags:'))
   assert int(flags,8)&os.O_CLOEXEC,'Private descriptor was not confined before renderer construction'
   if case in ['invalid-wire','foreign-capture']:
    if case=='invalid-wire':parent.sendall(b'OVC1'+struct.pack('<II',0,65537))
    else:
     body=json.dumps({'type':'prepare','id':1,'captureId':'0'*48,'mode':'streaming','microphoneId':''}).encode();assert capture_id!='0'*48;parent.sendall(b'OVC1'+struct.pack('<II',0,len(body))+body)
    assert process.wait(timeout=5)==2;assert process.stdout.read(1)==b'';assert select.select([pidfd],[],[],1)[0]
    rows.append({'case':case,'exit':2,'samples':0,'confirmedClose':False,'inputRefusedBeforePermission':True,'privateDescriptorCloexec':True,'stdoutBytes':0});continue
   send('prepare',1,mode='buffered'if case=='buffered'else'streaming',microphoneId='')
   while permission is None:receive()
   if case=='cancel-permission':
    send('close',2);assert result(2)=={'cleanup':'confirmed'}
   else:
    # A stale decision cannot approve the outstanding real permission.
    send('permission',2,requestId='not-the-owned-request',allow=True);assert result(2,False)['error']=='stale_permission'
    send('permission',3,requestId=permission,allow=case!='denied');result(3)
    if case=='denied':
     assert result(1,False)['error']=='capture_failed';send('close',4);assert result(4)=={'cleanup':'confirmed'}
    else:
     result(1);send('record',4);result(4)
     if case=='peer-lost':
      parent.shutdown(socket.SHUT_RDWR);parent.close()
     else:
      if case=='buffered':
       until=time.monotonic()+1.25
       while time.monotonic()<until:receive()
      else:
       while packets<3:receive()
      send('stop',5);stopped=result(5);assert stopped['mode']==case and stopped['pcmBytes']==samples*4
      assert samples>0 and nonzero>0 and levels>0
      send('close',6);assert result(6)=={'cleanup':'confirmed'}
   exit_code=process.wait(timeout=5);assert exit_code==(2 if case=='peer-lost'else 0),(case,exit_code)
   assert select.select([pidfd],[],[],1)[0],'Capture process exit not observed';assert process.stdout.read(1)==b'','Unexpected capture stdout'
   if case in ['denied','cancel-permission']:assert samples==0
   rows.append({'case':case,'exit':exit_code,'samples':samples,'nonzero':nonzero,'pcmPackets':packets,'pcmSha256':pcm.hexdigest(),'levelEvents':levels,'confirmedClose':case!='peer-lost','peerLossNeverClaimedConfirmed':case=='peer-lost','realPermissionRoundTrip':True,'privateDescriptorCloexec':True,'stdoutBytes':0})
  finally:
   if process.poll()is None:
    os.killpg(process.pid,signal.SIGTERM)
    try:process.wait(timeout=2)
    except subprocess.TimeoutExpired:os.killpg(process.pid,signal.SIGKILL);process.wait(timeout=2)
   os.close(pidfd);parent.close();process.stdout.close();log.close()
 return rows
