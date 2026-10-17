//! Native T3 Connect CLI adapter. No OAuth requests, token files or service ownership.
//! Events contain only allowlisted state, never raw CLI output. Linux/Unix only.
use serde_json::{json, Value};
use std::{
    fs::File,
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::process::CommandExt,
    },
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

type Result<T> = std::result::Result<T, String>;
const OUTPUT_LIMIT: usize = 256 * 1024;

/// Parent supplies its existing native CLI and native base directory, not a new store.
#[derive(Clone)]
pub struct NativeCli {
    pub executable: PathBuf,
    pub base_dir: PathBuf,
    pub timeout: Duration,
}
impl NativeCli {
    pub fn new(executable: PathBuf, base_dir: PathBuf) -> Self {
        Self {
            executable,
            base_dir,
            timeout: Duration::from_secs(15 * 60),
        }
    }
    pub fn arguments(&self, action: &str) -> Result<Vec<String>> {
        let verb = match action {
            "read" => "status",
            "login" | "register" => "login",
            "link" | "link-publish-only" => "link",
            "unlink" => "unlink",
            "logout" => "logout",
            _ => return Err("invalid_action".into()),
        };
        if !self.base_dir.is_absolute() {
            return Err("invalid_base_dir".into());
        }
        let base = self.base_dir.to_str().ok_or("invalid_base_dir")?;
        let mut args = vec![
            "connect".into(),
            verb.into(),
            "--base-dir".into(),
            base.into(),
        ];
        if verb == "status" {
            args.push("--json".into());
        }
        if verb == "login" || verb == "link" {
            args.push("--headless".into());
        }
        if action == "link-publish-only" {
            args.push("--publish-only".into());
        }
        Ok(args)
    }
}

