"""Own all exact Electron/core/sidebar/worker cases inside one private namespace."""
from pathlib import Path
import os,subprocess,json,time
assert Path('/run/.containerenv').is_file()
environment={'ELECTRON_RUN_AS_NODE':'1','HOME':'/nonexistent','TMPDIR':'/tmp','PATH':'/usr/bin:/bin','LANG':'C.UTF-8'}
for name,command in [
    ('core',['/electron/electron','/case/core.cjs']),
    ('original-sidebar',['/electron/electron','/case/sidebar-original.cjs']),
    ('worker',['python3','/case/protocol.py']),
]:
    child=subprocess.Popen(command,stdin=subprocess.DEVNULL,stdout=subprocess.PIPE,stderr=subprocess.PIPE,env=environment)
    stdout,stderr=child.communicate()
    print(name+' stdout '+stdout.decode(),flush=True)
    print(name+' stderr '+stderr.decode(),flush=True)
    assert child.returncode==0,(name,child.returncode)
print('NATIVE_SIDEBAR_COMPONENT_PASS',flush=True)
