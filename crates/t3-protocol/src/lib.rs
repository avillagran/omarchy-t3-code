//! Portable T3 Code protocol. No filesystem, processes, GUI, keyring or Python.
//! Call blocking methods on a worker thread. Hosts own account login and key storage.
use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64, Engine};
use p256::ecdsa::{signature::Signer, Signature, SigningKey};
use reqwest::{
    blocking::{Client as Http, Response},
    Url,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    io::Read,
    net::{TcpStream, ToSocketAddrs},
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tungstenite::{client::IntoClientRequest, Message};
use zeroize::Zeroizing;
pub mod ffi;
pub mod activity;
pub mod models;
pub mod sidebar;
pub mod compaction;
pub mod changes;
pub mod composer;
pub mod queue;
pub mod questions;
pub mod pull_requests;
pub mod documents;
pub mod projects;
pub mod tasks;
pub mod i18n;
pub mod stream;
pub mod projection;

pub type Result<T> = std::result::Result<T, String>;
const MAX_BYTES: u64 = 8 * 1024 * 1024;
const GRANT: &str = "urn:ietf:params:oauth:grant-type:token-exchange";
const ACCESS: &str = "urn:ietf:params:oauth:token-type:access_token";
fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
fn field<'a>(v: &'a Value, k: &str) -> Result<&'a str> {
    v[k].as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "malformed".into())
}
fn token(s: &str) -> Result<()> {
    if s.len() < 8 || s.chars().any(|c| c.is_whitespace() || c.is_control()) {
        Err("unauthorized".into())
    } else {
        Ok(())
    }
}
fn http_url(s: &str) -> Result<Url> {
    let u = Url::parse(s).map_err(|_| "invalid_endpoint")?;
    if !matches!(u.scheme(), "https" | "http")
        || u.host_str().is_none()
        || u.port() == Some(0)
        || !u.username().is_empty()
        || u.password().is_some()
        || u.query().is_some()
        || u.fragment().is_some()
    {
        return Err("invalid_endpoint".into());
    }
    Ok(u)
}
fn endpoint(base: &Url, path: &str) -> Result<Url> {
    if !path.starts_with('/') || path.starts_with("//") || path.contains(['#', '\\']) {
        return Err("invalid_endpoint".into());
    }
    let u = base.join(path).map_err(|_| "invalid_endpoint")?;
    if u.origin() != base.origin() {
        return Err("invalid_endpoint".into());
    }
    Ok(u)
}
fn http() -> Result<Http> {
    Http::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(12))
        .build()
        .map_err(|_| "local_error".into())
}
fn decode(response: Response) -> Result<Value> {
    let code = response.status().as_u16();
    if code == 401 || code == 403 {
        return Err("unauthorized".into());
    }
    if !(200..300).contains(&code) {
        return Err(format!("http_{code}"));
    }
    if response
        .headers()
        .get("content-type")
        .and_then(|s| s.to_str().ok())
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        != "application/json"
    {
        return Err("not_json".into());
    }
    if !matches!(
        response
            .headers()
            .get("content-encoding")
            .and_then(|s| s.to_str().ok()),
        None | Some("identity") | Some("")
    ) {
        return Err("unsupported_encoding".into());
    }
    let mut bytes = Zeroizing::new(Vec::new());
    response
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "unreachable")?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("too_large".into());
    }
    serde_json::from_slice(&bytes).map_err(|_| "malformed".into())
}

/// Private material never implements Debug or Serialize. Persist only via the host's secure store.
pub struct DpopKey(SigningKey);
impl DpopKey {
    pub fn generate() -> Self {
        Self(SigningKey::random(&mut rand_core::OsRng))
    }
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        SigningKey::from_slice(bytes)
            .map(Self)
            .map_err(|_| "invalid_dpop_key".into())
    }
    pub fn secret_bytes(&self) -> Zeroizing<Vec<u8>> {
        Zeroizing::new(self.0.to_bytes().to_vec())
    }
    pub fn public_jwk(&self) -> Value {
        let p = self.0.verifying_key().to_encoded_point(false);
        json!({"crv":"P-256","kty":"EC","x":B64.encode(p.x().unwrap()),"y":B64.encode(p.y().unwrap())})
    }
    pub fn thumbprint(&self) -> String {
        B64.encode(Sha256::digest(self.public_jwk().to_string().as_bytes()))
    }
    pub fn proof(&self, method: &str, url: &str, access_token: Option<&str>) -> Result<String> {
        let mut u = http_url_without_query(url)?;
        u.set_query(None);
        u.set_fragment(None);
        let header =
            B64.encode(json!({"typ":"dpop+jwt","alg":"ES256","jwk":self.public_jwk()}).to_string());
        let mut claims = json!({"htm":method.to_ascii_uppercase(),"htu":u.as_str(),"jti":uuid::Uuid::new_v4().to_string(),"iat":now()});
        if let Some(t) = access_token {
            claims["ath"] = json!(B64.encode(Sha256::digest(t.as_bytes())))
        }
        let input = format!("{header}.{}", B64.encode(claims.to_string()));
        let sig: Signature = self.0.sign(input.as_bytes());
        Ok(format!("{input}.{}", B64.encode(sig.to_bytes())))
    }
}
fn http_url_without_query(s: &str) -> Result<Url> {
    let mut u = Url::parse(s).map_err(|_| "invalid_endpoint")?;
    u.set_query(None);
    u.set_fragment(None);
    http_url(u.as_str())
}

