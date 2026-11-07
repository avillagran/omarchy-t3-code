"""Private native-platform context baseline; executed only inside rootless Podman."""
import json
import hashlib
import os
import pathlib
import resource
import selectors
import select
import subprocess
import time

assert resource.getrlimit(resource.RLIMIT_CORE) == (0, 0)
for record in json.loads(pathlib.Path('/fixture/runtime.json').read_text())['files']:
    file = pathlib.Path('/electron') / record['path']
    assert not file.is_symlink() and file.is_file()
    assert file.stat().st_size == record['bytes']
    assert bool(file.stat().st_mode & 0o111) == record['executable']
    assert hashlib.sha256(file.read_bytes()).hexdigest() == record['sha256']
os.mkdir('/tmp/home', 0o700)
os.mkdir('/tmp/runtime', 0o700)
pathlib.Path('/tmp/sway.conf').write_text('xwayland disable\noutput * mode 1280x1024\nseat seat0 fallback true\ndefault_border none\n')
environment = dict(os.environ, WLR_BACKENDS='headless', WLR_RENDERER='pixman',
                   WLR_HEADLESS_OUTPUTS='1', WLR_LIBINPUT_NO_DEVICES='1',
                   LD_LIBRARY_PATH='/opt/sway-runtime/usr/lib')
