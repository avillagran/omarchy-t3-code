//! An explicitly selected native cohort owns its X server and authentication.
//! Cookies stay in sealed anonymous descriptors, never files or child arguments.
use crate::Result;
use std::{
    fs,
    io::Read,
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
    os::unix::{net::UnixStream, process::CommandExt},
    path::Path,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

pub struct Display {
    child: Child,
    authorization: OwnedFd,
    number: u32,
}
impl Display {
    pub fn launch(executable: &Path) -> Result<Self> {
        let metadata =
            fs::symlink_metadata(executable).map_err(|_| "browser_display_unavailable")?;
        if !executable.is_absolute() || !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err("browser_display_unavailable".into());
        }
        let authorization = authorization()?;
        let mut pipe = [-1; 2];
        if unsafe { libc::pipe2(pipe.as_mut_ptr(), libc::O_CLOEXEC | libc::O_NONBLOCK) } != 0 {
            return Err("browser_display_unavailable".into());
        }
        let read = unsafe { OwnedFd::from_raw_fd(pipe[0]) };
        let write = unsafe { OwnedFd::from_raw_fd(pipe[1]) };
        let number_source = crate::desktop_browser_ipc::duplicate_child_source(write.as_raw_fd())
            .map_err(|_| "browser_display_unavailable")?;
        let number_fd = number_source.as_raw_fd();
        let parent = unsafe { libc::getpid() };
        let mut command = Command::new(executable);
        command
            .args([
                "-screen",
                "0",
                "1280x1024x24",
                "-nolisten",
                "tcp",
                "-extension",
                "GLX",
            ])
            .arg("-auth")
            .arg(format!("/proc/{parent}/fd/{}", authorization.as_raw_fd()))
            .arg("-displayfd")
            .arg(number_fd.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0);
        unsafe {
            command.pre_exec(move || {
                if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) != 0
                    || libc::getppid() != parent
                {
                    return Err(std::io::Error::other("browser_parent_gone"));
                }
                crate::desktop_browser_ipc::admit_child_source(number_fd)?;
                Ok(())
            });
        }
        let child = command.spawn().map_err(|_| "browser_display_unavailable")?;
        drop(write);
        drop(number_source);
        let mut owned = Self {
            child,
            authorization,
            number: 0,
        };
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut input = std::fs::File::from(read);
        let mut bytes = Vec::new();
        loop {
            let mut block = [0_u8; 32];
            match input.read(&mut block) {
                Ok(0) => return Err("browser_display_unavailable".into()),
                Ok(count) => {
                    if bytes.len() + count > 16 {
                        return Err("browser_display_unavailable".into());
                    }
                    bytes.extend_from_slice(&block[..count]);
                    if bytes.last() == Some(&b'\n') {
                        break;
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(_) => return Err("browser_display_unavailable".into()),
            }
            if Instant::now() >= deadline
                || owned
                    .child
                    .try_wait()
                    .map_err(|_| "browser_display_unavailable")?
                    .is_some()
            {
                return Err("browser_display_unavailable".into());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let number = bytes
            .strip_suffix(b"\n")
            .filter(|value| !value.is_empty() && value.iter().all(u8::is_ascii_digit))
            .ok_or("browser_display_unavailable")?;
        owned.number = std::str::from_utf8(number)
            .ok()
            .and_then(|value| value.parse::<u32>().ok())
            .filter(|value| *value <= 65535)
            .ok_or("browser_display_unavailable")?;
        let _ = owned.connect()?;
        Ok(owned)
    }
    pub fn name(&self) -> String {
        format!(":{}", self.number)
    }
    pub fn server_pid(&self) -> u32 {
        self.child.id()
    }
    pub fn authorization_path(&self) -> String {
        format!(
            "/proc/{}/fd/{}",
            unsafe { libc::getpid() },
            self.authorization.as_raw_fd()
        )
    }
    pub fn authorization(&self) -> &OwnedFd {
        &self.authorization
    }
    pub fn connect(&self) -> Result<UnixStream> {
        let stream = UnixStream::connect(format!("/tmp/.X11-unix/X{}", self.number))
            .map_err(|_| "browser_display_unavailable")?;
        let mut peer: libc::ucred = unsafe { std::mem::zeroed() };
        let mut size = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
        if unsafe {
            libc::getsockopt(
                stream.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_PEERCRED,
                (&mut peer as *mut libc::ucred).cast(),
                &mut size,
            )
        } != 0
            || size as usize != std::mem::size_of::<libc::ucred>()
            || peer.pid as u32 != self.child.id()
            || peer.uid != unsafe { libc::geteuid() }
        {
            return Err("browser_display_unavailable".into());
        }
        Ok(stream)
    }
}
impl Drop for Display {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
fn authorization() -> Result<OwnedFd> {
    let mut cookie = zeroize::Zeroizing::new([0_u8; 16]);
    let mut offset = 0;
    while offset < cookie.len() {
        let read = unsafe {
            libc::getrandom(
                cookie[offset..].as_mut_ptr().cast(),
                cookie.len() - offset,
                0,
            )
        };
        if read < 0 && std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
            continue;
        }
        if read <= 0 {
            return Err("browser_display_unavailable".into());
        }
        offset += read as usize;
    }
    let raw = unsafe {
        libc::memfd_create(
            c"o3-native-display-auth".as_ptr(),
            libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING,
        )
    };
    if raw < 0 {
        return Err("browser_display_unavailable".into());
    }
    let descriptor = unsafe { OwnedFd::from_raw_fd(raw) };
    let mut record = zeroize::Zeroizing::new(Vec::new());
    record.extend_from_slice(&65535_u16.to_be_bytes());
    for field in [&b""[..], &b""[..], &b"MIT-MAGIC-COOKIE-1"[..], &cookie[..]] {
        record.extend_from_slice(&(field.len() as u16).to_be_bytes());
        record.extend_from_slice(field);
    }
    let mut offset = 0;
    while offset < record.len() {
        let written =
            unsafe { libc::write(raw, record[offset..].as_ptr().cast(), record.len() - offset) };
        if written < 0 && std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted
        {
            continue;
        }
        if written <= 0 {
            return Err("browser_display_unavailable".into());
        }
        offset += written as usize;
    }
    if unsafe {
        libc::fcntl(
            raw,
            libc::F_ADD_SEALS,
            libc::F_SEAL_WRITE | libc::F_SEAL_GROW | libc::F_SEAL_SHRINK | libc::F_SEAL_SEAL,
        )
    } != 0
    {
        return Err("browser_display_unavailable".into());
    }
    Ok(descriptor)
}
