"""Private Wayland host for the actual NativeMain component input boundary."""
import hashlib, json, os, pathlib, resource, subprocess, time

assert resource.getrlimit(resource.RLIMIT_CORE) == (0, 0)
baseline = pathlib.Path('/gui-baseline')
for name in ['pointer', 'input-method-peer', 'qml/O3/Browser/libo3browserplatform.so']:
    expected = json.loads((baseline/'artifacts.json').read_text())[name]
    assert hashlib.sha256((baseline/name).read_bytes()).hexdigest() == expected
pathlib.Path('/tmp/home').mkdir(mode=0o700)
runtime = pathlib.Path('/tmp/wayland-runtime')
runtime.mkdir(mode=0o700)
environment = dict(os.environ, HOME='/tmp/home', XDG_RUNTIME_DIR=str(runtime))
pathlib.Path('/tmp/sway.conf').write_text('xwayland disable\noutput * mode 1280x1024\nseat seat0 fallback true\ndefault_border none\nfor_window [title="Owned native entrypoint input"] floating enable, move position 100 100\n')
display = subprocess.Popen(['/opt/sway-runtime/usr/bin/sway', '--config', '/tmp/sway.conf'], env=dict(environment,LD_LIBRARY_PATH='/opt/sway-runtime/usr/lib',WLR_BACKENDS='headless',WLR_RENDERER='pixman',WLR_HEADLESS_OUTPUTS='1',WLR_LIBINPUT_NO_DEVICES='1'), stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
keyboard = None
try:
    for _ in range(150):
        sockets = [item for item in runtime.glob('wayland-*') if item.is_socket()]
        if sockets:break
        assert display.poll() is None
        time.sleep(.02)
    else:raise AssertionError('private compositor unavailable')
    environment.update(WAYLAND_DISPLAY=sockets[0].name, SWAYSOCK=str(next(runtime.glob('sway-ipc.*.sock'))))
    keyboard = subprocess.Popen(['wtype', '-k', 'Shift_L', '-s', '120000'], env=environment, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    qml = pathlib.Path('/tmp/qml')
    qml.mkdir()
    (qml/'UnderTest').symlink_to('/proof/root')
    (qml/'shell.qml').write_text('//@ pragma OmarchyLoadPatch\n//@ pragma RespectSystemStyle\n'+pathlib.Path('/proof/native-entrypoint-gui.qml').read_text())
    result = subprocess.run(['quickshell', '-p', str(qml/'shell.qml')], env=dict(environment, QML_IMPORT_PATH='/gui-baseline/qml'), capture_output=True, text=True, timeout=85)
    for name in ['pids.current','pids.peak','pids.max','pids.events','memory.events']:
        path=pathlib.Path('/sys/fs/cgroup')/name
        if path.is_file():print('PRIVATE_CGROUP_'+name+' '+path.read_text().strip())
    print(result.stdout)
    print(result.stderr)
    assert result.returncode == 0, result.returncode
    assert 'ACTUAL_NATIVE_ENTRYPOINT_IME_PASS' in result.stdout + result.stderr
finally:
    if keyboard is not None and keyboard.poll() is None:
        keyboard.terminate()
        keyboard.wait(timeout=3)
    display.terminate()
    display.wait(timeout=3)
