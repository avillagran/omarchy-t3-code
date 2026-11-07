//! Qt guest ownership below the authorized native preview bridge. Callers must
//! supply authoritative native snapshots. Automation uses only its separately
//! admitted private descriptor; it never shares public UI/status messages.
mod private;
use crate::{json, text, Result};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    ffi::OsString,
    fs,
    io::{BufRead, BufReader, Write},
    os::{
        fd::AsRawFd,
        unix::{fs::PermissionsExt, net::UnixStream, process::CommandExt},
    },
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};
type Callback = Arc<dyn Fn(Value) + Send + Sync>;
#[derive(Clone)]
pub struct Config {
    executable: PathBuf,
    environment: HashMap<OsString, OsString>,
    cache: PathBuf,
    basic_store: bool,
    native_display: Option<PathBuf>,
    mapped_native_host: bool,
    clipboard: Option<crate::gui_clipboard::Shared>,
}
impl Config {
    pub fn import_assets(&self) -> [PathBuf; 2] {
        let root = self
            .executable
            .parent()
            .unwrap_or(Path::new("/nonexistent"));
        [
            root.join("libt3-browser.so"),
            root.join("../ui/browser/T3BrowserSurface.qml"),
        ]
    }
    pub fn storage_root(&self) -> PathBuf {
        PathBuf::from(
            self.environment
                .get(std::ffi::OsStr::new("T3CODE_HOME"))
                .expect("explicit browser home"),
        )
        .join("userdata/ot3-browser-profiles")
    }
    pub fn client_settings_path(&self) -> PathBuf {
        PathBuf::from(
            self.environment
                .get(std::ffi::OsStr::new("T3CODE_HOME"))
                .expect("explicit browser home"),
        )
        .join("userdata/client-settings.json")
    }
    pub fn native() -> Result<Self> {
        let mut environment = HashMap::new();
        for key in [
            "HOME",
            "LANG",
            "LC_ALL",
            "TZ",
            "XDG_CONFIG_HOME",
            "XDG_CACHE_HOME",
            "XDG_DATA_HOME",
            "XDG_STATE_HOME",
            "XDG_RUNTIME_DIR",
            "DBUS_SESSION_BUS_ADDRESS",
        ] {
            if let Some(value) = std::env::var_os(key) {
                environment.insert(key.into(), value);
            }
        }
        environment.insert("T3CODE_HOME".into(), crate::t3_home().into_os_string());
        Ok(Self {
            executable: std::env::current_exe().map_err(|_| "browser_runtime_unavailable")?,
            environment,
            cache: crate::xdg("XDG_CACHE_HOME", ".cache"),
            basic_store: false,
            native_display: None,
            mapped_native_host: false,
            clipboard: None,
        })
    }
    /// Keep the actual GUI registration with this factory. Ordinary Qt guests
    /// do not register an X cohort or request clipboard contents.
    pub fn with_gui_clipboard(mut self, platform: Option<crate::gui_clipboard::Shared>) -> Self {
        self.clipboard = platform;
        self
    }
    /// Explicit internal opt-in with a private authenticated display owned by
    /// each cohort. This does not advertise or select a browser capability.
    pub fn with_owned_native_electron(
        self,
        runtime: &Path,
        assets: &Path,
        x_server: &Path,
    ) -> Result<Self> {
        if !x_server.is_absolute() {
            return Err("invalid_browser_fixture".into());
        }
        let mut owned = self.with_native_electron(runtime, assets, ":owned")?;
        owned.native_display = Some(x_server.into());
        Ok(owned)
    }
    /// Map the genuine native host only on this factory's owned display.
    /// No existing desktop display or service is adopted by this option.
    pub fn with_mapped_native_host(mut self) -> Result<Self> {
        if self.native_display.is_none() {
            return Err("browser_owned_display_required".into());
        }
        self.mapped_native_host = true;
        Ok(self)
    }
    /// Explicit internal renderer opt-in on an already isolated display.
    pub fn with_native_electron(
        mut self,
        runtime: &Path,
        assets: &Path,
        display: &str,
    ) -> Result<Self> {
        if !runtime.is_absolute() || !assets.is_absolute() || display.is_empty() {
            return Err("invalid_browser_fixture".into());
        }
        self.environment
            .insert("O3_BROWSER_ENGINE".into(), "native-electron".into());
        self.environment
            .insert("O3_NATIVE_ELECTRON_RUNTIME".into(), runtime.into());
        self.environment
            .insert("O3_NATIVE_ELECTRON_ASSETS".into(), assets.into());
        self.environment.insert("DISPLAY".into(), display.into());
        if self.basic_store {
            if let Some(home) = self.environment.get(std::ffi::OsStr::new("HOME")) {
                self.environment.insert(
                    "O3_NATIVE_ELECTRON_DIAGNOSTICS".into(),
                    Path::new(home).join("native-electron.log").into_os_string(),
                );
            }
        }

        Ok(self)
    }
    /// Explicit fixture configuration, with no real user bus, keyring or browser.
    pub fn isolated(executable: PathBuf, root: &Path) -> Result<Self> {
        if !root.is_absolute() {
            return Err("invalid_browser_fixture".into());
        }
        let mut environment = HashMap::new();
        for (key, folder) in [
            ("HOME", "home"),
            ("T3CODE_HOME", "native"),
            ("XDG_CONFIG_HOME", "config"),
            ("XDG_CACHE_HOME", "cache"),
            ("XDG_DATA_HOME", "data"),
            ("XDG_STATE_HOME", "state"),
            ("XDG_RUNTIME_DIR", "runtime"),
        ] {
            let path = root.join(folder);
            fs::create_dir_all(&path).map_err(|_| "browser_storage_unavailable")?;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
                .map_err(|_| "browser_storage_unavailable")?;
            environment.insert(key.into(), path.into_os_string());
        }
        environment.insert(
            "DBUS_SESSION_BUS_ADDRESS".into(),
            format!("unix:path={}", root.join("no-session-bus").display()).into(),
        );
        environment.insert("LANG".into(), "C.UTF-8".into());
        // Preserve the private harness's audited software-rendering choice
        // across the renderer's intentionally cleared environment.
        for key in ["LIBGL_ALWAYS_SOFTWARE", "__EGL_VENDOR_LIBRARY_FILENAMES"] {
            if let Some(value) = std::env::var_os(key) {
                environment.insert(key.into(), value);
            }
        }
        Ok(Self {
            executable,
            environment,
            cache: root.join("cache/omarchy-t3-code"),
            basic_store: true,
            native_display: None,
            mapped_native_host: false,
            clipboard: None,
        })
    }
}
#[derive(Clone)]
struct View {
    id: String,
    environment: String,
    origin: String,
    process: String,
    thread: String,
    tab: String,
}
struct CookieCheck {
    host: String,
    name: String,
    path: String,
    secure: bool,
    http_only: bool,
    same_site: i32,
    expires: Option<i64>,
}
impl CookieCheck {
    fn expected(cookie: &crate::browser_cookies::ImportedCookie) -> Result<Self> {
        let url = crate::Url::parse(&cookie.url).map_err(|_| "browser_cookie_import_unknown")?;
        let host = cookie
            .domain
            .as_deref()
            .unwrap_or(url.host_str().ok_or("browser_cookie_import_unknown")?)
            .to_owned();
        let host = if cookie.domain.is_none() {
            host.trim_start_matches('[')
                .trim_end_matches(']')
                .to_owned()
        } else {
            host
        };
        Ok(Self {
            host,
            name: cookie.name.clone(),
            path: cookie.path.clone(),
            secure: cookie.secure,
            http_only: cookie.http_only,
            same_site: match cookie.same_site {
                "strict" => 2,
                "lax" => 1,
                "no_restriction" => 0,
                _ => -1,
            },
            expires: cookie.expiration_date.map(|v| v.floor() as i64),
        })
    }
    fn key(&self) -> (String, String, String) {
        (self.host.clone(), self.name.clone(), self.path.clone())
    }
}
fn verify_import(
    path: &Path,
    checks: &HashMap<(String, String, String), CookieCheck>,
) -> Result<()> {
    let verify = (|| -> Result<()> {
        let db = crate::browser_cookie_database::Database::snapshot(path)?;
        let rows=db.rows("select host_key,name,path,is_secure,is_httponly,samesite,has_expires,expires_utc/1000000 from cookies where top_frame_site_key = ''")?;
        let mut found = HashSet::new();
        for r in rows {
            let key = (
                r[0].text()?.to_owned(),
                r[1].text()?.to_owned(),
                r[2].text()?.to_owned(),
            );
            let Some(expected) = checks.get(&key) else {
                continue;
            };
            if r[3].number()? != f64::from(u8::from(expected.secure))
                || r[4].number()? != f64::from(u8::from(expected.http_only))
                || r[5].number()? != f64::from(expected.same_site)
            {
                return Err("browser_cookie_import_unknown".into());
            }
            match expected.expires {
                Some(expires)
                    if r[6].number()? == 1.
                        && r[7].number()? - 11_644_473_600. == expires as f64 => {}
                None if r[6].number()? == 0. => {}
                _ => return Err("browser_cookie_import_unknown".into()),
            };
            found.insert(key);
        }
        if found.len() != checks.len() {
            return Err("browser_cookie_import_unknown".into());
        }
        Ok(())
    })();
    verify.map_err(|_| "browser_cookie_import_unknown".into())
}
/// Owned metadata-only persistence check; may run outside the coordinator so
/// another profile's browsing is not blocked by Chromium's write schedule.
pub struct ImportVerification {
    path: PathBuf,
    checks: HashMap<(String, String, String), CookieCheck>,
    alive: Arc<AtomicBool>,
}
impl ImportVerification {
    pub fn verify(self) -> Result<()> {
        if self.checks.is_empty() {
            return Ok(());
        }
        let until = Instant::now() + Duration::from_secs(40);
        loop {
            if !self.alive.load(Ordering::SeqCst) {
                return Err("browser_cookie_import_unknown".into());
            }
            if verify_import(&self.path, &self.checks).is_ok() {
                return Ok(());
            }
            if Instant::now() >= until {
                return Err("browser_cookie_import_unknown".into());
            }
            thread::sleep(Duration::from_millis(100));
        }
    }
}
struct Session {
    _display: Option<crate::browser_native_display::Display>,
    _engine: Option<crate::browser_engine_identity::Engine>,
    _clipboard: Option<crate::gui_clipboard::CohortLease>,
    child: Child,
    input: ChildStdin,
    id: String,
    origin: String,
    tabs: HashSet<(String, String)>,
    alive: Arc<AtomicBool>,
    frames: Arc<Mutex<Option<PathBuf>>>,
    reader: Option<thread::JoinHandle<()>>,
    stopped: Option<Result<()>>,
    cookie_replies: std::sync::mpsc::Receiver<Value>,
    cookie_checks: HashMap<(String, String, String), CookieCheck>,
    cookie_database: PathBuf,
    private: Option<private::Guest>,
}
impl Session {
    fn send(&mut self, value: Value) -> Result<()> {
        writeln!(self.input, "{value}").map_err(|_| "browser_runtime_closed".into())
    }
    fn stop(&mut self) -> Result<()> {
        if let Some(result) = &self.stopped {
            return result.clone();
        }
        if let Some(private) = &self.private {
            private.close();
        }
        self.alive.store(false, Ordering::SeqCst);
        let _ = writeln!(self.input, "{{\"type\":\"close\"}}");
        let until = Instant::now() + Duration::from_secs(2);
        while Instant::now() < until {
            if self.child.try_wait().ok().flatten().is_some() {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        if self.child.try_wait().ok().flatten().is_none() {
            unsafe {
                libc::kill(-(self.child.id() as i32), libc::SIGKILL);
            }
            let _ = self.child.kill();
        }
        let success = self.child.wait().is_ok_and(|s| s.success());
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
        self.private.take();
        if let Some(path) = self.frames.lock().unwrap().take() {
            let _ = crate::frame_files::reclaim(&path);
        }
        let result = if success {
            Ok(())
        } else {
            Err("browser_profile_flush_unknown".into())
        };
        self.stopped = Some(result.clone());
        result
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
struct PrivateBinding {
    origin: String,
    receive: private::Native,
    turns: Arc<Mutex<HashSet<String>>>,
}
pub struct Pool {
    config: Config,
    sessions: HashMap<(String, String), Session>,
    visited: HashSet<(String, String)>,
    active: Arc<Mutex<Option<View>>>,
    receive: Callback,
    private_bindings: HashMap<String, PrivateBinding>,
    private_registry: Arc<Mutex<crate::browser_automation::Registry>>,
}
impl Pool {
    pub fn new(config: Config, receive: impl Fn(Value) + Send + Sync + 'static) -> Self {
        Self {
            config,
            sessions: HashMap::new(),
            visited: HashSet::new(),
            active: Arc::new(Mutex::new(None)),
            receive: Arc::new(receive),
            private_bindings: HashMap::new(),
            private_registry: Arc::new(Mutex::new(Default::default())),
        }
    }
    /// Trusted owned-backend coordinator only. This callback writes native
    /// browser events to its dedicated backend descriptor, never the UI output.
    pub fn bind_private_source(
        &mut self,
        environment: &str,
        origin_key: &str,
        receive: impl Fn(Value) + Send + Sync + 'static,
    ) -> Result<()> {
        if environment.is_empty() || origin_key.is_empty() || origin_key.len() > 128 {
            return Err("preview_scope_changed".into());
        }
        if self
            .sessions
            .keys()
            .any(|(current, _)| current == environment)
        {
            return Err("browser_guest_mode_busy".into());
        }
        self.private_bindings.insert(
            environment.into(),
            PrivateBinding {
                origin: origin_key.into(),
                receive: Arc::new(receive),
                turns: Arc::new(Mutex::new(HashSet::new())),
            },
        );
        Ok(())
    }
    pub fn has_private_source(&self, environment: &str, origin_key: &str) -> bool {
        self.private_bindings
            .get(environment)
            .is_some_and(|binding| binding.origin == origin_key)
    }
    pub fn unbind_private_source(&mut self, environment: &str) {
        self.close_environment(environment);
        self.private_bindings.remove(environment);
    }
    /// Caller must first perform fresh native RPC permission/snapshot admission.
    /// A backend opened event can create this view before its open RPC completes.
    pub fn ensure_private(
        &mut self,
        environment: &str,
        origin_key: &str,
        snapshot: &Value,
        size: (u64, u64),
        zoom: &Value,
    ) -> Result<()> {
        if !self.has_private_source(environment, origin_key) {
            return Err("preview_scope_changed".into());
        }
        let snapshot = crate::preview_transport::snapshot(snapshot)?;
        let profile = snapshot["profileId"].as_str().unwrap_or("default");
        crate::browser_runtime::partition(environment, profile)?;
        let viewport = crate::browser_preferences::validate(
            "browserDefaultViewport",
            &json!({"_tag":"freeform","width":size.0,"height":size.1}),
        )?;
        let zoom = crate::browser_preferences::validate("browserDefaultZoomFactor", zoom)?;
        let key = (environment.to_owned(), profile.to_owned());
        if self.sessions.get_mut(&key).is_some_and(|session| {
            session.origin != origin_key
                || session.child.try_wait().ok().flatten().is_some()
                || !session
                    .private
                    .as_ref()
                    .is_some_and(private::Guest::is_open)
        }) {
            self.sessions.remove(&key);
        }
        if !self.sessions.contains_key(&key) {
            if self.sessions.len() >= 16 {
                return Err("browser_guest_limit".into());
            }
            let session = self.launch(
                environment,
                profile,
                origin_key,
                (
                    viewport["width"].as_u64().unwrap(),
                    viewport["height"].as_u64().unwrap(),
                ),
                &zoom,
                false,
            )?;
            self.sessions.insert(key.clone(), session);
        }
        let session = self.sessions.get_mut(&key).unwrap();
        let thread = text(&snapshot, "threadId");
        let tab = text(&snapshot, "tabId");
        let instance = session
            .private
            .as_ref()
            .ok_or("browser_private_unavailable")?
            .approve(thread, tab)?;
        if session.tabs.insert((thread.into(), tab.into())) {
            let url = if snapshot["navStatus"]["_tag"] == "Idle" {
                Value::Null
            } else {
                snapshot["navStatus"]["url"].clone()
            };
            session.send(json!({"type":"open","threadId":thread,"tabId":tab,"url":url,"background":true,"automation":true,"automationInstance":instance,"serverControlled":snapshot["runtime"]=="server"}))?;
        } else {
            session.send(json!({"type":"automation","threadId":thread,"tabId":tab,"automationInstance":instance}))?;
        }
        Ok(())
    }
    pub fn private_command(
        &mut self,
        environment: &str,
        origin_key: &str,
        command: &Value,
    ) -> Result<()> {
        if !self.has_private_source(environment, origin_key) {
            return Err("preview_scope_changed".into());
        }
        let command = crate::desktop_browser_ipc::decode_command(command)
            .ok_or("browser_private_command_invalid")?;
        let thread = text(&command, "threadId");
        let tab = text(&command, "tabId");
        if command["type"] == "turn" {
            let binding = self
                .private_bindings
                .get(environment)
                .ok_or("preview_scope_changed")?;
            let mut turns = binding.turns.lock().map_err(|_| "local_error")?;
            if command["active"] == true {
                turns.insert(thread.into());
            } else {
                turns.remove(thread);
            }
        }
        for ((current, _), session) in &self.sessions {
            if current != environment || session.origin != origin_key {
                continue;
            }
            let Some(private) = &session.private else {
                continue;
            };
            for (current_thread, current_tab) in &session.tabs {
                if current_thread == thread && (command["type"] == "turn" || current_tab == tab) {
                    private.command(current_thread, current_tab, command.clone())?;
                }
            }
        }
        // Native DesktopBrowserHost ignores commands for an absent tab.
        Ok(())
    }
    pub fn private_lease(
        &self,
        environment: &str,
        profile: &str,
        thread: &str,
        tab: &str,
    ) -> Option<crate::browser_automation::Lease> {
        let session = self.sessions.get(&(environment.into(), profile.into()))?;
        self.private_registry.lock().ok()?.current(
            &crate::browser_automation::Source {
                environment: environment.into(),
                profile: profile.into(),
                guest: session.id.clone(),
            },
            thread,
            tab,
        )
    }
    fn launch(
        &mut self,
        environment: &str,
        profile: &str,
        origin: &str,
        size: (u64, u64),
        zoom: &Value,
        initialize_cookies: bool,
    ) -> Result<Session> {
        let (width, height) = size;
        let root = self
            .config
            .executable
            .parent()
            .ok_or("browser_runtime_unavailable")?;
        let native_electron = self
            .config
            .environment
            .get(std::ffi::OsStr::new("O3_BROWSER_ENGINE"))
            .is_some_and(|engine| engine == "native-electron");
        if !native_electron && !root.join("libt3-browser.so").is_file() {
            return Err("browser_runtime_unavailable".into());
        }
        let native_display = self
            .config
            .native_display
            .as_ref()
            .map(|executable| crate::browser_native_display::Display::launch(executable))
            .transpose()?;
        let identity = native_display
            .as_ref()
            .map(|_| crate::browser_engine_identity::Pending::new())
            .transpose()?;
        let identity_fd = identity
            .as_ref()
            .map(|pending| pending.source().as_raw_fd());
        let id = uuid::Uuid::new_v4().to_string();
        let mut private_source = None;
        let mut private_fd = None;
        let private = if let Some(binding) = self.private_bindings.get(environment) {
            if binding.origin != origin {
                return Err("preview_scope_changed".into());
            }
            let (parent, child) = UnixStream::pair().map_err(|_| "browser_private_unavailable")?;
            let source = crate::desktop_browser_ipc::duplicate_child_source(child.as_raw_fd())
                .map_err(|_| "browser_private_unavailable")?;
            private_fd = Some(source.as_raw_fd());
            private_source = Some(source);
            Some(private::Guest::start(
                crate::browser_automation::Source {
                    environment: environment.into(),
                    profile: profile.into(),
                    guest: id.clone(),
                },
                parent,
                self.private_registry.clone(),
                binding.receive.clone(),
                binding.turns.clone(),
            )?)
        } else {
            None
        };
        let mut command = Command::new(&self.config.executable);
        command
            .arg("browser-render")
            .env_clear()
            .envs(&self.config.environment)
            .env("PATH", "/usr/bin:/bin")
            .env(
                "QTWEBENGINE_CHROMIUM_FLAGS",
                if self.config.basic_store {
                    "--disable-gpu --password-store=basic"
                } else {
                    "--disable-gpu"
                },
            )
            .process_group(0)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        if let Some(display) = &native_display {
            command
                .env("DISPLAY", display.name())
                .env("XAUTHORITY", display.authorization_path());
        }
        // The GUI worker may crash or be replaced. Do not orphan its Qt guest.
        let parent = unsafe { libc::getpid() };
        unsafe {
            command.pre_exec(move || {
                if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) != 0
                    || libc::getppid() != parent
                {
                    return Err(std::io::Error::other("browser_parent_gone"));
                }
                if let Some(source) = &private_source {
                    crate::desktop_browser_ipc::admit_child_source(source.as_raw_fd())?;
                }
                if let Some(identity) = identity_fd {
                    crate::desktop_browser_ipc::admit_child_source(identity)?;
                }
                Ok(())
            });
        }
        let mut child = command.spawn().map_err(|_| "browser_runtime_unavailable")?;
        let input = child.stdin.take().ok_or("browser_runtime_unavailable")?;
        let stdout = child.stdout.take().ok_or("browser_runtime_unavailable")?;
        let alive = Arc::new(AtomicBool::new(true));
        let frames = Arc::new(Mutex::new(None));
        let (cookie_tx, cookie_replies) = std::sync::mpsc::sync_channel(16);
        let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(1);
        let restore = self
            .visited
            .contains(&(environment.to_owned(), profile.to_owned()));
        let mut session = Session {
            _display: native_display,
            _engine: None,
            _clipboard: None,
            child,
            input,
            id: id.clone(),
            origin: origin.into(),
            tabs: HashSet::new(),
            alive: alive.clone(),
            frames: frames.clone(),
            reader: None,
            stopped: None,
            cookie_replies,
            cookie_checks: HashMap::new(),
            private,
            cookie_database: self
                .config
                .storage_root()
                .join(
                    text(
                        &crate::browser_runtime::partition(environment, profile)?,
                        "partition",
                    )
                    .trim_start_matches("persist:"),
                )
                .join("storage/Cookies"),
        };
        let mut payload = json!({"environmentId":environment,"profileId":profile,"width":width,"height":height,"zoomFactor":zoom,"initializeCookieStore":initialize_cookies||!restore,"restoreSessionCookies":restore});
        if self.config.mapped_native_host && session._display.is_some() {
            payload["ownedNativeHost"] = json!(true);
        }
        if let Some(fd) = private_fd {
            payload["privateBrowserFd"] = json!(fd);
        }
        if let Some(fd) = identity_fd {
            payload["privateClipboardIdentityFd"] = json!(fd);
        }
        let payload = payload.to_string().into_bytes();
        session
            .input
            .write_all(&(payload.len() as u64).to_be_bytes())
            .and_then(|_| session.input.write_all(&payload))
            .map_err(|_| "browser_runtime_closed")?;
        session._engine = identity
            .map(|pending| pending.admit(session.child.id()))
            .transpose()?;
        if let (Some(gui), Some(display), Some(engine)) =
            (&self.config.clipboard, &session._display, &session._engine)
        {
            session._clipboard = Some(crate::gui_clipboard::register(
                gui.clone(),
                &session.id,
                display,
                engine,
            )?);
        }

        let active = self.active.clone();
        let receive = self.receive.clone();
        let cache = self.config.cache.join("browser-sessions");
        let environment = environment.to_owned();
        let visited_key = (environment.clone(), profile.to_owned());
        let origin = origin.to_owned();
        let reader_profile = profile.to_owned();
        session.reader = Some(thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if !alive.load(Ordering::SeqCst) {
                    break;
                }
                if line.len() > 2 * 1024 * 1024 {
                    break;
                }
                let Ok(value) = serde_json::from_str::<Value>(&line) else {
                    continue;
                };
                match text(&value, "type") {
                    "native-dialog-request"
                        if value["options"]["message"]
                            .as_str()
                            .is_some_and(|text| text.len() <= 16384)
                            && value["options"]["buttons"]
                                .as_array()
                                .is_some_and(|buttons| {
                                    buttons.len() == 2
                                        && buttons.iter().all(|button| {
                                            button.as_str().is_some_and(|text| text.len() <= 1024)
                                        })
                                })
                            && value["options"]["defaultId"] == 1
                            && value["options"]["cancelId"] == 1 =>
                    {
                        let request = text(&value, "requestId");
                        let origin_value = text(&value, "origin");
                        if uuid::Uuid::parse_str(request).is_ok() && origin_value.len() <= 8192 {
                            receive(
                                json!({"kind":"permission-request","environment":environment,"originKey":origin,"processId":id,"profileId":reader_profile,"nativeRequestId":request,"thread":value["threadId"],"tabId":value["tabId"],"origin":origin_value,"nativeOptions":value["options"]}),
                            );
                        }
                    }
                    "permission-request" if value["permission"] == "clipboard-read" => {
                        let request = text(&value, "requestId");
                        let origin_value = text(&value, "origin");
                        if uuid::Uuid::parse_str(request).is_ok() && !origin_value.is_empty() {
                            receive(
                                json!({"kind":"permission-request","environment":environment,"originKey":origin,"processId":id,"profileId":reader_profile,"nativeRequestId":request,"thread":value["threadId"],"tabId":value["tabId"],"origin":origin_value}),
                            );
                        }
                    }
                    "permission-cancelled" => {
                        receive(
                            json!({"kind":"permission-cancelled","processId":id,"nativeRequestId":value["requestId"]}),
                        );
                    }
                    "cookie-imported" => {
                        // Dedicated response channel: cookie results never enter the UI/event callback.
                        let _ = cookie_tx.try_send(
                            json!({"requestId":value["requestId"],"state":value["state"]}),
                        );
                    }
                    "ready" => {
                        let path = PathBuf::from(text(&value, "frameDirectory"));
                        if path.parent() == Some(cache.as_path())
                            && path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .is_some_and(|n| uuid::Uuid::parse_str(n).is_ok())
                            && fs::symlink_metadata(&path)
                                .is_ok_and(|m| m.is_dir() && !m.file_type().is_symlink())
                        {
                            *frames.lock().unwrap() = Some(path);
                            let _ = ready_tx.try_send(());
                        }
                    }
                    "input-rejected" => {
                        if text(&value, "error") != "browser_native_input_focus_required" {
                            continue;
                        }
                        let view = active.lock().unwrap().clone();
                        if let Some(view) = view.filter(|v| {
                            v.process == id
                                && v.id == text(&value, "surfaceId")
                                && serde_json::to_string(&json!([v.thread, v.tab]))
                                    .is_ok_and(|key| key == text(&value, "tabKey"))
                        }) {
                            receive(
                                json!({"kind":"input-rejected","environment":view.environment,"originKey":view.origin,"thread":view.thread,"tabId":view.tab,"requestId":view.id,"error":"browser_native_input_focus_required","inputType":value["inputType"]}),
                            );
                        }
                    }
                    "frame" => {
                        let view = active.lock().unwrap().clone();
                        let Some(view) = view.filter(|v| {
                            v.process == id
                                && v.id == text(&value, "surfaceId")
                                && serde_json::to_string(&json!([v.thread, v.tab]))
                                    .is_ok_and(|key| key == text(&value, "tabKey"))
                        }) else {
                            continue;
                        };
                        let name = text(&value, "file");
                        if !matches!(name, "frame-0.png" | "frame-1.png") {
                            continue;
                        }
                        let Some(directory) = frames.lock().unwrap().clone() else {
                            continue;
                        };
                        let path = directory.join(name);
                        if !fs::symlink_metadata(&path).is_ok_and(|m| {
                            m.is_file()
                                && !m.file_type().is_symlink()
                                && m.len() <= 32 * 1024 * 1024
                        }) {
                            continue;
                        }
                        let Ok(url) = crate::Url::from_file_path(path) else {
                            continue;
                        };
                        receive(
                            json!({"kind":"frame","environment":view.environment,"originKey":view.origin,"thread":view.thread,"tabId":view.tab,"requestId":view.id,"url":url.to_string(),"revision":value["revision"],"width":value["width"],"height":value["height"],"nativeInput":native_electron && value["nativeInput"] == true}),
                        );
                    }
                    "status" => {
                        if let Ok(snapshot) = crate::preview_transport::snapshot(
                            &json!({"threadId":value["threadId"],"tabId":value["tabId"],"navStatus":value["navStatus"],"canGoBack":value["canGoBack"],"canGoForward":value["canGoForward"],"updatedAt":"","profileId":value["profileId"],"runtime":"desktop"}),
                        ) {
                            receive(
                                json!({"kind":"status","environment":environment,"originKey":origin,"processId":id,"snapshot":snapshot}),
                            );
                        }
                    }
                    _ => {} // No raw JS evaluation, cookies or unhandled native prompts escape.
                }
            }
            receive(json!({"kind":"permission-source-closed","processId":id}));
            if alive.load(Ordering::SeqCst) {
                let view = active.lock().unwrap().clone();
                if let Some(view) = view.filter(|v| v.process == id) {
                    receive(
                        json!({"kind":"closed","environment":view.environment,"thread":view.thread,"tabId":view.tab,"requestId":view.id,"error":"browser_runtime_closed"}),
                    );
                }
            }
        }));
        ready_rx
            .recv_timeout(Duration::from_secs(6))
            .map_err(|_| "browser_runtime_unavailable")?;
        self.visited.insert(visited_key);
        Ok(session)
    }
    /// Snapshot and origin must come from an authorized native read, not UI input.
    pub fn show(
        &mut self,
        environment: &str,
        origin: &str,
        snapshot: &Value,
        view_id: &str,
        size: (u64, u64),
        zoom: &Value,
    ) -> Result<()> {
        let (width, height) = size;
        let snapshot = crate::preview_transport::snapshot(snapshot)?;
        if snapshot["runtime"] == "server" && !self.has_private_source(environment, origin) {
            return Err("server_browser_surface_required".into());
        }
        if view_id.is_empty() || view_id.len() > 128 || origin.is_empty() || origin.len() > 128 {
            return Err("invalid_browser_view".into());
        }
        let profile = snapshot
            .get("profileId")
            .and_then(Value::as_str)
            .unwrap_or("default");
        crate::browser_runtime::partition(environment, profile)?;
        let size = crate::browser_preferences::validate(
            "browserDefaultViewport",
            &json!({"_tag":"freeform","width":width,"height":height}),
        )?;
        let zoom = crate::browser_preferences::validate("browserDefaultZoomFactor", zoom)?;
        let key = (environment.to_owned(), profile.to_owned());
        self.hide();
        if self.sessions.get(&key).is_some_and(|s| s.origin != origin) {
            self.sessions.remove(&key);
        }
        // A dead child may leave storage locks until wait/drop reaps its group.
        if self
            .sessions
            .get_mut(&key)
            .is_some_and(|s| s.child.try_wait().ok().flatten().is_some())
        {
            self.sessions.remove(&key);
        }
        if !self.sessions.contains_key(&key) {
            if self.sessions.len() >= 16 {
                return Err("browser_guest_limit".into());
            }
            let session =
                self.launch(environment, profile, origin, (width, height), &zoom, false)?;
            self.sessions.insert(key.clone(), session);
        }
        let session = self.sessions.get_mut(&key).unwrap();
        let thread = text(&snapshot, "threadId");
        let tab = text(&snapshot, "tabId");
        *self.active.lock().unwrap() = Some(View {
            id: view_id.into(),
            environment: environment.into(),
            origin: origin.into(),
            process: session.id.clone(),
            thread: thread.into(),
            tab: tab.into(),
        });
        session.send(json!({"type":"resize","width":size["width"],"height":size["height"]}))?;
        let instance = session
            .private
            .as_ref()
            .map(|private| private.approve(thread, tab))
            .transpose()?;
        let automation = session
            .private
            .as_ref()
            .is_some_and(private::Guest::is_open);
        if session.tabs.insert((thread.into(), tab.into())) {
            let url = if snapshot["navStatus"]["_tag"] == "Idle" {
                Value::Null
            } else {
                snapshot["navStatus"]["url"].clone()
            };
            session.send(
                json!({"type":"open","threadId":thread,"tabId":tab,"surfaceId":view_id,"url":url,"automation":automation,"automationInstance":instance,"serverControlled":snapshot["runtime"]=="server"}),
            )?;
        } else {
            session.send(
                json!({"type":"activate","threadId":thread,"tabId":tab,"surfaceId":view_id,"automation":automation,"automationInstance":instance,"serverControlled":snapshot["runtime"]=="server"}),
            )?;
        }
        session.send(json!({"type":"zoom","threadId":thread,"tabId":tab,"value":zoom}))?;
        Ok(())
    }
    pub fn hide(&mut self) {
        let previous = self.active.lock().unwrap().take();
        if let Some(previous) = previous {
            for session in self.sessions.values_mut() {
                if session.id == previous.process {
                    let _ = session.send(json!({"type":"deactivate"}));
                }
            }
        }
    }
    pub fn input(&mut self, view_id: &str, event: &Value) -> Result<()> {
        let view = self
            .active
            .lock()
            .unwrap()
            .clone()
            .filter(|v| v.id == view_id)
            .ok_or("browser_view_changed")?;
        if event.to_string().len() > 8192
            || !matches!(
                text(event, "type"),
                "press"
                    | "release"
                    | "move"
                    | "double"
                    | "wheel"
                    | "key"
                    | "keyup"
                    | "text"
                    | "composition"
                    | "resize"
                    | "capture"
            )
        {
            return Err("invalid_browser_input".into());
        }
        let session = self
            .sessions
            .values_mut()
            .find(|s| s.id == view.process)
            .ok_or("browser_runtime_closed")?;
        session.send(event.clone())
    }
    pub(crate) fn permission_decision(
        &mut self,
        source: &Value,
        allow: bool,
        remember: bool,
    ) -> Result<()> {
        let session = self
            .sessions
            .get_mut(&(
                text(source, "environment").into(),
                text(source, "profileId").into(),
            ))
            .filter(|session| {
                session.id == text(source, "processId")
                    && session.origin == text(source, "originKey")
            })
            .ok_or("browser_view_changed")?;
        session.send(json!({"type":"permission-decision","requestId":source["nativeRequestId"],"allow":allow,"remember":remember}))
    }
    pub fn close_tab(
        &mut self,
        environment: &str,
        profile: &str,
        thread: &str,
        tab: &str,
    ) -> Result<()> {
        if self
            .active
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|v| v.environment == environment && v.thread == thread && v.tab == tab)
        {
            self.hide();
        }
        let key = (environment.to_owned(), profile.to_owned());
        if let Some(session) = self.sessions.get_mut(&key) {
            if session.tabs.remove(&(thread.into(), tab.into())) {
                if let Some(private) = &session.private {
                    private.remove(thread, tab);
                }
                session.send(json!({"type":"close-tab","threadId":thread,"tabId":tab}))?;
            }
            if session.tabs.is_empty() {
                self.sessions.remove(&key);
            }
        }
        Ok(())
    }
    pub fn close_environment(&mut self, environment: &str) {
        if self
            .active
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|v| v.environment == environment)
        {
            self.hide();
        }
        self.sessions.retain(|(e, _), _| e != environment);
    }
    pub fn close_profile(&mut self, environment: &str, profile: &str) -> Result<()> {
        let key = (environment.to_owned(), profile.to_owned());
        let process = self.sessions.get(&key).map(|s| s.id.clone());
        if self
            .active
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|v| Some(&v.process) == process.as_ref())
        {
            self.hide();
        }
        if let Some(mut session) = self.sessions.remove(&key) {
            session.stop()?;
            if !session.cookie_checks.is_empty() {
                verify_import(&session.cookie_database, &session.cookie_checks)?;
            }
        }
        Ok(())
    }
    pub fn has_pages(&self, environment: &str, profile: &str) -> bool {
        self.sessions
            .get(&(environment.into(), profile.into()))
            .is_some_and(|session| !session.tabs.is_empty())
    }
    pub fn live_import_verification(
        &mut self,
        environment: &str,
        profile: &str,
    ) -> Result<ImportVerification> {
        let session = self
            .sessions
            .get_mut(&(environment.into(), profile.into()))
            .ok_or("browser_cookie_import_unknown")?;
        Ok(ImportVerification {
            path: session.cookie_database.clone(),
            checks: std::mem::take(&mut session.cookie_checks),
            alive: session.alive.clone(),
        })
    }
    pub fn verify_live_import(&mut self, environment: &str, profile: &str) -> Result<()> {
        self.live_import_verification(environment, profile)?
            .verify()
    }
    /// Caller must own a preflighted profile reservation. No evaluation or
    /// import command is accepted by the public page-input path.
    /// A true receipt is provisional: close_profile or verify_live_import must
    /// verify persistence before success or new-profile registration.
    pub fn import_cookie(
        &mut self,
        environment: &str,
        profile: &str,
        origin: &str,
        cookie: &crate::browser_cookies::ImportedCookie,
    ) -> Result<bool> {
        use zeroize::Zeroizing;
        crate::browser_runtime::partition(environment, profile)?;
        if profile == "incognito" || origin.is_empty() || origin.len() > 128 {
            return Err("invalid_browser_profile".into());
        }
        #[derive(serde::Serialize)]
        struct WireCookie<'a> {
            url: &'a str,
            name: &'a str,
            value: &'a str,
            path: &'a str,
            secure: bool,
            #[serde(rename = "httpOnly")]
            http_only: bool,
            #[serde(skip_serializing_if = "Option::is_none")]
            domain: Option<&'a str>,
            #[serde(rename = "expirationDate", skip_serializing_if = "Option::is_none")]
            expiration_date: Option<f64>,
            #[serde(rename = "sameSite")]
            same_site: &'a str,
        }
        #[derive(serde::Serialize)]
        struct Command<'a> {
            #[serde(rename = "type")]
            kind: &'a str,
            #[serde(rename = "requestId")]
            request_id: &'a str,
            cookie: WireCookie<'a>,
        }
        let id = uuid::Uuid::new_v4().to_string();
        let command = Command {
            kind: "import-cookie",
            request_id: &id,
            cookie: WireCookie {
                url: &cookie.url,
                name: &cookie.name,
                value: &cookie.value,
                path: &cookie.path,
                secure: cookie.secure,
                http_only: cookie.http_only,
                domain: cookie.domain.as_deref(),
                expiration_date: cookie.expiration_date,
                same_site: cookie.same_site,
            },
        };
        let bytes =
            Zeroizing::new(serde_json::to_vec(&command).map_err(|_| "invalid_browser_cookie")?);
        if bytes.len() > 65536 {
            return Ok(false);
        }
        let key = (environment.to_owned(), profile.to_owned());
        if self.sessions.get(&key).is_some_and(|s| s.origin != origin) {
            return Err("browser_profile_scope_changed".into());
        }
        if !self.sessions.contains_key(&key) {
            if self.sessions.len() >= 16 {
                return Err("browser_guest_limit".into());
            }
            let session = self.launch(environment, profile, origin, (240, 240), &json!(1), true)?;
            self.sessions.insert(key.clone(), session);
        }
        let session = self.sessions.get_mut(&key).unwrap();
        // Once any bytes reach Qt, an I/O failure is an unknown outcome.
        session
            .input
            .write_all(&bytes)
            .and_then(|_| session.input.write_all(b"\n"))
            .map_err(|_| "browser_cookie_import_unknown")?;
        let until = Instant::now() + Duration::from_secs(10);
        loop {
            let reply = session
                .cookie_replies
                .recv_timeout(until.saturating_duration_since(Instant::now()))
                .map_err(|_| "browser_cookie_import_unknown")?;
            if reply["requestId"] != id {
                continue;
            }
            return match text(&reply, "state") {
                "complete" | "present" => {
                    let expected = CookieCheck::expected(cookie)?;
                    session.cookie_checks.insert(expected.key(), expected);
                    Ok(true)
                }
                "failed" => Ok(false),
                _ => Err("browser_cookie_import_unknown".into()),
            };
        }
    }
    pub fn process_count(&self) -> usize {
        self.sessions.len()
    }
    pub fn close_all(&mut self) {
        self.hide();
        self.sessions.clear();
    }
    pub fn process_id(&self, environment: &str, profile: &str) -> Option<String> {
        self.sessions
            .get(&(environment.into(), profile.into()))
            .map(|s| s.id.clone())
    }
    /// Only the native-session coordinator may call this after lifecycle
    /// admission. The public pixel input path cannot issue these commands.
    pub fn admitted(
        &mut self,
        environment: &str,
        profile: &str,
        thread: &str,
        tab: &str,
        event: &Value,
    ) -> Result<()> {
        if !matches!(
            text(event, "type"),
            "navigate" | "refresh" | "zoom" | "back" | "forward" | "stop"
        ) {
            return Err("invalid_browser_command".into());
        }
        let session = self
            .sessions
            .get_mut(&(environment.into(), profile.into()))
            .ok_or("browser_runtime_closed")?;
        if !session.tabs.contains(&(thread.into(), tab.into())) {
            return Err("browser_view_changed".into());
        }
        let mut command = json!({"type":event["type"],"threadId":thread,"tabId":tab});
        match text(event, "type") {
            "navigate" => {
                let url = text(event, "url");
                if url.len() > 8192
                    || !(url.starts_with("http://")
                        || url.starts_with("https://")
                        || url == "about:blank")
                {
                    return Err("invalid_browser_command".into());
                }
                command["url"] = json!(url);
            }
            "zoom" => {
                command["value"] = crate::browser_preferences::validate(
                    "browserDefaultZoomFactor",
                    &event["value"],
                )?
            }
            "refresh" => command["bypassCache"] = json!(event["bypassCache"] == true),
            _ => {}
        }
        session.send(command)
    }
}
impl Drop for Pool {
    fn drop(&mut self) {
        self.hide();
        self.sessions.clear();
    }
}

