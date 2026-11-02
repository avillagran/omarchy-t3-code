//! Signed asset reads: an `assets.createUrl` grant over RPC, then an
//! unauthenticated GET of its `relativeUrl`, same origin, under `/api/assets/`.
//! The grant authorizes the GET, so no credential header is sent with it.
use super::*;

/// Largest asset any call may read.
pub const MAX_ASSET_BYTES: u64 = 64 * 1024 * 1024;
const FAVICON_MAX_BYTES: u64 = 2 * 1024 * 1024;
const FAVICON_MISSING: &str = "/project-favicon-missing";

/// Bytes of a signed asset and the media type the server served (lowercase, no parameters).
pub struct AssetBytes {
    pub bytes: Vec<u8>,
    pub mime: String,
}

/// Limit and deadline of one signed GET.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetOptions {
    limit: u64,
    timeout: Duration,
}
impl AssetOptions {
    /// At most `limit` bytes (1 byte to 64 MiB), with a 30 s deadline.
    pub fn new(limit: u64) -> Self {
        Self {
            limit,
            timeout: Duration::from_secs(30),
        }
    }
    /// Deadline of the GET itself, up to 300 s. The grant RPC keeps `rpc`'s.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
    pub fn limit(&self) -> u64 {
        self.limit
    }
    pub fn timeout(&self) -> Duration {
        self.timeout
    }
}

/// Error codes of one family of signed reads.
pub(crate) struct Codes {
    unreachable: &'static str,
    unavailable: &'static str,
    too_large: &'static str,
}
const ASSET: Codes = Codes {
    unreachable: "asset_unreachable",
    unavailable: "asset_unavailable",
    too_large: "asset_too_large",
};
pub(crate) const IMAGE: Codes = Codes {
    unreachable: "image_unreachable",
    unavailable: "image_unavailable",
    too_large: "image_limit_10MiB",
};

/// The `assets.createUrl` resource of an attachment. `inline` asks for
/// `disposition: inline` (display); without it the server picks its default.
pub fn attachment_resource(id: &str, name: &str, mime: &str, inline: bool) -> Result<Value> {
    if id.is_empty() || id.len() > 256 || name.len() > 512 || mime.len() > 128 {
        return Err("invalid_attachment".into());
    }
    let mut resource = json!({"_tag":"attachment","attachmentId":id,"fileName":name,"mimeType":mime});
    if inline {
        resource["disposition"] = json!("inline");
    }
    Ok(resource)
}

impl Connection {
    /// Any signed asset, at most `limit` bytes, with a 30 s GET deadline.
    ///
    /// Errors: the grant RPC's own, `invalid_asset_url` (missing, foreign or not
    /// under `/api/assets/`), `asset_unreachable`, `asset_unavailable` (non-2xx or
    /// broken body), `asset_too_large`, `invalid_asset_limit`.
    pub fn asset_bytes(&self, resource: Value, limit: u64) -> Result<AssetBytes> {
        self.asset_bytes_with(resource, &AssetOptions::new(limit))
    }
    /// `asset_bytes` with an explicit deadline.
    pub fn asset_bytes_with(&self, resource: Value, options: &AssetOptions) -> Result<AssetBytes> {
        if !(1..=MAX_ASSET_BYTES).contains(&options.limit) {
            return Err("invalid_asset_limit".into());
        }
        if options.timeout.is_zero() || options.timeout > Duration::from_secs(300) {
            return Err("invalid_asset_timeout".into());
        }
        let url = self.asset_url(resource)?;
        self.signed_get(url, options, &ASSET)
    }
    /// A project's favicon, at most 2 MiB within 3 s. `Ok(None)` when the server
    /// says the project has none; the missing-favicon URL is never fetched.
    /// `path` is the projection's `faviconPath`, a cache hint the server re-checks.
    pub fn project_favicon(&self, cwd: &str, path: Option<&str>) -> Result<Option<AssetBytes>> {
        let valid = |s: &str| !s.trim().is_empty() && s.len() <= 1024 && !s.chars().any(char::is_control);
        if !valid(cwd) || path.is_some_and(|p| !valid(p)) {
            return Err("invalid_asset_resource".into());
        }
        let mut resource = json!({"_tag":"project-favicon","cwd":cwd});
        if let Some(p) = path {
            resource["path"] = json!(p);
        }
        let url = self.asset_url(resource)?;
        if url.path().ends_with(FAVICON_MISSING) {
            return Ok(None);
        }
        let options = AssetOptions::new(FAVICON_MAX_BYTES).with_timeout(Duration::from_secs(3));
        self.signed_get(url, &options, &ASSET).map(Some)
    }
    /// Grant for `resource`, resolved and checked against this connection's origin.
    pub(crate) fn asset_url(&self, resource: Value) -> Result<Url> {
        let grant = crate::api::assets::grant(self, resource)?;
        let relative = grant["relativeUrl"].as_str().unwrap_or("");
        if !relative.starts_with("/api/assets/") {
            return Err("invalid_asset_url".into());
        }
        let url = endpoint(&self.base, relative).map_err(|_| "invalid_asset_url")?;
        // Dot segments resolve before this check: /api/assets/../x is refused.
        if !url.path().starts_with("/api/assets/") {
            return Err("invalid_asset_url".into());
        }
        Ok(url)
    }
    pub(crate) fn signed_get(&self, url: Url, options: &AssetOptions, codes: &Codes) -> Result<AssetBytes> {
        let response = self
            .http
            .get(url)
            .timeout(options.timeout)
            .send()
            .map_err(|_| codes.unreachable)?;
        if !response.status().is_success() {
            return Err(codes.unavailable.into());
        }
        let mime = media_type(&response);
        let mut bytes = vec![];
        read_limited(response, options.limit, &mut bytes).map_err(|e| match e {
            BodyError::TooLarge => codes.too_large,
            BodyError::Unreadable => codes.unavailable,
        })?;
        Ok(AssetBytes { bytes, mime })
    }
}