display = subprocess.Popen(['/opt/sway-runtime/usr/bin/sway', '--config', '/tmp/sway.conf'],
                           env=environment, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
engine = peer = keyboard = pointer = None
popup_retired = False
retire_popup = None
try:
    for _ in range(100):
        sockets = [file for file in pathlib.Path('/tmp/runtime').glob('wayland-*') if file.is_socket()]
        if sockets:
            break
        assert display.poll() is None, 'owned compositor exited'
        time.sleep(.02)
    else:
        raise AssertionError('private Wayland display unavailable')
    environment = dict(os.environ, WAYLAND_DISPLAY=sockets[0].name,
                       XDG_SESSION_TYPE='wayland',
                       SWAYSOCK=str(next(pathlib.Path('/tmp/runtime').glob('sway-ipc.*.sock'))))
    environment.pop('DISPLAY', None)
    peer = subprocess.Popen(['/fixture/native-ime-context-peer'], env=environment,
                            stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                            stderr=subprocess.DEVNULL, bufsize=0)
    poller = selectors.DefaultSelector()
    poller.register(peer.stdout, selectors.EVENT_READ, 'peer')
    buffers, records = {}, []

    def wait_for(predicate, seconds=8):
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            if predicate():
                return
            for key, _ in poller.select(.05):
                data = os.read(key.fd, 4096)
                assert data, ('owned native process stdout closed', key.data,
                              engine.poll() if engine is not None else None)
                source = key.data
                buffers[source] = buffers.get(source, b'') + data
                assert len(buffers[source]) <= 16384, 'bounded native metadata line'
                while b'\n' in buffers[source]:
                    line, buffers[source] = buffers[source].split(b'\n', 1)
                    record = json.loads(line)
                    assert record.get('kind') in ['ready', 'engine-ready', 'context', 'rectangle', 'renderer-gone', 'fixture-error', 'peer-error', 'diagnostic', 'capture', 'source-click-complete', 'popup-closed']
                    assert record.get('kind') not in ['renderer-gone', 'fixture-error', 'peer-error'], record
                    records.append(dict(record, source=source))
                    assert len(records) <= 256, 'bounded synthetic context trace'
                    print(json.dumps(records[-1]), flush=True)
        raise AssertionError('native context condition unavailable')

    wait_for(lambda: any(row['kind'] == 'ready' for row in records))
    def retire_popup():
        global popup_retired
        if popup_retired:
            return
        assert peer.poll() is None, 'observer must own popup retirement'
        peer.stdin.write(b'close-popup\n')
        peer.stdin.flush()
        wait_for(lambda: any(row['kind'] == 'popup-closed' for row in records), seconds=3)
        popup_retired = True
    keyboard = subprocess.Popen(['wtype', '-k', 'Shift_L', '-s', '20000'],
                                env=environment, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    # Exact native platform selection derives from the real private session
    # environment. No ozone/IME/graphics command-line switch is added.
    engine = subprocess.Popen(['/electron/electron', '--disable-setuid-sandbox',
                                '/fixture/native-ime-context.cjs'], env=environment,
                               stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                               stderr=subprocess.DEVNULL, bufsize=0)
    poller.register(engine.stdout, selectors.EVENT_READ, 'engine')
    wait_for(lambda: any(row['kind'] == 'engine-ready' for row in records))
    control = dict(environment, LD_LIBRARY_PATH='/opt/sway-runtime/usr/lib')
    mapped = {}
    def owning_window_mapped():
        tree = subprocess.check_output(['/opt/sway-runtime/usr/bin/swaymsg', '-r', '-t', 'get_tree'],
                                       env=control, timeout=2)
        assert len(tree) <= 1024 * 1024
        def visit(node):
            if node.get('pid') == engine.pid and node.get('name') == 'Owned native context baseline':
                mapped.update(node)
            for child in node.get('nodes', []) + node.get('floating_nodes', []):
                visit(child)
        visit(json.loads(tree))
        return bool(mapped)
    wait_for(owning_window_mapped)
    subprocess.run(['/opt/sway-runtime/usr/bin/swaymsg', '[con_id='+str(mapped['id'])+']', 'focus'],
                    env=control, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    focus_start = len(records)
    engine.stdin.write(b'inspect\n')
    engine.stdin.flush()
    wait_for(lambda: any(row['kind'] == 'diagnostic' and row['windowFocused']
                         and row['parentFocused'] for row in records[focus_start:]))
    engine.stdin.write(b'capture\n')
    engine.stdin.flush()
    wait_for(lambda: any(row['kind'] == 'capture' for row in records))
    pointer = subprocess.Popen(['/fixture/pointer'], env=dict(environment, O3_PRIVATE_INPUT_FIXTURE='1'),
                               stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
    assert select.select([pointer.stdout], [], [], 3)[0], 'owned pointer ready timeout'
    assert pointer.stdout.readline() == b'ready\n'
    rect = mapped['rect']
    for command in ['move '+str(rect['x']+50)+' '+str(rect['y']+70), 'press', 'release']:
        pointer.stdin.write((command+'\n').encode())
        pointer.stdin.flush()
        assert select.select([pointer.stdout], [], [], 3)[0], 'owned pointer acknowledgement timeout'
        assert pointer.stdout.readline() == b'ok\n'
    if os.environ.get('O3_CONTEXT_SOURCE_INPUT') == '1':
        engine.stdin.write(b'source-click\n')
        engine.stdin.flush()
        wait_for(lambda: any(row['kind'] == 'source-click-complete' for row in records))
    subprocess.run(['wtype', '-M', 'ctrl', '-k', 'End', '-m', 'ctrl'], env=environment,
                    check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    typed_edit = os.environ.get('O3_CONTEXT_TYPED_EDIT') == '1'
    if typed_edit:
        subprocess.run(['wtype', 'Q'], env=environment, check=True,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    input_start = len(records)
    engine.stdin.write(b'inspect\n')
    engine.stdin.flush()
    wait_for(lambda: any(row['kind'] == 'diagnostic' for row in records[input_start:]))
    require_surrounding = os.environ.get('O3_CONTEXT_REQUIRE_SURROUNDING') == '1'
    def end_context(row):
        expected = 13 if typed_edit else 12
        return (row['kind'] == 'context' and row['active'] == 1 and row['available'] == 1
                and row['matchesSynthetic'] == 1 and row['bytes'] == expected
                and row['cursor'] == expected and row['anchor'] == expected)
    # First establish that the original engine genuinely received the fixed
    # edit. A hypothetical full-context getter is not the baseline's oracle.
    final_state = next(row for row in reversed(records) if row['kind'] == 'diagnostic')
    assert final_state['focusedFrame'] and final_state['focused']
    assert final_state['matchesSynthetic'] and final_state['units'] == (7 if typed_edit else 6)
    assert final_state['start'] == final_state['end'] == final_state['units']
    assert final_state['inputs']['keys'] >= 2
    if os.environ.get('O3_CONTEXT_SOURCE_INPUT') == '1':
        assert final_state['pointers'] >= 1
    if require_surrounding:
        wait_for(lambda: any(end_context(row) for row in records))
        start = len(records)
        subprocess.run(['wtype', '-M', 'shift', '-k', 'Left', '-k', 'Left', '-m', 'shift'],
                        env=environment, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        wait_for(lambda: any(row['kind'] == 'context' and row['active'] == 1
                             and row['matchesSynthetic'] == 1 and row['bytes'] == (13 if typed_edit else 12)
                             and row['cursor'] == (11 if typed_edit else 8)
                             and row['anchor'] == (13 if typed_edit else 12) for row in records[start:]))
    wait_for(lambda: any(row['kind'] == 'rectangle' and row['height'] > 0
                         and row['width'] >= 0 for row in records))
    retire_popup()
    engine.stdin.write(b'quit\n')
    engine.stdin.flush()
    assert engine.wait(timeout=5) == 0
    poller.unregister(engine.stdout)
    wait_for(lambda: any(row['kind'] == 'context' and row['active'] == 0 for row in records))
    surrounding_observed = any(row['kind'] == 'context' and row['available'] == 1
                               and row['matchesSynthetic'] == 1 for row in records)
    print(json.dumps({'kind':'baseline-result','scope':'original-engine private fixed-field lifetime only',
                      'actualEditingObserved':True,'surroundingObserved':surrounding_observed,
                      'popupRectangleObserved':True,'qtQueryImplemented':False}),flush=True)
    peer.stdin.write(b'quit\n')
    peer.stdin.flush()
    assert peer.wait(timeout=5) == 0
    print('NATIVE_CONTEXT_BASELINE_OBSERVED actual-editing native-protocol explicit-unknown-context', flush=True)
finally:
    # Never close a native view with this observer's popup still mapped.
    # A failed ownership barrier remains a failure; killing only the owned
    # compositor avoids executing the unreviewed graceful path after it.
    if engine is not None and not popup_retired:
        try:
            assert retire_popup is not None
            retire_popup()
        except Exception:
            if display.poll() is None:
                display.kill()
                display.wait(timeout=5)
            print(json.dumps({'kind':'fixture-teardown','popupRetirement':False,
                              'ownedCompositorKilled':True}),flush=True)
    for child in [peer, engine, pointer, keyboard, display]:
        if child is not None and child.poll() is None:
            child.terminate()
            child.wait(timeout=5)
