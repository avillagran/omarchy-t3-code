//! Private parent-side desktop-browser IPC. Raw CDP never enters status/watch
//! projections. Codec intake: native 32a0531748c44b25072d7495ce0b9bd7e9450a87.
use serde_json::{Map, Value};
use std::io;
use std::net::Shutdown;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::process::Command;

pub const MAX_CONTROL_WIRE_BYTES: usize = 256 * 1024 * 1024;
pub struct ParentEndpoints {
    pub events: UnixStream,
    pub control: UnixStream,
}
pub struct ChildEndpoints {
    pub events: UnixStream,
    pub control: UnixStream,
}
/// Server input receives events; server output supplies control commands.
/// UnixStream descriptors start CLOEXEC; the launcher alone admits child copies.
pub fn endpoints() -> io::Result<(ParentEndpoints, ChildEndpoints)> {
    let (events_parent, events_child) = UnixStream::pair()?;
    let (control_child, control_parent) = UnixStream::pair()?;
    events_parent.shutdown(Shutdown::Read)?;
    events_child.shutdown(Shutdown::Write)?;
    control_parent.shutdown(Shutdown::Write)?;
    control_child.shutdown(Shutdown::Read)?;
    Ok((
        ParentEndpoints {
            events: events_parent,
            control: control_parent,
        },
        ChildEndpoints {
            events: events_child,
            control: control_child,
        },
    ))
}
pub(crate) fn duplicate_child_source(source: RawFd) -> io::Result<OwnedFd> {
    // Reserve standard I/O and launcher bookkeeping slots. Each live duplicate
    // gets a distinct kernel-chosen descriptor and remains CLOEXEC in parent.
    let duplicate = unsafe { libc::fcntl(source, libc::F_DUPFD_CLOEXEC, 64) };
    if duplicate < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { OwnedFd::from_raw_fd(duplicate) })
}
pub(crate) fn admit_child_source(fd: RawFd) -> io::Result<()> {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
impl ChildEndpoints {
    /// Consume the child endpoints and retain their high-numbered duplicates in
    /// Command until exec. Only the forked child's flags become inheritable.
    pub fn configure(self, command: &mut Command) -> io::Result<(RawFd, RawFd)> {
        let events = duplicate_child_source(self.events.as_raw_fd())?;
        let control = duplicate_child_source(self.control.as_raw_fd())?;
        let descriptors = (events.as_raw_fd(), control.as_raw_fd());
        unsafe {
            command.pre_exec(move || {
                admit_child_source(events.as_raw_fd())?;
                admit_child_source(control.as_raw_fd())?;
                Ok(())
            });
        }
        Ok(descriptors)
    }
}
impl ParentEndpoints {
    /// Shutdown does not acquire a writer/global lifecycle lock and interrupts
    /// pending I/O on clones of these actual socket endpoints.
    pub fn close(&self) {
        let _ = self.events.shutdown(Shutdown::Both);
        let _ = self.control.shutdown(Shutdown::Both);
    }
}
fn trimmed(value: &str) -> &str {
    value.trim_matches(|c|matches!(c,'\u{9}'..='\u{d}'|' '|'\u{a0}'|'\u{1680}'|'\u{2000}'..='\u{200a}'|'\u{2028}'|'\u{2029}'|'\u{202f}'|'\u{205f}'|'\u{3000}'|'\u{feff}'))
}
fn string(object: &Map<String, Value>, name: &str, trim: bool) -> Option<Value> {
    let value = object.get(name)?.as_str()?;
    let value = if trim { trimmed(value) } else { value };
    if trim && value.is_empty() {
        return None;
    }
    Some(Value::String(value.into()))
}
fn project(value: &Value, command: bool) -> Option<Value> {
    let object = value.as_object()?;
    let kind = object.get("type")?.as_str()?;
    if !(if command {
        matches!(kind, "cdp" | "release" | "pointer" | "turn")
    } else {
        matches!(kind, "attached" | "detached" | "cdp" | "shown")
    }) {
        return None;
    }
    let mut output = Map::new();
    output.insert("type".into(), Value::String(kind.into()));
    output.insert("threadId".into(), string(object, "threadId", true)?);
    if kind != "turn" {
        output.insert("tabId".into(), string(object, "tabId", true)?);
    }
    match kind {
        "cdp" => {
            output.insert("message".into(), string(object, "message", false)?);
        }
        "shown" | "turn" => {
            let field = if kind == "shown" { "shown" } else { "active" };
            output.insert(field.into(), Value::Bool(object.get(field)?.as_bool()?));
        }
        "pointer" => {
            let phase = object.get("phase")?.as_str()?;
            if !matches!(phase, "move" | "click") {
                return None;
            }
            output.insert("phase".into(), Value::String(phase.into()));
            for field in ["x", "y"] {
                let value = object.get(field)?;
                if !value.as_f64()?.is_finite() {
                    return None;
                }
                output.insert(field.into(), value.clone());
            }
        }
        _ => {}
    }
    Some(Value::Object(output))
}
pub fn decode_command(value: &Value) -> Option<Value> {
    project(value, true)
}
pub fn decode_event(value: &Value) -> Option<Value> {
    project(value, false)
}
pub fn encode_event(value: &Value) -> Option<Vec<u8>> {
    let value = decode_event(value)?;
    let mut count = WireSize(0);
    serde_json::to_writer(&mut count, &value).ok()?;
    let mut bytes = Vec::with_capacity(count.0 + 1);
    serde_json::to_writer(&mut bytes, &value).ok()?;
    bytes.push(b'\n');
    Some(bytes)
}
struct WireSize(usize);
impl io::Write for WireSize {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0 = self
            .0
            .checked_add(bytes.len())
            .filter(|size| *size < MAX_CONTROL_WIRE_BYTES)
            .ok_or_else(|| io::Error::other("private_browser_wire_limit"))?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn json_number(token: &str) -> bool {
    let bytes = token.as_bytes();
    let mut i = usize::from(bytes.first() == Some(&b'-'));
    if bytes.get(i) == Some(&b'0') {
        i += 1;
    } else if bytes.get(i).is_some_and(|b| matches!(b, b'1'..=b'9')) {
        while bytes.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
    } else {
        return false;
    }
    if bytes.get(i) == Some(&b'.') {
        i += 1;
        let start = i;
        while bytes.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        if i == start {
            return false;
        }
    }
    if bytes.get(i).is_some_and(|b| matches!(b, b'e' | b'E')) {
        i += 1;
        if bytes.get(i).is_some_and(|b| matches!(b, b'+' | b'-')) {
            i += 1;
        }
        let start = i;
        while bytes.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        if i == start {
            return false;
        }
    }
    i == bytes.len()
}
/// Normalize only overflowing JSON numeric tokens for this schema projection.
/// Native JSON.parse accepts Infinity, which is rejected by Finite and ignored
/// in stripped fields. No accepted numeric field here permits Infinity. String
/// contents, including opaque CDP messages, remain byte-for-byte untouched.
fn projection_json(line: &str) -> Option<Value> {
    let bytes = line.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    let mut quoted = false;
    let mut escaped = false;
    while index < bytes.len() {
        let byte = bytes[index];
        if quoted {
            output.push(byte);
            index += 1;
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                quoted = false;
            }
        } else if byte == b'"' {
            quoted = true;
            output.push(byte);
            index += 1;
        } else if byte == b'-' || byte.is_ascii_digit() {
            let start = index;
            index += 1;
            while index < bytes.len()
                && matches!(bytes[index], b'0'..=b'9' | b'e' | b'E' | b'+' | b'-' | b'.')
            {
                index += 1;
            }
            let token = &line[start..index];
            if json_number(token) && token.parse::<f64>().is_ok_and(|value| !value.is_finite()) {
                output.extend_from_slice(b"null");
            } else {
                output.extend_from_slice(token.as_bytes());
            }
        } else {
            output.push(byte);
            index += 1;
        }
    }
    serde_json::from_slice(&output).ok()
}
pub fn decode_command_line(line: &str) -> Option<Value> {
    decode_command(&projection_json(line)?)
}

#[derive(Default)]
pub struct Lines {
    pub values: Vec<String>,
    pub cancelled: bool,
}
/// Actual native parent pipeline: TextDecoder streaming lossy UTF-8, splitLines
/// (CR/LF/CRLF), then filter length>0. Malformed JSON/schema lines are ignored by
/// decode_command_line; they do not poison subsequent commands.
pub struct ControlDecoder {
    utf8: Vec<u8>,
    line: String,
    first: bool,
    after_cr: bool,
    wire_bytes: usize,
    max_wire_bytes: usize,
    closed: bool,
}
impl Default for ControlDecoder {
    fn default() -> Self {
        Self::new(MAX_CONTROL_WIRE_BYTES)
    }
}
impl ControlDecoder {
    pub fn new(max_wire_bytes: usize) -> Self {
        Self {
            utf8: Vec::new(),
            line: String::new(),
            first: true,
            after_cr: false,
            wire_bytes: 0,
            max_wire_bytes,
            closed: false,
        }
    }
    fn push_text(&mut self, text: &str, output: &mut Lines) {
        for character in text.chars() {
            if self.first {
                self.first = false;
                if character == '\u{feff}' {
                    continue;
                }
            }
            match character {
                '\r' => {
                    if !self.line.is_empty() {
                        output.values.push(std::mem::take(&mut self.line));
                    }
                    self.after_cr = true;
                }
                '\n' => {
                    if !self.after_cr && !self.line.is_empty() {
                        output.values.push(std::mem::take(&mut self.line));
                    }
                    self.after_cr = false;
                }
                value => {
                    self.after_cr = false;
                    self.line.push(value);
                }
            }
        }
    }
    fn decode_bytes(&mut self, part: &[u8], output: &mut Lines) {
        let mut bytes = std::mem::take(&mut self.utf8);
        bytes.extend_from_slice(part);
        let mut offset = 0;
        loop {
            match std::str::from_utf8(&bytes[offset..]) {
                Ok(text) => {
                    self.push_text(text, output);
                    break;
                }
                Err(error) => {
                    let valid = error.valid_up_to();
                    self.push_text(
                        std::str::from_utf8(&bytes[offset..offset + valid])
                            .expect("UTF8 valid prefix"),
                        output,
                    );
                    offset += valid;
                    match error.error_len() {
                        Some(length) => {
                            self.push_text("\u{fffd}", output);
                            offset += length;
                        }
                        None => {
                            self.utf8.extend_from_slice(&bytes[offset..]);
                            break;
                        }
                    }
                }
            }
        }
    }
    pub fn feed(&mut self, bytes: &[u8]) -> Lines {
        let mut output = Lines::default();
        if self.closed {
            output.cancelled = true;
            return output;
        }
        for part in bytes.split_inclusive(|byte| matches!(byte, b'\r' | b'\n')) {
            let terminated = part
                .last()
                .is_some_and(|byte| matches!(byte, b'\r' | b'\n'));
            self.wire_bytes = self
                .wire_bytes
                .saturating_add(part.len() - usize::from(terminated));
            if self.wire_bytes > self.max_wire_bytes {
                self.close();
                output.cancelled = true;
                break;
            }
            self.decode_bytes(part, &mut output);
            if terminated {
                self.wire_bytes = 0;
            }
        }
        output
    }
    pub fn finish(&mut self) -> Lines {
        let mut output = Lines::default();
        if self.closed {
            output.cancelled = true;
            return output;
        }
        let tail = std::mem::take(&mut self.utf8);
        self.push_text(&String::from_utf8_lossy(&tail), &mut output);
        if !self.line.is_empty() {
            output.values.push(std::mem::take(&mut self.line));
        }
        self.closed = true;
        output
    }
    pub fn close(&mut self) {
        self.closed = true;
        self.utf8.clear();
        self.line.clear();
        self.wire_bytes = 0;
    }
}
