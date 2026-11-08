"""Read the real pinned runtime through mounts; never copy or execute Electron."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tarfile
sys.path.insert(0, '/repo/tools')
import native_voice_bundle as voice

parser = argparse.ArgumentParser()
parser.add_argument('--client', action='store_true')
args = parser.parse_args()
assert Path('/run/.containerenv').exists()
bundle = Path('/package/bin/native-voice')
receipt = voice.verify(bundle)


class Sink:
    def __init__(self):
        self.sha = hashlib.sha256()
        self.bytes = 0
    def write(self, data):
        self.sha.update(data)
        self.bytes += len(data)
        return len(data)
    def flush(self):
        pass


sink = Sink()
with tarfile.open(fileobj=sink, mode='w|') as archive:
    voice.add_bundle(archive, bundle)
client_result = None
if args.client:
    client = '/package/bin/omarchy-t3-client'
    environment = {'PATH': '/usr/bin:/bin', 'LANG': 'C.UTF-8',
        'O3_NATIVE_VOICE_RUNTIME': '/invalid-partial-startup-override'}
    client_result = json.loads(subprocess.check_output([client, '__native-voice-bundle'],
        env=environment, text=True, timeout=45))
    assert client_result == voice.compiled(receipt)
print(json.dumps({'complete': True, 'runtimeMembers': 72, 'bundleMembers': 75,
    'streamedTarBytes': sink.bytes, 'streamedTarSha256': sink.sha.hexdigest(),
    'persistedRuntimeCopies': 0, 'compiledClientTested': args.client,
    'compiledClient': client_result,
    'scope': 'Actual pinned immutable runtime/addon/license packaging admission and tar stream; '
        'optional compiled client default lookup. No installation, audio, display or Electron execution.'}))
