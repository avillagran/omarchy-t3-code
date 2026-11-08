//! Kernel-reported identity of the native engine that owns a cohort's X client.
use crate::{text, Result};
use serde_json::Value;
use std::{
    mem,
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
};
pub struct Pending {
    socket: OwnedFd,
    source: OwnedFd,
    early_main: std::sync::Mutex<Option<Engine>>,
}
pub struct Engine {
    pid: libc::pid_t,
    pidfd: OwnedFd,
}
impl Engine {
    pub fn pid(&self) -> libc::pid_t {
        self.pid
    }
    pub fn pidfd(&self) -> &OwnedFd {
        &self.pidfd
    }
}
impl Pending {
    pub fn new() -> Result<Self> {
        let mut pair = [-1; 2];
        if unsafe {
            libc::socketpair(
                libc::AF_UNIX,
                libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC,
                0,
                pair.as_mut_ptr(),
            )
        } != 0
        {
            return Err("browser_engine_identity_unavailable".into());
        }
        let socket = unsafe { OwnedFd::from_raw_fd(pair[0]) };
        let child = unsafe { OwnedFd::from_raw_fd(pair[1]) };
        let enabled = 1_i32;
        if unsafe {
            libc::setsockopt(
                socket.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_PASSCRED,
                (&enabled as *const i32).cast(),
                mem::size_of_val(&enabled) as _,
            )
        } != 0
        {
            return Err("browser_engine_identity_unavailable".into());
        }
        let timeout = libc::timeval {
            tv_sec: 5,
            tv_usec: 0,
        };
        if unsafe {
            libc::setsockopt(
                socket.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_RCVTIMEO,
                (&timeout as *const libc::timeval).cast(),
                mem::size_of_val(&timeout) as _,
            )
        } != 0
        {
            return Err("browser_engine_identity_unavailable".into());
        }
        let source = crate::desktop_browser_ipc::duplicate_child_source(child.as_raw_fd())
            .map_err(|_| "browser_engine_identity_unavailable")?;
        Ok(Self {
            socket,
            source,
            early_main: std::sync::Mutex::new(None),
        })
    }
    pub fn source(&self) -> &OwnedFd {
        &self.source
    }
    pub fn admit(&self, renderer: u32) -> Result<Engine> {
        self.receive(renderer, Some(false))
            .map(|(_, engine)| engine)
    }
    /// Admit the actual spawned child before GTK/Ozone can need its display.
    /// The renderer reports a kernel pidfd, never a supplied engine PID label.
    pub fn admit_spawn(&self, renderer: u32) -> Result<Engine> {
        let (spawn, engine) = self.receive(renderer, None)?;
        if spawn {
            return Ok(engine);
        }
        *self
            .early_main
            .lock()
            .map_err(|_| "browser_engine_identity_unavailable")? = Some(engine);
        let spawned = self.receive(renderer, Some(true))?.1;
        let main = self
            .early_main
            .lock()
            .map_err(|_| "browser_engine_identity_unavailable")?;
        if !main
            .as_ref()
            .is_some_and(|main| main.pid == spawned.pid && live(main.pidfd.as_raw_fd()))
        {
            return Err("browser_engine_identity_unavailable".into());
        }
        Ok(spawned)
    }
    /// The later trusted-main addon must independently identify the same child.
    pub fn confirm(&self, renderer: u32, spawned: &Engine) -> Result<()> {
        let early = self
            .early_main
            .lock()
            .map_err(|_| "browser_engine_identity_unavailable")?
            .take();
        let confirmed = match early {
            Some(engine) => engine,
            None => self.receive(renderer, Some(false))?.1,
        };
        if confirmed.pid != spawned.pid
            || !live(confirmed.pidfd.as_raw_fd())
            || !live(spawned.pidfd.as_raw_fd())
        {
            return Err("browser_engine_identity_unavailable".into());
        }
        Ok(())
    }
    fn receive(&self, renderer: u32, expected: Option<bool>) -> Result<(bool, Engine)> {
        let mut bytes = [0_u8; 64];
        let mut data = libc::iovec {
            iov_base: bytes.as_mut_ptr().cast(),
            iov_len: bytes.len(),
        };
        let mut ancillary = [0_usize; 32];
        let mut message: libc::msghdr = unsafe { mem::zeroed() };
        message.msg_iov = &mut data;
        message.msg_iovlen = 1;
        message.msg_control = ancillary.as_mut_ptr().cast();
        message.msg_controllen = mem::size_of_val(&ancillary);
        let count = unsafe {
            libc::recvmsg(
                self.socket.as_raw_fd(),
                &mut message,
                libc::MSG_CMSG_CLOEXEC,
            )
        };
        if count < 0 {
            return Err("browser_engine_identity_unavailable".into());
        }
        let mut credentials = None;
        let mut descriptors = Vec::new();
        let mut unknown = false;
        unsafe {
            let mut header = libc::CMSG_FIRSTHDR(&message);
            while !header.is_null() {
                let h = &*header;
                if h.cmsg_level == libc::SOL_SOCKET
                    && h.cmsg_type == libc::SCM_CREDENTIALS
                    && h.cmsg_len == libc::CMSG_LEN(mem::size_of::<libc::ucred>() as _) as usize
                {
                    credentials = Some(std::ptr::read_unaligned(
                        libc::CMSG_DATA(header).cast::<libc::ucred>(),
                    ));
                } else if h.cmsg_level == libc::SOL_SOCKET && h.cmsg_type == libc::SCM_RIGHTS {
                    let size = h.cmsg_len.saturating_sub(libc::CMSG_LEN(0) as usize)
                        / mem::size_of::<i32>();
                    for index in 0..size {
                        descriptors.push(OwnedFd::from_raw_fd(std::ptr::read_unaligned(
                            libc::CMSG_DATA(header).cast::<i32>().add(index),
                        )));
                    }
                } else {
                    unknown = true;
                }
                header = libc::CMSG_NXTHDR(&message, header);
            }
        }
        let credential = credentials.ok_or("browser_engine_identity_unavailable")?;
        let packet = bytes
            .get(..count as usize)
            .ok_or("browser_engine_identity_unavailable")?;
        let spawn = if packet == b"o3-native-engine-spawn-v1" {
            true
        } else if packet == b"o3-native-engine-identity-v1" {
            false
        } else {
            return Err("browser_engine_identity_unavailable".into());
        };
        if expected.is_some_and(|value| value != spawn)
            || message.msg_flags & (libc::MSG_TRUNC | libc::MSG_CTRUNC) != 0
            || unknown
            || descriptors.len() != 1
            || credential.pid <= 0
            || credential.uid != unsafe { libc::geteuid() }
        {
            return Err("browser_engine_identity_unavailable".into());
        }
        let pidfd = descriptors.pop().unwrap();
        let info = std::fs::read_to_string(format!("/proc/self/fdinfo/{}", pidfd.as_raw_fd()))
            .map_err(|_| "browser_engine_identity_unavailable")?;
        let pid = info
            .lines()
            .find_map(|line| {
                line.strip_prefix("Pid:")
                    .and_then(|value| value.trim().parse::<i32>().ok())
            })
            .filter(|pid| *pid > 0)
            .ok_or("browser_engine_identity_unavailable")?;
        if (spawn && credential.pid as u32 != renderer)
            || (!spawn && credential.pid != pid)
            || !live(pidfd.as_raw_fd())
        {
            return Err("browser_engine_identity_unavailable".into());
        }
        let status = std::fs::read_to_string(format!("/proc/{pid}/status"))
            .map_err(|_| "browser_engine_identity_unavailable")?;
        if !status.lines().any(|line| {
            line.strip_prefix("PPid:")
                .and_then(|value| value.trim().parse::<u32>().ok())
                == Some(renderer)
        }) || !live(pidfd.as_raw_fd())
        {
            return Err("browser_engine_identity_unavailable".into());
        }
        Ok((spawn, Engine { pid, pidfd }))
    }
}
fn live(fd: i32) -> bool {
    let mut watch = libc::pollfd {
        fd,
        events: libc::POLLIN,
        revents: 0,
    };
    unsafe { libc::poll(&mut watch, 1, 0) == 0 }
}
/// Called only by the actual renderer immediately after Command::spawn.
/// This endpoint stays CLOEXEC in the renderer; only the separate trusted-main
/// identity duplicate is inherited by the engine.
pub(crate) fn report_spawn(fd: i32, child: u32) -> Result<()> {
    report_pidfd(fd, child, b"o3-native-engine-spawn-v1")
}
fn report_pidfd(fd: i32, child: u32, bytes: &[u8]) -> Result<()> {
    let raw = unsafe { libc::syscall(libc::SYS_pidfd_open, child, 0) as i32 };
    if raw < 0 {
        return Err("browser_engine_identity_unavailable".into());
    }
    let pidfd = unsafe { OwnedFd::from_raw_fd(raw) };
    if !live(raw) {
        return Err("browser_engine_identity_unavailable".into());
    }
    let mut payload = libc::iovec {
        iov_base: bytes.as_ptr().cast_mut().cast(),
        iov_len: bytes.len(),
    };
    let mut ancillary = [0_usize; 4];
    let mut message: libc::msghdr = unsafe { mem::zeroed() };
    message.msg_iov = &mut payload;
    message.msg_iovlen = 1;
    message.msg_control = ancillary.as_mut_ptr().cast();
    message.msg_controllen = unsafe { libc::CMSG_SPACE(mem::size_of::<i32>() as _) as usize };
    unsafe {
        let header = libc::CMSG_FIRSTHDR(&message);
        (*header).cmsg_level = libc::SOL_SOCKET;
        (*header).cmsg_type = libc::SCM_RIGHTS;
        (*header).cmsg_len = libc::CMSG_LEN(mem::size_of::<i32>() as _) as usize;
        std::ptr::write_unaligned(libc::CMSG_DATA(header).cast::<i32>(), pidfd.as_raw_fd());
    }
    if unsafe { libc::sendmsg(fd, &message, libc::MSG_NOSIGNAL | libc::MSG_DONTWAIT) }
        != bytes.len() as isize
    {
        return Err("browser_engine_identity_unavailable".into());
    }
    Ok(())
}
/// Confine the inherited endpoint before loading a GUI library or launching any
/// descendant. An unrelated/non-native renderer may not receive this endpoint.
pub fn confine(payload: &Value) -> Result<()> {
    let Some(value) = payload.get("privateClipboardIdentityFd") else {
        return Ok(());
    };
    let fd = value
        .as_i64()
        .and_then(|fd| i32::try_from(fd).ok())
        .filter(|fd| *fd >= 64)
        .ok_or("invalid_browser_private_descriptor")?;
    let mut kind = 0_i32;
    let mut size = mem::size_of_val(&kind) as libc::socklen_t;
    let mut peer: libc::ucred = unsafe { mem::zeroed() };
    let mut peer_size = mem::size_of_val(&peer) as libc::socklen_t;
    let native = std::env::var("O3_BROWSER_ENGINE").as_deref() == Ok("native-electron");
    if !native
        || text(payload, "environmentId").is_empty()
        || unsafe {
            libc::getsockopt(
                fd,
                libc::SOL_SOCKET,
                libc::SO_TYPE,
                (&mut kind as *mut i32).cast(),
                &mut size,
            )
        } != 0
        || kind != libc::SOCK_SEQPACKET
        || unsafe {
            libc::getsockopt(
                fd,
                libc::SOL_SOCKET,
                libc::SO_PEERCRED,
                (&mut peer as *mut libc::ucred).cast(),
                &mut peer_size,
            )
        } != 0
        || peer.pid != unsafe { libc::getppid() }
        || peer.uid != unsafe { libc::geteuid() }
    {
        return Err("invalid_browser_private_descriptor".into());
    }
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFD, flags | libc::FD_CLOEXEC) } != 0 {
        return Err("invalid_browser_private_descriptor".into());
    }
    Ok(())
}

