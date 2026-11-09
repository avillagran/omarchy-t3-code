//! Dedicated, bounded current-state delivery to the actual GUI input-method owner.
//! Text never crosses worker stdout/QML, the registration socket, or an outbox.
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::{json, Value};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

const LIMIT: usize = 16384;
fn unavailable() -> String {
    "browser_input_context_unavailable".into()
}

#[derive(Clone, PartialEq, Eq)]
pub struct Binding {
    pub scope: String,
    pub connection: String,
    pub surface: String,
    pub tab: String,
    pub frame: String,
}
impl Binding {
    fn valid(&self, owner: &str) -> bool {
        [
            &self.scope,
            &self.connection,
            &self.surface,
            &self.tab,
            &self.frame,
        ]
        .iter()
        .all(|field| !field.is_empty() && field.encode_utf16().count() <= 256)
            && self.connection.starts_with(&format!("{owner}:"))
    }
    fn json(&self) -> Value {
        json!({"scope":self.scope,"connection":self.connection,"surface":self.surface,
               "tab":self.tab,"frame":self.frame})
    }
    fn parse(value: &Value) -> Option<Self> {
        (value.as_object()?.len() == 5).then_some(())?;
        Some(Self {
            scope: value["scope"].as_str()?.into(),
            connection: value["connection"].as_str()?.into(),
            surface: value["surface"].as_str()?.into(),
            tab: value["tab"].as_str()?.into(),
            frame: value["frame"].as_str()?.into(),
        })
    }
}
pub struct Surrounding<'a> {
    pub text: &'a str,
    pub cursor_byte: u32,
    pub anchor_byte: u32,
}
pub struct State<'a> {
    pub serial: u32,
    pub caret: Option<[i32; 4]>,
    pub surrounding: Option<Surrounding<'a>>,
    pub hints: u16,
    pub purpose: u8,
}
pub struct Lease {
    session: crate::gui_clipboard::Shared,
    id: String,
    socket: Option<OwnedFd>,
    gui: libc::pid_t,
}
impl Lease {
    /// Registration authenticates the real engine pidfd and ancestry. The GUI
    /// independently checks each message's kernel credentials on this new FD.
    pub fn register(
        session: crate::gui_clipboard::Shared,
        id: &str,
        engine: &crate::browser_engine_identity::Engine,
    ) -> crate::Result<Self> {
        if uuid::Uuid::parse_str(id)
            .map(|value| value.to_string())
            .as_deref()
            != Ok(id)
        {
            return Err(unavailable());
        }
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
            return Err(unavailable());
        }
        let socket = unsafe { OwnedFd::from_raw_fd(pair[0]) };
        let gui_socket = unsafe { OwnedFd::from_raw_fd(pair[1]) };
        let enabled = 1_i32;
        if unsafe {
            libc::setsockopt(
                socket.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_PASSCRED,
                (&enabled as *const i32).cast(),
                std::mem::size_of_val(&enabled) as _,
            )
        } != 0
        {
            return Err(unavailable());
        }
        let gui = session
            .lock()
            .map_err(|_| unavailable())?
            .register_input_context(
                id,
                engine.pid(),
                &[gui_socket.as_raw_fd(), engine.pidfd().as_raw_fd()],
            )
            .map_err(|_| unavailable())?;
        Ok(Self {
            session,
            id: id.into(),
            socket: Some(socket),
            gui,
        })
    }
    fn retire_socket(&mut self) {
        if let Some(socket) = self.socket.take() {
            unsafe {
                libc::shutdown(socket.as_raw_fd(), libc::SHUT_RDWR);
            }
        }
    }
    fn send(&mut self, mut record: Value) -> crate::Result<()> {
        // This is our closed, typed GUI protocol. No unknown native CDP/context
        // payload is sorted or reserialized through this channel.
        record.sort_all_objects();
        let bytes = serde_json::to_vec(&record).map_err(|_| unavailable())?;
        let Some(socket) = &self.socket else {
            return Err(unavailable());
        };
        let sent = if bytes.len() <= LIMIT {
            unsafe {
                libc::send(
                    socket.as_raw_fd(),
                    bytes.as_ptr().cast(),
                    bytes.len(),
                    libc::MSG_DONTWAIT | libc::MSG_NOSIGNAL,
                )
            }
        } else {
            -1
        };
        if sent != bytes.len() as isize {
            self.retire_socket();
            return Err(unavailable());
        }
        Ok(())
    }
    pub fn publish(&mut self, binding: &Binding, state: &State<'_>) -> crate::Result<()> {
        if !binding.valid(&self.id) {
            return Err(unavailable());
        }
        if state.hints > 1023
            || state.purpose > 13
            || state
                .caret
                .is_some_and(|rectangle| rectangle[2] < 0 || rectangle[3] < 0)
        {
            self.invalidate(binding)?;
            return Err(unavailable());
        }
        let mut value = json!({"serial":state.serial,"hints":state.hints,"purpose":state.purpose});
        if let Some(rectangle) = state.caret {
            value["caret"] = json!(rectangle);
        }
        if let Some(surrounding) = &state.surrounding {
            let text = surrounding.text;
            if text.len() > 4000
                || text.contains('\0')
                || !text.is_char_boundary(surrounding.cursor_byte as usize)
                || !text.is_char_boundary(surrounding.anchor_byte as usize)
            {
                self.invalidate(binding)?;
                return Err(unavailable());
            }
            value["surroundingUtf8"] = STANDARD.encode(text).into();
            value["cursorByte"] = surrounding.cursor_byte.into();
            value["anchorByte"] = surrounding.anchor_byte.into();
        }
        self.send(json!({"binding":binding.json(),"op":"state","state":value,"version":1}))
    }
    pub fn invalidate(&mut self, binding: &Binding) -> crate::Result<()> {
        if !binding.valid(&self.id) {
            return Err(unavailable());
        }
        self.send(json!({"binding":binding.json(),"op":"invalidate","version":1}))
    }
    /// Poll one authenticated GUI metadata request. No reconnect, hidden queue,
    /// context history or automatic replay is introduced on failure.
    pub fn request(&mut self) -> crate::Result<Option<Binding>> {
        let result = self.receive();
        if result.is_err() {
            self.retire_socket();
        }
        result
    }
    fn receive(&self) -> crate::Result<Option<Binding>> {
        let socket = self.socket.as_ref().ok_or_else(unavailable)?;
        let mut bytes = [0_u8; LIMIT];
        let mut ancillary = [0_usize; 32];
        let mut data = libc::iovec {
            iov_base: bytes.as_mut_ptr().cast(),
            iov_len: bytes.len(),
        };
        let mut message: libc::msghdr = unsafe { std::mem::zeroed() };
        message.msg_iov = &mut data;
        message.msg_iovlen = 1;
        message.msg_control = ancillary.as_mut_ptr().cast();
        message.msg_controllen = std::mem::size_of_val(&ancillary);
        let count = unsafe {
            libc::recvmsg(
                socket.as_raw_fd(),
                &mut message,
                libc::MSG_DONTWAIT | libc::MSG_CMSG_CLOEXEC,
            )
        };
        if count < 0
            && matches!(
                std::io::Error::last_os_error().raw_os_error(),
                Some(libc::EAGAIN | libc::EINTR)
            )
        {
            return Ok(None);
        }
        if count < 0 || (count == 0 && message.msg_controllen == 0) {
            return Err(unavailable());
        }
        let mut credential = false;
        let mut unexpected = false;
        unsafe {
            let mut header = libc::CMSG_FIRSTHDR(&message);
            while !header.is_null() {
                let value = &*header;
                if value.cmsg_level == libc::SOL_SOCKET
                    && value.cmsg_type == libc::SCM_CREDENTIALS
                    && value.cmsg_len
                        == libc::CMSG_LEN(std::mem::size_of::<libc::ucred>() as _) as usize
                {
                    let sender =
                        std::ptr::read_unaligned(libc::CMSG_DATA(header).cast::<libc::ucred>());
                    credential = sender.pid == self.gui && sender.uid == libc::geteuid();
                } else {
                    unexpected = true;
                    if value.cmsg_level == libc::SOL_SOCKET && value.cmsg_type == libc::SCM_RIGHTS {
                        let length = value.cmsg_len.saturating_sub(libc::CMSG_LEN(0) as usize)
                            / std::mem::size_of::<i32>();
                        for index in 0..length {
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
            || !credential
            || unexpected
            || message.msg_flags & (libc::MSG_TRUNC | libc::MSG_CTRUNC) != 0
        {
            return Err(unavailable());
        }
        let mut value: Value =
            serde_json::from_slice(&bytes[..count as usize]).map_err(|_| unavailable())?;
        value.sort_all_objects();
        if serde_json::to_vec(&value).map_err(|_| unavailable())? != bytes[..count as usize]
            || value.as_object().is_none_or(|row| row.len() != 3)
            || value["version"].as_u64() != Some(1)
            || value["op"] != "request"
        {
            return Err(unavailable());
        }
        let binding = Binding::parse(&value["binding"])
            .filter(|binding| binding.valid(&self.id))
            .ok_or_else(unavailable)?;
        Ok(Some(binding))
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        self.retire_socket();
        if let Ok(mut session) = self.session.lock() {
            let _ = session.retire_input_context(&self.id);
        }
    }
}
