"""Private, authenticated socket transport prototype; not a product entry point.

Endpoints must already be connected. Authentication precedes all reads. The
observer receives byte copies only; ancillary descriptors remain opaque.
"""
import array
import collections
import os
import selectors
import socket
import struct
import time


class TransportError(Exception):
    pass


def credentials(peer):
    return struct.unpack('3i', peer.getsockopt(socket.SOL_SOCKET, socket.SO_PEERCRED, 12))


def forward(engine, compositor, expected_engine, expected_compositor, observer,
            deadline, byte_budget=1024 * 1024, fd_budget=64, cancelled=None):
    """Forward once, without reconnect/replay; expected tuples are (pid, uid).

    The compositor's peer is the process that created its connection, which is
    deliberately not represented here as the engine PID. An error retires the
    whole observation lease and closes both owned sockets and queued descriptors.
    """
    peers = (engine, compositor)
    selector = selectors.DefaultSelector()
    queues = [collections.deque(), collections.deque()]
    queued_bytes, queued_fds = [0, 0], [0, 0]
    eof, half_closed = [False, False], [False, False]
    try:
        for peer, expected in zip(peers, (expected_engine, expected_compositor)):
            pid, uid, _gid = credentials(peer)
            if (pid, uid) != tuple(expected):
                raise TransportError('native-peer-identity')
            peer.set_inheritable(False)
            peer.setblocking(False)
        if byte_budget < 65536 or fd_budget < 1:
            raise TransportError('native-transport-budget')
        while True:
            if cancelled is not None and cancelled.is_set():
                raise TransportError('native-transport-cancelled')
            for index, peer in enumerate(peers):
                destination = 1 - index
                events = 0
                if (not eof[index] and queued_bytes[destination] < byte_budget
                        and queued_fds[destination] < fd_budget):
                    events |= selectors.EVENT_READ
                if queues[index]:
                    events |= selectors.EVENT_WRITE
                try:
                    selector.unregister(peer)
                except KeyError:
                    pass
                if events:
                    selector.register(peer, events, index)
                if eof[destination] and not queues[index] and not half_closed[index]:
                    peer.shutdown(socket.SHUT_WR)
                    half_closed[index] = True
            if all(eof) and not any(queues):
                return
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise TransportError('native-transport-deadline')
            for key, events in selector.select(min(remaining, .05)):
                index, peer = key.data, key.fileobj
                if events & selectors.EVENT_WRITE:
                    item = queues[index][0]
                    data, descriptors = item
                    ancillary = [(socket.SOL_SOCKET, socket.SCM_RIGHTS,
                                  array.array('i', descriptors))] if descriptors else []
                    try:
                        count = peer.sendmsg([data], ancillary, socket.MSG_NOSIGNAL)
                    except BlockingIOError:
                        count = 0
                    if count:
                        # Ancillary data is transferred by the first successful
                        # write, even when the byte payload is only partial.
                        for descriptor in descriptors:
                            os.close(descriptor)
                        queued_fds[index] -= len(descriptors)
                        item[1] = []
                        queued_bytes[index] -= count
                        if count == len(data):
                            queues[index].popleft()
                        else:
                            item[0] = data[count:]
                if events & selectors.EVENT_READ:
                    destination = 1 - index
                    room = min(65536, byte_budget - queued_bytes[destination])
                    descriptors = []
                    try:
                        data, ancillary, flags, _address = peer.recvmsg(
                            room, socket.CMSG_SPACE(fd_budget * array.array('i').itemsize),
                            socket.MSG_CMSG_CLOEXEC)
                    except BlockingIOError:
                        continue
                    try:
                        unknown_ancillary = False
                        for level, kind, value in ancillary:
                            if (level, kind) != (socket.SOL_SOCKET, socket.SCM_RIGHTS):
                                unknown_ancillary = True
                                continue
                            received = array.array('i')
                            received.frombytes(value[:len(value) - len(value) % received.itemsize])
                            descriptors.extend(received)
                        if unknown_ancillary:
                            raise TransportError('native-ancillary-type')
                        if flags & (socket.MSG_CTRUNC | socket.MSG_TRUNC):
                            raise TransportError('native-ancillary-truncated')
                        if len(descriptors) + queued_fds[destination] > fd_budget:
                            raise TransportError('native-descriptor-budget')
                        if any(os.get_inheritable(fd) for fd in descriptors):
                            raise TransportError('native-descriptor-inheritance')
                        if not data:
                            if descriptors:
                                raise TransportError('native-empty-descriptor-record')
                            eof[index] = True
                            continue
                        observer.feed('client' if index == 0 else 'server', data)
                        queues[destination].append([data, descriptors])
                        queued_bytes[destination] += len(data)
                        queued_fds[destination] += len(descriptors)
                        descriptors = []
                    finally:
                        for descriptor in descriptors:
                            os.close(descriptor)
    finally:
        observer.close('native-transport-closed')
        selector.close()
        for queue in queues:
            for _data, descriptors in queue:
                for descriptor in descriptors:
                    os.close(descriptor)
        for peer in peers:
            peer.close()
