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
    parent: libc::pid_t,
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
    let enabled = 1_i32;
    if unsafe {
        libc::setsockopt(
            raw,
            libc::SOL_SOCKET,
            libc::SO_PASSCRED,
            (&enabled as *const i32).cast(),
            std::mem::size_of_val(&enabled) as _,
        )
    } != 0
    {
        return Err(std::io::Error::last_os_error());
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
    let row = response(&socket, peer.pid)?;
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
        parent: peer.pid,
    })
}

pub type Shared = std::sync::Arc<std::sync::Mutex<Session>>;
/// A physical cohort lease. Its retirement can never clear the GUI's retained
/// current offer, or a replacement cohort with another opaque identity.
pub struct CohortLease {
    session: Shared,
    id: String,
}
impl Drop for CohortLease {
    fn drop(&mut self) {
        if let Ok(mut session) = self.session.lock() {
            let _ = session.retire(&self.id);
        }
    }
}
pub fn register(
    session: Shared,
    id: &str,
    display: &crate::browser_native_display::Display,
    engine: &crate::browser_engine_identity::Engine,
) -> crate::Result<CohortLease> {
    let stream = display.connect()?;
    let descriptors = [
        stream.as_raw_fd(),
        display.authorization().as_raw_fd(),
        engine.pidfd().as_raw_fd(),
    ];
    let registered = session
        .lock()
        .map_err(|_| "browser_clipboard_unavailable")?
        .register(id, engine.pid(), &descriptors);
    registered.map_err(|_| "browser_clipboard_unavailable")?;
    Ok(CohortLease {
        session,
        id: id.into(),
    })
}
impl Session {
    pub(crate) fn register_input_context(
        &mut self,
        id: &str,
        engine: i32,
        rights: &[i32],
    ) -> std::io::Result<libc::pid_t> {
        let body = self.metadata("register-input-context", id, Some(engine))?;
        let row = self.exchange(&body, rights)?;
        if row["type"] == "input-context-registered"
            && row["cohort"] == id
            && row.as_object().is_some_and(|row| row.len() == 2)
        {
            Ok(self.parent)
        } else {
            Err(std::io::Error::other("browser_input_context_unavailable"))
        }
    }
    pub(crate) fn retire_input_context(&mut self, id: &str) -> std::io::Result<()> {
        let body = self.metadata("retire-input-context", id, None)?;
        let row = self.exchange(&body, &[])?;
        if row["type"] == "input-context-retired"
            && row["cohort"] == id
            && row.as_object().is_some_and(|row| row.len() == 2)
        {
            Ok(())
        } else {
            Err(std::io::Error::other("browser_input_context_unavailable"))
        }
    }
    fn metadata(&self, op: &str, id: &str, engine: Option<i32>) -> std::io::Result<Vec<u8>> {
        let mut row = std::collections::BTreeMap::from([
            ("binding", serde_json::json!(self.binding)),
            ("cohort", serde_json::json!(id)),
            ("op", serde_json::json!(op)),
            ("version", serde_json::json!(1)),
        ]);
        if let Some(pid) = engine {
            row.insert("enginePid", serde_json::json!(pid));
        }
        serde_json::to_vec(&row).map_err(std::io::Error::other)
    }
    fn exchange(&mut self, body: &[u8], rights: &[i32]) -> std::io::Result<serde_json::Value> {
        let invalid = || std::io::Error::other("invalid_gui_clipboard_registration");
        if body.len() > 4096 || rights.len() > 3 || !self.alive() {
            return Err(invalid());
        }
        let mut ancillary = [0_usize; 8];
        let mut payload = libc::iovec {
            iov_base: body.as_ptr().cast_mut().cast(),
            iov_len: body.len(),
        };
        let mut message: libc::msghdr = unsafe { std::mem::zeroed() };
        message.msg_iov = &mut payload;
        message.msg_iovlen = 1;
        if !rights.is_empty() {
            message.msg_control = ancillary.as_mut_ptr().cast();
            message.msg_controllen =
                unsafe { libc::CMSG_SPACE(std::mem::size_of_val(rights) as _) as usize };
            unsafe {
                let header = libc::CMSG_FIRSTHDR(&message);
                (*header).cmsg_level = libc::SOL_SOCKET;
                (*header).cmsg_type = libc::SCM_RIGHTS;
                (*header).cmsg_len = libc::CMSG_LEN(std::mem::size_of_val(rights) as _) as usize;
                std::ptr::copy_nonoverlapping(
                    rights.as_ptr(),
                    libc::CMSG_DATA(header).cast::<i32>(),
                    rights.len(),
                );
            }
        }
        let sent = unsafe { libc::sendmsg(self.socket.as_raw_fd(), &message, libc::MSG_NOSIGNAL) };
        let result = if sent != body.len() as isize {
            Err(invalid())
        } else {
            response(&self.socket, self.parent)
        };
        if result.is_err() {
            unsafe {
                libc::shutdown(self.socket.as_raw_fd(), libc::SHUT_RDWR);
            }
        }
        result
    }
    fn register(&mut self, id: &str, engine: i32, rights: &[i32]) -> std::io::Result<()> {
        let body = self.metadata("register", id, Some(engine))?;
        let row = self.exchange(&body, rights)?;
        if row["type"] == "registered"
            && row["cohort"] == id
            && row.as_object().is_some_and(|row| row.len() == 2)
        {
            Ok(())
        } else {
            Err(std::io::Error::other("clipboard_cohort_unavailable"))
        }
    }
    fn retire(&mut self, id: &str) -> std::io::Result<()> {
        let body = self.metadata("retire", id, None)?;
        let row = self.exchange(&body, &[])?;
        if row["type"] == "retired"
            && row["cohort"] == id
            && row.as_object().is_some_and(|row| row.len() == 2)
        {
            Ok(())
        } else {
            Err(std::io::Error::other("clipboard_cohort_unavailable"))
        }
    }
}
fn response(socket: &OwnedFd, parent: libc::pid_t) -> std::io::Result<serde_json::Value> {
    let invalid = || std::io::Error::other("invalid_gui_clipboard_registration");
    let mut bytes = [0_u8; 4096];
    let mut ancillary = [0_usize; 16];
    let mut payload = libc::iovec {
        iov_base: bytes.as_mut_ptr().cast(),
        iov_len: bytes.len(),
    };
    let mut message: libc::msghdr = unsafe { std::mem::zeroed() };
    message.msg_iov = &mut payload;
    message.msg_iovlen = 1;
    message.msg_control = ancillary.as_mut_ptr().cast();
    message.msg_controllen = std::mem::size_of_val(&ancillary);
    let count = unsafe { libc::recvmsg(socket.as_raw_fd(), &mut message, libc::MSG_CMSG_CLOEXEC) };
    let mut admitted = false;
    let mut unknown = false;
    unsafe {
        let mut header = libc::CMSG_FIRSTHDR(&message);
        while !header.is_null() {
            let value = &*header;
            if value.cmsg_level == libc::SOL_SOCKET
                && value.cmsg_type == libc::SCM_CREDENTIALS
                && value.cmsg_len
                    == libc::CMSG_LEN(std::mem::size_of::<libc::ucred>() as _) as usize
            {
                let credential =
                    std::ptr::read_unaligned(libc::CMSG_DATA(header).cast::<libc::ucred>());
                admitted = credential.pid == parent && credential.uid == libc::geteuid();
            } else {
                unknown = true;
                if value.cmsg_level == libc::SOL_SOCKET && value.cmsg_type == libc::SCM_RIGHTS {
                    let size = value.cmsg_len.saturating_sub(libc::CMSG_LEN(0) as usize)
                        / std::mem::size_of::<i32>();
                    for index in 0..size {
                        libc::close(std::ptr::read_unaligned(
                            libc::CMSG_DATA(header).cast::<i32>().add(index),
                        ));
                    }
                }
            }
            header = libc::CMSG_NXTHDR(&message, header);
        }
    }
    if count <= 0
        || count as usize > bytes.len()
        || message.msg_flags & (libc::MSG_CTRUNC | libc::MSG_TRUNC) != 0
        || !admitted
        || unknown
    {
        return Err(invalid());
    }
    let row = serde_json::from_slice(&bytes[..count as usize]).map_err(|_| invalid())?;
    Ok(row)
}
