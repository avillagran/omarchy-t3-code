#!/usr/bin/env python3
"""Metadata-only peer for the actual singleton; no auth, browser, or real RPC."""
import json
import sys

if '--inert-preflight' in sys.argv:
    raise SystemExit(0)
if 'watch' not in sys.argv:
    print(json.dumps(dict(models=[], recorder=False)), flush=True)
    raise SystemExit(0)
settings = dict(revision='1', browserProfiles=[dict(id='work', name='Work', kind='persistent')],
    browserDefaultProfileId='work', browserRecordingFrameRate=30)
read_request = None
reports = []
revision = 1

def emit(kind, value):
    print(json.dumps(dict(type=kind, data=value)), flush=True)

def target(environment, session, config, operable=True):
    emit('browser-profile-target', dict(environment=environment, sessionId=session,
        configId=config, operable=operable))

def snapshot(preferences=False, stage=''):
    value = dict(threads=[], projects=[], environments=[dict(id='a', state='ready'), dict(id='b', state='ready')],
        stale=False, working=0, pending=0, profileFixture=dict(reports=reports, stage=stage))
    if preferences:
        value['clientPreferences'] = settings
    emit('snapshot', value)

for line in sys.stdin:
    packet = json.loads(line)
    op = packet.get('op')
    if op == 'client-settings':
        if packet.get('action') == 'read':
            read_request = packet['requestId']
            target('a', 'socket-a', 'config-a')
            target('b', 'socket-b', 'config-b')
            target('denied', 'socket-denied', 'config-denied', False)
            snapshot(stage='unhydrated')
        else:
            revision += 1
            settings.update(packet['patch'])
            settings['revision'] = str(revision)
            emit('action', dict(op=op, requestId=packet['requestId'], result=settings))
    elif op == 'browser-profile-report':
        reports.append(packet)
        snapshot(stage='reported')
    elif op == 'profile-fixture':
        action = packet['action']
        if action == 'hydrate':
            assert read_request
            emit('action', dict(op='client-settings', requestId=read_request, result=settings))
        elif action == 'clone':
            snapshot(True, 'cloned')
        elif action == 'stale-close':
            emit('rpc-session-state', dict(environment='a', write=True, sessionId='obsolete', state='closed'))
            snapshot(stage='stale-close')
        elif action == 'config-change':
            target('a', 'socket-a', 'config-a2')
        elif action == 'close':
            emit('rpc-session-state', dict(environment='a', write=True, sessionId='socket-a', state='closed'))
            snapshot(stage='closed')
        elif action == 'reopen':
            target('a', 'socket-a2', 'config-a3')
        elif action == 'revoke':
            target('b', 'socket-b', 'config-b', False)
            snapshot(stage='revoked')
        elif action == 'grant':
            target('b', 'socket-b', 'config-b')
        else:
            raise AssertionError('Unknown fixed fixture action')
