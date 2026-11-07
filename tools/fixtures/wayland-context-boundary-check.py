"""Private native-wire checks. Wire bytes stay in memory and are never logged."""
import importlib.util
import json
import os
import pathlib
import resource
import select
import socket
import struct
import subprocess

assert resource.getrlimit(resource.RLIMIT_CORE) == (0, 0)
spec = importlib.util.spec_from_file_location('context_boundary', '/fixture/wayland-context-boundary.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
parent, child = socket.socketpair()
process = subprocess.Popen(['/fixture/wire-client', str(child.fileno())], pass_fds=(child.fileno(),),
                           stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
child.close()
checks = 0


def expect(value, label):
    global checks
    if not value:
        raise AssertionError(label)
    checks += 1


def response():
    expect(bool(select.select([process.stdout], [], [], 3)[0]), 'native client response deadline')
    line = process.stdout.readline()
    expect(0 < len(line) <= 1024, 'bounded typed native client response')
    return json.loads(line)


def harvest():
    result = bytearray()
    while select.select([parent], [], [], 0)[0]:
        chunk = parent.recv(65536)
        expect(bool(chunk), 'native marshaller must remain alive')
        result.extend(chunk)
        expect(len(result) <= 65536, 'bounded synthetic native corpus')
    return bytes(result)


def command(value):
    process.stdin.write((value + '\n').encode())
    process.stdin.flush()
    expect(response()['kind'] == 'sent', 'actual native API completed')
    return harvest()


def event(object_id, opcode, *arguments):
    # Server lifecycle inputs follow the public v1 protocol. Client request
    # encodings come independently from actual generated libwayland bindings.
    return struct.pack('<II', object_id, ((8 + 4 * len(arguments)) << 16) | opcode) + struct.pack('<' + 'I' * len(arguments), *arguments)


def fresh(initial, expected=True):
    published, invalidated = [], []
    boundary = module.ContextBoundary({'owned-context-wire-nonce': 'owned-lease'} if expected else {},
                                      published.append, lambda *row: invalidated.append(row))
    boundary.feed('client', initial)
    boundary.feed('server', event(ids['input'], 0, ids['surface']))
    return boundary, published, invalidated


try:
    ids = response()
    expect(ids['kind'] == 'ids', 'actual public proxy IDs observed')
    initial = harvest()
    state = command('state')
    moved = command('move')
    empty = command('empty')
    rectangle_only = command('rectangle-only')
    renamed = command('rename')
    disabled = command('disable')
    bad = command('bad-boundary')
    destroyed = command('destroy-context')
    boundary, published, invalidated = fresh(initial)
    boundary.feed('client', state)
    expect(not published, 'state is not published before native done acknowledgement')
    boundary.feed('server', event(ids['input'], 5, 0))
    expect(not published, 'older done serial cannot publish the newest context')
    boundary.feed('server', event(ids['input'], 5, 1))
    expect(len(published) == 1, 'one acknowledged native state')
    value = published[-1]
    expect(value.rectangle == (-3, 17, 0, 18), 'real signed native rectangle survives')
    expect(value.text == 'A😀日本ZQ' and value.cursor_byte == value.anchor_byte == 13,
           'native UTF8 byte offsets preserve supplementary Unicode')
    metadata = json.dumps(value.metadata())
    expect('A😀' not in metadata and '日本' not in metadata, 'metadata never logs native text')
    boundary.feed('client', moved)
    boundary.feed('server', event(ids['input'], 5, 2))
    expect(published[-1].rectangle == (12, 34, 1, 19) and published[-1].cursor_byte == 11,
           'native state updates current selection and caret')
    boundary.feed('client', empty)
    boundary.feed('server', event(ids['input'], 5, 3))
    expect(published[-1].text == '' and published[-1].metadata()['surroundingAvailable'],
           'known empty native text is distinct from unknown')
    boundary.feed('client', rectangle_only)
    boundary.feed('server', event(ids['input'], 5, 4))
    expect(published[-1].text is None and published[-1].rectangle == (22, 33, 0, 18),
           'new activation does not invent or retain surrounding text')
    count = len(published)
    boundary.feed('client', renamed)
    boundary.feed('client', state)
    boundary.feed('server', event(ids['input'], 5, 5))
    expect(len(published) == count, 'foreign window identity cannot receive context')
    expect(any(row[1] == 'native-window-identity-changed' for row in invalidated),
           'window identity replacement invalidates existing query state')

    foreign, values, _ = fresh(initial, expected=False)
    foreign.feed('client', state)
    foreign.feed('server', event(ids['input'], 5, 1))
    expect(not values, 'unadmitted root never publishes private native text')
    for split in range(len(initial) + 1):
        values = []
        test = module.ContextBoundary({'owned-context-wire-nonce': 'owned-lease'}, values.append, lambda *row: None)
        test.feed('client', initial[:split])
        test.feed('client', initial[split:])
        test.feed('server', event(ids['input'], 0, ids['surface']))
        test.feed('client', state)
        test.feed('server', event(ids['input'], 5, 1))
        expect(len(values) == 1 and values[0].rectangle == (-3, 17, 0, 18),
               'every native registry/object stream split preserves identity')
    for split in range(len(state) + 1):
        test, values, _ = fresh(initial)
        test.feed('client', state[:split])
        test.feed('client', state[split:])
        test.feed('server', event(ids['input'], 5, 1))
        expect(len(values) == 1 and values[0].cursor_byte == 13,
               'every native text/header/Unicode split preserves context')
    test, values, invalidated = fresh(initial)
    test.feed('client', state)
    test.feed('server', event(ids['input'], 5, 1))
    count = len(values)
    test.feed('server', event(ids['input'], 1, ids['surface'] + 10000))
    test.feed('client', moved)
    test.feed('server', event(ids['input'], 5, 2))
    expect(len(values) == count + 1, 'unrelated leave does not retire the admitted surface')
    test.feed('server', event(ids['input'], 1, ids['surface']))
    count = len(values)
    test.feed('server', event(ids['input'], 5, 2))
    expect(len(values) == count, 'actual surface leave fences late acknowledgements')
    expect(any(row[1] == 'native-context-leave' for row in invalidated), 'real leave invalidates native cache')
    test, values, invalidated = fresh(initial)
    test.feed('client', state)
    test.feed('client', disabled)
    test.feed('server', event(ids['input'], 5, 1))
    expect(not values, 'disable fences older native done')
    test.feed('client', destroyed)
    expect(ids['input'] not in test.contexts, 'native object destruction retires context identity')
    test, values, invalidated = fresh(initial)
    try:
        test.feed('client', bad)
        raise AssertionError('invalid UTF8 byte boundary was accepted')
    except module.BoundaryError:
        expect(test.disabled and not values, 'malformed native offsets retire observation without publishing')
    test, values, _ = fresh(initial)
    try:
        test.feed('client', struct.pack('<II', 1, 7 << 16))
        raise AssertionError('invalid native header was accepted')
    except module.BoundaryError:
        expect(test.disabled and not values, 'invalid framing cannot leave a live query cache')
    test, values, _ = fresh(initial)
    opaque = struct.pack('<II', 424242, (12 << 16) | 77) + b'\xff\x00\x01\x02'
    test.feed('client', opaque)
    test.feed('client', state)
    test.feed('server', event(ids['input'], 5, 1))
    expect(len(values) == 1, 'unrelated protocol payload stays opaque')
    test.close()
    count = len(values)
    test.feed('server', event(ids['input'], 5, 1))
    expect(len(values) == count and not test.contexts, 'closed socket scope cannot reactivate')
    print('WAYLAND_CONTEXT_NATIVE_WIRE_CHECKS ' + json.dumps({'checks': checks, 'rawWireLogged': False,
           'nativeClientMarshaller': True, 'serverLifecycle': 'synthetic v1 protocol', 'qtQueryImplemented': False}))
finally:
    if process.poll() is None:
        process.stdin.write(b'quit\n')
        process.stdin.flush()
        expect(process.wait(timeout=3) == 0, 'owned native wire client exits normally')
    parent.close()
