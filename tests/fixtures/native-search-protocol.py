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

    # The original sidebar separates title matches from message-only rows.
    original_rows = [row('thread-1', 'Fix workspace search'), row('thread-2', 'Review providers'),
                     row('thread-3', 'WORKTREE cleanup'), row('thread-4', 'Homework notes')]
    clocks = ['2026-10-20T00:00:00.000Z', '2026-10-22T00:00:00.000Z',
              '2026-09-01T00:00:00.000Z', '2026-10-22T00:00:00.000Z']
    for value, clock in zip(original_rows, clocks):
        value['thread'].update(createdAt='2026-01-01T00:00:00.000Z', updatedAt=clock,
                               latestUserMessageAt=clock, branch=None, opens=None)
        value['projectTitle'] = 'Work project'
    result('snapshot', rows=original_rows)
    content = [{'threadId': 'thread-1', 'environmentId': 'private', 'snippet': 'Work details', 'hits': 2},
               {'threadId': 'thread-2', 'environmentId': 'private', 'snippet': 'work', 'hits': 1}]
    ordered = result('sidebar', query='work', keys=[value['key'] for value in original_rows],
                     messageQuery='work', messageMatches=content, now=1792713600000)
    assert ordered['titleKeys'] == ['thread-1', 'thread-3', 'thread-4']
    assert ordered['keys'] == ['thread-1', 'thread-3', 'thread-4', 'thread-2']
    assert [match['key'] for match in ordered['contentMatches']] == ['thread-2']
    scenarios += 1

    branch = row('branch', 'Unrelated title')
    branch['thread']['branch'] = 'feature/日本語-café'
    result('snapshot', rows=[branch])
    assert result('sidebar', query='日本語 cafe', keys=['branch'])['keys'] == ['branch']
    assert result('sidebar', query='Private project', keys=['branch'])['keys'] == []
    scenarios += 1

    topics = [row('s' + str(index), 'Topic ' + str(index)) for index in range(3)]
    result('snapshot', rows=topics)
    content = [{'threadId': 's' + str(index), 'environmentId': 'private',
                'snippet': 'needle', 'quality': 1, 'hits': 10-index} for index in range(2)]
    first = result('sidebar', query='needle', keys=['s0', 's1', 's2'],
                   messageQuery='needle', messageMatches=content)
    late = {'threadId': 's2', 'environmentId': 'private', 'snippet': 'needle', 'quality': 1, 'hits': 1000}
    second = result('sidebar', query='needle', keys=['s0', 's1', 's2'],
                    messageQuery='needle', messageMatches=content+[late], shownOrder=first['shownOrder'])
    assert first['keys'] == ['s0', 's1'] and second['keys'] == ['s0', 's1', 's2']
    scenarios += 1

    accent = {**content[0], 'messageId': 'entry-native', 'snippet': 'Cafeteria / Café / fixed'}
    found = result('sidebar', query='fix cafe', keys=['s0'],
                   messageQuery='fix cafe', messageMatches=[accent])
    assert found['contentMatches'][0]['find'] == {'query': 'Café', 'start': {'entryId': 'entry-native', 'occurrence': 0}}
    scenarios += 1

    unrelated = result('sidebar', query='billing', keys=['s0', 's1', 's2'],
                       messageQuery='needle', messageMatches=content)
    assert unrelated['keys'] == [] and unrelated['contentMatches'] == []
    scenarios += 1

    assert result('query', query='ab') == {'query': 'ab', 'searchMessages': False, 'canSearch': False, 'visibleQuery': ''}
    assert result('query', query='日本') == {'query': '日本', 'searchMessages': True, 'canSearch': True, 'visibleQuery': ''}
    assert result('query', query='ab', anyOlderSettled=True)['canSearch'] is True
    assert result('query', query='needle', titlesOnly=True)['searchMessages'] is False
    assert result('query', query='  ' + 'x' * 220 + '  ')['query'] == 'x' * 200
    scenarios += 1

    assert result('query', query='CÁFÉ', previousQuery='cafe reconnect')['visibleQuery'] == 'cafe reconnect'
    assert result('query', query='billing', previousQuery='cafe reconnect')['visibleQuery'] == ''
    packed = call('query', payloadJson=json.dumps({'query': 'a' * 199 + '😀'}))
    assert json.loads(packed['resultJson'])['query'] == 'a' * 199 + '\ud83d'
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
