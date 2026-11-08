//! Internal app-private Wayland hosting. No host compositor is adopted.
use crate::{browser_engine_identity::Engine, browser_wayland_transport as transport, Result};
use std::{
    fs,
    io::Write,
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::{
            fs::{DirBuilderExt, FileTypeExt, MetadataExt, OpenOptionsExt},
            net::UnixListener,
            process::CommandExt,
        },
    },
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};
#[derive(Clone)]
pub(crate) struct Config {
    executable: PathBuf,
    libraries: PathBuf,
}
impl Config {
    pub(crate) fn new(executable: &Path, libraries: &Path) -> Result<Self> {
        let metadata = fs::symlink_metadata(executable).map_err(|_| error())?;
        let directory = fs::symlink_metadata(libraries).map_err(|_| error())?;
        if !executable.is_absolute()
            || !libraries.is_absolute()
            || !metadata.is_file()
            || metadata.file_type().is_symlink()
            || !directory.is_dir()
            || directory.file_type().is_symlink()
        {
            return Err(error());
        }
        Ok(Self {
            executable: executable.into(),
            libraries: libraries.into(),
        })
    }
}
fn error() -> String {
    "browser_wayland_host_unavailable".into()
}
fn directory(path: &Path) -> Result<()> {
    fs::DirBuilder::new()
        .mode(0o700)
        .create(path)
        .map_err(|_| error())?;
    let metadata = fs::symlink_metadata(path).map_err(|_| error())?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != unsafe { libc::geteuid() }
    {
        return Err(error());
    }
    Ok(())
}

