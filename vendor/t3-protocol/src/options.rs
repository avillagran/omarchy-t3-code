//! HTTP transport options. The defaults are the library's historical values, so
//! `Connection::new` behaves exactly as before. WebSocket RPC keeps its own
//! deadlines (8 s connect, 12 s read/write, 20 s `rpc`); see `rpc_with_timeout`.
use super::*;

/// Longest request or connect timeout a host may ask for.
const MAX_TIMEOUT: Duration = Duration::from_secs(300);

/// Per-connection HTTP policy. Build with `ConnectionOptions::default()` and the
/// `with_*` setters; values are validated when a client is built.
///
/// | option | default |
/// | --- | --- |
/// | request timeout (whole request, body included) | 12 s |
/// | connect timeout | none (bounded by the request timeout) |
/// | pool idle timeout | 90 s |
/// | response limit for `request_json`, after decompression | 8 MiB |
///
/// Proxies and redirects are always disabled; they are not options. Responses
/// are negotiated as gzip or brotli (see `Connection::request_json`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectionOptions {
    request_timeout: Duration,
    connect_timeout: Option<Duration>,
    pool_idle_timeout: Option<Duration>,
    max_bytes: u64,
}
impl Default for ConnectionOptions {
    fn default() -> Self {
        Self {
            request_timeout: Duration::from_secs(12),
            connect_timeout: None,
            pool_idle_timeout: Some(Duration::from_secs(90)),
            max_bytes: MAX_BYTES,
        }
    }
}
impl ConnectionOptions {
    /// Total deadline of one HTTP request, from connect to the last body byte.
    pub fn with_request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }
    /// Deadline of the TCP/TLS connect alone. `None` leaves it to the request timeout.
    pub fn with_connect_timeout(mut self, timeout: Option<Duration>) -> Self {
        self.connect_timeout = timeout;
        self
    }
    /// How long an idle pooled connection is kept. `None` keeps it until the server closes it.
    pub fn with_pool_idle_timeout(mut self, timeout: Option<Duration>) -> Self {
        self.pool_idle_timeout = timeout;
        self
    }
    /// Default response limit of `request_json` (1 byte to 64 MiB). Bounded thread
    /// snapshots keep their 64 MiB allowance; `request_json_with_limit` overrides both.
    pub fn with_max_bytes(mut self, max_bytes: u64) -> Self {
        self.max_bytes = max_bytes;
        self
    }
    pub fn request_timeout(&self) -> Duration {
        self.request_timeout
    }
    pub fn connect_timeout(&self) -> Option<Duration> {
        self.connect_timeout
    }
    pub fn pool_idle_timeout(&self) -> Option<Duration> {
        self.pool_idle_timeout
    }
    pub fn max_bytes(&self) -> u64 {
        self.max_bytes
    }
    pub(crate) fn validate(&self) -> Result<()> {
        let timeout = |d: Duration| !d.is_zero() && d <= MAX_TIMEOUT;
        if !timeout(self.request_timeout)
            || !self.connect_timeout.is_none_or(timeout)
            || !valid_limit(self.max_bytes)
        {
            return Err("invalid_connection_options".into());
        }
        Ok(())
    }
    /// A `reqwest` client with this policy, no proxy and no redirects. Hosts may
    /// share it through `Connection::with_http_client` to reuse one pool.
    pub fn build_http_client(&self) -> Result<Http> {
        self.validate()?;
        let mut builder = Http::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(self.request_timeout)
            .pool_idle_timeout(self.pool_idle_timeout);
        if let Some(connect) = self.connect_timeout {
            builder = builder.connect_timeout(connect);
        }
        builder.build().map_err(|_| "local_error".into())
    }
}
/// Response limits accepted anywhere a host chooses one.
pub(crate) fn valid_limit(max_bytes: u64) -> bool {
    (1..=THREAD_SNAPSHOT_MAX_BYTES).contains(&max_bytes)
}
