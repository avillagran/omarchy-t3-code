//! Owned Wayland stream forwarding. No requests are interpreted or replayed.
use crate::Result;
use std::{
    collections::VecDeque,
    mem,
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
    os::unix::net::UnixStream,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
const BYTE_BUDGET: usize = 1024 * 1024;
const FD_BUDGET: usize = 64;
struct Chunk {
    bytes: Vec<u8>,
    offset: usize,
    rights: Vec<OwnedFd>,
}
#[derive(Default)]
struct Queue {
    chunks: VecDeque<Chunk>,
    bytes: usize,
    allocated: usize,
    rights: usize,
}
fn fail() -> String {
    "browser_wayland_transport_unavailable".into()
}
fn retry() -> bool {
    matches!(
        std::io::Error::last_os_error().kind(),
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
    )
}
pub(crate) fn peer(stream: &UnixStream, pid: i32) -> Result<()> {
    let mut credentials: libc::ucred = unsafe { mem::zeroed() };
    let mut size = mem::size_of_val(&credentials) as libc::socklen_t;
    let flags = unsafe { libc::fcntl(stream.as_raw_fd(), libc::F_GETFD) };
    if flags < 0 || flags & libc::FD_CLOEXEC == 0 {
        return Err(fail());
    }
    if unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&mut credentials as *mut libc::ucred).cast(),
            &mut size,
        )
    } != 0
        || size as usize != mem::size_of_val(&credentials)
        || pid <= 1
        || credentials.pid != pid
        || credentials.uid != unsafe { libc::geteuid() }
    {
        return Err(fail());
    }
    Ok(())
}
fn live(fd: i32) -> bool {
    let mut watch = libc::pollfd {
        fd,
        events: libc::POLLIN,
        revents: 0,
    };
    fd >= 0 && unsafe { libc::poll(&mut watch, 1, 0) } == 0
}
/// One nonblocking connection attempt: never wait for a stalled accept backlog.
/// The operation owns at most one socket and forwards no protocol bytes.
pub(crate) fn connect_owned(
    path: &std::path::Path,
    pid: i32,
    watched: &[&OwnedFd],
    cancel: &AtomicBool,
) -> Result<UnixStream> {
    let active = || {
        !cancel.load(Ordering::Acquire)
            && !watched.is_empty()
            && watched.iter().all(|watch| live(watch.as_raw_fd()))
    };
    if !active() {
        return Err(fail());
    }
    let bytes = path.as_os_str().as_encoded_bytes();
    let mut address: libc::sockaddr_un = unsafe { mem::zeroed() };
    if bytes.is_empty() || bytes.len() >= address.sun_path.len() || bytes.contains(&0) {
        return Err(fail());
    }
    address.sun_family = libc::AF_UNIX as _;
    for (index, byte) in bytes.iter().enumerate() {
        address.sun_path[index] = *byte as libc::c_char;
    }
    let raw = unsafe {
        libc::socket(
            libc::AF_UNIX,
            libc::SOCK_STREAM | libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC,
            0,
        )
    };
    if raw < 0 {
        return Err(fail());
    }
    let socket = unsafe { OwnedFd::from_raw_fd(raw) };
    let length = mem::offset_of!(libc::sockaddr_un, sun_path) + bytes.len() + 1;
    if unsafe {
        libc::connect(
            raw,
            (&address as *const libc::sockaddr_un).cast(),
            length as _,
        )
    } != 0
    {
        return Err(fail());
    }
    let stream = UnixStream::from(socket);
    peer(&stream, pid)?;
    if !active() {
        return Err(fail());
    }
    Ok(stream)
}
fn receive(fd: i32, queue: &mut Queue) -> Result<Option<bool>> {
    let mut bytes = vec![0; (BYTE_BUDGET - queue.allocated).min(65536)];
    if bytes.is_empty() || queue.rights >= FD_BUDGET {
        return Ok(None);
    }
    let mut payload = libc::iovec {
        iov_base: bytes.as_mut_ptr().cast(),
        iov_len: bytes.len(),
    };
    let mut ancillary = [0_usize; 80];
    let mut message: libc::msghdr = unsafe { mem::zeroed() };
    message.msg_iov = &mut payload;
    message.msg_iovlen = 1;
    message.msg_control = ancillary.as_mut_ptr().cast();
    message.msg_controllen = unsafe {
        libc::CMSG_SPACE(((FD_BUDGET - queue.rights) * mem::size_of::<i32>()) as _) as usize
    };
    let count = unsafe {
        libc::recvmsg(
            fd,
            &mut message,
            libc::MSG_CMSG_CLOEXEC | libc::MSG_DONTWAIT,
        )
    };
    if count < 0 {
        return if retry() { Ok(None) } else { Err(fail()) };
    }
    let mut rights = Vec::new();
    let mut unknown = false;
    unsafe {
        let mut header = libc::CMSG_FIRSTHDR(&message);
        while !header.is_null() {
            let h = &*header;
            if h.cmsg_level == libc::SOL_SOCKET && h.cmsg_type == libc::SCM_RIGHTS {
                let size = h.cmsg_len.saturating_sub(libc::CMSG_LEN(0) as usize);
                if !size.is_multiple_of(mem::size_of::<i32>()) {
                    unknown = true;
                }
                for index in 0..size / mem::size_of::<i32>() {
                    rights.push(OwnedFd::from_raw_fd(std::ptr::read_unaligned(
                        libc::CMSG_DATA(header).cast::<i32>().add(index),
                    )));
                }
            } else {
                unknown = true;
            }
            header = libc::CMSG_NXTHDR(&message, header);
        }
    }
    if rights.len() + queue.rights > FD_BUDGET
        || unknown
        || message.msg_flags & (libc::MSG_CTRUNC | libc::MSG_TRUNC) != 0
    {
        return Err(fail());
    }
    if count == 0 {
        return if rights.is_empty() {
            Ok(Some(false))
        } else {
            Err(fail())
        };
    }
    bytes.truncate(count as usize);
    if bytes.capacity() > BYTE_BUDGET - queue.allocated {
        return Err(fail());
    }
    queue.allocated += bytes.capacity();
    queue.bytes += bytes.len();
    queue.rights += rights.len();
    queue.chunks.push_back(Chunk {
        bytes,
        offset: 0,
        rights,
    });
    Ok(Some(true))
}
fn send(fd: i32, queue: &mut Queue) -> Result<()> {
    let Some(chunk) = queue.chunks.front_mut() else {
        return Ok(());
    };
    let mut payload = libc::iovec {
        iov_base: chunk.bytes[chunk.offset..].as_ptr().cast_mut().cast(),
        iov_len: chunk.bytes.len() - chunk.offset,
    };
    let mut ancillary = [0_usize; 80];
    let mut message: libc::msghdr = unsafe { mem::zeroed() };
    message.msg_iov = &mut payload;
    message.msg_iovlen = 1;
    if !chunk.rights.is_empty() {
        message.msg_control = ancillary.as_mut_ptr().cast();
        message.msg_controllen =
            unsafe { libc::CMSG_SPACE((chunk.rights.len() * mem::size_of::<i32>()) as _) as usize };
        unsafe {
            let header = libc::CMSG_FIRSTHDR(&message);
            (*header).cmsg_level = libc::SOL_SOCKET;
            (*header).cmsg_type = libc::SCM_RIGHTS;
            (*header).cmsg_len =
                libc::CMSG_LEN((chunk.rights.len() * mem::size_of::<i32>()) as _) as usize;
            for (index, right) in chunk.rights.iter().enumerate() {
                std::ptr::write_unaligned(
                    libc::CMSG_DATA(header).cast::<i32>().add(index),
                    right.as_raw_fd(),
                );
            }
        }
    }
    let count = unsafe { libc::sendmsg(fd, &message, libc::MSG_NOSIGNAL | libc::MSG_DONTWAIT) };
    if count < 0 {
        return if retry() { Ok(()) } else { Err(fail()) };
    }
    if count == 0 {
        return Err(fail());
    }
    // The first positive send transfers all ancillary ownership even on a
    // partial byte send. Retaining rights for the suffix would duplicate them.
    queue.rights -= chunk.rights.len();
    chunk.rights.clear();
    queue.bytes -= count as usize;
    chunk.offset += count as usize;
    if chunk.offset == chunk.bytes.len() {
        queue.allocated -= chunk.bytes.capacity();
        queue.chunks.pop_front();
    }
    Ok(())
}
pub(crate) fn forward(
    front: UnixStream,
    backend: UnixStream,
    identities: (i32, i32),
    watched: [OwnedFd; 2],
    cancel: Arc<AtomicBool>,
) -> Result<()> {
    let (engine, compositor) = identities;
    let [engine_pidfd, compositor_pidfd] = watched;
    peer(&front, engine)?;
    peer(&backend, compositor)?;
    front.set_nonblocking(true).map_err(|_| fail())?;
    backend.set_nonblocking(true).map_err(|_| fail())?;
    let streams = [front, backend];
    let mut queues = [Queue::default(), Queue::default()];
    let mut eof = [false; 2];
    let mut shut = [false; 2];
    loop {
        if cancel.load(Ordering::Acquire) {
            return Ok(());
        }
        let mut descriptors = [
            libc::pollfd {
                fd: engine_pidfd.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: compositor_pidfd.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: streams[0].as_raw_fd(),
                events: 0,
                revents: 0,
            },
            libc::pollfd {
                fd: streams[1].as_raw_fd(),
                events: 0,
                revents: 0,
            },
        ];
        for index in 0..2 {
            let destination = 1 - index;
            if !eof[index]
                && queues[destination].allocated < BYTE_BUDGET
                && queues[destination].rights < FD_BUDGET
            {
                descriptors[index + 2].events |= libc::POLLIN;
            }
            if !queues[index].chunks.is_empty() {
                descriptors[index + 2].events |= libc::POLLOUT;
            }
            if eof[destination] && queues[index].chunks.is_empty() && !shut[index] {
                streams[index]
                    .shutdown(std::net::Shutdown::Write)
                    .map_err(|_| fail())?;
                shut[index] = true;
            }
        }
        for descriptor in &mut descriptors[2..] {
            if descriptor.events == 0 {
                descriptor.fd = -1;
            }
        }
        if eof.iter().all(|v| *v) && queues.iter().all(|q| q.chunks.is_empty()) {
            return Ok(());
        }
        let count = unsafe { libc::poll(descriptors.as_mut_ptr(), descriptors.len() as _, 50) };
        if count < 0 {
            if retry() {
                continue;
            }
            return Err(fail());
        }
        if cancel.load(Ordering::Acquire) {
            return Ok(());
        }
        if descriptors[0].revents != 0 || descriptors[1].revents != 0 {
            return Err(fail());
        }
        for index in 0..2 {
            let events = descriptors[index + 2].revents;
            if events & libc::POLLNVAL != 0 {
                return Err(fail());
            }
            if events & libc::POLLOUT != 0 {
                send(streams[index].as_raw_fd(), &mut queues[index])?;
            }
            if events & (libc::POLLIN | libc::POLLHUP | libc::POLLERR) != 0
                && !eof[index]
                && queues[1 - index].allocated < BYTE_BUDGET
                && queues[1 - index].rights < FD_BUDGET
                && receive(streams[index].as_raw_fd(), &mut queues[1 - index])? == Some(false)
            {
                eof[index] = true;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    fn watch() -> OwnedFd {
        let raw = unsafe { libc::syscall(libc::SYS_pidfd_open, libc::getpid(), 0) as i32 };
        assert!(raw >= 0);
        unsafe { OwnedFd::from_raw_fd(raw) }
    }
    #[test]
    fn actual_bidirectional_rights_and_cancellation() {
        let (mut engine, front) = UnixStream::pair().unwrap();
        let (backend, mut compositor) = UnixStream::pair().unwrap();
        let stopped = Arc::new(AtomicBool::new(false));
        let cancel = stopped.clone();
        let pid = unsafe { libc::getpid() };
        let lifetime = watch();
        let worker = std::thread::spawn(move || {
            forward(front, backend, (pid, pid), [lifetime, watch()], cancel)
        });
        engine
            .set_read_timeout(Some(std::time::Duration::from_secs(2)))
            .unwrap();
        compositor
            .set_read_timeout(Some(std::time::Duration::from_secs(2)))
            .unwrap();
        let descriptor = std::fs::File::open("/dev/null").unwrap();
        let mut outbound = Queue::default();
        outbound.bytes = 4;
        outbound.allocated = 4;
        outbound.rights = 1;
        let copy = unsafe { libc::fcntl(descriptor.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 64) };
        assert!(copy >= 0);
        outbound.chunks.push_back(Chunk {
            bytes: b"test".to_vec(),
            offset: 0,
            rights: vec![unsafe { OwnedFd::from_raw_fd(copy) }],
        });
        send(engine.as_raw_fd(), &mut outbound).unwrap();
        assert_eq!(outbound.rights, 0);
        let mut received = Queue::default();
        let mut ready = libc::pollfd {
            fd: compositor.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        assert_eq!(unsafe { libc::poll(&mut ready, 1, 2000) }, 1);
        assert_eq!(
            receive(compositor.as_raw_fd(), &mut received).unwrap(),
            Some(true)
        );
        let chunk = received.chunks.front().unwrap();
        assert_eq!(chunk.bytes, b"test");
        assert_eq!(chunk.rights.len(), 1);
        assert_ne!(
            unsafe { libc::fcntl(chunk.rights[0].as_raw_fd(), libc::F_GETFD) } & libc::FD_CLOEXEC,
            0
        );
        compositor.write_all(b"back").unwrap();
        let mut reply = [0; 4];
        engine.read_exact(&mut reply).unwrap();
        assert_eq!(&reply, b"back");
        stopped.store(true, Ordering::Release);
        assert!(worker.join().unwrap().is_ok());
    }
    #[test]
    fn partial_send_transfers_rights_only_on_first_positive_write() {
        let (sender, receiver) = UnixStream::pair().unwrap();
        sender.set_nonblocking(true).unwrap();
        let capacity = 4096_i32;
        assert_eq!(
            unsafe {
                libc::setsockopt(
                    sender.as_raw_fd(),
                    libc::SOL_SOCKET,
                    libc::SO_SNDBUF,
                    (&capacity as *const i32).cast(),
                    mem::size_of_val(&capacity) as _,
                )
            },
            0
        );
        let source = std::fs::File::open("/dev/null").unwrap();
        let copy = unsafe { libc::fcntl(source.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 64) };
        assert!(copy >= 0);
        let bytes = vec![0x5a; 256 * 1024];
        let mut outbound = Queue {
            bytes: bytes.len(),
            allocated: bytes.capacity(),
            rights: 1,
            ..Default::default()
        };
        outbound.chunks.push_back(Chunk {
            bytes: bytes.clone(),
            offset: 0,
            rights: vec![unsafe { OwnedFd::from_raw_fd(copy) }],
        });
        send(sender.as_raw_fd(), &mut outbound).unwrap();
        assert!(outbound.bytes > 0 && outbound.bytes < bytes.len());
        assert_eq!(outbound.allocated, bytes.capacity());
        assert_eq!(outbound.rights, 0);
        assert_eq!(unsafe { libc::fcntl(copy, libc::F_GETFD) }, -1);
        let mut inbound = Queue::default();
        let mut actual = Vec::new();
        let mut received_rights = 0;
        let mut turns = 0;
        while actual.len() < bytes.len() {
            let _ = receive(receiver.as_raw_fd(), &mut inbound).unwrap();
            for chunk in inbound.chunks.drain(..) {
                actual.extend_from_slice(&chunk.bytes);
                received_rights += chunk.rights.len();
            }
            inbound = Queue::default();
            send(sender.as_raw_fd(), &mut outbound).unwrap();
            turns += 1;
            assert!(turns < 1000);
        }
        assert_eq!(received_rights, 1);
        assert_eq!(outbound.bytes, 0);
        assert_eq!(outbound.allocated, 0);
        assert_eq!(actual, bytes);
        assert_ne!(
            unsafe { libc::fcntl(source.as_raw_fd(), libc::F_GETFD) },
            -1
        );
    }
    #[test]
    fn short_reads_are_bounded_by_retained_capacity_not_payload_length() {
        let (sender, receiver) = UnixStream::pair().unwrap();
        let mut queue = Queue::default();
        for _ in 0..16 {
            assert_eq!(
                unsafe {
                    libc::send(
                        sender.as_raw_fd(),
                        b"x".as_ptr().cast(),
                        1,
                        libc::MSG_NOSIGNAL,
                    )
                },
                1
            );
            assert_eq!(
                receive(receiver.as_raw_fd(), &mut queue).unwrap(),
                Some(true)
            );
        }
        assert_eq!(queue.bytes, 16);
        assert_eq!(queue.allocated, BYTE_BUDGET);
        assert_eq!(queue.chunks.len(), 16);
        assert_eq!(
            unsafe {
                libc::send(
                    sender.as_raw_fd(),
                    b"y".as_ptr().cast(),
                    1,
                    libc::MSG_NOSIGNAL,
                )
            },
            1
        );
        assert_eq!(receive(receiver.as_raw_fd(), &mut queue).unwrap(), None);
        assert_eq!(queue.allocated, BYTE_BUDGET);
        assert_eq!(queue.bytes, 16);
    }
    #[test]
    fn failed_send_keeps_owned_rights_until_queue_retirement() {
        let (sender, receiver) = UnixStream::pair().unwrap();
        drop(receiver);
        let source = std::fs::File::open("/dev/null").unwrap();
        let copy = unsafe { libc::fcntl(source.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 64) };
        assert!(copy >= 0);
        {
            let mut queue = Queue {
                bytes: 1,
                allocated: 1,
                rights: 1,
                ..Default::default()
            };
            queue.chunks.push_back(Chunk {
                bytes: vec![1],
                offset: 0,
                rights: vec![unsafe { OwnedFd::from_raw_fd(copy) }],
            });
            assert!(send(sender.as_raw_fd(), &mut queue).is_err());
            assert_eq!(queue.rights, 1);
            assert_ne!(unsafe { libc::fcntl(copy, libc::F_GETFD) }, -1);
        }
        assert_eq!(unsafe { libc::fcntl(copy, libc::F_GETFD) }, -1);
        assert_ne!(
            unsafe { libc::fcntl(source.as_raw_fd(), libc::F_GETFD) },
            -1
        );
    }
    fn listener() -> (std::path::PathBuf, std::os::unix::net::UnixListener) {
        let path =
            std::env::temp_dir().join(format!("owned-wayland-connect-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&path).unwrap();
        let socket = path.join("socket");
        let listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
        listener.set_nonblocking(true).unwrap();
        (socket, listener)
    }
    #[test]
    fn nonblocking_connect_refuses_full_backlog_without_waiting() {
        let (path, listener) = listener();
        assert_eq!(unsafe { libc::listen(listener.as_raw_fd(), 1) }, 0);
        let lifetime = watch();
        let stopped = AtomicBool::new(false);
        let pid = unsafe { libc::getpid() };
        let mut connections = Vec::new();
        let mut refused = false;
        for _ in 0..16 {
            let start = std::time::Instant::now();
            match connect_owned(&path, pid, &[&lifetime], &stopped) {
                Ok(connection) => connections.push(connection),
                Err(_) => {
                    refused = true;
                    assert!(start.elapsed() < std::time::Duration::from_millis(250));
                    break;
                }
            }
        }
        assert!(refused);
        assert!(connections.len() <= 2);
        for connection in &connections {
            assert_ne!(
                unsafe { libc::fcntl(connection.as_raw_fd(), libc::F_GETFD) } & libc::FD_CLOEXEC,
                0
            );
        }
    }
    #[test]
    fn cancelled_or_dead_peer_connect_creates_no_route() {
        let (path, listener) = listener();
        let lifetime = watch();
        let pid = unsafe { libc::getpid() };
        let stopped = AtomicBool::new(true);
        assert!(connect_owned(&path, pid, &[&lifetime], &stopped).is_err());
        assert!(listener.accept().is_err());
        let child = unsafe { libc::fork() };
        assert!(child >= 0);
        if child == 0 {
            unsafe {
                libc::_exit(0);
            }
        }
        let raw = unsafe { libc::syscall(libc::SYS_pidfd_open, child, 0) as i32 };
        assert!(raw >= 0);
        let exited = unsafe { OwnedFd::from_raw_fd(raw) };
        assert_eq!(
            unsafe { libc::waitpid(child, std::ptr::null_mut(), 0) },
            child
        );
        stopped.store(false, Ordering::Release);
        assert!(connect_owned(&path, pid, &[&exited], &stopped).is_err());
        assert!(listener.accept().is_err());
    }
    #[test]
    fn wrong_kernel_peer_is_refused_before_forwarding() {
        let (front, _) = UnixStream::pair().unwrap();
        let (backend, _) = UnixStream::pair().unwrap();
        let pid = unsafe { libc::getpid() };
        assert!(forward(
            front,
            backend,
            (pid + 1, pid),
            [watch(), watch()],
            Arc::new(AtomicBool::new(false))
        )
        .is_err());
    }
}