#[cfg(test)]
mod spawn_tests {
    use super::*;
    struct Cohort(libc::pid_t);
    impl Drop for Cohort {
        fn drop(&mut self) {
            if self.0 <= 0 {
                return;
            }
            unsafe {
                libc::kill(-self.0, libc::SIGKILL);
                libc::waitpid(self.0, std::ptr::null_mut(), 0);
            }
        }
    }
    #[test]
    fn spawn_record_refuses_foreign_renderer_and_false_main_sender() {
        let pending = Pending::new().unwrap();
        let engine = unsafe { libc::fork() };
        assert!(engine >= 0);
        if engine == 0 {
            unsafe {
                libc::setpgid(0, 0);
            }
            loop {
                unsafe {
                    libc::pause();
                }
            }
        }
        unsafe {
            libc::setpgid(engine, engine);
        }
        let _cohort = Cohort(engine);
        report_spawn(pending.source.as_raw_fd(), engine as _).unwrap();
        assert!(pending
            .admit_spawn(unsafe { libc::getpid() } as u32 + 1)
            .is_err());
        // Possessing a child pidfd does not make the renderer the main sender.
        report_pidfd(
            pending.source.as_raw_fd(),
            engine as _,
            b"o3-native-engine-identity-v1",
        )
        .unwrap();
        assert!(pending.admit(unsafe { libc::getpid() } as _).is_err());
        report_spawn(pending.source.as_raw_fd(), engine as _).unwrap();
        let admitted = pending.admit_spawn(unsafe { libc::getpid() } as _).unwrap();
        assert_eq!(admitted.pid(), engine);
        assert_eq!(
            unsafe {
                libc::syscall(
                    libc::SYS_pidfd_send_signal,
                    admitted.pidfd().as_raw_fd(),
                    libc::SIGTERM,
                    std::ptr::null::<libc::siginfo_t>(),
                    0,
                )
            },
            0
        );
        let mut watch = libc::pollfd {
            fd: admitted.pidfd().as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        assert_eq!(unsafe { libc::poll(&mut watch, 1, 1000) }, 1);
        assert!(!live(admitted.pidfd().as_raw_fd()));
        assert!(report_spawn(pending.source.as_raw_fd(), engine as _).is_err());
    }
    #[test]
    fn spawn_pidfd_precedes_independent_same_engine_confirmation() {
        roundtrip(false);
    }
    #[test]
    fn independently_identified_main_may_arrive_before_spawn_record() {
        roundtrip(true);
    }
    fn roundtrip(main_first: bool) {
        let pending = Pending::new().unwrap();
        let mut barrier = [-1; 2];
        assert_eq!(
            unsafe { libc::pipe2(barrier.as_mut_ptr(), libc::O_CLOEXEC) },
            0
        );
        let mut acknowledged = [-1; 2];
        assert_eq!(
            unsafe { libc::pipe2(acknowledged.as_mut_ptr(), libc::O_CLOEXEC) },
            0
        );
        let renderer = unsafe { libc::fork() };
        assert!(renderer > -1);
        if renderer == 0 {
            unsafe {
                libc::setpgid(0, 0);
            }
            let engine = unsafe { libc::fork() };
            if engine == 0 {
                unsafe {
                    libc::close(barrier[1]);
                }
                let mut ready = 0_u8;
                if unsafe { libc::read(barrier[0], (&mut ready as *mut u8).cast(), 1) } != 1 {
                    unsafe {
                        libc::_exit(2);
                    }
                }
                unsafe {
                    libc::close(barrier[0]);
                }
                // A synthetic trusted-main sender: kernel credentials differ from
                // the renderer. Actual addon/Electron proof is a separate layer.
                if report_pidfd(
                    pending.source.as_raw_fd(),
                    unsafe { libc::getpid() } as _,
                    b"o3-native-engine-identity-v1",
                )
                .is_err()
                {
                    unsafe {
                        libc::_exit(3);
                    }
                }
                if main_first {
                    let ready = 1_u8;
                    unsafe {
                        libc::close(acknowledged[0]);
                    }
                    if unsafe { libc::write(acknowledged[1], (&ready as *const u8).cast(), 1) } != 1
                    {
                        unsafe {
                            libc::_exit(7);
                        }
                    }
                }
                unsafe {
                    libc::close(acknowledged[1]);
                }
                loop {
                    unsafe {
                        libc::pause();
                    }
                }
            }
            if engine < 0 {
                unsafe {
                    libc::_exit(4);
                }
            }
            unsafe {
                libc::close(barrier[0]);
            }
            unsafe {
                libc::close(acknowledged[1]);
            }
            if main_first {
                let ready = 1_u8;
                if unsafe { libc::write(barrier[1], (&ready as *const u8).cast(), 1) } != 1 {
                    unsafe {
                        libc::_exit(8);
                    }
                }
                let mut acknowledgement = 0_u8;
                if unsafe {
                    libc::read(acknowledged[0], (&mut acknowledgement as *mut u8).cast(), 1)
                } != 1
                {
                    unsafe {
                        libc::_exit(9);
                    }
                }
            }
            unsafe {
                libc::close(acknowledged[0]);
            }
            if report_spawn(pending.source.as_raw_fd(), engine as _).is_err() {
                unsafe {
                    libc::kill(engine, libc::SIGKILL);
                    libc::_exit(5);
                }
            }
            let ready = 1_u8;
            if !main_first
                && unsafe { libc::write(barrier[1], (&ready as *const u8).cast(), 1) } != 1
            {
                unsafe {
                    libc::kill(engine, libc::SIGKILL);
                    libc::_exit(6);
                }
            }
            unsafe {
                libc::close(barrier[1]);
                libc::waitpid(engine, std::ptr::null_mut(), 0);
                libc::_exit(0);
            }
        }
        let mut cohort = Cohort(renderer);
        unsafe {
            libc::setpgid(renderer, renderer);
            libc::close(barrier[0]);
            libc::close(barrier[1]);
            libc::close(acknowledged[0]);
            libc::close(acknowledged[1]);
        }
        let engine = pending.admit_spawn(renderer as _).unwrap();
        assert_ne!(engine.pid(), renderer);
        pending.confirm(renderer as _, &engine).unwrap();
        assert_eq!(
            unsafe {
                libc::syscall(
                    libc::SYS_pidfd_send_signal,
                    engine.pidfd().as_raw_fd(),
                    libc::SIGTERM,
                    std::ptr::null::<libc::siginfo_t>(),
                    0,
                )
            },
            0
        );
        let mut status = 0;
        assert_eq!(unsafe { libc::waitpid(renderer, &mut status, 0) }, renderer);
        cohort.0 = 0;
        assert!(libc::WIFEXITED(status) && libc::WEXITSTATUS(status) == 0);
        assert!(!live(engine.pidfd().as_raw_fd()));
    }
}
