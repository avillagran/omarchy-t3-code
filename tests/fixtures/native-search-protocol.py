"""Actual admitted Electron worker protocol, native ranking and owned retirement."""
from pathlib import Path
import json, os, selectors, subprocess, time

assert Path('/run/.containerenv').is_file()
directory = Path('/tmp/native-search-case')
directory.mkdir(mode=0o700)
source = (Path('/source/NativeSearchRuntime.cjs').read_text() + '\n' +
          Path('/source/search-worker.cjs').read_text())
environment = {'ELECTRON_RUN_AS_NODE': '1', 'HOME': str(directory),
               'TMPDIR': str(directory), 'PATH': '/usr/bin:/bin', 'LANG': 'C.UTF-8'}
child = subprocess.Popen(['/electron/electron', '--eval', source], env=environment,
                         cwd=directory, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                         stderr=subprocess.PIPE)
selector = selectors.DefaultSelector()
selector.register(child.stdout, selectors.EVENT_READ)
buffered = bytearray()
sequence = 0
scenarios = 0

def packet():
    deadline = time.monotonic() + 5
    while b'\n' not in buffered:
        remaining = deadline - time.monotonic()
        assert remaining > 0 and selector.select(remaining), 'Actual worker reply timed out'
        data = os.read(child.stdout.fileno(), 65536)
        assert data, 'Actual worker pipe ended before reply'
        buffered.extend(data)
    line, rest = buffered.split(b'\n', 1)
    buffered[:] = rest
    return json.loads(line)

def call(op, **values):
    global sequence
    sequence += 1
    request = {'id': str(sequence), 'op': op, **values}
    child.stdin.write(json.dumps(request, ensure_ascii=True).encode() + b'\n')
    child.stdin.flush()
    response = packet()
    assert response['id'] == request['id']
    return response

def result(op, **values):
    response = call(op, **values)
    assert 'error' not in response, response
    return response['result']

def row(key, title, opens=None):
    thread = {'id': key, 'environmentId': 'private', 'title': title,
              'createdAt': '2026-10-01T00:00:00.000Z',
              'updatedAt': '2026-10-02T00:00:00.000Z'}
    if opens is not None:
        thread['opens'] = opens
    return {'key': key, 'thread': thread, 'projectTitle': 'Private project',
            'environmentLabel': None}

def item(key, title):
    return {'value': 'thread:' + key, 'threadKey': key, 'title': title,
            'searchTerms': [title], 'kind': 'action'}

now = 1794160800000
rows = [row('mid', 'Fix oauth redirect', {'openedAt': now - 5000, 'score': 10000, 'picks': []}),
        row('prefix', 'Authentication cleanup'), row('whole', 'Share auth across worktrees')]
