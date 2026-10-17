use omarchy_t3_client::browser_sources::{list_at, parse_firefox_profiles};
use serde_json::{json, Value};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    os::unix::fs::symlink,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
struct Home(PathBuf);
impl Home {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("ot3-source-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&p).unwrap();
        Self(p)
    }
}
impl Drop for Home {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn db(path: &Path, sql: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    assert!(Command::new("sqlite3")
        .arg(path)
        .arg(sql)
        .status()
        .unwrap()
        .success());
}
fn source(rows: &Value, id: &str) -> Value {
    rows.as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == id)
        .unwrap()
        .clone()
}
fn hostname() -> String {
    let mut bytes = [0u8; 256];
    unsafe {
        libc::gethostname(bytes.as_mut_ptr().cast(), bytes.len());
    }
    String::from_utf8_lossy(&bytes[..bytes.iter().position(|b| *b == 0).unwrap()]).into_owned()
}
#[test]
fn firefox_parser_matches_original_function_results() {
    let cases: Value =
        serde_json::from_str(include_str!("../data/browser-source-cases.json")).unwrap();
    for case in cases["cases"].as_array().unwrap() {
        assert_eq!(
            json!(parse_firefox_profiles(case["input"].as_str().unwrap())),
            case["profiles"],
            "{}",
            case["input"]
        );
    }
}
#[test]
fn discovery_reuses_original_roots_counts_current_databases_and_firefox_containers() {
    let home = Home::new();
    let chrome = home.0.join(".config/google-chrome");
    db(
        &chrome.join("Profile 1/Cookies"),
        "create table cookies(value);insert into cookies values('STALE_PRIVATE');",
    );
    db(
        &chrome.join("Profile 1/Network/Cookies"),
        "create table cookies(value);insert into cookies values('COOKIE_PRIVATE'),('another');",
    );
    fs::write(chrome.join("Local State"),json!({"profile":{"info_cache":{"Profile 1":{"name":" Personal "},"../outside":{"name":"unsafe"},"empty":{"name":"Empty"}}}}).to_string()).unwrap();
    fs::create_dir_all(home.0.join(".config/microsoft-edge")).unwrap();
    let firefox = home.0.join(".mozilla/firefox");
    db(&firefox.join("abc/cookies.sqlite"),"create table moz_cookies(originAttributes,value);insert into moz_cookies values('','COOKIE_PRIVATE'),('^userContextId=1','container');");
    fs::write(
        firefox.join("profiles.ini"),
        "[Profile0]\nName=Main\nPath=abc\n[Profile1]\nPath=never-launched",
    )
    .unwrap();
    let snap = home.0.join("snap/firefox/common/.mozilla/firefox");
    db(
        &snap.join("snapprofile/cookies.sqlite"),
        "create table moz_cookies(originAttributes);insert into moz_cookies values('');",
    );
    let rows = list_at(&home.0).unwrap();
    assert!(!rows.to_string().contains("COOKIE_PRIVATE"));
    assert!(!rows.to_string().contains("STALE_PRIVATE"));
    let chrome = source(&rows, "chrome");
    assert!(chrome.get("unavailable").is_none());
    let profiles = chrome["profiles"].as_array().unwrap();
    assert_eq!(
        profiles
            .iter()
            .find(|p| p["directory"] == "Profile 1")
            .unwrap(),
        &json!({"directory":"Profile 1","name":"Personal","cookieCount":2})
    );
    assert_eq!(
        profiles.len(),
        2,
        "original metadata keeps declared not-yet-launched Chromium profiles"
    );
    assert_eq!(source(&rows, "edge")["unavailable"], "notInstalled");
    assert_eq!(source(&rows, "arc")["unavailable"], "unsupportedPlatform");
    assert_eq!(
        source(&rows, "safari")["unavailable"],
        "unsupportedPlatform"
    );
    let firefox = source(&rows, "firefox");
    assert_eq!(firefox["profiles"].as_array().unwrap().len(), 2);
    assert_eq!(firefox["profiles"][0]["cookieCount"], 1);
    assert_eq!(
        firefox["profiles"][1]["directory"],
        json!(snap.join("snapprofile"))
    );
}
#[test]
fn malformed_metadata_falls_back_and_unknown_database_counts_stay_absent() {
    let home = Home::new();
    let brave = home.0.join(".config/BraveSoftware/Brave-Browser");
    fs::create_dir_all(brave.join("Profile 2/Network")).unwrap();
    fs::write(brave.join("Profile 2/Network/Cookies"), "not sqlite").unwrap();
    fs::write(
        brave.join("Local State"),
        json!({"profile":{"info_cache":{"Default":{"name":2}}}}).to_string(),
    )
    .unwrap();
    let firefox = home.0.join(".mozilla/firefox");
    db(
        &firefox.join("literal\\profile/cookies.sqlite"),
        "create table moz_cookies(originAttributes);",
    );
    let rows = list_at(&home.0).unwrap();
    assert_eq!(
        source(&rows, "firefox")["profiles"][0]["directory"],
        "literal\\profile",
        "Unix Firefox paths do not inherit Chromium segment rules"
    );
    let p = source(&rows, "brave");
    assert_eq!(
        p["profiles"],
        json!([{"directory":"Profile 2","name":"Profile 2"}])
    );
}
#[test]
fn chromium_distinguishes_active_stale_and_foreign_singleton_locks() {
    let home = Home::new();
    let chrome = home.0.join(".config/google-chrome");
    db(
        &chrome.join("Default/Cookies"),
        "create table cookies(value);",
    );
    let lock = chrome.join("SingletonLock");
    for target in [
        format!("{}-{}", hostname(), std::process::id()),
        "foreign-host-2147483647".into(),
        "invalid".into(),
    ] {
        symlink(target, &lock).unwrap();
        let rows = list_at(&home.0).unwrap();
        let c = source(&rows, "chrome");
        assert_eq!(c["unavailable"], "browserRunning");
        assert_eq!(c["profiles"], json!([]));
        fs::remove_file(&lock).unwrap();
    }
    symlink(format!("{}-2147483647", hostname()), &lock).unwrap();
    let rows = list_at(&home.0).unwrap();
    assert!(source(&rows, "chrome").get("unavailable").is_none());
    assert!(
        fs::symlink_metadata(&lock).is_ok(),
        "discovery must not delete stale locks"
    );
}
#[test]
fn firefox_uses_kernel_parent_lock_and_lists_after_browser_releases_it() {
    let home = Home::new();
    let firefox = home.0.join(".mozilla/firefox");
    let dir = firefox.join("profile");
    db(
        &dir.join("cookies.sqlite"),
        "create table moz_cookies(originAttributes);",
    );
    let lock = dir.join(".parentlock");
    fs::write(&lock, "").unwrap();
    let mut holder=Command::new("python3").arg("-c").arg("import fcntl,sys\nf=open(sys.argv[1],'r+')\nfcntl.lockf(f,fcntl.LOCK_EX|fcntl.LOCK_NB)\nprint('ready',flush=True)\nsys.stdin.read()\n").arg(&lock).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
    let mut ready = String::new();
    BufReader::new(holder.stdout.take().unwrap())
        .read_line(&mut ready)
        .unwrap();
    assert_eq!(ready.trim(), "ready");
    let rows = list_at(&home.0).unwrap();
    assert_eq!(source(&rows, "firefox")["unavailable"], "browserRunning");
    holder.stdin.take().unwrap().flush().unwrap();
    holder.wait().unwrap();
    let rows = list_at(&home.0).unwrap();
    assert!(source(&rows, "firefox").get("unavailable").is_none());
    assert!(
        lock.exists(),
        "leftover marker file is not a running browser"
    );
}

