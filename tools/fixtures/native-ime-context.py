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
import socket
import threading
import importlib.util
import fcntl
import base64
import struct

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
proxy_thread = listener = None
proxy_cancel, proxy_ready = threading.Event(), threading.Event()
proxy_errors, native_snapshots = [], []
proxy_connections = []
snapshot_lock = threading.Lock()
native_current = [None]
gui_display = gui_keyboard = gui_query = None
typed_boundary = os.environ.get('O3_CONTEXT_TYPED_BOUNDARY') == '1'
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
    engine_environment = dict(environment)
    if typed_boundary:
        def load_module(name):
            spec = importlib.util.spec_from_file_location(name, '/fixture/'+name+'.py')
            module = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(module)
            return module
        boundary_module = load_module('wayland-context-boundary')
        transport = load_module('wayland-context-transport')
        listener = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        listener.set_inheritable(False)
        listener.bind('/tmp/runtime/owned-context-proxy')
        os.chmod('/tmp/runtime/owned-context-proxy', 0o600)
        listener.listen(8)
        listener.settimeout(.1)
        engine_environment['WAYLAND_DISPLAY'] = 'owned-context-proxy'

        def relay_connection(upstream, connection_id):
            downstream = None
            try:
                assert proxy_ready.wait(3), 'actual engine PID admission deadline'
                downstream = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
                downstream.connect(str(sockets[0]))
                def scoped_observe(value):
                    metadata = value.metadata()
                    metadata['connectionId'] = connection_id
                    with snapshot_lock:
                        assert len(native_snapshots) < 128, 'bounded typed native context observations'
                        native_snapshots.append(metadata)
                        native_current[0] = (connection_id, value)
                def invalidate(object_id, _reason):
                    with snapshot_lock:
                        current = native_current[0]
                        if current is not None and current[0] == connection_id and current[1].object_id == object_id:
                            native_current[0] = None
                boundary = boundary_module.ContextBoundary(
                    {'Owned native context baseline': 'owned-native-window'}, scoped_observe,
                    invalidate)
                transport.forward(upstream, downstream, (engine.pid, os.getuid()),
                                  (display.pid, os.getuid()), boundary,
                                  time.monotonic()+30, cancelled=proxy_cancel)
            except Exception as error:
                # Retain a category only; neither raw protocol nor text appears.
                category = str(error) if isinstance(error, (transport.TransportError, boundary_module.BoundaryError)) else type(error).__name__
                proxy_errors.append(category)
                print(json.dumps({'kind':'typed-boundary-failure','category':category}),flush=True)
            finally:
                for endpoint in (upstream, downstream):
                    if endpoint is not None:
                        endpoint.close()
        def relay():
            while not proxy_cancel.is_set():
                try:
                    upstream, _ = listener.accept()
                except socket.timeout:
                    continue
                except OSError:
                    if proxy_cancel.is_set():
                        break
                    raise
                # GTK and Ozone independently open their own displays in the
                # exact Chromium source. Object IDs cannot cross these sockets.
                if len(proxy_connections) >= 8:
                    upstream.close()
                    proxy_errors.append('native-connection-budget')
                    break
                connection = threading.Thread(target=relay_connection,
                    args=(upstream, len(proxy_connections)+1))
                proxy_connections.append(connection)
                connection.start()
        proxy_thread = threading.Thread(target=relay)
        proxy_thread.start()
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
                                '/fixture/native-ime-context.cjs'], env=engine_environment,
                               stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                               stderr=subprocess.DEVNULL, bufsize=0)
    proxy_ready.set()
    poller.register(engine.stdout, selectors.EVENT_READ, 'engine')
    wait_for(lambda: any(row['kind'] == 'engine-ready' for row in records))
    control = dict(environment, LD_LIBRARY_PATH='/opt/sway-runtime/usr/lib')
    mapped = {}
    def owning_window_mapped():
        tree = subprocess.check_output(['/opt/sway-runtime/usr/bin/swaymsg', '-r', '-t', 'get_tree'],
                                       env=control, timeout=2)
        assert len(tree) <= 1024 * 1024
        def visit(node):
            # Sway sees the creator of the relay's backend socket, not Electron.
            expected_pid = os.getpid() if typed_boundary else engine.pid
            if node.get('pid') == expected_pid and node.get('name') == 'Owned native context baseline':
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
    if typed_boundary:
        wait_for(lambda: any(row['rectangle'] is not None for row in native_snapshots))
        assert not proxy_errors, 'authenticated native protocol relay failed'
        print(json.dumps({'kind':'typed-boundary-result',
                          'scope':'actual engine client protocol; no Qt projection',
                          'enginePeerPidVerified':True,'compositorPeerPidVerified':True,
                          'compositorSeesProxyCreator':True,
                          'connectionCount':len(proxy_connections),
                          'states':native_snapshots,'rawWireLogged':False}),flush=True)
    if os.environ.get('O3_CONTEXT_QT_QUERY') == '1':
        assert typed_boundary
        with snapshot_lock:
            assert native_current[0] is not None
            connection_id, value = native_current[0]
            assert value.text is not None and value.cursor_byte == value.anchor_byte == 13
            current = value.metadata()
            current['surroundingUtf8'] = base64.b64encode(value.text.encode()).decode('ascii')
        # Separate GUI and engine seats keep the genuine native focused frame
        # alive. No host display, clipboard or toolkit listener is adopted.
        os.mkdir('/tmp/gui-runtime',0o700)
        gui_environment = dict(os.environ,XDG_RUNTIME_DIR='/tmp/gui-runtime',
            WLR_BACKENDS='headless',WLR_RENDERER='pixman',WLR_HEADLESS_OUTPUTS='1',
            WLR_LIBINPUT_NO_DEVICES='1',LD_LIBRARY_PATH='/opt/sway-runtime/usr/lib')
        gui_display = subprocess.Popen(['/opt/sway-runtime/usr/bin/sway','--config','/tmp/sway.conf'],
            env=gui_environment,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
        for _ in range(100):
            gui_sockets = [p for p in pathlib.Path('/tmp/gui-runtime').glob('wayland-*') if p.is_socket()]
            if gui_sockets: break
            assert gui_display.poll() is None, 'owned GUI compositor exited'
            time.sleep(.02)
        else:
            raise AssertionError('owned GUI display unavailable')
        gui_environment.update(WAYLAND_DISPLAY=gui_sockets[0].name,
            QT_QPA_PLATFORM='wayland',QT_QUICK_BACKEND='software')
        gui_keyboard = subprocess.Popen(['wtype','-k','Shift_L','-s','20000'],
            env=gui_environment,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
        reader, writer = os.pipe2(os.O_CLOEXEC)
        private_reader = fcntl.fcntl(reader,fcntl.F_DUPFD_CLOEXEC,64)
        os.close(reader)
        try:
            gui_query = subprocess.Popen(['/fixture/query','--context-fd',str(private_reader)],
                env=gui_environment,pass_fds=(private_reader,),stdout=subprocess.PIPE,
                stderr=subprocess.DEVNULL)
            body = json.dumps({'state':current,'bounds':final_state['bounds'],
                               'contentBounds':final_state['contentBounds']}).encode()
            assert 0 < len(body) <= 16384
            packet = memoryview(struct.pack('>I',len(body))+body)
            while packet:
                packet = packet[os.write(writer,packet):]
        finally:
            os.close(private_reader)
            os.close(writer)
        output, _ = gui_query.communicate(timeout=10)
        assert gui_query.returncode == 0 and len(output) <= 4096 and b'BROWSER_NATIVE_CONTEXT_QUERY_PASS' in output
        print(json.dumps({'kind':'qt-live-query-result','realNativeCurrentState':True,
            'dedicatedPrivateDescriptor':True,'actualQtQueryEvent':True,
            'nativeSerial':value.serial,'connectionId':connection_id,
            'surroundingBytes':len(value.text.encode()),'cursorByte':value.cursor_byte,
            'nativeAndGuiSeatsSeparate':True,'textLogged':False}),flush=True)
    retire_popup()
    engine.stdin.write(b'quit\n')
    engine.stdin.flush()
    assert engine.wait(timeout=5) == 0
    if proxy_thread is not None:
        for connection in proxy_connections:
            connection.join(3)
        proxy_cancel.set()
        proxy_thread.join(3)
        assert not proxy_thread.is_alive() and not any(t.is_alive() for t in proxy_connections) and not proxy_errors, 'native relay normal peer shutdown'
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
    for child in [peer, engine, pointer, keyboard, gui_query, gui_keyboard, gui_display, display]:
        if child is not None and child.poll() is None:
            child.terminate()
            child.wait(timeout=5)
    proxy_cancel.set()
    if listener is not None:
        listener.close()
    if proxy_thread is not None:
        proxy_thread.join(3)
        assert not proxy_thread.is_alive(), 'owned proxy stopped'
    for connection in proxy_connections:
        connection.join(3)
        assert not connection.is_alive(), 'owned connection stopped'