struct Active {
    id: String,
    cancel: Arc<AtomicBool>,
    awaiting_install: Arc<AtomicBool>,
    input: mpsc::Sender<bool>,
}
/// Keep one manager for the worker lifetime. All entry points return immediately.
/// Parent must reject non-local environments before dispatch; this adapter is local-only.
#[derive(Clone, Default)]
pub struct CloudManager {
    active: Arc<Mutex<Option<Active>>>,
}
impl CloudManager {
    pub fn start<F>(
        &self,
        cli: NativeCli,
        request_id: &str,
        action: &str,
        confirmed: bool,
        emit: F,
    ) -> Result<()>
    where
        F: Fn(Value) + Send + 'static,
    {
        cli.arguments(action)?;
        if request_id.is_empty()
            || request_id.len() > 128
            || request_id.chars().any(char::is_control)
        {
            return Err("invalid_request_id".into());
        }
        if matches!(action, "unlink" | "logout") && !confirmed {
            return Err("confirmation_required".into());
        }
        let mut active = self.active.lock().map_err(|_| "local_error")?;
        if active.is_some() {
            return Err("connection_cloud_busy".into());
        }
        let cancel = Arc::new(AtomicBool::new(false));
        let awaiting_install = Arc::new(AtomicBool::new(false));
        let (input, receiver) = mpsc::channel();
        *active = Some(Active {
            id: request_id.into(),
            cancel: cancel.clone(),
            awaiting_install: awaiting_install.clone(),
            input,
        });
        let manager = self.clone();
        let id = request_id.to_owned();
        let action = action.to_owned();
        let spawn = thread::Builder::new().name("t3-connect".into()).spawn(move || {
            let event = |stage: &str, data: Value| {
                if stage == "install-confirmation" { awaiting_install.store(true, Ordering::SeqCst); }
                emit(json!({"type":"connection-cloud", "environment":"local", "requestId":id, "action":action, "busy":true, "stage":stage, "data":data}));
            };
            event("starting", json!({}));
            let outcome = run(&cli, &action, &cancel, &receiver, &event);
            // Verify saved state separately: never retry a partially successful mutation.
            let mut status = Value::Null;
            let mut status_error = String::new();
            let mut error = outcome.as_ref().err().cloned().unwrap_or_default();
            if !cancel.load(Ordering::SeqCst) {
                if action == "read" {
                    if let Ok(output) = &outcome { match parse_status(output) { Ok(v) => status = v, Err(e) => error = e } }
                } else {
                    event("refreshing", json!({}));
                    match run(&cli, "read", &cancel, &receiver, &|_, _| {}) {
                        Ok(output) => match parse_status(&output) { Ok(v) => status = v, Err(e) => status_error = e },
                        Err(e) => status_error = e,
                    }
                }
            }
            if cancel.load(Ordering::SeqCst) { error = "cancelled".into(); }
            let stage = if error.is_empty() { "finished" } else if error == "cancelled" { "cancelled" } else { "failed" };
            // Release before terminal event so an immediate refresh can start safely.
            awaiting_install.store(false, Ordering::SeqCst);
            if let Ok(mut active) = manager.active.lock() { *active = None; }
            emit(json!({"type":"connection-cloud", "environment":"local", "requestId":id, "action":action, "busy":false, "stage":stage, "error":error, "statusError":status_error, "status":status, "device":null}));
        });
        if spawn.is_err() {
            *active = None;
            return Err("local_error".into());
        }
        Ok(())
    }
    pub fn cancel(&self, request_id: &str) -> Result<()> {
        let active = self.active.lock().map_err(|_| "local_error")?;
        let active = active
            .as_ref()
            .filter(|a| a.id == request_id)
            .ok_or("unknown_request")?;
        active.cancel.store(true, Ordering::SeqCst);
        Ok(())
    }
    /// On worker EOF/shutdown, cancel the CLI and allow its terminal event before exit.
    pub fn cancel_active(&self) -> Result<()> {
        if let Some(active) = self.active.lock().map_err(|_| "local_error")?.as_ref() {
            active.cancel.store(true, Ordering::SeqCst);
        }
        Ok(())
    }
    /// Answer only the native relay-download confirmation. No general terminal input.
    pub fn confirm_install(&self, request_id: &str, approved: bool) -> Result<()> {
        let active = self.active.lock().map_err(|_| "local_error")?;
        let active = active
            .as_ref()
            .filter(|a| a.id == request_id)
            .ok_or("unknown_request")?;
        if !active.awaiting_install.swap(false, Ordering::SeqCst) {
            return Err("confirmation_not_requested".into());
        }
        active
            .input
            .send(approved)
            .map_err(|_| "unknown_request".into())
    }
}

/// Drop unknown fields (including any future credential fields) before crossing IPC.
pub fn parse_status(output: &str) -> Result<Value> {
    let clean = plain_terminal(output);
    let start = clean.find('{').ok_or("t3_connect_invalid_status")?;
    let end = clean.rfind('}').ok_or("t3_connect_invalid_status")?;
    let v: Value =
        serde_json::from_str(&clean[start..=end]).map_err(|_| "t3_connect_invalid_status")?;
    let mut result = json!({"savedSetupOnly":true});
    for field in [
        "desired",
        "authenticated",
        "linked",
        "publishAgentActivity",
    ] {
        result[field] = json!(v[field].as_bool().ok_or("t3_connect_invalid_status")?);
    }
    // Older shipped CLI versions predate revoked-state reporting.
    result["revoked"] = match v.get("revoked") {
        None => Value::Null,
        Some(value) => json!(value.as_bool().ok_or("t3_connect_invalid_status")?),
    };
    // No identity/token scraping. Native status exposes only the linked user ID.
    result["cloudUserId"] = v["cloudUserId"]
        .as_str()
        .filter(|s| s.len() <= 512 && !s.chars().any(char::is_control))
        .map_or(Value::Null, |s| json!(s));
    let relay = v["relayClient"]["status"]
        .as_str()
        .filter(|s| matches!(*s, "available" | "missing" | "unsupported"))
        .ok_or("t3_connect_invalid_status")?;
    result["relayClient"] = json!({"status":relay});
    Ok(result)
}