#[derive(Clone)]
pub enum Credential {
    Bearer(Zeroizing<String>),
    Dpop {
        access_token: Zeroizing<String>,
        key: Arc<DpopKey>,
        expires_at: i64,
    },
}
impl Credential {
    pub fn bearer(value: &str) -> Result<Self> {
        token(value)?;
        Ok(Self::Bearer(Zeroizing::new(value.into())))
    }
    pub fn dpop(value: &str, key: Arc<DpopKey>, expires_at: i64) -> Result<Self> {
        token(value)?;
        if expires_at <= now() {
            return Err("session_expired".into());
        }
        Ok(Self::Dpop {
            access_token: Zeroizing::new(value.into()),
            key,
            expires_at,
        })
    }
    pub fn expires_at(&self) -> Option<i64> {
        match self {
            Self::Dpop { expires_at, .. } => Some(*expires_at),
            _ => None,
        }
    }
    pub fn headers(&self, method: &str, url: &str) -> Result<Vec<(String, String)>> {
        match self {
            Self::Bearer(t) => Ok(vec![(
                "authorization".into(),
                format!("Bearer {}", t.as_str()),
            )]),
            Self::Dpop {
                access_token,
                key,
                expires_at,
            } => {
                if *expires_at <= now() {
                    return Err("session_expired".into());
                }
                Ok(vec![
                    (
                        "authorization".into(),
                        format!("DPoP {}", access_token.as_str()),
                    ),
                    ("dpop".into(), key.proof(method, url, Some(access_token))?),
                ])
            }
        }
    }
}

