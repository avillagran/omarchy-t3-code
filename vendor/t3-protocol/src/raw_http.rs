//! Lossless same-origin HTTP JSON, sharing the credentialed request factory.
use crate::{Connection, RawJson, Result};
use std::io::Read;
use zeroize::Zeroizing;
fn decode(response: reqwest::blocking::Response, max_bytes: u64) -> Result<Box<RawJson>> {
    crate::check_status(&response)?;
    if crate::media_type(&response) != "application/json" {
        return Err("not_json".into());
    }
    if !response
        .headers()
        .get("content-encoding")
        .and_then(|value| value.to_str().ok())
        .is_none_or(|value| value.is_empty() || value.eq_ignore_ascii_case("identity"))
    {
        return Err("unsupported_encoding".into());
    }
    let mut bytes = Zeroizing::new(Vec::new());
    response
        .take(max_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "unreachable")?;
    if bytes.len() as u64 > max_bytes {
        return Err("too_large".into());
    }
    serde_json::from_slice(&bytes).map_err(|_| "malformed".into())
}
impl Connection {
    /// Original same-origin/authentication/protocol/response-limit rules, without
    /// decoding native strings or applying compact-ID convenience transforms.
    pub fn request_json_raw(
        &self,
        method: &str,
        path: &str,
        body: Option<&RawJson>,
        protocol: bool,
    ) -> Result<Box<RawJson>> {
        let (response, bounded) = self.send_serialized(method, path, body, protocol)?;
        let limit = if bounded {
            crate::THREAD_SNAPSHOT_MAX_BYTES.max(self.max_bytes)
        } else {
            self.max_bytes
        };
        decode(response, limit)
    }
    pub fn request_json_raw_with_limit(
        &self,
        method: &str,
        path: &str,
        body: Option<&RawJson>,
        protocol: bool,
        max_bytes: u64,
    ) -> Result<Box<RawJson>> {
        if !crate::valid_limit(max_bytes) {
            return Err("invalid_max_bytes".into());
        }
        decode(
            self.send_serialized(method, path, body, protocol)?.0,
            max_bytes,
        )
    }
}
