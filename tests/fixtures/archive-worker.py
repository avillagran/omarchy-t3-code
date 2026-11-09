#!/usr/bin/env python3
"""Inert archived-row transport peer; no server, credentials or deletion."""
import json
from pathlib import Path
import sys

if not Path('/run/.containerenv').is_file():
    raise SystemExit('This fixture requires rootless Podman')
if '--inert-preflight' in sys.argv:
    raise SystemExit(0)

def emit(kind, data):
    print(json.dumps(dict(type=kind, data=data)), flush=True)

if 'watch' in sys.argv:
    emit('ui-state', {})
    emit('snapshot', dict(environments=[], projects=[], threads=[], stale=False,
        working=0, pending=0))
for line in sys.stdin:
    # All action outcomes are supplied explicitly by the QML fixture. Receiving
    # an archive command can never reach an RPC, shell, filesystem or profile.
    packet = json.loads(line)
    if packet.get('op') == 'quit':
        break
