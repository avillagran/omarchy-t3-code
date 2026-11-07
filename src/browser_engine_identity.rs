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
        Ok(Self { socket, source })
    }
    pub fn source(&self) -> &OwnedFd {
        &self.source
    }
    pub fn admit(&self, renderer: u32) -> Result<Engine> {
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
        if count != b"o3-native-engine-identity-v1".len() as isize
            || bytes[..count.max(0) as usize] != *b"o3-native-engine-identity-v1"
            || message.msg_flags & (libc::MSG_TRUNC | libc::MSG_CTRUNC) != 0
            || unknown
            || descriptors.len() != 1
            || credential.pid <= 0
            || credential.uid != unsafe { libc::geteuid() }
        {
            return Err("browser_engine_identity_unavailable".into());
        }
        let status = std::fs::read_to_string(format!("/proc/{}/status", credential.pid))
            .map_err(|_| "browser_engine_identity_unavailable")?;
        if !status.lines().any(|line| {
            line.strip_prefix("PPid:")
                .and_then(|value| value.trim().parse::<u32>().ok())
                == Some(renderer)
        }) {
            return Err("browser_engine_identity_unavailable".into());
        }
        let pidfd = descriptors.pop().unwrap();
        let info = std::fs::read_to_string(format!("/proc/self/fdinfo/{}", pidfd.as_raw_fd()))
            .map_err(|_| "browser_engine_identity_unavailable")?;
        let mut poll = libc::pollfd {
            fd: pidfd.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        if !info.lines().any(|line| {
            line.strip_prefix("Pid:")
                .and_then(|value| value.trim().parse::<i32>().ok())
                == Some(credential.pid)
        }) || unsafe { libc::poll(&mut poll, 1, 0) } != 0
        {
            return Err("browser_engine_identity_unavailable".into());
        }
        Ok(Engine {
            pid: credential.pid,
            pidfd,
        })
    }
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
