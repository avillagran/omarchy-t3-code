"""Actual admitted private consent workers; no host state or audio access."""
import fcntl
import hashlib
import json
import os
import pathlib
import select
import signal
import socket
import struct
import subprocess
import time

SOURCE=pathlib.Path('/source')
STATE=pathlib.Path('/state')
RUNTIME=pathlib.Path('/electron')
AUDIO='t3code:voice-audio-grant:v1'
ENVIRONMENT={'kind':'environment','environmentId':'real-env'}
CLOUD={'kind':'cloud','environmentId':'real-env','provider':'openai'}
environment={'PATH':'/usr/bin:/bin','LANG':'C.UTF-8','HOME':str(STATE/'home'),'TMPDIR':str(STATE/'tmp'),'ELECTRON_RUN_AS_NODE':'1'}
for name in ['home','tmp']:
    (STATE/name).mkdir(mode=0o700)
rows=[]
workers=[]

class Worker:
    def __init__(self,name,directory):
        self.parent,child=socket.socketpair(socket.AF_UNIX,socket.SOCK_STREAM)
        inherited=fcntl.fcntl(child.fileno(),fcntl.F_DUPFD_CLOEXEC,198)
        self.log=(STATE/(name+'.log')).open('xb')
        self.process=subprocess.Popen([str(RUNTIME/'electron'),str(SOURCE/'consent-helper.cjs'),str(inherited),str(os.getpid()),str(directory),str(SOURCE/'electron-private-fd.node')],env=environment,stdin=subprocess.DEVNULL,stdout=subprocess.PIPE,stderr=self.log,pass_fds=(inherited,),start_new_session=True)
        os.close(inherited)
        child.close()
        self.pidfd=os.pidfd_open(self.process.pid)
        self.sequence=0
        self.buffer=b''
        self.changed=0
        workers.append(self)
        assert self.receive()['type']=='ready'
        flags=next(line.split()[1]for line in pathlib.Path(f'/proc/{self.process.pid}/fdinfo/198').read_text().splitlines()if line.startswith('flags:'))
        assert int(flags,8)&os.O_CLOEXEC
    def receive(self,seconds=5):
        deadline=time.monotonic()+seconds
        while b'\n' not in self.buffer:
            left=deadline-time.monotonic()
            assert left>0 and select.select([self.parent],[],[],left)[0],'Consent response deadline'
            data=self.parent.recv(65536)
            assert data,'Consent helper closed before receipt'
            self.buffer+=data
            assert len(self.buffer)<=32*1024*1024
        line,self.buffer=self.buffer.split(b'\n',1)
        return json.loads(line)
    def call(self,op,**fields):
        self.sequence+=1
        data=json.dumps({'id':self.sequence,'op':op,**fields},ensure_ascii=True,separators=(',',':')).encode()+b'\n'
        for at in range(0,len(data),7):
            self.parent.sendall(data[at:at+7])
        while True:
            response=self.receive()
            if response['type']=='changed':
                self.changed+=1
                continue
            assert response['type']=='result'and response['id']==self.sequence and response['ok'] is True,response
            return response
    def choose(self,destination):
        return self.call('choose-audio',destinationJson=json.dumps(destination,ensure_ascii=True))
    def bind(self,grant):
        return self.call('bind',grantIdJson=json.dumps(grant['id'],ensure_ascii=True))['binding']
    def allowed(self,binding):
        return self.call('check',binding=binding)['allowed']
    def close(self):
        self.call('close')
        assert self.process.wait(timeout=5)==0
        assert select.select([self.pidfd],[],[],1)[0]
        assert self.process.stdout.read()==b''
    def cleanup(self):
        if self.process.poll()is None:
            os.killpg(self.process.pid,signal.SIGKILL)
            self.process.wait(timeout=5)
        self.parent.close()
        self.process.stdout.close()
        self.log.close()
        os.close(self.pidfd)

def choice(value):
    return json.loads(value['choiceJson'])

def key_path(directory,key):
    return directory/(hashlib.sha256(key.encode('utf-16le','surrogatepass')).hexdigest()+'.consent')

def replace_record(directory,key,value):
    key_bytes=key.encode('utf-16le','surrogatepass')
    value_bytes=value.encode('utf-16le','surrogatepass')
    target=key_path(directory,key)
    temporary=target.with_name(target.name+'.fixture-'+os.urandom(8).hex())
    with temporary.open('xb')as output:
        output.write(b'OVS1'+struct.pack('<II',len(key_bytes),len(value_bytes))+key_bytes+value_bytes)
    temporary.chmod(0o600)
    temporary.rename(target)

