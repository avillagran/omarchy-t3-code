use omarchy_t3_client::browser_guest::{Config, Pool};
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};
struct Site {
    base: String,
    requests: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}
impl Site {
    fn new() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let stop = Arc::new(AtomicBool::new(false));
        let requests = Arc::new(Mutex::new(vec![]));
        let (s, r) = (stop.clone(), requests.clone());
        let worker = thread::spawn(move || {
            while !s.load(Ordering::Relaxed) {
                let (mut stream, _) = match listener.accept() {
                    Ok(v) => v,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2));
                        continue;
                    }
                    Err(e) => panic!("{e}"),
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut header = vec![];
                let mut b = [0];
                while !header.ends_with(b"\r\n\r\n") {
                    if stream.read_exact(&mut b).is_err() {
                        break;
                    }
                    header.push(b[0]);
                }
                let request = String::from_utf8_lossy(&header).to_string();
                let first = request.lines().next().unwrap_or("").to_owned();
                r.lock().unwrap().push(request);
                let cookie = if first.starts_with("GET /one ") {
                    "Set-Cookie: guest=yes; Path=/; Max-Age=3600\r\n"
                } else if first.starts_with("GET /private-set ") {
                    "Set-Cookie: private=yes; Path=/; Max-Age=3600\r\n"
                } else {
                    ""
                };
                let page="<!doctype html><title>Fixture</title><style>body{margin:0;background:#203040;color:white}button{width:160px;height:80px}</style><button onclick=\"fetch('/clicked');this.innerText='Clicked';document.body.style.background='green'\">Click</button>";
                let body=format!("{page}<script>if(location.pathname==='/one')localStorage.setItem('fixture','persisted');if(location.pathname==='/persisted'||location.pathname==='/after-clear')fetch('/storage-'+location.pathname.slice(1)+'?value='+(localStorage.getItem('fixture')||'empty'))</script>");
                let _=write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n{}Connection: close\r\n\r\n{}",body.len(),cookie,body);
            }
        });
        Self {
            base,
            requests,
            stop,
            worker: Some(worker),
        }
    }
    fn request(&self, path: &str) -> String {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .find(|r| r.starts_with(&format!("GET {path} ")))
            .unwrap_or_else(|| panic!("missing request {path}"))
            .clone()
    }
}
impl Drop for Site {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.worker.take().unwrap().join().unwrap();
    }
}
fn snapshot(site: &Site, tab: &str, path: &str, profile: &str) -> Value {
    json!({"threadId":"thread","tabId":tab,"navStatus":{"_tag":"Success","url":format!("{}{path}",site.base),"title":"Fixture"},"canGoBack":false,"canGoForward":false,"updatedAt":"2026-10-17T00:00:00Z","profileId":profile,"runtime":"desktop"})
}
fn wait(rx: &mpsc::Receiver<Value>, id: &str, success: bool) -> Value {
    let until = Instant::now() + Duration::from_secs(10);
    let mut observed = vec![];
    loop {
        let v = rx
            .recv_timeout(until.saturating_duration_since(Instant::now()))
            .unwrap_or_else(|e|panic!("browser guest fixture deadline id={id} success={success}: {e}; observed={observed:?}"));
        observed.push(v.clone());
        if success {
            if v["kind"] == "status"
                && v["snapshot"]["tabId"] == id
                && v["snapshot"]["navStatus"]["_tag"] == "Success"
            {
                return v;
            }
        } else if v["kind"] == "frame" && v["requestId"] == id {
            return v;
        }
    }
}
fn frame_bytes(value: &Value) -> Vec<u8> {
    let path = reqwest::Url::parse(value["url"].as_str().unwrap())
        .unwrap()
        .to_file_path()
        .unwrap();
    fs::read(path).unwrap()
}