#[cfg(test)]
mod import_verification_tests {
    use super::*;
    #[test]
    fn persisted_metadata_mismatch_cannot_be_reported_as_a_finished_import() {
        use std::process::Command;
        let root = std::env::temp_dir().join(format!("ot3-import-verify-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("Cookies");
        let sql="create table cookies(host_key,name,path,is_secure,is_httponly,samesite,has_expires,expires_utc,top_frame_site_key);insert into cookies values('example.test','fixture','/',1,1,1,0,0,'');";
        assert!(Command::new("sqlite3")
            .arg(&path)
            .arg(sql)
            .status()
            .unwrap()
            .success());
        let expected = CookieCheck {
            host: "example.test".into(),
            name: "fixture".into(),
            path: "/".into(),
            secure: true,
            http_only: true,
            same_site: 2,
            expires: None,
        };
        let mut checks = HashMap::new();
        checks.insert(expected.key(), expected);
        assert_eq!(
            verify_import(&path, &checks).unwrap_err(),
            "browser_cookie_import_unknown",
            "a present cookie with a different SameSite policy is not final success"
        );
        assert!(Command::new("sqlite3")
            .arg(&path)
            .arg("update cookies set samesite=2;")
            .status()
            .unwrap()
            .success());
        assert!(verify_import(&path, &checks).is_ok());
        assert!(Command::new("sqlite3")
            .arg(&path)
            .arg("delete from cookies;")
            .status()
            .unwrap()
            .success());
        assert!(
            verify_import(&path, &checks).is_err(),
            "missing persisted row cannot acknowledge an in-memory event"
        );
    }
}