#[test]
fn chromium_declared_profile_order_matches_javascript_object_entries() {
    let home = Home::new();
    let chrome = home.0.join(".config/google-chrome");
    db(
        &chrome.join("Default/Cookies"),
        "create table cookies(value);",
    );
    fs::write(chrome.join("Local State"),r#"{"profile":{"info_cache":{"Profile 2":{"name":"Two"},"Default":{},"10":{},"2":{},"Profile 1":{}}}}"#).unwrap();
    let rows = list_at(&home.0).unwrap();
    let chrome = source(&rows, "chrome");
    let dirs = chrome["profiles"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["directory"].clone())
        .collect::<Vec<_>>();
    assert_eq!(
        dirs,
        json!(["2", "10", "Profile 2", "Default", "Profile 1"])
            .as_array()
            .unwrap()
            .clone()
    );
}

#[test]
fn actual_cli_and_watch_expose_only_discovery_metadata_from_isolated_home() {
    let home = Home::new();
    let chrome = home.0.join(".config/google-chrome");
    db(
        &chrome.join("Default/Cookies"),
        "create table cookies(value);insert into cookies values('NEVER_EXPORT_COOKIE');",
    );
    let bin = env!("CARGO_BIN_EXE_omarchy-t3-client");
    let command = || {
        let mut c = Command::new(bin);
        c.env_clear()
            .env("HOME", &home.0)
            .env("T3CODE_HOME", home.0.join("native"))
            .env("XDG_CACHE_HOME", home.0.join("cache"))
            .env("PATH", "/usr/bin:/bin");
        c
    };
    let output = command().arg("browser-import-sources").output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let rows: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(source(&rows, "chrome")["profiles"][0]["cookieCount"], 1);
    assert!(!String::from_utf8_lossy(&output.stdout).contains("NEVER_EXPORT_COOKIE"));
    struct Child(std::process::Child);
    impl Drop for Child {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let mut child = Child(
        command()
            .arg("watch")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let stdout = child.0.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout)
            .lines()
            .map_while(std::result::Result::ok)
        {
            if let Ok(v) = serde_json::from_str::<Value>(&line) {
                if tx.send(v).is_err() {
                    return;
                }
            }
        }
    });
    let input = json!({"op":"browser-import-sources","requestId":"fixture-discovery"});
    writeln!(child.0.stdin.as_mut().unwrap(), "{input}").unwrap();
    child.0.stdin.as_mut().unwrap().flush().unwrap();
    let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let v = rx
            .recv_timeout(until.saturating_duration_since(std::time::Instant::now()))
            .expect("source discovery worker receipt");
        if v["type"] == "action" && v["data"]["requestId"] == "fixture-discovery" {
            assert!(v["data"]["error"].is_null(), "{v}");
            assert_eq!(v["data"]["result"], rows);
            assert!(!v.to_string().contains("NEVER_EXPORT_COOKIE"));
            break;
        }
    }
    drop(child);
    reader.join().unwrap();
}
