#!/usr/bin/env python3
"""Synthetic metadata transport only; never opens capture/network/filesystem profiles."""
import json
import os
import sys

if '--inert-preflight' in sys.argv:
    raise SystemExit(0)
preferences = dict(voiceMicrophone='', voiceShortcutMode='auto', speechLanguage='auto',
                  speechTranslateToEnglish=False, speechCustomWords=[], speechSilenceFiltering=True,
                  speechRemoveFillerWords=True, speechPostProcessingPrompt=dict(mode='default', customInstructions=''),
                  speechCorrectionWord='')
if 'voice-watch' not in sys.argv:
    for line in sys.stdin:
        packet = json.loads(line)
        if packet.get('op') == 'quit':
            break
        if packet.get('op') == 'client-settings':
            print(json.dumps({'type':'action', 'data':{'op':'client-settings', 'requestId':packet.get('requestId'), 'result':dict(preferences, revision='0')}}), flush=True)
    raise SystemExit(0)

choice, cleanup, revision, sequence = None, None, 0, 0
prepared = {}

def emit(value):
    print(json.dumps({'type':'native-voice', 'data':value}), flush=True)

def snapshot():
    return dict(choiceJson=json.dumps(choice), cleanupJson=json.dumps(cleanup))

def reply(packet, value=None, error=None):
    emit(dict(kind='result', requestId=packet['requestId'], session=packet.get('session'), value=value or {}, error=error))

emit(dict(kind='ready', available=True))
for line in sys.stdin:
    packet = json.loads(line)
    action = packet.get('action')
    if action == 'owner':
        continue
    if action == 'abort':
        held = prepared.pop(packet.get('session'), None)
        if held:
            reply(held, error='voice_cancelled')
        continue
    if action == 'abort-request':
        continue
    if action == 'snapshot':
        reply(packet, snapshot())
    elif action == 'decode-preferences':
        raw = json.loads(packet['valueJson'])
        value = dict(preferences)
        value.update({key:raw[key] for key in preferences if key in raw})
        reply(packet, dict(valueJson=json.dumps(value)))
    elif action == 'update-preferences':
        preferences.update(packet['patch']); revision += 1
        reply(packet, dict(preferences, revision=str(revision)))
    elif action == 'choose-audio':
        destination = json.loads(packet['destinationJson'])
        if destination is not None and destination.get('kind') != 'environment':
            raise RuntimeError('No synthetic cloud-key workflow is admitted')
        sequence += 1
        choice = dict(id='grant-'+str(sequence), destination=destination) if destination else 'off'
        reply(packet, snapshot())
    elif action == 'choose-cleanup':
        sequence += 1
        cleanup = 'cleanup-'+str(sequence) if packet['enabled'] else None
        reply(packet, snapshot())
    elif action == 'project-context':
        reply(packet, dict(valueJson=json.dumps(dict(speechProjectCustomWords=[]))))
    elif action == 'begin':
        sequence += 1
        reply(packet, dict(session='session-'+str(sequence), consent=snapshot()))
    elif action == 'status':
        reply(packet, dict(valueJson=json.dumps(dict(supported=True, supportsStreaming=True, supportsBufferedStreaming=False, supportsTranslation=True))))
    elif action == 'stream-ticket':
        reply(packet)
    elif action == 'stream-connect':
        reply(packet)
    elif action == 'stream-finish':
        reply(packet, dict(text='spoken'))
    elif action == 'configure':
        reply(packet)
    elif action == 'capture-prepare':
        prepared[packet['session']] = packet
        emit(dict(kind='permission', session=packet['session'], permissionId='permission-'+packet['session']))
    elif action == 'permission':
        held = prepared.pop(packet['session'], None)
        if held:
            reply(held, error=None if packet['allow'] else 'capture_permission_denied')
        reply(packet)
        if held and packet['allow']:
            emit(dict(kind='stream', session=packet['session'], value=dict(event='ready')))
    elif action == 'record':
        reply(packet)
        emit(dict(kind='level', session=packet['session'], value=0.65))
        emit(dict(kind='stream', session=packet['session'], value=dict(event='text', text=dict(committed='<b>confirmed</b> ', tentative='<i>partial</i>'))))
    elif action == 'stop':
        reply(packet, dict(uri='o3-voice-recording:'+packet['session']))
    elif action == 'transcribe':
        reply(packet, dict(text='spoken'))
    elif action == 'post-process':
        reply(packet, dict(text='clean spoken'))
    elif action in ['release-capture', 'delete-recording', 'dispose']:
        reply(packet)
    else:
        raise RuntimeError('Unexpected synthetic action '+str(action))
