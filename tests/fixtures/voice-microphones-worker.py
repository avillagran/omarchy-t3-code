#!/usr/bin/env python3
"""Metadata-only transport peer; no audio devices, PCM, network or user profiles."""
import json
import os
from pathlib import Path
import sys
import uuid

if '--inert-preflight' in sys.argv:
    raise SystemExit(0)
USB = 'opaque-usb:input/alpha?channel=2'
LINE = 'opaque-line-input'
preferences = dict(voiceMicrophone='', voiceShortcutMode='auto', speechLanguage='auto',
    speechTranslateToEnglish=False, speechCustomWords=[], speechSilenceFiltering=True,
    speechRemoveFillerWords=True, speechPostProcessingPrompt=dict(mode='default', customInstructions=''),
    speechCorrectionWord='')
revision = 10
if 'voice-watch' not in sys.argv:
    for line in sys.stdin:
        packet = json.loads(line)
        if packet.get('op') == 'client-settings':
            assert packet.get('action', 'read') == 'read', 'Voice changes must use native CAS RPC'
            print(json.dumps(dict(type='action', data=dict(op='client-settings',
                requestId=packet.get('requestId'), result=dict(preferences, revision=str(revision))))), flush=True)
    raise SystemExit(0)

proof = dict(scope='Metadata transport only; no hardware, PCM or Rust CAS execution',
    permissions=[], preferences=[], captures=[], scopes=[], actions=[], staleMetadata=0, explicitChoices=0)
choice = None
prepared = {}
sequence = 0

def save():
    root = Path('/evidence')
    temporary = root / ('voice-microphones-peer-' + uuid.uuid4().hex + '.tmp')
    descriptor = os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    with os.fdopen(descriptor, 'w') as target:
        json.dump(proof, target)
    temporary.replace(root / 'voice-microphones-peer.json')

def emit(value):
    print(json.dumps(dict(type='native-voice', data=value)), flush=True)

def snapshot():
    return dict(choiceJson=json.dumps(choice), cleanupJson='null')

def reply(packet, value=None, error=None):
    emit(dict(kind='result', requestId=packet['requestId'], session=packet.get('session'),
        value=value or {}, error=error))

def microphones(session, successor=False):
    devices = [dict(deviceId='default', label='Fixture default input'),
        dict(deviceId=USB, label='<b>USB & "quoted"</b>'),
        dict(deviceId=LINE, label='Line input')]
    if successor:
        devices = [dict(deviceId=LINE, label='Successor line input'),
            dict(deviceId='default', label='Successor default input'),
            dict(deviceId=USB, label='<i>USB successor & literal</i>')]
    emit(dict(kind='microphones', session=session, devices=devices))

emit(dict(kind='ready', available=True))
emit(dict(kind='microphones', session='never-owned', devices=[dict(deviceId='foreign-before-scope', label='Foreign')]))
save()
for line in sys.stdin:
    packet = json.loads(line)
    action = packet.get('action')
    proof['actions'].append(action)
    save()
    if action in ['owner', 'abort-request']:
        continue
    if action == 'snapshot':
        reply(packet, snapshot())
    elif action == 'decode-preferences':
        raw = json.loads(packet['valueJson'])
        value = dict(preferences)
        value.update({key: raw[key] for key in preferences if key in raw})
        reply(packet, dict(valueJson=json.dumps(value)))
    elif action == 'update-preferences':
        assert packet['revision'] == str(revision), 'Actual revision must reach the CAS boundary'
        patch = packet['patch']
        assert set(patch) == {'voiceMicrophone'} and patch['voiceMicrophone'] in ['', USB]
        proof['preferences'].append(dict(patch=patch, revision=packet['revision']))
        preferences.update(patch)
        revision += 1
        reply(packet, dict(preferences, revision=str(revision)))
    elif action == 'choose-audio':
        destination = json.loads(packet['destinationJson'])
        assert destination == dict(kind='environment', environmentId='env')
        proof['explicitChoices'] += 1
        choice = dict(id='explicit-fixture-grant', destination=destination)
        reply(packet, snapshot())
    elif action == 'project-context':
        reply(packet, dict(valueJson=json.dumps(dict(speechProjectCustomWords=[]))))
    elif action == 'begin':
        sequence += 1
        session = 'microphone-scope-' + str(sequence)
        proof['scopes'].append(session)
        reply(packet, dict(session=session, consent=snapshot()))
    elif action == 'status':
        reply(packet, dict(valueJson=json.dumps(dict(supported=True, supportsStreaming=True,
            supportsBufferedStreaming=False, supportsTranslation=False))))
    elif action in ['configure', 'stream-ticket', 'stream-connect', 'record']:
        reply(packet)
    elif action == 'stop':
        reply(packet, dict(uri=None))
    elif action == 'capture-prepare':
        prepared[packet['session']] = packet
        proof['captures'].append(dict(session=packet['session'], microphoneId=packet['microphoneId']))
        emit(dict(kind='permission', session=packet['session'], permissionId='metadata-permission-' + packet['session']))
    elif action == 'permission':
        held = prepared.pop(packet['session'])
        assert packet['allow'] is True, 'Fixture requires its explicit permission action'
        proof['permissions'].append(dict(session=packet['session'], allow=True))
        microphones(packet['session'], successor=len(proof['scopes']) > 1)
        if len(proof['scopes']) > 1:
            emit(dict(kind='microphones', session=proof['scopes'][0],
                devices=[dict(deviceId='late-old-device', label='<b>Old scope</b>')]))
            proof['staleMetadata'] += 1
        reply(held)
        reply(packet)
        emit(dict(kind='stream', session=packet['session'], value=dict(event='ready')))
    elif action in ['abort', 'dispose', 'release-capture', 'delete-recording']:
        held = prepared.pop(packet.get('session'), None)
        if held:
            reply(held, error='voice_cancelled')
        if action != 'abort':
            reply(packet)
    else:
        proof['protocolError'] = action
        save()
        raise RuntimeError('Unexpected metadata fixture action ' + str(action))
    save()
