//! Environment discovery and Bearer pairing, before any credential exists.
//! Same transport policy as every other call: no proxy, no redirects,
//! same-origin paths only. The descriptor is read and its environment checked
//! before the pairing secret is sent.
use super::*;

const DESCRIPTOR: &str = "/.well-known/t3/environment";
const BOOTSTRAP: &str = "urn:t3:params:oauth:token-type:environment-bootstrap";
const DEVICE_TYPES: [&str; 5] = ["desktop", "mobile", "tablet", "bot", "unknown"];

/// A direct environment's base URL: http(s), origin only.
fn environment_base(http_base: &str) -> Result<Url> {
    let base = http_url(http_base)?;
    if base.path() != "/" {
        return Err("invalid_endpoint".into());
    }
    Ok(base)
}
/// The raw descriptor document, without credentials.
pub(crate) fn descriptor(http: &Http, base: &Url) -> Result<Value> {
    decode(
        http.get(endpoint(base, DESCRIPTOR)?)
            .header("Accept", "application/json")
            .header("Accept-Encoding", "identity")
            .send()
            .map_err(|_| "unreachable")?,
    )
}

/// `GET <base>/.well-known/t3/environment` with no credential and the default
/// policy. The descriptor must name a non-empty `environmentId` (else `malformed`).
pub fn fetch_descriptor(http_base: &str) -> Result<Value> {
    fetch_descriptor_with(http_base, &ConnectionOptions::default())
}
/// `fetch_descriptor` with the host's HTTP policy.
pub fn fetch_descriptor_with(http_base: &str, options: &ConnectionOptions) -> Result<Value> {
    let base = environment_base(http_base)?;
    let descriptor = descriptor(&options.build_http_client()?, &base)?;
    field(&descriptor, "environmentId")?;
    Ok(descriptor)
}
/// The descriptor, only if it names `environment_id`; else `t3_environment_mismatch`.
/// Run it before sending any credential to a forwarded or discovered endpoint.
pub fn verify_environment(
    http_base: &str,
    environment_id: &str,
    options: &ConnectionOptions,
) -> Result<Value> {
    let descriptor = fetch_descriptor_with(http_base, options)?;
    if descriptor["environmentId"] != environment_id {
        return Err("t3_environment_mismatch".into());
    }
    Ok(descriptor)
}

/// How the paired client presents itself in T3's session list. Every field is optional.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PairingClient {
    label: Option<String>,
    device_type: Option<String>,
    os: Option<String>,
}
impl PairingClient {
    pub fn with_label(mut self, label: &str) -> Self {
        self.label = Some(label.into());
        self
    }
    /// One of `desktop`, `mobile`, `tablet`, `bot`, `unknown`.
    pub fn with_device_type(mut self, device_type: &str) -> Self {
        self.device_type = Some(device_type.into());
        self
    }
    pub fn with_os(mut self, os: &str) -> Self {
        self.os = Some(os.into());
        self
    }
    fn validate(&self) -> Result<()> {
        let text = |s: &Option<String>| {
            s.as_deref().is_none_or(|s| {
                !s.trim().is_empty() && s.len() <= 256 && !s.chars().any(char::is_control)
            })
        };
        let device = self
            .device_type
            .as_deref()
            .is_none_or(|d| DEVICE_TYPES.contains(&d));
        if !text(&self.label) || !text(&self.os) || !device {
            return Err("invalid_pairing_client".into());
        }
        Ok(())
    }
}

/// A Bearer session issued for a pairing secret. Holds a secret: no Debug, no
/// Serialize. Persist `access_token` only in the host's secure store.
pub struct PairedBearer {
    pub access_token: Zeroizing<String>,
    pub environment_id: String,
    /// The descriptor checked before the exchange (label, platform, version, ...).
    pub descriptor: Value,
    pub scope: String,
    pub expires_in: Option<i64>,
    http_base: String,
}
impl PairedBearer {
    pub fn credential(&self) -> Result<Credential> {
        Credential::bearer(&self.access_token)
    }
    /// A connection to the paired environment with this session.
    pub fn connection(&self, options: &ConnectionOptions) -> Result<Connection> {
        Connection::with_options(&self.http_base, None, self.credential()?, options)
    }
}