fn plain_terminal(value: &str) -> String {
    let mut chars = value.chars();
    let mut result = String::new();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            match chars.next() {
                Some('[') => {
                    for x in chars.by_ref() {
                        if ('@'..='~').contains(&x) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    while let Some(x) = chars.next() {
                        if x == '\u{7}' || (x == '\u{1b}' && chars.next() == Some('\\')) {
                            break;
                        }
                    }
                }
                _ => {}
            }
        } else if c == '\n' || c == '\t' || !c.is_control() {
            result.push(c);
        }
    }
    result
}

/// The CLI has no device-flow JSON flag. Recognize its exact public prompt only.
pub fn device_prompt(output: &str) -> Option<Value> {
    let clean = plain_terminal(output);
    let prompt = clean
        .rsplit("Open this URL on a device with a browser:")
        .next()?;
    if prompt == clean {
        return None;
    }
    let url = prompt.lines().find(|l| !l.trim().is_empty())?.trim();
    let parsed = reqwest::Url::parse(url).ok()?;
    if parsed.scheme() != "https"
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return None;
    }
    if url.len() > 4096 {
        return None;
    }
    // Authorization URLs may carry the public user_code, never bearer/device tokens.
    if parsed
        .query_pairs()
        .any(|(k, _)| !matches!(k.as_ref(), "user_code"))
        || parsed.fragment().is_some()
    {
        return None;
    }
    let code = prompt
        .split("Confirm this code when asked: ")
        .nth(1)?
        .lines()
        .next()?
        .trim();
    if code.is_empty()
        || code.len() > 64
        || !code.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return None;
    }
    let minutes = prompt
        .split("Waiting for approval (expires in ")
        .nth(1)?
        .split(" min)")
        .next()?
        .parse::<u64>()
        .ok()?;
    Some(json!({"verificationUrl":url, "userCode":code, "expiresInMinutes":minutes.min(1440)}))
}