#[test]
fn real_guest_pool_preserves_tabs_scopes_input_and_flushes_owned_processes() {
    let root = std::env::temp_dir().join(format!("ot3-guest-{}", uuid::Uuid::new_v4()));
    let executable = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("bin/omarchy-t3-client");
    let config = Config::isolated(executable, &root).unwrap();
    let storage = config.storage_root();
    let site = Site::new();
    let (tx, rx) = mpsc::channel();
    let mut pool = Pool::new(config, move |v| {
        let _ = tx.send(v);
    });
    let one = snapshot(&site, "one", "/one", "default");
    pool.show("fixture", "origin", &one, "view-one", 640, 400, &json!(1))
        .unwrap();
    wait(&rx, "one", true);
    let first = wait(&rx, "view-one", false);
    assert_eq!(first["tabId"], "one");
    assert_eq!(first["environment"], "fixture");
    let before = frame_bytes(&first);
    assert_eq!(
        pool.input("old-view", &json!({"type":"press","x":50,"y":40}))
            .unwrap_err(),
        "browser_view_changed"
    );
    assert_eq!(
        pool.input(
            "view-one",
            &json!({"type":"evaluate","script":"document.cookie"})
        )
        .unwrap_err(),
        "invalid_browser_input"
    );
    pool.input(
        "view-one",
        &json!({"type":"press","x":50,"y":40,"button":1,"buttons":1}),
    )
    .unwrap();
    pool.input(
        "view-one",
        &json!({"type":"release","x":50,"y":40,"button":1,"buttons":0}),
    )
    .unwrap();
    let until = Instant::now() + Duration::from_secs(3);
    while !site
        .requests
        .lock()
        .unwrap()
        .iter()
        .any(|r| r.starts_with("GET /clicked "))
        && Instant::now() < until
    {
        thread::sleep(Duration::from_millis(10));
    }
    assert!(site.request("/clicked").contains("guest=yes"));
    let mut changed = false;
    let until = Instant::now() + Duration::from_secs(3);
    while Instant::now() < until {
        let v = rx.recv_timeout(Duration::from_millis(200)).unwrap();
        if v["kind"] == "frame" && v["requestId"] == "view-one" && frame_bytes(&v) != before {
            changed = true;
            break;
        }
    }
    assert!(changed, "real click must repaint the owned frame");
    let two = snapshot(&site, "two", "/two", "default");
    pool.show("fixture", "origin", &two, "view-two", 640, 400, &json!(1))
        .unwrap();
    wait(&rx, "two", true);
    wait(&rx, "view-two", false);
    assert_eq!(
        pool.process_count(),
        1,
        "same partition tabs share one Qt profile process"
    );
    assert!(site.request("/two").contains("guest=yes"));
    let mut another_thread = snapshot(&site, "one", "/another-thread", "default");
    another_thread["threadId"] = json!("another-thread");
    pool.show(
        "fixture",
        "origin",
        &another_thread,
        "view-another-thread",
        640,
        400,
        &json!(1),
    )
    .unwrap();
    let loaded = wait(&rx, "one", true);
    assert_eq!(loaded["snapshot"]["threadId"], "another-thread");
    let frame = wait(&rx, "view-another-thread", false);
    assert_eq!(frame["thread"], "another-thread");
    assert_eq!(pool.process_count(), 1);
    assert!(site.request("/another-thread").contains("guest=yes"));
    pool.close_tab("fixture", "default", "another-thread", "one")
        .unwrap();
    pool.show(
        "fixture",
        "origin",
        &one,
        "view-restored",
        640,
        400,
        &json!(1),
    )
    .unwrap();
    wait(&rx, "view-restored", false);
    assert_eq!(
        site.requests
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.starts_with("GET /one "))
            .count(),
        1,
        "activating an existing tab must retain its page state"
    );
    pool.hide();
    while rx.try_recv().is_ok() {}
    thread::sleep(Duration::from_millis(250));
    assert!(
        !rx.try_iter().any(|v| v["kind"] == "frame"),
        "hidden surface must not publish frames"
    );
    assert!(pool
        .input("view-restored", &json!({"type":"key","key":65,"text":"a"}))
        .is_err());
    let work = snapshot(&site, "work", "/work", "work");
    pool.show("fixture", "origin", &work, "view-work", 640, 400, &json!(1))
        .unwrap();
    wait(&rx, "work", true);
    wait(&rx, "view-work", false);
    assert!(!site.request("/work").to_lowercase().contains("cookie:"));
    assert_eq!(pool.process_count(), 2);
    let other = snapshot(&site, "other", "/other", "default");
    pool.show(
        "other-environment",
        "origin",
        &other,
        "view-other",
        640,
        400,
        &json!(1),
    )
    .unwrap();
    wait(&rx, "other", true);
    wait(&rx, "view-other", false);
    assert!(!site.request("/other").to_lowercase().contains("cookie:"));
    assert_eq!(pool.process_count(), 3);
    let mut server = one.clone();
    server["runtime"] = json!("server");
    assert_eq!(
        pool.show(
            "fixture",
            "origin",
            &server,
            "no-server",
            640,
            400,
            &json!(1)
        )
        .unwrap_err(),
        "server_browser_surface_required"
    );
    pool.close_tab("fixture", "default", "thread", "one")
        .unwrap();
    assert_eq!(pool.process_count(), 3);
    pool.close_tab("fixture", "default", "thread", "two")
        .unwrap();
    assert_eq!(
        pool.process_count(),
        2,
        "last tab flushes and reaps only its owned partition process"
    );
    let restored = snapshot(&site, "persisted", "/persisted", "default");
    pool.show(
        "fixture",
        "origin",
        &restored,
        "view-persisted",
        640,
        400,
        &json!(1),
    )
    .unwrap();
    wait(&rx, "persisted", true);
    assert!(
        site.request("/persisted").contains("guest=yes"),
        "graceful process close must flush persistent cookies"
    );
    while rx.try_recv().is_ok() {}
    pool.show(
        "fixture",
        "new-origin",
        &restored,
        "view-new-origin",
        640,
        400,
        &json!(1),
    )
    .unwrap();
    wait(&rx, "persisted", true);
    let switched = wait(&rx, "view-new-origin", false);
    assert_eq!(switched["originKey"], "new-origin");
    assert_eq!(
        pool.process_count(),
        3,
        "origin change replaces rather than concurrently opens the same partition"
    );
    assert!(pool
        .input("view-persisted", &json!({"type":"press","x":20,"y":20}))
        .is_err());
    assert!(
        site.request("/storage-persisted?value=persisted")
            .contains("guest=yes"),
        "localStorage persists across graceful Qt process restart"
    );
    pool.close_profile("fixture", "default").unwrap();
    let lock =
        omarchy_t3_client::browser_runtime::lock_profile(&storage, "fixture", "default").unwrap();
    omarchy_t3_client::browser_runtime::clear_owned_storage(&storage, "fixture", "default", &lock)
        .unwrap();
    drop(lock);
    let cleared = snapshot(&site, "cleared", "/after-clear", "default");
    pool.show(
        "fixture",
        "new-origin",
        &cleared,
        "view-cleared",
        640,
        400,
        &json!(1),
    )
    .unwrap();
    wait(&rx, "cleared", true);
    let until = Instant::now() + Duration::from_secs(3);
    while !site
        .requests
        .lock()
        .unwrap()
        .iter()
        .any(|r| r.starts_with("GET /storage-after-clear?value=empty "))
        && Instant::now() < until
    {
        thread::sleep(Duration::from_millis(10));
    }
    assert!(
        !site
            .request("/after-clear")
            .to_lowercase()
            .contains("cookie:"),
        "cleared Qt partition loses persistent cookies"
    );
    assert!(
        !site
            .request("/storage-after-clear?value=empty")
            .to_lowercase()
            .contains("cookie:"),
        "cleared Qt partition loses localStorage too"
    );
    assert_eq!(
        pool.process_count(),
        3,
        "clearing default preserves other profile/environment processes"
    );
    pool.close_profile("fixture", "work").unwrap();
    assert_eq!(
        pool.process_count(),
        2,
        "profile flush leaves unrelated partitions running"
    );
    pool.close_environment("fixture");
    assert_eq!(pool.process_count(), 1);
    pool.close_environment("other-environment");
    assert_eq!(pool.process_count(), 0);
    let private = snapshot(&site, "private", "/private-set", "incognito");
    pool.show(
        "fixture",
        "origin",
        &private,
        "view-private",
        640,
        400,
        &json!(1),
    )
    .unwrap();
    wait(&rx, "private", true);
    pool.close_tab("fixture", "incognito", "thread", "private")
        .unwrap();
    let restarted = snapshot(&site, "private-restarted", "/private-empty", "incognito");
    pool.show(
        "fixture",
        "origin",
        &restarted,
        "view-private-restarted",
        640,
        400,
        &json!(1),
    )
    .unwrap();
    wait(&rx, "private-restarted", true);
    assert!(!site
        .request("/private-empty")
        .to_lowercase()
        .contains("cookie:"));
    drop(pool);
    let frames = root.join("cache/omarchy-t3-code/browser-sessions");
    assert_eq!(
        fs::read_dir(frames).unwrap().count(),
        0,
        "all private frame directories removed on shutdown"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn private_import_channel_initializes_cold_profile_acknowledges_and_persists_without_ui_values() {
    use omarchy_t3_client::browser_cookies::ImportedCookie;
    let root = std::env::temp_dir().join(format!("ot3-cookie-target-{}", uuid::Uuid::new_v4()));
    let executable = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("bin/omarchy-t3-client");
    let config = Config::isolated(executable, &root).unwrap();
    let storage = config.storage_root();
    let site = Site::new();
    let (tx, rx) = mpsc::channel();
    let mut pool = Pool::new(config.clone(), move |v| {
        let _ = tx.send(v);
    });
    let cookie = ImportedCookie {
        url: format!("{}/", site.base),
        name: "imported".into(),
        value: "IMPORTED_PRIVATE".into(),
        domain: None,
        path: "/".into(),
        secure: false,
        http_only: true,
        expiration_date: Some(
            (std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
                + 3600) as f64,
        ),
        same_site: "strict",
    };
    assert!(
        pool.import_cookie("fixture", "work", "origin", &cookie)
            .unwrap(),
        "cold Qt cookie store acknowledges private write"
    );
    assert!(
        pool.import_cookie("fixture", "work", "origin", &cookie)
            .unwrap(),
        "explicit second import of an identical cookie acknowledges the existing desired state"
    );
    let session_cookie = ImportedCookie {
        url: format!("{}/", site.base),
        name: "imported-session".into(),
        value: "SESSION_PRIVATE".into(),
        domain: None,
        path: "/".into(),
        secure: false,
        http_only: true,
        expiration_date: None,
        same_site: "unspecified",
    };
    assert!(pool
        .import_cookie("fixture", "work", "origin", &session_cookie)
        .unwrap());
    assert_eq!(pool.process_count(), 1);
    assert!(
        !rx.try_iter()
            .any(|v| v.to_string().contains("IMPORTED_PRIVATE")),
        "cookie values never leave the guest owner's private reply channel"
    );
    let mut invalid = ImportedCookie {
        url: "file:///tmp/never".into(),
        name: "bad".into(),
        value: "INVALID_PRIVATE".into(),
        domain: None,
        path: "/".into(),
        secure: false,
        http_only: false,
        expiration_date: None,
        same_site: "unspecified",
    };
    assert!(
        !pool
            .import_cookie("fixture", "work", "origin", &invalid)
            .unwrap(),
        "known Qt rejection is skipped rather than acknowledged"
    );
    invalid.url = format!("{}/", site.base);
    invalid.value = "x".repeat(70000);
    assert!(
        !pool
            .import_cookie("fixture", "work", "origin", &invalid)
            .unwrap(),
        "oversized private wire command rejected before dispatch"
    );
    assert_eq!(
        pool.import_cookie("fixture", "work", "changed-origin", &cookie)
            .unwrap_err(),
        "browser_profile_scope_changed"
    );
    assert_eq!(
        pool.import_cookie("fixture", "incognito", "origin", &cookie)
            .unwrap_err(),
        "invalid_browser_profile"
    );
    pool.close_profile("fixture", "work").unwrap();
    let identity = omarchy_t3_client::browser_runtime::partition("fixture", "work").unwrap();
    let db = storage
        .join(
            identity["partition"]
                .as_str()
                .unwrap()
                .trim_start_matches("persist:"),
        )
        .join("storage/Cookies");
    let output = std::process::Command::new("sqlite3")
        .arg(db)
        .arg("select host_key,is_httponly,samesite from cookies where name='imported';")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "127.0.0.1|1|2",
        "actual persisted Chromium metadata preserves host-only HttpOnly/Strict scope"
    );
    let page = snapshot(&site, "imported-page", "/imported-page", "work");
    pool.show(
        "fixture",
        "origin",
        &page,
        "view-imported",
        640,
        400,
        &json!(1),
    )
    .unwrap();
    wait(&rx, "imported-page", true);
    assert!(
        site.request("/imported-page")
            .contains("imported=IMPORTED_PRIVATE"),
        "real HTTP request uses imported cookie after process flush/restart"
    );
    assert!(
        site.request("/imported-page")
            .contains("imported-session=SESSION_PRIVATE"),
        "session cookies remain usable across in-app Qt profile flush/restart"
    );
    assert!(!rx
        .try_iter()
        .any(|v| v.to_string().contains("IMPORTED_PRIVATE")));
    drop(pool);
    let (tx, rx) = mpsc::channel();
    let mut restarted = Pool::new(config, move |v| {
        let _ = tx.send(v);
    });
    let fresh = snapshot(&site, "new-app", "/new-app", "work");
    restarted
        .show(
            "fixture",
            "origin",
            &fresh,
            "view-new-app",
            640,
            400,
            &json!(1),
        )
        .unwrap();
    wait(&rx, "new-app", true);
    let request = site.request("/new-app");
    assert!(
        request.contains("imported=IMPORTED_PRIVATE"),
        "persistent cookie remains after owner restart"
    );
    assert!(
        !request.contains("imported-session=SESSION_PRIVATE"),
        "session cookie is cleared before first navigation of a new owner"
    );
    drop(restarted);
    fs::remove_dir_all(root).unwrap();
}