/// Exchanges a pairing secret (the bootstrap credential of a pairing link) for
/// a Bearer session: `POST <base>/oauth/token`, form-encoded token exchange
/// with `subject_token_type=urn:t3:params:oauth:token-type:environment-bootstrap`.
///
/// The descriptor is read first. It must name a UUID environment
/// (`invalid_t3_environment_id`) equal to `expected_environment` when given
/// (`t3_environment_mismatch`); only then is the secret sent. A non-2xx answer
/// is `pairing_rejected`, a token other than Bearer `unsupported_pairing_token`.
pub fn pair_bearer(
    http_base: &str,
    pairing_secret: &str,
    expected_environment: Option<&str>,
    client: &PairingClient,
    options: &ConnectionOptions,
) -> Result<PairedBearer> {
    let secret = pairing_secret.trim();
    if secret.is_empty() || secret.len() > 4096 || secret.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err("invalid_pairing_code".into());
    }
    client.validate()?;
    let base = environment_base(http_base)?;
    let http = options.build_http_client()?;
    let descriptor = descriptor(&http, &base)?;
    let environment = field(&descriptor, "environmentId")?.to_string();
    uuid::Uuid::parse_str(&environment).map_err(|_| "invalid_t3_environment_id")?;
    if expected_environment.is_some_and(|e| !e.is_empty() && e != environment) {
        return Err("t3_environment_mismatch".into());
    }
    let mut form = vec![
        ("grant_type", GRANT),
        ("subject_token", secret),
        ("subject_token_type", BOOTSTRAP),
        ("requested_token_type", ACCESS),
    ];
    for (name, value) in [
        ("client_label", &client.label),
        ("client_device_type", &client.device_type),
        ("client_os", &client.os),
    ] {
        if let Some(v) = value {
            form.push((name, v.as_str()));
        }
    }
    let access = decode(
        http.post(endpoint(&base, "/oauth/token")?)
            .header("Accept", "application/json")
            .header("Accept-Encoding", "identity")
            .form(&form)
            .send()
            .map_err(|_| "unreachable")?,
    )
    .map_err(|e| {
        if e == "unauthorized" || e.starts_with("http_") {
            "pairing_rejected".into()
        } else {
            e
        }
    })?;
    if access["token_type"] != "Bearer" {
        return Err("unsupported_pairing_token".into());
    }
    let access_token = field(&access, "access_token")?;
    token(access_token).map_err(|_| "unsupported_pairing_token")?;
    Ok(PairedBearer {
        access_token: Zeroizing::new(access_token.into()),
        environment_id: environment,
        descriptor,
        scope: access["scope"].as_str().unwrap_or("").into(),
        expires_in: access["expires_in"].as_i64(),
        http_base: base.as_str().into(),
    })
}

impl Connection {
    /// Lists this environment's client sessions (`GET /api/auth/clients`), lets
    /// the host's policy pick the ones to retire, and revokes each with
    /// `POST /api/auth/clients/revoke` `{"sessionId"}`. Returns how many
    /// revocations succeeded; one failed revocation does not stop the others.
    /// IDs that are empty or longer than 128 bytes are skipped.
    pub fn revoke_stale_sessions(&self, select: impl FnOnce(&Value) -> Vec<String>) -> Result<usize> {
        let clients = self.request_json("GET", "/api/auth/clients", None, false)?;
        let mut revoked = 0;
        for id in select(&clients) {
            if id.is_empty() || id.len() > 128 {
                continue;
            }
            let body = json!({"sessionId":id});
            if self
                .request_ok("POST", "/api/auth/clients/revoke", Some(&body), false)
                .is_ok()
            {
                revoked += 1;
            }
        }
        Ok(revoked)
    }
}