fn pty_command(cli: &NativeCli, action: &str) -> Result<(std::process::Child, File)> {
    let (mut master, mut slave) = (-1, -1);
    let size = libc::winsize {
        ws_row: 40,
        ws_col: 160,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    if unsafe {
        libc::openpty(
            &mut master,
            &mut slave,
            std::ptr::null_mut(),
            std::ptr::null(),
            &size,
        )
    } != 0
    {
        return Err("t3_connect_spawn_failed".into());
    }
    let master = unsafe { File::from_raw_fd(master) };
    let slave = unsafe { File::from_raw_fd(slave) };
    unsafe {
        libc::fcntl(master.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC);
    }
    let flags = unsafe { libc::fcntl(master.as_raw_fd(), libc::F_GETFL) };
    if flags < 0
        || unsafe { libc::fcntl(master.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0
    {
        return Err("t3_connect_spawn_failed".into());
    }
    let mut command = Command::new(&cli.executable);
    command
        .args(cli.arguments(action)?)
        .env("TERM", "dumb")
        .env("NO_COLOR", "1")
        .stdin(Stdio::from(slave.try_clone().map_err(|_| "local_error")?))
        .stdout(Stdio::from(slave.try_clone().map_err(|_| "local_error")?))
        .stderr(Stdio::from(slave));
    let parent=unsafe{libc::getpid()};
    // Child becomes a new session/process group; no signal ever targets the native server.
    unsafe {
        command.pre_exec(move || {
            if libc::prctl(libc::PR_SET_PDEATHSIG,libc::SIGTERM)!=0||libc::getppid()!=parent {return Err(std::io::Error::last_os_error())}
            if libc::setsid() < 0 || libc::ioctl(0, libc::TIOCSCTTY, 0) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let child = command.spawn().map_err(|_| "t3_connect_spawn_failed")?;
    Ok((child, master))
}

fn run<F: Fn(&str, Value)>(
    cli: &NativeCli,
    action: &str,
    cancel: &AtomicBool,
    input: &mpsc::Receiver<bool>,
    event: &F,
) -> Result<String> {
    let (mut child, mut terminal) = pty_command(cli, action)?;
    let pid = child.id() as i32;
    let started = Instant::now();
    let timeout = if action == "read" {
        cli.timeout.min(Duration::from_secs(20))
    } else {
        cli.timeout
    };
    let mut output = Vec::new();
    let mut last_device = Value::Null;
    let mut install_prompt = false;
    let mut install_answered = false;
    let mut warned = false;
    let mut failure = None;
    loop {
        // Check even while a child continuously fills the PTY.
        if cancel.load(Ordering::SeqCst) {
            failure = Some("cancelled");
            break;
        }
        if started.elapsed() >= timeout {
            failure = Some("t3_connect_timeout");
            break;
        }
        let mut bytes = [0; 4096];
        match terminal.read(&mut bytes) {
            Ok(n) if n > 0 => {
                if output.len() + n > OUTPUT_LIMIT {
                    failure = Some("t3_connect_output_limit");
                    break;
                }
                output.extend_from_slice(&bytes[..n]);
                let clean = plain_terminal(&String::from_utf8_lossy(&output));
                if let Some(device) = device_prompt(&clean) {
                    if device != last_device {
                        event("authorization", device.clone());
                        last_device = device;
                    }
                }
                if !install_prompt && clean.contains("Download and install version ") {
                    install_prompt = true;
                    event("install-confirmation", json!({}));
                }
                if !warned
                    && (clean.contains("Could not revoke the relay-side")
                        || clean.contains("running server could not stop its tunnel"))
                {
                    warned = true;
                    event(
                        "warning",
                        json!({"code":"t3_connect_disconnect_incomplete"}),
                    );
                }
                for stage in [
                    "Checking existing installation",
                    "Waiting for installation lock",
                    "Downloading",
                    "Verifying download",
                    "Installing",
                    "Validating executable",
                    "Activating installation",
                ] {
                    if clean.ends_with(&format!("Relay client: {stage}...\n")) {
                        event("installing", json!({"step":stage}));
                    }
                }
                continue;
            }
            Err(e)
                if e.kind() != std::io::ErrorKind::WouldBlock
                    && e.raw_os_error() != Some(libc::EIO) =>
            {
                failure = Some("t3_connect_read_failed");
                break;
            }
            _ => {}
        }
        if cancel.load(Ordering::SeqCst) {
            failure = Some("cancelled");
            break;
        }
        if started.elapsed() >= timeout {
            failure = Some("t3_connect_timeout");
            break;
        }
        if install_prompt && !install_answered {
            if let Ok(approved) = input.try_recv() {
                if terminal
                    .write_all(if approved { b"y\r" } else { b"n\r" })
                    .is_err()
                {
                    failure = Some("t3_connect_write_failed");
                    break;
                }
                install_answered = true;
                if !approved {
                    failure = Some("cancelled");
                    break;
                }
                event("installing", json!({"step":"Installing"}));
            }
        }
        // WNOWAIT pins the leader PID until the group is cleaned, avoiding PID reuse.
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        let result = unsafe {
            libc::waitid(
                libc::P_PID,
                pid as u32,
                &mut info,
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        };
        if result < 0 {
            failure = Some("t3_connect_wait_failed");
            break;
        }
        if unsafe { info.si_pid() } == pid {
            break;
        }
        thread::sleep(Duration::from_millis(25));
    }
    if failure.is_some() {
        unsafe {
            libc::kill(-pid, libc::SIGINT);
        }
        thread::sleep(Duration::from_millis(250));
    }
    unsafe {
        libc::kill(-pid, libc::SIGKILL);
    }
    let status = child.wait().map_err(|_| "t3_connect_wait_failed")?;
    if let Some(error) = failure {
        return Err(error.into());
    }
    if !status.success() {
        let clean = plain_terminal(&String::from_utf8_lossy(&output));
        if clean.contains("T3 Connect authorization was denied in the browser.") {
            return Err("t3_connect_authorization_denied".into());
        }
        if clean.contains("Timed out waiting for T3 Connect authorization.") {
            return Err("t3_connect_authorization_expired".into());
        }
        return Err("t3_connect_cli_failed".into());
    }
    Ok(String::from_utf8_lossy(&output).into_owned())
}