items = [item(value['key'], value['thread']['title']) for value in rows]
retired = False
try:
    assert packet() == {'type': 'ready', 'protocol': 'o3-native-search-v1', 'electron': '44.10.1'}
    assert result('snapshot', rows=rows)['updated'] is True
    ranked = result('palette', query='auth', now=now, activeGroups=[], threadSearchItems=items)
    assert [value['value'] for value in ranked['groups'][0]['items']] == [
        'thread:whole', 'thread:prefix', 'thread:mid']
    scenarios += 1

    action = {'value': 'new', 'title': 'New thread', 'kind': 'action', 'searchTerms': ['New thread']}
    ranked = result('palette', query='>new', now=now,
                    activeGroups=[{'value': 'actions', 'label': 'Actions', 'items': [action]}],
                    threadSearchItems=items)
    assert [(group['value'], [value['value'] for value in group['items']])
            for group in ranked['groups']] == [('actions', ['new'])]
    scenarios += 1

    first_open = result('open', key='whole', query='  CÁFÉ  日本 😀  ', now=now)
    assert first_open == {'send': True, 'searchQuery': 'cafe 日本 😀'}
    assert result('open', key='whole', now=now + 1000) == {'send': False}
    recent = result('palette', query='', now=now + 1000, activeGroups=[
        {'value': 'recent-threads', 'label': 'Recent threads', 'items': items}])
    assert recent['groups'][0]['items'][0]['value'] == 'thread:whole'
    scenarios += 1

    first = result('recordOpen', previous=None, query='  CÁFÉ ', now=now)
    second = result('recordOpen', previous=first, query='next', now=now + 1000)
    assert first == {'openedAt': now, 'score': 1, 'picks': [{'query': 'cafe', 'at': now}]}
    assert second == {'openedAt': now, 'score': 1,
                      'picks': [{'query': 'next', 'at': now + 1000}, {'query': 'cafe', 'at': now}]}
    scenarios += 1

    message_rows = [row('m' + str(index), 'Topic ' + str(index)) for index in range(11)]
    result('snapshot', rows=rows + message_rows)
    matches = [{**item('m' + str(index), 'Topic ' + str(index)), 'quality': 1,
                'hits': 10 - index, 'messageCreatedAt': '2026-11-01T00:00:00Z'}
               for index in range(10)]
    before = result('palette', query='needle', messageQuery='needle', now=now,
                    activeGroups=[], messageMatches=matches)
    previous_keys = [value['value'] for value in before['groups'][0]['items']]
    late = {**item('m10', 'Topic 10'), 'quality': 1, 'hits': 1000}
    after = result('palette', query='needle', messageQuery='needle', now=now,
                   activeGroups=[], messageMatches=matches + [late], shownOrder=before['shownOrder'])
    after_keys = [value['value'] for value in after['groups'][0]['items']]
    assert len(previous_keys) == 10 and after_keys[:10] == previous_keys
    assert after_keys[10:] == ['thread:m10'] and after['shownOrder']['shown'] == 11
    scenarios += 1

    accent = {**matches[0], 'snippet': 'Cafeteria / Café / fixed', 'messageId': 'message-42'}
    found = result('palette', query='fix cafe', messageQuery='fix cafe', now=now,
                   activeGroups=[], messageMatches=[accent])
    assert found['groups'][0]['items'][0]['find'] == {
        'query': 'Café', 'start': {'entryId': 'message-42', 'occurrence': 0}}
    scenarios += 1

    unrelated = result('palette', query='unrelated', messageQuery='needle', now=now,
                       activeGroups=[], messageMatches=matches)
    assert unrelated['groups'] == []
    scenarios += 1

    assert call('snapshot', rows={})['error'] == 'native_search_request_failed'
    assert result('sidebar', query='auth', keys=['mid', 'prefix', 'whole'], now=now)['keys'] == [
        'whole', 'prefix', 'mid']
    scenarios += 1

    raw = call('recordOpen', payloadJson=json.dumps({'previous': None, 'now': now, 'query': 'x\udc00'}))
    assert 'error' not in raw and '\\udc00' in raw['resultJson']
    assert json.loads(raw['resultJson'])['picks'][0]['query'] == 'x\udc00'
    scenarios += 1

    assert result('quit')['closed'] is True
    child.stdin.close()
    assert child.wait(timeout=5) == 0
    retired = True
    assert child.stderr.read() == b''
    scenarios += 1
    print('NATIVE_SEARCH_PROTOCOL_PASS ' + json.dumps({'scenarios': scenarios,
          'requests': sequence, 'actualChildReaped': retired, 'electron': '44.10.1',
          'qualification': 'Real admitted worker protocol; Rust manager and Qt consumer remain separate gates.'}), flush=True)
finally:
    if child.poll() is None:
        try:
            call('quit')
            child.stdin.close()
        except (BrokenPipeError, AssertionError, ValueError):
            pass
        # Keep this exact child until actual exit. The outer namespace bound
        # remains authoritative if a failed worker cannot retire normally.
        while child.poll() is None:
            try:
                child.wait(timeout=1)
            except subprocess.TimeoutExpired:
                pass
    child.wait()
    selector.close()