/// A connection is independent of desktop discovery. Share it between host workers.
#[derive(Clone)]
pub struct Connection {
    http: Http,
    base: Url,
    ws: Url,
    credential: Credential,
}
impl Connection {
    pub fn new(http_base: &str, ws_base: Option<&str>, credential: Credential) -> Result<Self> {
        Self::with_http_client(http()?, http_base, ws_base, credential)
    }
    /// Allows a host to reuse its HTTP pool. The host must disable redirects.
    pub fn with_http_client(
        http: Http,
        http_base: &str,
        ws_base: Option<&str>,
        credential: Credential,
    ) -> Result<Self> {
        let base = http_url(http_base)?;
        if base.path() != "/" {
            return Err("invalid_endpoint".into());
        }
        let mut ws = if let Some(w) = ws_base {
            Url::parse(w).map_err(|_| "invalid_endpoint")?
        } else {
            base.clone()
        };
        if ws_base.is_none() {
            ws.set_scheme(if base.scheme() == "https" {
                "wss"
            } else {
                "ws"
            })
            .map_err(|_| "invalid_endpoint")?;
        }
        let mut check = ws.clone();
        check
            .set_scheme(match ws.scheme() {
                "ws" => "http",
                "wss" => "https",
                _ => return Err("invalid_endpoint".into()),
            })
            .map_err(|_| "invalid_endpoint")?;
        http_url(check.as_str())?;
        if check.origin() != base.origin() || check.path() != "/" {
            return Err("invalid_endpoint".into());
        }
        Ok(Self {
            http,
            base,
            ws,
            credential,
        })
    }
    pub fn base_url(&self) -> &str {
        self.base.as_str()
    }
    pub fn expires_at(&self) -> Option<i64> {
        self.credential.expires_at()
    }
    /// Streams an attachment from a host-owned reader (file, Android resolver, etc.).
    /// Uses T3's RPC grant and same-origin signed POST without buffering the file.
    pub fn upload_attachment(
        &self,
        name: &str,
        mime: &str,
        size: u64,
        reader: impl Read + Send + 'static,
    ) -> Result<Value> {
        if size == 0 || size > 50 * 1024 * 1024 {
            return Err("file_limit_50MiB".into());
        }
        if name.is_empty()
            || name.chars().count() > 255
            || mime.is_empty()
            || mime.len() > 100
            || mime.chars().any(|c| c.is_control())
        {
            return Err("invalid_file".into());
        }
        let image = matches!(
            mime,
            "image/png" | "image/jpeg" | "image/gif" | "image/webp"
        );
        if image && size > 10 * 1024 * 1024 {
            return Err("image_limit_10MiB".into());
        }
        let typ = if image { "image" } else { "file" };
        let grant = self.rpc(
            "attachments.createUploadUrl",
            json!({"type":typ,"name":name,"mimeType":mime,"sizeBytes":size}),
        )?;
        let relative = field(&grant, "relativeUrl")?;
        if !relative.starts_with("/api/attachments/upload/") || relative.contains(['?', '#', '\\'])
        {
            return Err("invalid_upload_url".into());
        }
        let target = endpoint(&self.base, relative).map_err(|_| "invalid_upload_url")?;
        if !target.path().starts_with("/api/attachments/upload/") {
            return Err("invalid_upload_url".into());
        }
        let response = self
            .http
            .post(target)
            .timeout(Duration::from_secs(90))
            .header("Content-Type", mime)
            .body(reqwest::blocking::Body::sized(reader.take(size), size))
            .send()
            .map_err(|_| "upload_failed")?;
        if !response.status().is_success() {
            return Err(format!("upload_http_{}", response.status().as_u16()));
        }
        Ok(
            json!({"type":typ,"id":grant["attachmentId"],"name":name,"mimeType":mime,"sizeBytes":size}),
        )
    }
    pub fn request_json(
        &self,
        method: &str,
        path: &str,
        body: Option<&Value>,
        protocol: bool,
    ) -> Result<Value> {
        let url = endpoint(&self.base, path)?;
        let method =
            reqwest::Method::from_bytes(method.as_bytes()).map_err(|_| "invalid_method")?;
        let mut req = self
            .http
            .request(method.clone(), url.clone())
            .header("Accept", "application/json")
            .header("Accept-Encoding", "identity");
        for (name, value) in self.credential.headers(method.as_str(), url.as_str())? {
            req = req.header(name, value);
        }
        if protocol {
            req = req.header("x-t3-orchestration-protocol", "2");
        }
        if let Some(b) = body {
            req = req.json(b);
        }
        decode(req.send().map_err(|_| "unreachable")?)
    }
    fn websocket(&self)->Result<tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<TcpStream>>>{
        let mut url = self.ws.clone();
        url.set_path("/ws");
        url.set_query(Some("orchestrationProtocol=2"));
        let mut bearer_headers = vec![];
        match &self.credential {
            Credential::Dpop { .. } => {
                let ticket =
                    self.request_json("POST", "/api/auth/websocket-ticket", None, false)?;
                url.query_pairs_mut()
                    .append_pair("wsTicket", field(&ticket, "ticket")?);
            }
            Credential::Bearer(_) => {
                bearer_headers = self.credential.headers("GET", self.base.as_str())?;
            }
        }
        let mut req = url
            .as_str()
            .into_client_request()
            .map_err(|_| "invalid_endpoint")?;
        for (name, value) in bearer_headers {
            req.headers_mut().insert(
                tungstenite::http::HeaderName::from_bytes(name.as_bytes())
                    .map_err(|_| "unauthorized")?,
                value.parse().map_err(|_| "unauthorized")?,
            );
        }
        let host = url.host_str().ok_or("invalid_endpoint")?;
        let port = url.port_or_known_default().ok_or("invalid_endpoint")?;
        let deadline = Instant::now() + Duration::from_secs(8);
        let mut stream = None;
        for addr in (host, port).to_socket_addrs().map_err(|_| "disconnected")? {
            let Some(left) = deadline.checked_duration_since(Instant::now()) else {
                break;
            };
            if let Ok(s) = TcpStream::connect_timeout(&addr, left) {
                stream = Some(s);
                break;
            }
        }
        let stream = stream.ok_or("disconnected")?;
        stream
            .set_read_timeout(Some(Duration::from_secs(12)))
            .map_err(|_| "disconnected")?;
        stream
            .set_write_timeout(Some(Duration::from_secs(12)))
            .map_err(|_| "disconnected")?;
        let config = tungstenite::protocol::WebSocketConfig::default()
            .max_message_size(Some(MAX_BYTES as usize))
            .max_frame_size(Some(MAX_BYTES as usize));
        let (ws, _) = tungstenite::client_tls_with_config(req, stream, Some(config), None)
            .map_err(|_| "rpc_connection_failed")?;
        Ok(ws)
    }
    pub fn rpc(&self, tag: &str, payload: Value) -> Result<Value> {
        let mut ws=self.websocket()?;
        ws.send(Message::Text(
            json!({"_tag":"Request","id":"1","tag":tag,"payload":payload,"headers":[]})
                .to_string()
                .into(),
        ))
        .map_err(|_| "rpc_outcome_unknown")?;
        let until = Instant::now() + Duration::from_secs(20);
        while Instant::now() < until {
            match ws.read().map_err(|_| "rpc_outcome_unknown")? {
                Message::Text(t) => {
                    let v: Value = serde_json::from_str(&t).map_err(|_| "rpc_outcome_unknown")?;
                    if v["_tag"] == "Exit" && v["requestId"] == "1" {
                        let _ = ws.close(None);
                        if v["exit"]["_tag"] == "Success" {
                            return Ok(v["exit"]["value"].clone());
                        }
                        if v["exit"]["_tag"] != "Failure" {
                            return Err("rpc_outcome_unknown".into());
                        }
                        let summary = v["exit"]["cause"].to_string();
                        return Err(if summary.contains("Auth")
                            || summary.contains("Forbidden")
                            || summary.contains("scope")
                        {
                            "write_not_authorized"
                        } else {
                            "rpc_rejected"
                        }
                        .into());
                    }
                }
                Message::Ping(p) => ws
                    .send(Message::Pong(p))
                    .map_err(|_| "rpc_outcome_unknown")?,
                Message::Close(_) => return Err("rpc_outcome_unknown".into()),
                _ => {}
            }
        }
        Err("rpc_outcome_unknown".into())
    }
}

