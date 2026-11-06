#!/usr/bin/env python3
"""Stage exact native modules/tests for a private metadata oracle; never copy a working tree."""
import argparse, hashlib, json
from pathlib import Path, PurePosixPath
import subprocess

parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('checkout');parser.add_argument('output')
args=parser.parse_args()
revision='32a0531748c44b25072d7495ce0b9bd7e9450a87'
output=Path(args.output)
output.mkdir(parents=True,exist_ok=True)
if list(output.iterdir()):raise SystemExit('Oracle output must be empty')
packages=['contracts','shared','client-runtime']
paths=[item for package in packages for item in ['packages/'+package+'/src','packages/'+package+'/package.json']]
def git(*arguments):return subprocess.check_output(['git','-C',args.checkout,*arguments])
files=git('ls-tree','-r','--name-only',revision,'--',*paths).decode().splitlines()
manifest={}
for name in files:
    relative=PurePosixPath(name)
    if relative.is_absolute() or '..' in relative.parts:raise SystemExit('Invalid source path')
    mode=git('ls-tree',revision,'--',name).decode().split()[0]
    if mode not in ['100644','100755']:raise SystemExit('Oracle inputs must be regular source files: '+name)
    data=git('show',revision+':'+name)
    destination=output/name;destination.parent.mkdir(parents=True,exist_ok=True);destination.write_bytes(data)
    manifest[name]=hashlib.sha256(data).hexdigest()
(output/'package.json').write_text('{"name":"native-metadata-oracle","private":true,"type":"module"}\n')
(output/'vitest.config.mjs').write_text('export default {cacheDir:"/tmp/vite-cache",test:{environment:"node",watch:false,fileParallelism:false,maxWorkers:1,testTimeout:10000,hookTimeout:10000,cache:false}};\n')
(output/'node_modules').symlink_to('/tmp/native-node_modules')
(output/'SOURCE.json').write_text(json.dumps({'revision':revision,'files':manifest},indent=2)+'\n')
print('Staged',len(manifest),'exact native source/test files at',revision)
