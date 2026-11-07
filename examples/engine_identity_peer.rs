//! Synthetic process peer for the private engine-identity acceptance.
use std::{
    io::BufRead,
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
};
fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    let fd = args[1].parse::<i32>().unwrap();
    let mode = &args[2];
    let pid = if mode == "foreign-pidfd" {
        unsafe { libc::getppid() }
    } else {
        unsafe { libc::getpid() }
    };
    let raw = if mode == "regular-fd" {
        unsafe { libc::memfd_create(c"owned-identity-fixture".as_ptr(), libc::MFD_CLOEXEC) }
    } else {
        unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) as i32 }
    };
    assert!(raw >= 0);
    let descriptor = unsafe { OwnedFd::from_raw_fd(raw) };
    let bytes = if mode == "bad-body" {
        &b"bad"[..]
    } else {
        &b"o3-native-engine-identity-v1"[..]
    };
    let rights = vec![
        descriptor.as_raw_fd();
        if mode == "many-rights" {
            128
        } else if mode == "extra-rights" {
            2
        } else {
            1
        }
    ];
    let mut control = vec![0_usize; 64 + rights.len()];
    let mut data = libc::iovec {
        iov_base: bytes.as_ptr().cast_mut().cast(),
        iov_len: bytes.len(),
    };
    let mut message: libc::msghdr = unsafe { std::mem::zeroed() };
    message.msg_iov = &mut data;
    message.msg_iovlen = 1;
    message.msg_control = control.as_mut_ptr().cast();
    message.msg_controllen =
        unsafe { libc::CMSG_SPACE((rights.len() * std::mem::size_of::<i32>()) as _) as usize };
    unsafe {
        let header = libc::CMSG_FIRSTHDR(&message);
        (*header).cmsg_level = libc::SOL_SOCKET;
        (*header).cmsg_type = libc::SCM_RIGHTS;
        (*header).cmsg_len =
            libc::CMSG_LEN((rights.len() * std::mem::size_of::<i32>()) as _) as usize;
        std::ptr::copy_nonoverlapping(
            rights.as_ptr(),
            libc::CMSG_DATA(header).cast::<i32>(),
            rights.len(),
        );
        assert_eq!(
            libc::sendmsg(fd, &message, libc::MSG_NOSIGNAL),
            bytes.len() as isize
        );
    }
    let _ = std::io::stdin().lock().lines().next();
}