/// Complete native T3 Connect flow. The host supplies its existing cloud JWT.
/// This does not borrow a browser's non-exportable proof key or its bound token.
pub fn connect_relay(
    relay_url: &str,
    cloud_jwt: &str,
    client_id: &str,
    environment_id: &str,
    key: Arc<DpopKey>,
) -> Result<Connection> {
    uuid::Uuid::parse_str(environment_id).map_err(|_| "invalid_environment_id")?;
    RelayClient::new(relay_url, cloud_jwt, client_id, key)?.connect_environment(environment_id)
}

/// Authenticated relay client. Reuse for multiple environments while the session is valid.
pub struct RelayClient {
    connection: Connection,
    key: Arc<DpopKey>,
}
impl RelayClient {
    pub fn new(
        relay_url: &str,
        cloud_jwt: &str,
        client_id: &str,
        key: Arc<DpopKey>,
    ) -> Result<Self> {
        token(cloud_jwt)?;
        if !matches!(client_id, "t3-mobile" | "t3-web") {
            return Err("invalid_client_id".into());
        }
        let relay = http_url(relay_url)?;
        let http = http()?;
        if relay.path() != "/"
            || (relay.scheme() != "https"
                && !matches!(
                    relay.host_str(),
                    Some("127.0.0.1" | "localhost" | "[::1]" | "::1")
                ))
        {
            return Err("invalid_endpoint".into());
        }
        let resource = relay.origin().ascii_serialization();
        let url = endpoint(&relay, "/v1/client/dpop-token")?;
        let response = decode(
            http.post(url.clone())
                .header("dpop", key.proof("POST", url.as_str(), None)?)
                .header("Accept-Encoding", "identity")
                .form(&[
                    ("grant_type", GRANT),
                    ("subject_token", cloud_jwt),
                    ("subject_token_type", "urn:ietf:params:oauth:token-type:jwt"),
                    ("requested_token_type", ACCESS),
                    ("resource", resource.as_str()),
                    ("scope", "environment:connect"),
                    ("client_id", client_id),
                ])
                .send()
                .map_err(|_| "unreachable")?,
        )?;
        if response["token_type"] != "DPoP"
            || response["scope"] != "environment:connect"
            || response["expires_in"]
                .as_i64()
                .is_none_or(|s| s <= 0 || s > 31536000)
        {
            return Err("invalid_relay_token".into());
        }
        let relay_token = Credential::dpop(
            field(&response, "access_token")?,
            key.clone(),
            now() + response["expires_in"].as_i64().unwrap(),
        )?;
        let relay_conn = Connection::new(relay_url, None, relay_token)?;
        Ok(Self {
            connection: relay_conn,
            key,
        })
    }
    pub fn connect_environment(&self, environment_id: &str) -> Result<Connection> {
        uuid::Uuid::parse_str(environment_id).map_err(|_| "invalid_environment_id")?;
        let key = self.key.clone();
        let http = &self.connection.http;
        let bootstrap = self
            .connection
            .request_json(
                "POST",
                &format!("/v1/environments/{environment_id}/connect"),
                Some(&json!({"clientProofKeyThumbprint":key.thumbprint()})),
                false,
            )
            .map_err(|e| {
                if e == "unauthorized" {
                    "relay_environment_not_authorized".into()
                } else {
                    e
                }
            })?;
        if bootstrap["environmentId"] != environment_id {
            return Err("t3_environment_mismatch".into());
        }
        let base = http_url(field(&bootstrap["endpoint"], "httpBaseUrl")?)?;
        let ws = field(&bootstrap["endpoint"], "wsBaseUrl")?;
        // Validate both endpoint origins before sending any bootstrap credential.
        Connection::new(
            base.as_str(),
            Some(ws),
            Credential::bearer("validation-only")?,
        )?;
        let descriptor = decode(
            http.get(endpoint(&base, "/.well-known/t3/environment")?)
                .header("Accept-Encoding", "identity")
                .send()
                .map_err(|_| "unreachable")?,
        )?;
        if descriptor["environmentId"] != environment_id {
            return Err("t3_environment_mismatch".into());
        }
        let url = endpoint(&base, "/oauth/token")?;
        let access = decode(
            http.post(url.clone())
                .header("dpop", key.proof("POST", url.as_str(), None)?)
                .header("Accept-Encoding", "identity")
                .form(&[
                    ("grant_type", GRANT),
                    ("subject_token", field(&bootstrap, "credential")?),
                    (
                        "subject_token_type",
                        "urn:t3:params:oauth:token-type:environment-bootstrap",
                    ),
                    ("requested_token_type", ACCESS),
                    ("scope", "orchestration:read orchestration:operate filesystem:read settings:write"),
                    ("client_label", "Omarchy/Omi"),
                ])
                .send()
                .map_err(|_| "unreachable")?,
        )
        .map_err(|e| {
            if e == "unauthorized" {
                "environment_bootstrap_rejected".into()
            } else {
                e
            }
        })?;
        if access["token_type"] != "DPoP"
            || access["expires_in"]
                .as_i64()
                .is_none_or(|s| s <= 0 || s > 31536000)
        {
            return Err("invalid_environment_token".into());
        }
        let scopes = field(&access, "scope")?
            .split_whitespace()
            .collect::<Vec<_>>();
        if !scopes.contains(&"orchestration:read") || !scopes.contains(&"orchestration:operate") {
            return Err("write_not_authorized".into());
        }
        Connection::new(
            base.as_str(),
            Some(ws),
            Credential::dpop(
                field(&access, "access_token")?,
                key,
                now() + access["expires_in"].as_i64().unwrap(),
            )?,
        )
    }
}