try:
    native=subprocess.run(['/voice-node','/source/consent-native-loader.mjs'],env={**environment,'ELECTRON_RUN_AS_NODE':''},stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=30)
    (STATE/'native-oracle.log').write_bytes(native.stdout+native.stderr)
    assert native.returncode==0,'Unbundled native consent oracle failed'
    bundle=subprocess.run([str(RUNTIME/'electron'),str(SOURCE/'consent-bundle-traces.cjs')],env=environment,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=30)
    (STATE/'bundle-oracle.log').write_bytes(bundle.stdout+bundle.stderr)
    assert bundle.returncode==0,'Actual Electron consent bundle traces failed'
    native_trace=json.loads((STATE/'native-traces.json').read_text())
    bundle_trace=json.loads((STATE/'bundle-traces.json').read_text())
    assert native_trace==bundle_trace,'Whole native traces diverged'
    rows.append({'case':'whole-native-producer','traces':len(native_trace['cases']),'actualNode':'26.13.0','actualElectron':'44.10.1','actualEmbeddedNode':'24.22.1','normalizedOpaqueIdentitiesOnly':True})
    directory=STATE/'grants'
    directory.mkdir(mode=0o700)
    first=Worker('first',directory)
    second=Worker('second',directory)
    assert choice(first.call('read'))is None
    grant=choice(first.choose(ENVIRONMENT))
    first_binding=first.bind(grant)
    second_binding=second.bind(grant)
    assert first.allowed(first_binding)and second.allowed(second_binding)
    selected=choice(second.choose(ENVIRONMENT))
    assert selected['id']!=grant['id']
    assert not first.allowed(first_binding)and not second.allowed(second_binding)
    rows.append({'case':'cross-process-same-destination-reselection','oldBindingsAllowed':False,'freshNativeIdentity':True})
    cleanup=first.call('choose-cleanup',enabled=True)['cleanupJson']
    ordinary=STATE/'ordinary-preferences.json'
    ordinary.write_text(json.dumps({'voiceDestination':ENVIRONMENT,'cleanupEnabled':True}))
    second.choose(None)
    assert first.call('read')['cleanupJson']==cleanup
    assert choice(first.call('read'))=='off'
    rows.append({'case':'independent-grants-and-stale-preferences','ordinaryFileCannotRestoreConsent':True})
    original=choice(first.choose(ENVIRONMENT))
    original_binding=first.bind(original)
    pending=second.call('pending-cloud',destinationJson=json.dumps(CLOUD))
    assert choice(pending)==original and first.allowed(original_binding)
    newer=choice(first.choose(ENVIRONMENT))
    assert choice(second.call('acknowledge',acknowledgement=pending['acknowledgement']))==newer
    rows.append({'case':'late-cloud-ack','newerChoicePreserved':True})
    old=first.bind(newer)
    pending=second.call('pending-cloud',destinationJson=json.dumps(CLOUD))
    assert first.allowed(old)
    cloud=choice(second.call('acknowledge',acknowledgement=pending['acknowledgement']))
    assert cloud['destination']==CLOUD and not first.allowed(old)
    cloud_binding=first.bind(cloud)
    assert first.allowed(cloud_binding)
    marker=AUDIO+':saved:'+cloud['id']
    marker_path=key_path(directory,marker)
    marker_path.chmod(0o000)
    assert not first.allowed(cloud_binding)
    assert choice(second.call('read'))=='off'
    marker_path.chmod(0o600)
    assert first.allowed(cloud_binding)
    replace_record(directory,marker,'false')
    assert not first.allowed(cloud_binding)
    assert choice(second.call('read'))=='off'
    rows.append({'case':'fresh-marker-read','unreadableAndNegativeRefused':True,'NoAuthoritativeValueCache':True})
    # A native legacy ID and an EnvironmentId longer than64 remain code-unit
    # faithful across UTF16 storage and the double-encoded JSON control field.
    legacy={'id':'legacy\ud800','destination':{'kind':'environment','environmentId':'x'*4096+'\udfff'}}
    replace_record(directory,AUDIO,json.dumps(legacy,ensure_ascii=True))
    assert choice(first.call('read'))==legacy
    legacy_binding=second.bind(legacy)
    assert second.allowed(legacy_binding)
    rows.append({'case':'arbitrary-native-utf16','grantIdUnits':len(legacy['id']),'environmentIdUnits':4097,'bindingAllowed':True})
    replace_record(directory,AUDIO,'{corrupt')
    assert not second.allowed(legacy_binding)
    assert choice(first.call('read'))is None
    restored=choice(first.choose(ENVIRONMENT))
    assert restored['destination']==ENVIRONMENT
    rows.append({'case':'corrupt-choice','oldBindingAllowed':False,'ExplicitChoiceCanRecover':True})
    path=key_path(directory,AUDIO)
    os.link(path,directory/'retained-linked-record')
    rebound=first.bind(restored)
    assert not first.allowed(rebound)
    # The intentionally linked record remains retained; replace its one named
    # entry with a fresh inode before proceeding, without unlinking either.
    replace_record(directory,AUDIO,json.dumps(restored))
    own=first.bind(restored)
    assert first.allowed(own)
    directory.rename(STATE/'retained-original-grants')
    directory.mkdir(mode=0o700)
    assert not first.allowed(own)
    assert not second.allowed(legacy_binding)
    third=Worker('replacement',directory)
    assert choice(third.call('read'))is None
    third.choose(ENVIRONMENT)
    assert not first.allowed(own)
    rows.append({'case':'linked-record-and-namespace-replacement','oldWorkerCannotRestoreGrant':True,'linkedEntriesRetained':True})
    assert not third.allowed('unknown-binding')
    third.close()
    first.close()
    second.close()
    assert first.changed>0 and second.changed>0
    rows.append({'case':'private-helper-lifetime','workers':3,'realPeerAndCloexec':True,'cleanExitObserved':True,'stdoutBytes':0,'crossProcessNotifications':True})
    (STATE/'consent-proof.json').write_text(json.dumps({'cases':rows,'sourceOracleTraces':len(native_trace['cases']),'scope':'Whole native data producer plus real private FS/peer/Node workers; Rust consumer, GUI, capture and SDK not activated'},indent=2)+'\n')
finally:
    for worker in workers:
        worker.cleanup()
