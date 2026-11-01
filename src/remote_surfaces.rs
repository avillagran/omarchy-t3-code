//! Opt-in snapshot and host-maintenance broker. Never uses the owner's T3 credential.
//! The owner publishes immutable, expiring snapshots for an exact bearer fingerprint.
use crate::*;
use base64::{engine::general_purpose::STANDARD, Engine};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
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
pub(crate) fn read_allowed(auth: &Value) -> bool {
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
            json!({"protocol":1,"session":session,"capabilities":["documents.read","gallery.read","memory.read","vault.read","host-sync.read","compaction.read","idle-release.read","build-cleanup.read","maintenance.confirm"]}),
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
    failures: u32,
    audited: u32,
    known_good: HashSet<String>,
    sessions: HashMap<String, u32>,
}
impl Rate {
    fn new() -> Self {
        Self {
            window: Instant::now(),
            total: 0,
            failures: 0,
            audited: 0,
            known_good: HashSet::new(),
            sessions: HashMap::new(),
        }
    }
    fn refresh(&mut self) {
        if self.window.elapsed() >= Duration::from_secs(60) {
            *self = Self::new();
        }
    }
    fn connection(&mut self) -> bool {
        self.refresh();
        self.total = self.total.saturating_add(1);
        self.total <= 120
    }
    fn session(&mut self, session: &str) -> bool {
        let used = self.sessions.entry(session.to_owned()).or_default();
        *used += 1;
        *used <= 30
    }
    fn admit(&mut self, session: &str) -> bool {
        self.connection() && self.session(session)
    }
}
/// A known-good fingerprint only bypasses the failed-auth gate, never T3 auth.
/// At most 120 authenticated admissions and fingerprints survive each window.
fn rated_dispatch(
    rate: &mut Rate,
    dir: &Path,
    path: &str,
    token: &str,
    settings: impl FnMut() -> Value,
    mut auth: impl FnMut(&str) -> std::result::Result<(), &'static str>,
) -> Reply {
    rate.refresh();
    if token.is_empty() {
        return Err("unauthorized");
    }
    let session = fingerprint(token);
    if rate.total >= 120
        || rate.sessions.get(&session).is_some_and(|used| *used >= 30)
        || (rate.failures >= 8 && !rate.known_good.contains(&session))
    {
        return Err("rate_limit");
    }
    let mut admitted = false;
    dispatch(dir, path, token, settings, |token| {
        if let Err(error) = auth(token) {
            rate.failures = rate.failures.saturating_add(1);
            rate.known_good.remove(&session);
            return Err(error);
        }
        if !admitted {
            if !rate.admit(&session) {
                return Err("rate_limit");
            }
            rate.known_good.insert(session.clone());
            admitted = true;
        }
        Ok(())
    })
}
fn authenticate(
    client: &reqwest::blocking::Client,
    base: &str,
    token: &str,
) -> std::result::Result<(), &'static str> {
    authenticate_permission(client, base, token, false)
}
fn authenticate_permission(
    client: &reqwest::blocking::Client,
    base: &str,
    token: &str,
    operate: bool,
) -> std::result::Result<(), &'static str> {
    let response = client
        .get(format!("{base}/api/auth/session"))
        // Serial listener: failed probes must not hold up all readers for seconds.
        .timeout(Duration::from_millis(250))
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
    if !read_allowed(&auth) || (operate && !crate::remote_maintenance::operate_allowed(&auth)) {
        return Err("unauthorized");
    }
    Ok(())
}
/// One GET or bounded maintenance POST per connection. No cookies, CORS or query strings,
/// redirects, caller-selected auth server, or path forwarding to native handlers.
#[cfg(test)]
fn request(stream: &mut TcpStream) -> std::result::Result<(String, String), &'static str> {
    let (method, path, token, _) = request_full(stream)?;
    if method != "GET" {
        return Err("invalid_request");
    }
    Ok((path, token))
}
fn request_full(
    stream: &mut TcpStream,
) -> std::result::Result<(String, String, String, Value), &'static str> {
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
        || !matches!(parts[0], "GET" | "POST")
        || (parts[0] == "POST"
            && !matches!(
                parts[1],
                "/v1/maintenance/prepare" | "/v1/maintenance/confirm"
            ))
        || parts[2] != "HTTP/1.1"
        || parts[1].len() > 128
        || parts[1].contains(['?', '%', '#'])
    {
        return Err("invalid_request");
    }
    let mut token = None;
    let mut length = None;
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
            "content-length" if parts[0] == "POST" && length.is_none() => {
                length = Some(
                    value
                        .trim()
                        .parse::<usize>()
                        .ok()
                        .filter(|n| *n > 0 && *n <= 4096)
                        .ok_or("request_limit")?,
                );
            }
            "cookie" | "origin" | "transfer-encoding" | "content-length" => {
                return Err("invalid_request")
            }
            _ => {}
        }
    }
    let token = token.ok_or("unauthorized")?;
    let body = if parts[0] == "POST" {
        if !matches!(
            parts[1],
            "/v1/maintenance/prepare" | "/v1/maintenance/confirm"
        ) {
            return Err("not_found");
        }
        let mut body = vec![0; length.ok_or("invalid_request")?];
        reader
            .read_exact(&mut body)
            .map_err(|_| "invalid_request")?;
        if deadline.elapsed() > Duration::from_secs(2) {
            return Err("request_limit");
        }
        serde_json::from_slice(&body).map_err(|_| "invalid_request")?
    } else {
        Value::Null
    };
    Ok((parts[0].into(), parts[1].into(), token, body))
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
    serve_with(base, port, settings, |_, _, _| {
        Err("surface_unavailable".into())
    })
}
pub fn serve_with(
    base: &str,
    port: u16,
    settings: fn() -> Value,
    backend: crate::remote_maintenance::Backend,
) -> Result<()> {
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
    let mut confirmations = crate::remote_maintenance::Confirmations::default();
    for stream in listener.incoming() {
        let Ok(mut stream) = stream else {
            continue;
        };
        rate.refresh();
        let mut session = String::new();
        let mut operation = "request";
        let result = (|| {
            let (method, path, token, input) = request_full(&mut stream)?;
            session = fingerprint(&token);
            operation = if method == "POST" {
                "maintenance.write"
            } else if crate::remote_maintenance::SURFACES
                .iter()
                .any(|s| path == format!("/v1/{s}"))
            {
                "maintenance.read"
            } else if path.starts_with("/v1/gallery") {
                "gallery.read"
            } else if path.starts_with("/v1/documents") {
                "documents.read"
            } else {
                "session.read"
            };
            if operation.starts_with("maintenance.") {
                maintenance_dispatch(
                    &mut rate,
                    &mut confirmations,
                    &root(),
                    &method,
                    &path,
                    &token,
                    &input,
                    settings,
                    |token| authenticate_permission(&client, base, token, method == "POST"),
                    backend,
                )
            } else {
                rated_dispatch(&mut rate, &root(), &path, &token, settings, |token| {
                    authenticate(&client, base, token)
                })
            }
        })();
        let code = match &result {
            Ok(_) => 200,
            Err("unauthorized") => 401,
            Err("consent_required") => 403,
            Err("not_found") => 404,
            Err("stale_report" | "already_claimed") => 409,
            Err("rate_limit" | "confirmation_limit") => 429,
            Err(
                "authentication_unavailable"
                | "store_busy"
                | "store_unavailable"
                | "store_limit"
                | "surface_unavailable"
                | "report_limit",
            ) => 503,
            Err(_) => 400,
        };
        // Only fixed operation/outcome and a one-way opaque fingerprint. No IDs, URLs or content.
        // Bound audit volume independently of either request budget.
        rate.audited = rate.audited.saturating_add(1);
        if rate.audited <= 121 {
            let outcome = match &result {
                Ok(value) if value["outcome"] == "unknown" => "unknown",
                Ok(_) => "ok",
                Err(error) => *error,
            };
            eprintln!(
                "{}",
                json!({"surfaceAudit":1,"operation":operation,"session":session,"outcome":outcome})
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
    fn enabled() -> Value {
        json!({"remoteDocumentsEnabled":true,"remoteDocumentsConsent":"epoch"})
    }
    fn call(
        rate: &mut Rate,
        token: &str,
        auth: impl FnMut(&str) -> std::result::Result<(), &'static str>,
    ) -> Reply {
        // Session metadata needs no published store, so both live auth checks run.
        rated_dispatch(
            rate,
            Path::new("/not-a-store"),
            "/v1/session",
            token,
            enabled,
            auth,
        )
    }
    #[test]
    fn missing_bearer_flood_leaves_authenticated_budget_untouched() {
        let mut rate = Rate::new();
        for _ in 0..120 {
            assert_eq!(
                call(&mut rate, "", |_| panic!("missing bearer reached T3")),
                Err("unauthorized")
            );
        }
        assert_eq!(rate.total, 0);
        assert_eq!(rate.failures, 0);
        assert!(rate.sessions.is_empty());
        let mut calls = 0;
        assert!(call(&mut rate, "valid", |_| {
            calls += 1;
            Ok(())
        })
        .is_ok());
        assert_eq!(calls, 2);
        assert_eq!(rate.total, 1);
    }
    #[test]
    fn random_bearers_are_bounded_and_known_good_still_revalidates() {
        let mut rate = Rate::new();
        assert!(call(&mut rate, "valid", |_| Ok(())).is_ok());
        let mut calls = 0;
        for i in 0..120 {
            let result = call(&mut rate, &format!("random-{i}"), |_| {
                calls += 1;
                Err("unauthorized")
            });
            assert_eq!(
                result,
                Err(if i < 8 { "unauthorized" } else { "rate_limit" })
            );
        }
        assert_eq!(calls, 8);
        assert_eq!(rate.sessions.len(), 1);
        assert_eq!(rate.known_good.len(), 1);
        assert!(call(&mut rate, "valid", |_| {
            calls += 1;
            Ok(())
        })
        .is_ok());
        assert_eq!(calls, 10);
        assert_eq!(
            call(&mut rate, "valid", |_| {
                calls += 1;
                Err("unauthorized")
            }),
            Err("unauthorized")
        );
        assert_eq!(calls, 11);
        assert!(!rate.known_good.contains(&fingerprint("valid")));
        assert_eq!(
            call(&mut rate, "valid", |_| panic!(
                "revoked fingerprint bypassed gate"
            )),
            Err("rate_limit")
        );
    }
    #[test]
    fn known_good_revocation_during_read_is_denied() {
        let mut rate = Rate::new();
        assert!(call(&mut rate, "valid", |_| Ok(())).is_ok());
        let mut calls = 0;
        assert_eq!(
            call(&mut rate, "valid", |_| {
                calls += 1;
                if calls == 2 {
                    Err("unauthorized")
                } else {
                    Ok(())
                }
            }),
            Err("unauthorized")
        );
        assert_eq!(calls, 2);
        assert!(rate.known_good.is_empty());
    }
    #[test]
    fn authenticated_session_and_global_limits_bound_all_state_and_reset() {
        let mut rate = Rate::new();
        for _ in 0..30 {
            assert!(call(&mut rate, "valid", |_| Ok(())).is_ok());
        }
        assert_eq!(
            call(&mut rate, "valid", |_| panic!(
                "over session limit reached T3"
            )),
            Err("rate_limit")
        );
        for i in 0..90 {
            assert!(call(&mut rate, &format!("valid-{i}"), |_| Ok(())).is_ok());
        }
        for i in 90..210 {
            assert_eq!(
                call(&mut rate, &format!("valid-{i}"), |_| panic!(
                    "over global limit reached T3"
                )),
                Err("rate_limit")
            );
        }
        assert_eq!(rate.total, 120);
        assert_eq!(rate.sessions.len(), 91);
        assert_eq!(rate.known_good.len(), 91);
        rate.failures = 8;
        rate.window = Instant::now() - Duration::from_secs(61);
        assert!(call(&mut rate, "new-window", |_| Ok(())).is_ok());
        assert_eq!(rate.failures, 0);
        assert_eq!(rate.sessions.len(), 1);
        assert_eq!(rate.known_good.len(), 1);
    }
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

/// Serialize the full stale-check/claim/apply boundary across broker processes.
pub(crate) fn operation_lock(dir: &Path) -> std::result::Result<fs::File, &'static str> {
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
        .open(dir.join("maintenance-operation.lock"))
        .map_err(|_| "store_unavailable")?;
    if !lock.metadata().is_ok_and(|m| m.is_file()) {
        return Err("store_unavailable");
    }
    if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return Err("store_busy");
    }
    Ok(lock)
}
/// Separate durable, bounded claims; a crash after this write is an unknown outcome.
pub(crate) fn claim(dir: &Path, id: &str) -> std::result::Result<(), &'static str> {
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
        .open(dir.join("maintenance.lock"))
        .map_err(|_| "store_unavailable")?;
    if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return Err("store_busy");
    }
    let path = dir.join("maintenance-claims.json");
    let mut claims: Vec<String> = match fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(&path)
    {
        Ok(f) => {
            if !f.metadata().is_ok_and(|m| m.is_file() && m.len() <= 65536) {
                return Err("store_limit");
            }
            let mut bytes = Vec::new();
            f.take(65537)
                .read_to_end(&mut bytes)
                .map_err(|_| "store_unavailable")?;
            if bytes.len() > 65536 {
                return Err("store_limit");
            }
            serde_json::from_slice(&bytes).map_err(|_| "store_unavailable")?
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(_) => return Err("store_unavailable"),
    };
    if claims.iter().any(|s| s == id) {
        return Err("already_claimed");
    }
    if claims.len() >= 128 || claims.iter().any(|s| !opaque(s)) {
        return Err("store_limit");
    }
    claims.push(id.into());
    let bytes = serde_json::to_vec(&claims).map_err(|_| "store_unavailable")?;
    let tmp = dir.join(format!(".{}", uuid::Uuid::new_v4()));
    let result = (|| -> std::io::Result<()> {
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&tmp)?;
        f.write_all(&bytes)?;
        f.sync_all()?;
        fs::rename(&tmp, &path)?;
        fs::File::open(dir)?.sync_all()
    })();
    let _ = fs::remove_file(tmp);
    result.map_err(|_| "store_unavailable")
}
#[allow(clippy::too_many_arguments)]
fn maintenance_dispatch(
    rate: &mut Rate,
    confirmations: &mut crate::remote_maintenance::Confirmations,
    dir: &Path,
    method: &str,
    path: &str,
    token: &str,
    input: &Value,
    mut settings: impl FnMut() -> Value,
    mut auth: impl FnMut(&str) -> std::result::Result<(), &'static str>,
    backend: crate::remote_maintenance::Backend,
) -> Reply {
    // Share the existing auth flood gate, session/global admission and live checks.
    rated_dispatch(rate, dir, "/v1/session", token, &mut settings, &mut auth)?;
    let before = settings();
    if method == "POST" && crate::remote_maintenance::epoch(&before).is_none() {
        return Err("consent_required");
    }
    let surface = path.strip_prefix("/v1/").ok_or("not_found")?;
    let first = if method == "GET" {
        Some(crate::remote_maintenance::report(
            backend, surface, &before, token,
        )?)
    } else {
        None
    };
    if let Err(error) = auth(token) {
        rate.known_good.remove(&fingerprint(token));
        rate.failures = rate.failures.saturating_add(1);
        return Err(error);
    }
    let after = settings();
    if consent(&before) != consent(&after)
        || consent(&after).is_none()
        || crate::remote_maintenance::epoch(&before) != crate::remote_maintenance::epoch(&after)
    {
        return Err("consent_required");
    }
    let session = fingerprint(token);
    match (method, path) {
        ("POST", "/v1/maintenance/prepare") => {
            confirmations.prepare(backend, &after, &session, token, input)
        }
        ("POST", "/v1/maintenance/confirm") => {
            confirmations.confirm_checked(dir, backend, &after, &session, token, input, || {
                if let Err(error) = auth(token) {
                    rate.known_good.remove(&session);
                    rate.failures = rate.failures.saturating_add(1);
                    return Err(error);
                }
                if settings() != after {
                    return Err("stale_report");
                }
                Ok(())
            })
        }
        ("GET", _) => {
            let last = crate::remote_maintenance::report(backend, surface, &after, token)?;
            if first.as_ref() != Some(&last) {
                return Err("stale_report");
            }
            Ok(last)
        }
        _ => Err("not_found"),
    }
}

