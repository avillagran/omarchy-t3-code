//! Private registration with the clipboard member in the actual GUI process.
//! This channel contains ownership metadata only; clipboard bodies never use
//! worker stdout, QML properties, or the generic durable mutation queue.
use std::{
    mem,
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
};

pub struct Session {
    socket: OwnedFd,
    binding: String,
}
impl Session {
    pub fn alive(&self) -> bool {
        let mut poll = libc::pollfd {
            fd: self.socket.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // The idle registration channel has no unsolicited messages. Any input,
        // EOF, error or hangup invalidates admission until a genuine replacement.
        unsafe { libc::poll(&mut poll, 1, 0) == 0 }
    }
    pub fn binding(&self) -> &str {
        &self.binding
    }
}
/// Missing or failed optional registration leaves the ordinary service/Qt path
/// available. It never activates a browser capability or fabricates admission.
pub fn start() -> Option<Session> {
    let endpoint = std::env::var("O3_GUI_CLIPBOARD_ENDPOINT").ok()?;
    connect(&endpoint).ok()
}
fn connect(endpoint: &str) -> std::io::Result<Session> {
    let invalid = || std::io::Error::other("invalid_gui_clipboard_registration");
    if !endpoint.starts_with("o3-clipboard-")
        || endpoint.len() > 96
        || !endpoint
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Err(invalid());
    }
    let raw = unsafe {
        libc::socket(
            libc::AF_UNIX,
            libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC | libc::SOCK_NONBLOCK,
            0,
        )
    };
    if raw < 0 {
        return Err(std::io::Error::last_os_error());
    }
    let socket = unsafe { OwnedFd::from_raw_fd(raw) };
    let mut address: libc::sockaddr_un = unsafe { mem::zeroed() };
    address.sun_family = libc::AF_UNIX as _;
    for (index, byte) in endpoint.bytes().enumerate() {
        address.sun_path[index + 1] = byte as _;
    }
    let length = mem::offset_of!(libc::sockaddr_un, sun_path) + 1 + endpoint.len();
    if unsafe {
        libc::connect(
            raw,
            (&address as *const libc::sockaddr_un).cast(),
            length as _,
        )
    } != 0
    {
        return Err(std::io::Error::last_os_error());
    }
    let flags = unsafe { libc::fcntl(raw, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(raw, libc::F_SETFL, flags & !libc::O_NONBLOCK) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    let mut peer: libc::ucred = unsafe { mem::zeroed() };
    let mut size = mem::size_of::<libc::ucred>() as libc::socklen_t;
    if unsafe {
        libc::getsockopt(
            raw,
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&mut peer as *mut libc::ucred).cast(),
            &mut size,
        )
    } != 0
        || size as usize != mem::size_of::<libc::ucred>()
        || peer.pid != unsafe { libc::getppid() }
        || peer.uid != unsafe { libc::geteuid() }
    {
        return Err(invalid());
    }
    let timeout = libc::timeval {
        tv_sec: 2,
        tv_usec: 0,
    };
    for option in [libc::SO_RCVTIMEO, libc::SO_SNDTIMEO] {
        if unsafe {
            libc::setsockopt(
                raw,
                libc::SOL_SOCKET,
                option,
                (&timeout as *const libc::timeval).cast(),
                mem::size_of_val(&timeout) as _,
            )
        } != 0
        {
            return Err(std::io::Error::last_os_error());
        }
    }
    let hello = br#"{"op":"hello","version":1}"#;
    if unsafe { libc::send(raw, hello.as_ptr().cast(), hello.len(), libc::MSG_NOSIGNAL) }
        != hello.len() as isize
    {
        return Err(invalid());
    }
    let mut bytes = [0_u8; 4096];
    let received =
        unsafe { libc::recv(raw, bytes.as_mut_ptr().cast(), bytes.len(), libc::MSG_TRUNC) };
    if received <= 0 || received as usize > bytes.len() {
        return Err(invalid());
    }
    let row: serde_json::Value =
        serde_json::from_slice(&bytes[..received as usize]).map_err(|_| invalid())?;
    let binding = row["binding"].as_str().ok_or_else(invalid)?;
    if row.as_object().map(|row| row.len()) != Some(3)
        || row["type"] != "admitted"
        || row["version"] != 1
        || uuid::Uuid::parse_str(binding).is_err()
    {
        return Err(invalid());
    }
    Ok(Session {
        socket,
        binding: binding.to_owned(),
    })
}