impl Connection {
 /// Read an image through a signed same-origin asset grant. No auth on the signed GET.
 pub fn attachment_image(&self,id:&str,name:&str,mime:&str)->Result<Vec<u8>>{
  if id.is_empty()||id.len()>256||!matches!(mime,"image/png"|"image/jpeg"|"image/gif"|"image/webp"){return Err("invalid_image".into())}
  let grant=self.rpc("assets.createUrl",json!({"resource":{"_tag":"attachment","attachmentId":id,"fileName":name,"mimeType":mime,"disposition":"inline"}}))?;
  let url=endpoint(&self.base,field(&grant,"relativeUrl")?)?;if !url.path().starts_with("/api/assets/"){return Err("invalid_asset_url".into())}
  let response=self.http.get(url).timeout(Duration::from_secs(30)).send().map_err(|_|"image_unreachable")?;if !response.status().is_success(){return Err("image_unavailable".into())}
  let mut bytes=vec![];response.take(10*1024*1024+1).read_to_end(&mut bytes).map_err(|_|"image_unavailable")?;
  if bytes.len()>10*1024*1024{return Err("image_limit_10MiB".into())}
  let valid=match mime{"image/png"=>bytes.starts_with(b"\x89PNG\r\n\x1a\n"),"image/jpeg"=>bytes.starts_with(b"\xff\xd8\xff"),"image/gif"=>bytes.starts_with(b"GIF87a")||bytes.starts_with(b"GIF89a"),"image/webp"=>bytes.starts_with(b"RIFF")&&bytes.get(8..12)==Some(b"WEBP"),_=>false};
  if !valid{return Err("invalid_image".into())}Ok(bytes)
 }
}

pub mod lifecycle;

pub mod settings;
