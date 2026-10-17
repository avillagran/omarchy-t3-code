use omarchy_t3_client::browser_cookies::{decrypt_linux, read_chromium, read_firefox, LinuxKeys};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("ot3-cookies-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&p).unwrap();
        Self(p)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn database(path: &Path, sql: &str) {
    assert!(Command::new("sqlite3")
        .arg(path)
        .arg(sql)
        .status()
        .unwrap()
        .success());
}
fn hex(s: &str) -> Vec<u8> {
    s.as_bytes()
        .chunks(2)
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect()
}
fn cases() -> Value {
    serde_json::from_str(include_str!("../data/browser-cookie-cases.json")).unwrap()
}
#[test]
fn linux_decryptor_matches_original_function_outputs() {
    let cases = cases();
    for case in cases["cases"].as_array().unwrap() {
        let keys = LinuxKeys::from_secret(Ok(if case["missingV11"] == true {
            None
        } else {
            Some("fixture secret")
        }))
        .unwrap();
        let value = decrypt_linux(
            &hex(case["hex"].as_str().unwrap()),
            &keys,
            case["host"].as_str().unwrap(),
            case["version"].as_u64().unwrap() as u32,
        );
        assert_eq!(
            serde_json::to_value(value).unwrap(),
            case["value"],
            "{}",
            case["id"]
        );
    }
    assert!(
        matches!(LinuxKeys::from_secret(Err("needsKeychainApproval".into())),Err(e) if e=="needsKeychainApproval")
    );
}
#[test]
fn firefox_preserves_scope_expiry_units_session_cookies_and_migration_samesite() {
    let f = Fixture::new();
    for version in [9, 10, 14, 15, 16, 26] {
        let path = f.0.join(format!("firefox-{version}.sqlite"));
        let raw = if (10..=14).contains(&version) {
            ",rawSameSite"
        } else {
            ""
        };
        let raw_value = if raw.is_empty() { "" } else { ",0" };
        let expiry = if version >= 16 {
            1_800_000_000_999i64
        } else {
            1_800_000_000
        };
        database(&path,&format!("pragma user_version={version};create table moz_cookies(host,name,value,path,expiry,isSecure,isHttpOnly,sameSite,originAttributes{raw});insert into moz_cookies values('example.test','__Host-session','SECRET','/',{expiry},1,1,1,''{raw_value}),('.example.test','domain','VALUE','/path',0,0,0,0,''{raw_value}),('::1','local','LOCAL','/',0,0,0,256,''{raw_value}),('example.test','container','NEVER_IMPORT','/',{expiry},1,1,1,'^userContextId=1'{raw_value});"));
        let original = fs::read(&path).unwrap();
        let read = read_firefox(&path).unwrap();
        assert_eq!(read.cookies.len(), 3);
        assert_eq!(read.undecryptable, 0);
        let c = &read.cookies[0];
        assert_eq!(c.name, "__Host-session");
        assert_eq!(c.value, "SECRET");
        assert_eq!(c.domain, None);
        assert_eq!(c.url, "https://example.test/");
        assert!(c.secure && c.http_only);
        assert_eq!(c.expiration_date, Some(1_800_000_000.));
        assert_eq!(
            c.same_site,
            if (10..=14).contains(&version) {
                "unspecified"
            } else {
                "lax"
            }
        );
        let c = &read.cookies[1];
        assert_eq!(c.domain.as_deref(), Some(".example.test"));
        assert_eq!(c.expiration_date, None);
        assert_eq!(c.same_site, "no_restriction");
        assert_eq!(c.url, "http://example.test/path");
        assert_eq!(read.cookies[2].url, "http://[::1]/");
        assert_eq!(read.cookies[2].same_site, "unspecified");
        assert_eq!(
            fs::read(&path).unwrap(),
            original,
            "snapshot must not change source database"
        );
    }
}
fn chromium_sql(version: u32, rows: &str) -> String {
    format!("create table meta(key,value);insert into meta values('version','{version}');create table cookies(host_key,name,value,encrypted_value,path,expires_utc,is_secure,is_httponly,samesite,top_frame_site_key);{rows}")
}
#[test]
fn chromium_accounts_for_partitioned_undecryptable_and_unavailable_key_rows() {
    let f = Fixture::new();
    let path = f.0.join("chromium.sqlite");
    let fixtures = cases();
    let encrypted = fixtures["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "v11-24-normal")
        .unwrap()["hex"]
        .as_str()
        .unwrap();
    database(&path,&chromium_sql(24,&format!("insert into cookies values('.example.test','encrypted','',X'{encrypted}','/',13444473600000000,1,1,2,''),('example.test','plain','PLAINTEXT',X'','/path',0,0,0,-1,''),('partitioned.test','partition','PARTITIONED',X'','/',0,0,0,0,'https://top.test'),('.broken.test','broken','',X'76313162726f6b656e','/',0,0,0,999,'');")));
    let keys = LinuxKeys::from_secret(Ok(Some("fixture secret"))).unwrap();
    let before = fs::read(&path).unwrap();
    let read = read_chromium(&path, &keys).unwrap();
    assert_eq!(read.cookies.len(), 2);
    assert_eq!(read.undecryptable, 2);
    assert_eq!(
        read.undecryptable_hosts,
        vec!["partitioned.test", "broken.test"]
    );
    assert_eq!(read.cookies[0].value, "synthetic-v11-✓");
    assert_eq!(read.cookies[0].same_site, "strict");
    assert_eq!(read.cookies[0].domain.as_deref(), Some(".example.test"));
    assert_eq!(read.cookies[0].expiration_date, Some(1_800_000_000.));
    assert_eq!(read.cookies[1].same_site, "unspecified");
    assert_eq!(read.cookies[1].expiration_date, None);
    assert_eq!(fs::read(&path).unwrap(), before);
    let missing = LinuxKeys::from_secret(Err("keychainItemMissing".into())).unwrap();
    let partial = read_chromium(&path, &missing).unwrap();
    assert_eq!(
        partial.cookies.len(),
        1,
        "plaintext survives missing key when another cookie imports"
    );
    assert_eq!(partial.undecryptable, 3);
    database(&path, "delete from cookies where name!='encrypted';");
    assert!(matches!(read_chromium(&path,&missing),Err(e) if e=="keychainItemMissing"));
}
#[test]
fn malformed_schemas_and_wrong_types_fail_without_claiming_partial_success() {
    let f = Fixture::new();
    let path = f.0.join("bad.sqlite");
    fs::write(&path, "not sqlite").unwrap();
    assert!(matches!(read_firefox(&path),Err(e) if e=="readFailed"));
    fs::remove_file(&path).unwrap();
    database(
        &path,
        &chromium_sql(
            23,
            "insert into cookies values('example.test','name','value',NULL,'/',0,0,0,0,'');",
        ),
    );
    let keys = LinuxKeys::from_secret(Ok(None)).unwrap();
    assert!(matches!(read_chromium(&path,&keys),Err(e) if e=="readFailed"));
}

#[test]
fn selected_source_requires_confirmation_fresh_membership_and_quit_before_keyring() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let f = Fixture::new();
    let chrome = f.0.join(".config/google-chrome");
    fs::create_dir_all(chrome.join("Default")).unwrap();
    database(
        &chrome.join("Default/Cookies"),
        &chromium_sql(
            23,
            "insert into cookies values('example.test','name','value',X'','/',0,0,0,0,'');",
        ),
    );
    let marker = f.0.join("helper-called");
    let helper = f.0.join("helper");
    fs::write(
        &helper,
        format!("#!/bin/sh\ntouch '{}'\nexit 2\n", marker.display()),
    )
    .unwrap();
    fs::set_permissions(&helper, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(
        matches!(omarchy_t3_client::browser_cookies::read_selected(&f.0,"chrome","Default",false,&helper),Err(e) if e=="confirmation_required")
    );
    assert!(
        matches!(omarchy_t3_client::browser_cookies::read_selected(&f.0,"chrome","../../outside",true,&helper),Err(e) if e=="unknownSourceProfile")
    );
    symlink("foreign-host-1234", chrome.join("SingletonLock")).unwrap();
    assert!(
        matches!(omarchy_t3_client::browser_cookies::read_selected(&f.0,"chrome","Default",true,&helper),Err(e) if e=="browserRunning")
    );
    assert!(
        !marker.exists(),
        "no keyring request before source admission"
    );
    fs::remove_file(chrome.join("SingletonLock")).unwrap();
    let read =
        omarchy_t3_client::browser_cookies::read_selected(&f.0, "chrome", "Default", true, &helper)
            .unwrap();
    assert_eq!(read.cookies[0].value, "value");
    assert!(marker.exists());
}
#[test]
fn helper_exit_codes_preserve_unlock_denial_and_unavailable_key_fallback() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    let helper = f.0.join("helper");
    for code in [2, 3, 4] {
        fs::write(&helper, format!("#!/bin/sh\nexit {code}\n")).unwrap();
        fs::set_permissions(&helper, fs::Permissions::from_mode(0o700)).unwrap();
        let keys = LinuxKeys::from_helper(&helper, "chrome");
        if code == 3 {
            assert!(matches!(keys,Err(e) if e=="needsKeychainApproval"))
        } else {
            assert!(keys.is_ok(), "v10 remains available without a v11 secret")
        }
    }
    fs::write(&helper, "#!/bin/sh\nprintf 'fixture secret'\n").unwrap();
    let keys = LinuxKeys::from_helper(&helper, "chromium").unwrap();
    let cases = cases();
    let c = cases["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "v11-24-normal")
        .unwrap();
    assert_eq!(
        decrypt_linux(&hex(c["hex"].as_str().unwrap()), &keys, ".example.test", 24).as_deref(),
        Some("synthetic-v11-✓")
    );
    assert!(matches!(LinuxKeys::from_helper(&helper,"arbitrary-key"),Err(e) if e=="unknownSource"));
}
