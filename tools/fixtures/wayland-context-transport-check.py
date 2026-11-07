"""Owned socket and descriptor checks; no compositor or engine is launched."""
import array
import importlib.util
import json
import os
import resource
import socket
import threading
import time

assert resource.getrlimit(resource.RLIMIT_CORE) == (0, 0)
spec = importlib.util.spec_from_file_location('transport', '/fixture/wayland-context-transport.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
checks = 0


def expect(value, reason):
    global checks
    assert value, reason
    checks += 1


class Observer:
    def __init__(self):
        self.counts = {'client': 0, 'server': 0}
        self.closed = False

    def feed(self, direction, data):
        self.counts[direction] += len(data)

    def close(self, _reason):
        self.closed = True


class Partial:
    def __init__(self, peer):
        self.peer = peer
        self.calls = self.rights = 0

    def __getattr__(self, key):
        return getattr(self.peer, key)

    def sendmsg(self, buffers, ancillary, flags):
        self.calls += 1
        if self.calls % 3 == 1:
            raise BlockingIOError()
        result = self.peer.sendmsg([buffers[0][:7]], ancillary, flags)
        if result and ancillary:
            self.rights += 1
        return result


def launch(partial=False, expected=None, fd_budget=64, lifetime=5):
    left, engine = socket.socketpair()
    compositor, right = socket.socketpair()
    for peer in (left, right):
        peer.settimeout(3)
    if partial:
        compositor = Partial(compositor)
    observer, cancel, errors = Observer(), threading.Event(), []

    def run():
        try:
            identity = (os.getpid(), os.getuid())
            module.forward(engine, compositor, expected or identity, identity, observer,
                           time.monotonic() + lifetime, byte_budget=65536,
                           fd_budget=fd_budget, cancelled=cancel)
        except Exception as error:
            errors.append(error)
    thread = threading.Thread(target=run)
    thread.start()
    return left, right, compositor, observer, cancel, errors, thread


def finish(state, error=None):
    left, right, _proxy, observer, cancel, errors, thread = state
    if error == 'native-transport-cancelled':
        cancel.set()
    elif error is None:
        left.shutdown(socket.SHUT_WR)
        right.shutdown(socket.SHUT_WR)
    thread.join(3)
    expect(not thread.is_alive(), 'bounded transport termination')
    expect(observer.closed, 'lease invalidated on every termination')
    if error is None:
        expect(not errors, 'normal half-close has no transport failure')
    else:
        expect(len(errors) == 1 and str(errors[0]) == error, 'fixed redacted failure category')
    left.close()
    right.close()


baseline = len(os.listdir('/proc/self/fd'))
state = launch(partial=True)
left, right, proxy, observer, _cancel, _errors, _thread = state
descriptor = os.memfd_create('owned-wire-fixture', os.MFD_CLOEXEC)
os.write(descriptor, b'owned-fixed-value')
payload = b'owned-wire-' * 100
left.sendmsg([payload], [(socket.SOL_SOCKET, socket.SCM_RIGHTS, array.array('i', [descriptor]))])
os.close(descriptor)
received, rights = bytearray(), []
while len(received) < len(payload):
    data, ancillary, flags, _ = right.recvmsg(4096, socket.CMSG_SPACE(64), socket.MSG_CMSG_CLOEXEC)
    expect(bool(data) and not flags & socket.MSG_CTRUNC, 'partial forwarding preserves stream')
    received.extend(data)
    for level, kind, raw in ancillary:
        expect((level, kind) == (socket.SOL_SOCKET, socket.SCM_RIGHTS), 'opaque descriptor type')
        values = array.array('i')
        values.frombytes(raw)
        rights.extend(values)
expect(received == payload and len(rights) == 1, 'exact bytes and exactly one descriptor')
expect(proxy.rights == 1 and proxy.calls > 3, 'rights sent once across partial writes and EAGAIN')
expect(not os.get_inheritable(rights[0]), 'received descriptor remains close-on-exec')
expect(os.pread(rights[0], 32, 0) == b'owned-fixed-value', 'actual descriptor identity remains valid')
os.close(rights[0])
right.sendall(b'fixed-server-reply')
expect(left.recv(128) == b'fixed-server-reply', 'independent reverse direction')
finish(state)

state = launch(expected=(os.getpid() + 1000000, os.getuid()))
finish(state, 'native-peer-identity')

state = launch()
finish(state, 'native-transport-cancelled')

state = launch(lifetime=.1)
finish(state, 'native-transport-deadline')

state = launch()
left, right, _proxy, observer, cancel, errors, thread = state
left.setblocking(False)
sent = 0
while sent < 4 * 1024 * 1024:
    try:
        sent += left.send(b'x' * 65536)
    except BlockingIOError:
        break
time.sleep(.1)
expect(sent > 0 and observer.counts['client'] < sent + 1, 'bounded backpressure stops upstream reads')
before = observer.counts['client']
time.sleep(.1)
expect(observer.counts['client'] == before, 'blocked consumer does not drain an unbounded queue')
finish(state, 'native-transport-cancelled')

state = launch(fd_budget=1)
left = state[0]
descriptors = [os.memfd_create('owned-limit', os.MFD_CLOEXEC) for _ in range(2)]
left.sendmsg([b'x'], [(socket.SOL_SOCKET, socket.SCM_RIGHTS, array.array('i', descriptors))])
for descriptor in descriptors:
    os.close(descriptor)
finish(state, 'native-descriptor-budget')

state = launch()
state[1].close()
state[0].sendall(b'owned-peer-loss')
state[-1].join(3)
expect(not state[-1].is_alive() and state[3].closed and state[5], 'peer loss retires owned connection')
state[0].close()
expect(len(os.listdir('/proc/self/fd')) == baseline, 'all original and received descriptors reclaimed')
print('WAYLAND_CONTEXT_TRANSPORT_CHECKS ' + json.dumps({'checks': checks,
      'rawWireLogged': False, 'compositorLaunched': False, 'engineLaunched': False}))
