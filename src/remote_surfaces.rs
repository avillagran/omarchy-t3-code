//! Opt-in, read-only snapshot broker. Never uses the desktop owner's T3 credential.
//! The owner publishes immutable, expiring snapshots for an exact bearer fingerprint.
use crate::*;
use base64::{engine::general_purpose::STANDARD, Engine};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    io::{BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    os::{
        fd::AsRawFd,
        unix::fs::{OpenOptionsExt, PermissionsExt},
    },
    path::Path,
    time::{Duration, Instant},
};

const STORE_LIMIT: u64 = 32 * 1024 * 1024;
const DOCUMENT_LIMIT: usize = 1024 * 1024;
const FRAME_LIMIT: usize = 512 * 1024;
const ITEMS: usize = 32;
type Reply = std::result::Result<Value, &'static str>;
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn opaque(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}
fn fingerprint(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}
fn consent(settings: &Value) -> Option<&str> {
    (settings["remoteDocumentsEnabled"] == true)
        .then(|| settings["remoteDocumentsConsent"].as_str())
        .flatten()
        .filter(|s| valid_id(s))
}
fn root() -> std::path::PathBuf {
    xdg("XDG_STATE_HOME", ".local/state").join("remote-surfaces")
}
fn read_store(dir: &Path) -> Reply {
    let file = match fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(dir.join("snapshots.json"))
    {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(json!([])),
        Err(_) => return Err("store_unavailable"),
    };
    if !file
        .metadata()
        .is_ok_and(|m| m.is_file() && m.len() <= STORE_LIMIT)
    {
        return Err("store_limit");
    }
    let mut bytes = Vec::new();
    file.take(STORE_LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "store_unavailable")?;
    if bytes.len() as u64 > STORE_LIMIT {
        return Err("store_limit");
    }
    let v: Value = serde_json::from_slice(&bytes).map_err(|_| "store_unavailable")?;
    if !v.as_array().is_some_and(|a| a.len() <= ITEMS) {
        return Err("store_limit");
    }
    Ok(v)
}
fn validate_snapshot(v: &Value, stamp: u64) -> std::result::Result<(), &'static str> {
    if !opaque(text(v, "session"))
        || !valid_id(text(v, "threadId"))
        || !valid_id(text(v, "projectId"))
        || !matches!(text(v, "kind"), "text" | "html")
    {
        return Err("invalid_snapshot");
    }
    if v["contents"]
        .as_str()
        .is_none_or(|s| s.len() > DOCUMENT_LIMIT)
        || v["title"]
            .as_str()
            .is_none_or(|s| s.len() > 256 || s.chars().any(char::is_control))
    {
        return Err("snapshot_limit");
    }
    let expires = v["expiresAt"].as_u64().ok_or("invalid_snapshot")?;
    if expires <= stamp || expires > stamp.saturating_add(3600) {
        return Err("invalid_expiry");
    }
    if let Some(frame) = v.get("previewPng") {
        let encoded = frame
            .as_str()
            .filter(|s| s.len() <= FRAME_LIMIT * 4 / 3 + 4)
            .ok_or("frame_limit")?;
        let bytes = STANDARD.decode(encoded).map_err(|_| "invalid_frame")?;
        if bytes.len() > FRAME_LIMIT {
            return Err("frame_limit");
        }
        // Decode under explicit allocation/dimension limits before accepting owner-produced frames.
        let mut reader =
            image::ImageReader::with_format(std::io::Cursor::new(bytes), image::ImageFormat::Png);
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(1920);
        limits.max_image_height = Some(1080);
        limits.max_alloc = Some(16 * 1024 * 1024);
        reader.limits(limits);
        reader.decode().map_err(|_| "invalid_frame")?;
        if v["kind"] != "html" {
            return Err("invalid_frame");
        }
    }
    Ok(())
}
/// Local owner stdin API only. Remote HTTP routes never call this function.
pub fn owner(input: &Value, settings: &Value) -> Result<Value> {
    owner_at(&root(), input, settings).map_err(str::to_owned)
}
fn owner_at(dir: &Path, input: &Value, settings: &Value) -> Reply {
    fs::create_dir_all(dir).map_err(|_| "store_unavailable")?;
    if fs::symlink_metadata(dir)
        .map_err(|_| "store_unavailable")?
        .file_type()
        .is_symlink()
    {
        return Err("store_unavailable");
    }
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700)).map_err(|_| "store_unavailable")?;
    let lock = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(dir.join("lock"))
        .map_err(|_| "store_unavailable")?;
    if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return Err("store_busy");
    }
    let mut store = read_store(dir)?;
    let rows = store.as_array_mut().ok_or("store_unavailable")?;
    rows.retain(|v| {
        v["expiresAt"].as_u64().is_some_and(|t| t > now())
            && v["consent"].as_str() == consent(settings)
    });
    let result = match text(input, "action") {
        "publish" => {
            let epoch = consent(settings).ok_or("consent_required")?;
            if input["confirmed"] != true {
                return Err("confirmation_required");
            }
            validate_snapshot(input, now())?;
            if rows.len() >= ITEMS {
                return Err("store_limit");
            }
            let id = uuid::Uuid::new_v4().to_string();
            let mut row = json!({"id":id,"session":input["session"],"consent":epoch,"threadId":input["threadId"],"projectId":input["projectId"],"title":input["title"],"kind":input["kind"],"contents":input["contents"],"expiresAt":input["expiresAt"]});
            if input.get("previewPng").is_some() {
                row["previewPng"] = input["previewPng"].clone();
            }
            rows.push(row);
            json!({"id":id})
        }
        "revoke" => {
            let id = text(input, "id");
            if uuid::Uuid::parse_str(id).is_err() {
                return Err("invalid_id");
            }
            rows.retain(|v| v["id"] != id);
            json!({"revoked":true})
        }
        "clear" if input["confirmed"] == true => {
            rows.clear();
            json!({"revoked":true})
        }
        _ => return Err("invalid_command"),
    };
    let bytes = serde_json::to_vec(&store).map_err(|_| "store_unavailable")?;
    if bytes.len() as u64 > STORE_LIMIT {
        return Err("store_limit");
    }
    let tmp = dir.join(format!(".{}", uuid::Uuid::new_v4()));
    let result_write = (|| {
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&tmp)?;
        f.write_all(&bytes)?;
        f.sync_all()?;
        fs::rename(&tmp, dir.join("snapshots.json"))?;
        fs::File::open(dir)?.sync_all()
    })();
    let _ = fs::remove_file(tmp);
    result_write.map_err(|_| "store_unavailable")?;
    Ok(result)
}
fn read_allowed(auth: &Value) -> bool {
    auth["authenticated"] == true
        && matches!(
            text(&auth["auth"], "policy"),
            "desktop-managed-local" | "loopback-browser" | "remote-reachable"
        )
        && auth["sessionMethod"] == "bearer-access-token"
        && auth
            .get("permissions")
            .unwrap_or(&auth["scopes"])
            .as_array()
            .is_some_and(|a| a.iter().any(|s| s == "orchestration:read"))
}
fn project(store: &Value, settings: &Value, session: &str, path: &str, stamp: u64) -> Reply {
    let epoch = consent(settings).ok_or("consent_required")?;
    if path == "/v1/session" {
        return Ok(
            json!({"protocol":1,"session":session,"capabilities":["documents.read","gallery.read"]}),
        );
    }
    let rows: Vec<&Value> = store
        .as_array()
        .ok_or("store_unavailable")?
        .iter()
        .filter(|v| {
            v["session"] == session
                && v["consent"] == epoch
                && v["expiresAt"].as_u64().is_some_and(|t| t > stamp)
        })
        .collect();
    if matches!(path, "/v1/documents" | "/v1/gallery") {
        return Ok(
            json!({"items":rows.into_iter().filter(|v| path != "/v1/gallery" || v["previewPng"].is_string()).map(|v| json!({"id":v["id"],"threadId":v["threadId"],"projectId":v["projectId"],"title":v["title"],"kind":v["kind"],"expiresAt":v["expiresAt"],"preview":v["previewPng"].is_string()})).collect::<Vec<_>>()}),
        );
    }
    let (id, frame) = if let Some(id) = path.strip_prefix("/v1/documents/") {
        (id, false)
    } else if let Some(id) = path.strip_prefix("/v1/gallery/") {
        (id, true)
    } else {
        return Err("not_found");
    };
    if uuid::Uuid::parse_str(id).is_err() {
        return Err("not_found");
    }
    let row = rows
        .into_iter()
        .find(|v| v["id"] == id)
        .ok_or("not_found")?;
    if frame {
        if !row["previewPng"].is_string() {
            return Err("not_found");
        }
        return Ok(json!({"id":id,"mimeType":"image/png","base64":row["previewPng"]}));
    }
    Ok(
        json!({"id":id,"threadId":row["threadId"],"projectId":row["projectId"],"title":row["title"],"kind":row["kind"],"contents":row["contents"],"executable":false}),
    )
}
struct Rate {
    window: Instant,
    total: u32,
    sessions: HashMap<String, u32>,
}
impl Rate {
    fn new() -> Self {
        Self {
            window: Instant::now(),
            total: 0,
            sessions: HashMap::new(),
        }
    }
    fn connection(&mut self) -> bool {
        if self.window.elapsed() >= Duration::from_secs(60) {
            *self = Self::new();
        }
        self.total = self.total.saturating_add(1);
        self.total <= 120
    }
    fn session(&mut self, session: &str) -> bool {
        let used = self.sessions.entry(session.to_owned()).or_default();
        *used += 1;
        *used <= 30
    }
    #[cfg(test)]
    fn admit(&mut self, session: &str) -> bool {
        self.connection() && self.session(session)
    }
}
fn authenticate(
    client: &reqwest::blocking::Client,
    base: &str,
    token: &str,
) -> std::result::Result<(), &'static str> {
    let response = client
        .get(format!("{base}/api/auth/session"))
        .bearer_auth(token)
        .send()
        .map_err(|_| "authentication_unavailable")?;
    if !response.status().is_success() {
        return Err("unauthorized");
    }
    let mut body = Vec::new();
    response
        .take(16385)
        .read_to_end(&mut body)
        .map_err(|_| "unauthorized")?;
    if body.len() > 16384 {
        return Err("unauthorized");
    }
    let auth: Value = serde_json::from_slice(&body).map_err(|_| "unauthorized")?;
    if !read_allowed(&auth) {
        return Err("unauthorized");
    }
    Ok(())
}
/// Only GET, one request per connection. No cookies, CORS, body, query strings,
/// redirects, caller-selected auth server, or path forwarding to native handlers.
fn request(stream: &mut TcpStream) -> std::result::Result<(String, String), &'static str> {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|_| "invalid_request")?;
    let deadline = Instant::now();
    let mut bytes = Vec::new();
    let mut reader = BufReader::new(stream);
    loop {
        let mut byte = [0];
        reader
            .read_exact(&mut byte)
            .map_err(|_| "invalid_request")?;
        bytes.push(byte[0]);
        if bytes.len() > 8192 || deadline.elapsed() > Duration::from_secs(2) {
            return Err("request_limit");
        }
        if bytes.ends_with(b"\r\n\r\n") {
            break;
        }
    }
    let raw = std::str::from_utf8(&bytes).map_err(|_| "invalid_request")?;
    let mut lines = raw.split("\r\n");
    let line = lines.next().ok_or("invalid_request")?;
    let parts: Vec<_> = line.split(' ').collect();
    if parts.len() != 3
        || parts[0] != "GET"
        || parts[2] != "HTTP/1.1"
        || parts[1].len() > 128
        || parts[1].contains(['?', '%', '#'])
    {
        return Err("invalid_request");
    }
    let mut token = None;
    for line in lines.filter(|s| !s.is_empty()) {
        let (name, value) = line.split_once(':').ok_or("invalid_request")?;
        if !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
            || value.chars().any(|c| c.is_control())
        {
            return Err("invalid_request");
        }
        match name.to_ascii_lowercase().as_str() {
            "authorization" => {
                if token.is_some() {
                    return Err("invalid_request");
                }
                token = Some(
                    value
                        .trim()
                        .strip_prefix("Bearer ")
                        .filter(|t| {
                            !t.is_empty()
                                && t.len() <= 4096
                                && t.bytes().all(|b| b.is_ascii_graphic())
                        })
                        .ok_or("unauthorized")?
                        .to_owned(),
                );
            }
            "cookie" | "origin" | "transfer-encoding" | "content-length" => {
                return Err("invalid_request")
            }
            _ => {}
        }
    }
    Ok((parts[1].to_owned(), token.ok_or("unauthorized")?))
}
fn dispatch(
    dir: &Path,
    path: &str,
    token: &str,
    mut settings: impl FnMut() -> Value,
    mut auth: impl FnMut(&str) -> std::result::Result<(), &'static str>,
) -> Reply {
    auth(token)?;
    let before = settings();
    if consent(&before).is_none() {
        return Err("consent_required");
    }
    let session = fingerprint(token);
    project(&read_store(dir)?, &before, &session, path, now())?;
    // Recheck live revocation/permissions and owner epoch before loading the
    // final atomic snapshot. A removed item cannot survive a slow auth request.
    auth(token)?;
    let after = settings();
    if consent(&before) != consent(&after) {
        return Err("consent_required");
    }
    project(&read_store(dir)?, &after, &session, path, now())
}
/// Manual launch only. Publish via an explicitly owner-configured OmaBridge
/// service route; this never modifies service configuration or starts a forwarder.
pub fn serve(base: &str, port: u16, settings: fn() -> Value) -> Result<()> {
    let url = reqwest::Url::parse(base).map_err(|_| "invalid_auth_base")?;
    if url.scheme() != "http"
        || url.host_str() != Some("127.0.0.1")
        || url.port().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
        || port == 0
    {
        return Err("invalid_auth_base".into());
    }
    let base = base.trim_end_matches('/');
    let client = reqwest::blocking::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(2))
        .build()
        .map_err(|_| "authentication_unavailable")?;
    let listener = TcpListener::bind(("127.0.0.1", port)).map_err(|_| "broker_unavailable")?;
    let mut rate = Rate::new();
    for stream in listener.incoming() {
        let Ok(mut stream) = stream else {
            continue;
        };
        let admitted = rate.connection();
        let mut session = String::new();
        let mut operation = "request";
        let result = (|| {
            if !admitted {
                return Err("rate_limit");
            }
            let (path, token) = request(&mut stream)?;
            session = fingerprint(&token);
            if !rate.session(&session) {
                return Err("rate_limit");
            }
            operation = if path.starts_with("/v1/gallery") {
                "gallery.read"
            } else if path.starts_with("/v1/documents") {
                "documents.read"
            } else {
                "session.read"
            };
            dispatch(&root(), &path, &token, settings, |token| {
                authenticate(&client, base, token)
            })
        })();
        let code = match &result {
            Ok(_) => 200,
            Err("unauthorized") => 401,
            Err("consent_required") => 403,
            Err("not_found") => 404,
            Err("rate_limit") => 429,
            Err("authentication_unavailable" | "store_unavailable" | "store_limit") => 503,
            Err(_) => 400,
        };
        // Only fixed operation/outcome and a one-way opaque fingerprint. No IDs, URLs or content.
        // Saturated connections are dropped before parsing; audit the first
        // saturation once/window rather than letting rejected floods fill logs.
        if rate.total <= 121 {
            eprintln!(
                "{}",
                json!({"surfaceAudit":1,"operation":operation,"session":session,"outcome":result.as_ref().err().copied().unwrap_or("ok")})
            );
        }
        let body = result
            .unwrap_or_else(|error| json!({"error":error}))
            .to_string();
        let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
        let _ = write!(stream, "HTTP/1.1 {code} Result\r\nContent-Type: application/json\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nContent-Security-Policy: default-src 'none'\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}", body.len(), body);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn settings() -> Value {
        json!({"remoteDocumentsEnabled":true,"remoteDocumentsConsent":"epoch-1"})
    }
    fn snapshot() -> Value {
        json!({"action":"publish","confirmed":true,"session":fingerprint("phone-token"),"threadId":"thread-1","projectId":"project-1","kind":"text","title":"Review","contents":"private document","expiresAt":now()+300})
    }
    struct Temp(std::path::PathBuf);
    impl Temp {
        fn new() -> Self {
            Self(std::env::temp_dir().join(format!("surface-test-{}", uuid::Uuid::new_v4())))
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn published(dir: &Path) -> String {
        owner_at(dir, &snapshot(), &settings()).unwrap()["id"]
            .as_str()
            .unwrap()
            .to_owned()
    }
    fn read(dir: &Path, session: &str, path: &str) -> Reply {
        project(&read_store(dir)?, &settings(), session, path, now())
    }
    fn png() -> String {
        let mut out = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(2, 2)
            .write_to(&mut out, image::ImageFormat::Png)
            .unwrap();
        STANDARD.encode(out.into_inner())
    }
    #[test]
    fn consent_is_default_off_and_malformed_fails_closed() {
        for v in [
            json!({}),
            json!({"remoteDocumentsEnabled":true}),
            json!({"remoteDocumentsEnabled":"true","remoteDocumentsConsent":"epoch-1"}),
        ] {
            assert!(consent(&v).is_none());
            assert_eq!(
                project(&json!([]), &v, "s", "/v1/session", now()),
                Err("consent_required")
            );
        }
    }
    #[test]
    fn owner_requires_consent_and_confirmation() {
        let dir = Temp::new();
        assert_eq!(
            owner_at(&dir.0, &snapshot(), &json!({})),
            Err("consent_required")
        );
        let mut v = snapshot();
        v["confirmed"] = json!(false);
        assert_eq!(
            owner_at(&dir.0, &v, &settings()),
            Err("confirmation_required")
        );
    }
    #[test]
    fn documents_owned_by_exact_session_and_projected_without_private_metadata() {
        let dir = Temp::new();
        let id = published(&dir.0);
        let path = format!("/v1/documents/{id}");
        assert_eq!(
            read(&dir.0, &fingerprint("phone-token"), &path).unwrap()["contents"],
            "private document"
        );
        assert_eq!(
            read(&dir.0, &fingerprint("other-phone"), &path),
            Err("not_found")
        );
        let list = read(&dir.0, &fingerprint("phone-token"), "/v1/documents").unwrap();
        assert_eq!(list["items"].as_array().unwrap().len(), 1);
        assert!(!list.to_string().contains("private document"));
        assert!(!list.to_string().contains("epoch-1"));
        assert_eq!(
            read(&dir.0, &fingerprint("other-phone"), "/v1/documents").unwrap()["items"],
            json!([])
        );
    }
    #[test]
    fn individual_revoke_and_clear_are_immediate() {
        let dir = Temp::new();
        let id = published(&dir.0);
        owner_at(&dir.0, &json!({"action":"revoke","id":id}), &settings()).unwrap();
        assert_eq!(
            read(
                &dir.0,
                &fingerprint("phone-token"),
                &format!("/v1/documents/{id}")
            ),
            Err("not_found")
        );
        published(&dir.0);
        owner_at(
            &dir.0,
            &json!({"action":"clear","confirmed":true}),
            &settings(),
        )
        .unwrap();
        assert_eq!(
            read(&dir.0, &fingerprint("phone-token"), "/v1/documents").unwrap()["items"],
            json!([])
        );
    }
    #[test]
    fn off_then_on_new_epoch_does_not_resurrect_grants() {
        let dir = Temp::new();
        let id = published(&dir.0);
        let mut next = settings();
        next["remoteDocumentsConsent"] = json!("epoch-2");
        assert_eq!(
            project(
                &read_store(&dir.0).unwrap(),
                &next,
                &fingerprint("phone-token"),
                &format!("/v1/documents/{id}"),
                now()
            ),
            Err("not_found")
        );
    }
    #[test]
    fn expiration_and_token_rotation_deny_old_documents() {
        let dir = Temp::new();
        let id = published(&dir.0);
        let store = read_store(&dir.0).unwrap();
        assert_eq!(
            project(
                &store,
                &settings(),
                &fingerprint("phone-token"),
                &format!("/v1/documents/{id}"),
                now() + 301
            ),
            Err("not_found")
        );
        assert_eq!(
            read(
                &dir.0,
                &fingerprint("rotated-phone-token"),
                &format!("/v1/documents/{id}")
            ),
            Err("not_found")
        );
        let mut v = snapshot();
        v["expiresAt"] = json!(now() + 3601);
        assert_eq!(validate_snapshot(&v, now()), Err("invalid_expiry"));
    }
    #[test]
    fn text_size_and_total_count_are_bounded() {
        let dir = Temp::new();
        let mut v = snapshot();
        v["contents"] = json!("a".repeat(DOCUMENT_LIMIT + 1));
        assert_eq!(owner_at(&dir.0, &v, &settings()), Err("snapshot_limit"));
        for _ in 0..ITEMS {
            published(&dir.0);
        }
        assert_eq!(
            owner_at(&dir.0, &snapshot(), &settings()),
            Err("store_limit")
        );
    }
    #[test]
    fn gallery_is_session_owned_static_png_only_and_revocable() {
        let dir = Temp::new();
        let mut v = snapshot();
        v["kind"] = json!("html");
        v["contents"] = json!("<script>secret()</script>");
        v["previewPng"] = json!(png());
        let id = owner_at(&dir.0, &v, &settings()).unwrap()["id"]
            .as_str()
            .unwrap()
            .to_owned();
        let path = format!("/v1/gallery/{id}");
        assert_eq!(
            read(&dir.0, &fingerprint("phone-token"), &path).unwrap()["mimeType"],
            "image/png"
        );
        assert_eq!(
            read(&dir.0, &fingerprint("other-phone"), &path),
            Err("not_found")
        );
        assert_eq!(
            read(
                &dir.0,
                &fingerprint("phone-token"),
                &format!("/v1/documents/{id}")
            )
            .unwrap()["executable"],
            false
        );
        owner_at(&dir.0, &json!({"action":"revoke","id":id}), &settings()).unwrap();
        assert_eq!(
            read(&dir.0, &fingerprint("phone-token"), &path),
            Err("not_found")
        );
    }
    #[test]
    fn gallery_rejects_oversize_and_non_png_frames() {
        let mut v = snapshot();
        v["kind"] = json!("html");
        v["previewPng"] = json!("a".repeat(FRAME_LIMIT * 4 / 3 + 5));
        assert_eq!(validate_snapshot(&v, now()), Err("frame_limit"));
        v["previewPng"] = json!(STANDARD.encode(b"<svg/>"));
        assert_eq!(validate_snapshot(&v, now()), Err("invalid_frame"));
        let mut out = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(1921, 1)
            .write_to(&mut out, image::ImageFormat::Png)
            .unwrap();
        v["previewPng"] = json!(STANDARD.encode(out.into_inner()));
        assert_eq!(validate_snapshot(&v, now()), Err("invalid_frame"));
    }
    #[test]
    fn scope_checks_deny_cookie_sessions_and_operate_without_read() {
        let allowed = json!({"authenticated":true,"auth":{"policy":"remote-reachable"},"sessionMethod":"bearer-access-token","scopes":["orchestration:read"]});
        assert!(read_allowed(&allowed));
        for patch in [
            json!({"auth":{"policy":"unsafe-no-auth"}}),
            json!({"auth":null}),
            json!({"authenticated":false}),
            json!({"sessionMethod":"browser-session-cookie"}),
            json!({"scopes":["orchestration:operate"]}),
            json!({"scopes":["*"]}),
        ] {
            let mut v = allowed.clone();
            for (k, value) in patch.as_object().unwrap() {
                v[k] = value.clone();
            }
            assert!(!read_allowed(&v));
        }
    }
    #[test]
    fn explicit_permissions_override_legacy_scopes() {
        let mut v = json!({"authenticated":true,"auth":{"policy":"remote-reachable"},"sessionMethod":"bearer-access-token","scopes":["orchestration:read"],"permissions":[]});
        assert!(!read_allowed(&v));
        v["permissions"] = json!(["orchestration:operate"]);
        assert!(!read_allowed(&v));
        v["scopes"] = json!([]);
        v["permissions"] = json!(["orchestration:read"]);
        assert!(read_allowed(&v));
        v["permissions"] = Value::Null;
        assert!(!read_allowed(&v));
    }
    #[test]
    fn per_session_and_global_rate_limit_reset_without_unbounded_state() {
        let mut rate = Rate::new();
        for _ in 0..30 {
            assert!(rate.admit("s"));
        }
        assert!(!rate.admit("s"));
        for i in 0..200 {
            rate.admit(&i.to_string());
        }
        assert!(rate.sessions.len() <= 120);
        assert!(!rate.admit("fresh"));
        rate.window = Instant::now() - Duration::from_secs(61);
        assert!(rate.admit("s"));
        assert_eq!(rate.sessions.len(), 1);
    }
    #[test]
    fn symlink_store_and_nonregular_files_refused() {
        let dir = Temp::new();
        fs::create_dir(&dir.0).unwrap();
        std::os::unix::fs::symlink("/dev/null", dir.0.join("snapshots.json")).unwrap();
        assert_eq!(read_store(&dir.0), Err("store_unavailable"));
        fs::remove_file(dir.0.join("snapshots.json")).unwrap();
        fs::create_dir(dir.0.join("snapshots.json")).unwrap();
        assert_eq!(read_store(&dir.0), Err("store_limit"));
    }
    fn parse(raw: &'static str) -> std::result::Result<(String, String), &'static str> {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let t = std::thread::spawn(move || {
            let mut c = TcpStream::connect(addr).unwrap();
            c.write_all(raw.as_bytes()).unwrap();
        });
        let (mut s, _) = listener.accept().unwrap();
        let result = request(&mut s);
        t.join().unwrap();
        result
    }
    #[test]
    fn http_accepts_only_explicit_bearer_get() {
        assert_eq!(parse("GET /v1/session HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer phone-token\r\n\r\n").unwrap(),("/v1/session".into(),"phone-token".into()));
        assert_eq!(
            parse("GET /v1/session HTTP/1.1\r\n\r\n"),
            Err("unauthorized")
        );
        for raw in ["POST /v1/session HTTP/1.1\r\n\r\n","GET /v1/session?token=x HTTP/1.1\r\n\r\n","GET /v1/session HTTP/1.1\r\nAuthorization: Bearer a\r\nAuthorization: Bearer b\r\n\r\n","GET /v1/session HTTP/1.1\r\nOrigin: http://evil\r\n\r\n","GET /v1/session HTTP/1.1\r\nCookie: x\r\n\r\n","GET /v1/session HTTP/1.1\r\nContent-Length: 1\r\n\r\nx","GET /v1/session HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n"] {assert_eq!(parse(raw),Err("invalid_request"));}
    }
    #[test]
    fn http_headers_have_a_hard_size_limit() {
        let raw = Box::leak(
            format!(
                "GET /v1/session HTTP/1.1\r\nX-Long: {}\r\n\r\n",
                "a".repeat(8192)
            )
            .into_boxed_str(),
        );
        assert_eq!(parse(raw), Err("request_limit"));
    }
    #[test]
    fn auth_rechecks_revocation_using_only_callers_credential() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let worker = std::thread::spawn(move || {
            for authenticated in [true, false] {
                let (mut s, _) = listener.accept().unwrap();
                let (path, token) = request(&mut s).unwrap();
                assert_eq!(path, "/api/auth/session");
                assert_eq!(token, "phone-token");
                let body=json!({"authenticated":authenticated,"auth":{"policy":"remote-reachable"},"sessionMethod":"bearer-access-token","scopes":["orchestration:read"]}).to_string();
                write!(
                    s,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                )
                .unwrap();
            }
        });
        let client = reqwest::blocking::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap();
        assert_eq!(authenticate(&client, &base, "phone-token"), Ok(()));
        assert_eq!(
            authenticate(&client, &base, "phone-token"),
            Err("unauthorized")
        );
        worker.join().unwrap();
    }
    #[test]
    fn absent_routes_never_forward_owner_operations_or_paths() {
        for path in [
            "/v1/memory",
            "/v1/devices",
            "/v1/browser",
            "/v1/transcribe",
            "/v1/capture",
            "/v1/documents/../../etc/passwd",
            "/v1/cleanup",
            "/v1/documents/input",
        ] {
            assert_eq!(
                project(
                    &json!([]),
                    &settings(),
                    &fingerprint("phone-token"),
                    path,
                    now()
                ),
                Err("not_found")
            );
        }
    }
}

#[cfg(test)]
mod ingress_tests {
    use super::*;
    #[test]
    fn denied_auth_never_reads_store_or_owner_settings() {
        assert_eq!(
            dispatch(
                Path::new("/not-a-store"),
                "/v1/documents",
                "bad",
                || panic!("settings accessed before authentication"),
                |_| Err("unauthorized")
            ),
            Err("unauthorized")
        );
    }
    #[test]
    fn token_revoke_during_read_cannot_return_content() {
        let dir = std::env::temp_dir().join(format!("surface-ingress-{}", uuid::Uuid::new_v4()));
        let mut calls = 0;
        let result = dispatch(
            &dir,
            "/v1/session",
            "phone",
            || json!({"remoteDocumentsEnabled":true,"remoteDocumentsConsent":"epoch"}),
            |_| {
                calls += 1;
                if calls == 2 {
                    Err("unauthorized")
                } else {
                    Ok(())
                }
            },
        );
        assert_eq!(result, Err("unauthorized"));
        assert_eq!(calls, 2);
    }
    #[test]
    fn owner_revoke_during_auth_cannot_return_documents_or_frames() {
        for gallery in [false, true] {
            let dir =
                std::env::temp_dir().join(format!("surface-ingress-{}", uuid::Uuid::new_v4()));
            let settings =
                || json!({"remoteDocumentsEnabled":true,"remoteDocumentsConsent":"epoch"});
            let mut out = std::io::Cursor::new(Vec::new());
            image::DynamicImage::new_rgb8(2, 2)
                .write_to(&mut out, image::ImageFormat::Png)
                .unwrap();
            let id=owner_at(&dir,&json!({"action":"publish","confirmed":true,"session":fingerprint("phone"),"threadId":"t","projectId":"p","kind":"html","title":"Review","contents":"private HTML","previewPng":STANDARD.encode(out.into_inner()),"expiresAt":now()+300}),&settings()).unwrap()["id"].as_str().unwrap().to_owned();
            let path = format!("/v1/{}/{id}", if gallery { "gallery" } else { "documents" });
            let mut calls = 0;
            let result = dispatch(&dir, &path, "phone", settings, |_| {
                calls += 1;
                if calls == 2 {
                    owner_at(&dir, &json!({"action":"revoke","id":id}), &settings()).unwrap();
                }
                Ok(())
            });
            assert_eq!(result, Err("not_found"));
            let _ = fs::remove_dir_all(dir);
        }
    }
    #[test]
    fn disabling_consent_during_auth_prevents_response() {
        let mut calls = 0;
        let result = dispatch(
            Path::new("/not-a-store"),
            "/v1/session",
            "phone",
            || {
                calls += 1;
                json!({"remoteDocumentsEnabled":calls==1,"remoteDocumentsConsent":"epoch"})
            },
            |_| Ok(()),
        );
        assert_eq!(result, Err("consent_required"));
    }
}

#[cfg(test)]
mod configuration_tests {
    use super::*;
    #[test]
    fn authority_must_be_fixed_loopback_origin_without_credentials() {
        for base in [
            "https://127.0.0.1:1234",
            "http://localhost:1234",
            "http://192.168.1.1:1234",
            "http://127.0.0.1",
            "http://owner:secret@127.0.0.1:1234",
            "http://127.0.0.1:1234/path",
            "http://127.0.0.1:1234/?q=x",
            "http://127.0.0.1:1234/#fragment",
        ] {
            assert_eq!(
                serve(base, 12345, || json!({})),
                Err("invalid_auth_base".into())
            );
        }
    }
    #[test]
    fn store_size_is_checked_before_reading() {
        let dir = std::env::temp_dir().join(format!("surface-limit-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&dir).unwrap();
        let f = fs::File::create(dir.join("snapshots.json")).unwrap();
        f.set_len(STORE_LIMIT + 1).unwrap();
        assert_eq!(read_store(&dir), Err("store_limit"));
        fs::remove_dir_all(dir).unwrap();
    }
}