#[cfg(test)]
mod maintenance_ingress_tests {
    use super::*;
    fn settings() -> Value {
        json!({"remoteDocumentsEnabled":true,"remoteDocumentsConsent":"r","remoteMaintenanceEnabled":true,"remoteMaintenanceConsent":"w"})
    }
    fn backend(_: &Value, _: &Value, _: &str) -> Result<Value> {
        Ok(json!({"status":"idle"}))
    }
    fn invoke(
        rate: &mut Rate,
        method: &str,
        settings: impl FnMut() -> Value,
        auth: impl FnMut(&str) -> std::result::Result<(), &'static str>,
    ) -> Reply {
        maintenance_dispatch(
            rate,
            &mut crate::remote_maintenance::Confirmations::default(),
            Path::new("/unavailable"),
            method,
            if method == "GET" {
                "/v1/memory"
            } else {
                "/v1/maintenance/prepare"
            },
            "phone",
            &json!({}),
            settings,
            auth,
            backend,
        )
    }
    #[test]
    fn denied_and_revoked_requests_never_reach_backend() {
        let mut rate = Rate::new();
        assert_eq!(
            invoke(
                &mut rate,
                "GET",
                || panic!("denied settings read"),
                |_| Err("unauthorized")
            ),
            Err("unauthorized")
        );
        let mut calls = 0;
        assert_eq!(
            invoke(&mut rate, "GET", settings, |_| {
                calls += 1;
                if calls == 3 {
                    Err("unauthorized")
                } else {
                    Ok(())
                }
            }),
            Err("unauthorized")
        );
        assert!(!rate.known_good.contains(&fingerprint("phone")));
    }
    #[test]
    fn maintenance_shares_existing_session_budget() {
        let mut rate = Rate::new();
        for _ in 0..30 {
            assert!(invoke(&mut rate, "GET", settings, |_| Ok(())).is_ok())
        }
        assert_eq!(
            invoke(&mut rate, "POST", settings, |_| panic!("over-limit auth")),
            Err("rate_limit")
        );
    }
    #[test]
    fn disabling_owner_consent_during_report_denies_response() {
        let mut calls = 0;
        let result = invoke(
            &mut Rate::new(),
            "GET",
            || {
                calls += 1;
                let mut s = settings();
                if calls >= 4 {
                    s["remoteDocumentsEnabled"] = json!(false)
                }
                s
            },
            |_| Ok(()),
        );
        assert_eq!(result, Err("consent_required"));
    }
    #[test]
    fn default_off_write_consent_denies_mutations() {
        assert_eq!(
            invoke(
                &mut Rate::new(),
                "POST",
                || json!({"remoteDocumentsEnabled":true,"remoteDocumentsConsent":"r"}),
                |_| Ok(())
            ),
            Err("consent_required")
        );
    }
}
