# Dedicated private admission peer. No filesystem fixture removal.
import array,json,os,socket,subprocess,sys
peer=socket.socket(socket.AF_UNIX,socket.SOCK_SEQPACKET)
peer.settimeout(3)
peer.connect('\0'+sys.argv[1])
peer.send(b'{"op":"hello","version":1}')
try:
 reply=json.loads(peer.recv(4096));assert reply['type']=='admitted' and reply['version']==1
 print('admitted',flush=True)
except (OSError,ValueError,AssertionError):
 print('rejected',flush=True);sys.exit(0)
for line in sys.stdin:
 command=line.strip()
 if command=='quit':break
 if command=='transfer':
  peer.send(b'{"op":"probe","version":1}');assert peer.recv(4096)==b'alive'
  child=subprocess.run([sys.executable,'-c',"import socket,sys;s=socket.socket(fileno=int(sys.argv[1]));s.send(b'{\\\"op\\\":\\\"probe\\\",\\\"version\\\":1}');s.close()",str(peer.fileno())],pass_fds=(peer.fileno(),),check=True)
 elif command=='oversized':peer.send(b'x'*4097)
 elif command=='rights':
  descriptor=os.memfd_create('owned-admission-test',os.MFD_CLOEXEC)
  peer.sendmsg([b'{"op":"hello","version":1}'],[(socket.SOL_SOCKET,socket.SCM_RIGHTS,array.array('i',[descriptor]))]);os.close(descriptor)
 else:raise AssertionError('Unknown fixture command')
 assert peer.recv(4096)==b'';print('closed',flush=True)
peer.close()