fn duplicate(fd: i32) -> Result<OwnedFd> {
    let copy = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 64) };
    if copy < 0 {
        return Err(error());
    }
    Ok(unsafe { OwnedFd::from_raw_fd(copy) })
}
pub(crate) struct Display {
    child: Child,
    compositor_watch: Option<OwnedFd>,
    frontend: Option<UnixListener>,
    frontend_path: PathBuf,
    compositor_path: PathBuf,
    socket_identity: (u64, u64),
    cancel: Arc<AtomicBool>,
    join: Option<thread::JoinHandle<()>>,
}
impl Display {
    pub(crate) fn launch(config: &Config) -> Result<Self> {
        // These are ephemeral display sockets, not profile/bridge identity.
        // Keep their private namespace within Unix sockaddr_un's real limit.
        let base = std::env::temp_dir().join(format!("o3-wayland-{}", uuid::Uuid::new_v4()));
        directory(&base)?;
        let compositor = base.join("compositor");
        directory(&compositor)?;
        let frontend = base.join("engine");
        directory(&frontend)?;
        let home = base.join("home");
        directory(&home)?;
        let configuration = base.join("sway.conf");
        fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&configuration)
   .and_then(|mut file|file.write_all(b"xwayland disable\nswaybg_command -\noutput * mode 1280x1024\nseat seat0 fallback true\ndefault_border none\n")).map_err(|_|error())?;
        let frontend_path = frontend.join("engine.sock");
        // Fail before creating a path the platform cannot address; never truncate.
        if frontend_path.as_os_str().as_encoded_bytes().len() >= 107 {
            return Err(error());
        }
        let listener = UnixListener::bind(&frontend_path).map_err(|_| error())?;
        listener.set_nonblocking(true).map_err(|_| error())?;
        let flags = unsafe { libc::fcntl(listener.as_raw_fd(), libc::F_GETFD) };
        if flags < 0 || flags & libc::FD_CLOEXEC == 0 {
            return Err(error());
        }
        if unsafe { libc::listen(listener.as_raw_fd(), 8) } != 0 {
            return Err(error());
        }
        let metadata = fs::symlink_metadata(&frontend_path).map_err(|_| error())?;
        let supervisor = unsafe { libc::getpid() };
        let mut command = Command::new(&config.executable);
        command
            .arg("--config")
            .arg(&configuration)
            .env_clear()
            .env("PATH", "/usr/bin")
            .env("HOME", home)
            .env("XDG_RUNTIME_DIR", &compositor)
            .env("SWAYSOCK", compositor.join("ipc.sock"))
            .env("WLR_BACKENDS", "headless")
            .env("WLR_RENDERER", "pixman")
            .env("WLR_HEADLESS_OUTPUTS", "1")
            .env("WLR_LIBINPUT_NO_DEVICES", "1")
            .env("LD_LIBRARY_PATH", &config.libraries)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0);
        unsafe {
            command.pre_exec(move || {
                if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) != 0
                    || libc::getppid() != supervisor
                {
                    return Err(std::io::Error::other("browser_parent_gone"));
                }
                Ok(())
            });
        }
        let child = command.spawn().map_err(|_| error())?;
        let mut owned = Self {
            child,
            compositor_watch: None,
            frontend: Some(listener),
            frontend_path,
            compositor_path: PathBuf::new(),
            socket_identity: (metadata.dev(), metadata.ino()),
            cancel: Arc::new(AtomicBool::new(false)),
            join: None,
        };
        let raw = unsafe { libc::syscall(libc::SYS_pidfd_open, owned.child.id(), 0) as i32 };
        if raw < 0 {
            return Err(error());
        }
        owned.compositor_watch = Some(unsafe { OwnedFd::from_raw_fd(raw) });
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if owned.child.try_wait().map_err(|_| error())?.is_some() {
                return Err(error());
            }
            for entry in fs::read_dir(&compositor).map_err(|_| error())? {
                let entry = entry.map_err(|_| error())?;
                if entry
                    .file_name()
                    .as_encoded_bytes()
                    .starts_with(b"wayland-")
                    && entry.file_type().map_err(|_| error())?.is_socket()
                {
                    let _peer = transport::connect_owned(
                        &entry.path(),
                        owned.child.id() as _,
                        &[owned.compositor_watch.as_ref().ok_or_else(error)?],
                        &owned.cancel,
                    )?;
                    owned.compositor_path = entry.path();
                    return Ok(owned);
                }
            }
            if Instant::now() >= deadline {
                return Err(error());
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
    pub(crate) fn name(&self) -> &Path {
        &self.frontend_path
    }
    pub(crate) fn runtime(&self) -> &Path {
        self.frontend_path.parent().unwrap()
    }
    pub(crate) fn healthy(&self) -> bool {
        let Some(watch) = self.compositor_watch.as_ref() else {
            return false;
        };
        let mut poll = libc::pollfd {
            fd: watch.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        !self.cancel.load(Ordering::Acquire) && unsafe { libc::poll(&mut poll, 1, 0) } == 0
    }
    pub(crate) fn attach(&mut self, engine: &Engine) -> Result<()> {
        let listener = self.frontend.take().ok_or_else(error)?;
        let pid = engine.pid();
        let pidfd = duplicate(engine.pidfd().as_raw_fd())?;
        let compositor = self.compositor_path.clone();
        let compositor_pid = self.child.id() as i32;
        let compositor_watch = duplicate(
            self.compositor_watch
                .as_ref()
                .ok_or_else(error)?
                .as_raw_fd(),
        )?;
        let cancel = self.cancel.clone();
        self.join = Some(
            thread::Builder::new()
                .name("browser-wayland-owner".into())
                .spawn(move || {
                    let mut workers = Vec::new();
                    let mut accepted = 0;
                    while !cancel.load(Ordering::Acquire) {
                        let mut watch = [
                            libc::pollfd {
                                fd: listener.as_raw_fd(),
                                events: libc::POLLIN,
                                revents: 0,
                            },
                            libc::pollfd {
                                fd: pidfd.as_raw_fd(),
                                events: libc::POLLIN,
                                revents: 0,
                            },
                            libc::pollfd {
                                fd: compositor_watch.as_raw_fd(),
                                events: libc::POLLIN,
                                revents: 0,
                            },
                        ];
                        let result =
                            unsafe { libc::poll(watch.as_mut_ptr(), watch.len() as _, 50) };
                        if result < 0 {
                            if std::io::Error::last_os_error().kind()
                                == std::io::ErrorKind::Interrupted
                            {
                                continue;
                            }
                            break;
                        }
                        if watch[1].revents != 0
                            || watch[2].revents != 0
                            || watch[0].revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL)
                                != 0
                        {
                            break;
                        }
                        if watch[0].revents & libc::POLLIN == 0 {
                            continue;
                        }
                        let Ok((front, _)) = listener.accept() else {
                            continue;
                        };
                        // No request is read, nor any backend socket delegated, before this gate.
                        if transport::peer(&front, pid).is_err() {
                            continue;
                        }
                        if accepted >= 8 {
                            break;
                        }
                        let Ok(backend) = transport::connect_owned(
                            &compositor,
                            compositor_pid,
                            &[&pidfd, &compositor_watch],
                            &cancel,
                        ) else {
                            break;
                        };
                        let Ok(engine_watch) = duplicate(pidfd.as_raw_fd()) else {
                            break;
                        };
                        let Ok(server_watch) = duplicate(compositor_watch.as_raw_fd()) else {
                            break;
                        };
                        let stopped = cancel.clone();
                        let terminate = cancel.clone();
                        match thread::Builder::new()
                            .name("browser-wayland-stream".into())
                            .spawn(move || {
                                if transport::forward(
                                    front,
                                    backend,
                                    (pid, compositor_pid),
                                    [engine_watch, server_watch],
                                    stopped,
                                )
                                .is_err()
                                {
                                    terminate.store(true, Ordering::Release);
                                }
                            }) {
                            Ok(worker) => workers.push(worker),
                            Err(_) => break,
                        }
                        accepted += 1;
                    }
                    cancel.store(true, Ordering::Release);
                    for worker in workers {
                        let _ = worker.join();
                    }
                    // A failed authenticated stream retires only this exact kernel engine.
                    unsafe {
                        libc::syscall(
                            libc::SYS_pidfd_send_signal,
                            pidfd.as_raw_fd(),
                            libc::SIGTERM,
                            std::ptr::null::<libc::siginfo_t>(),
                            0,
                        );
                    }
                })
                .map_err(|_| error())?,
        );
        Ok(())
    }
}
impl Drop for Display {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
        self.frontend.take();
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        if fs::symlink_metadata(&self.frontend_path)
            .is_ok_and(|m| m.file_type().is_socket() && (m.dev(), m.ino()) == self.socket_identity)
        {
            let _ = fs::remove_file(&self.frontend_path);
        }
        // Private directories/configuration are retained; no recursive cleanup.
    }
}
