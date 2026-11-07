//! Public registry SVG fetches. Dedicated credential-free client, no proxy or redirects.
use crate::{assets::AssetBytes, Result};
use std::{io::Read, time::Duration};
pub const MAX_BYTES: u64 = 256 * 1024;
pub fn validate(raw: &str) -> Result<reqwest::Url> {
    let u = reqwest::Url::parse(raw).map_err(|_| "invalid_registry_icon")?;
    if u.scheme() != "https"
        || u.host_str() != Some("cdn.agentclientprotocol.com")
        || !u.username().is_empty()
        || u.password().is_some()
        || u.port().is_some()
        || u.query().is_some()
        || u.fragment().is_some()
        || !u.path().starts_with("/registry/v1/")
        || !u.path().ends_with(".svg")
    {
        return Err("invalid_registry_icon".into());
    }
    Ok(u)
}
/// Cancellation is observed before/after HTTP and each body chunk; network waits have a 5s deadline.
pub fn bytes(raw: &str, cancel: impl Fn() -> bool) -> Result<AssetBytes> {
    fetch(validate(raw)?, cancel)
}
fn fetch(u: reqwest::Url, cancel: impl Fn() -> bool) -> Result<AssetBytes> {
    if cancel() {
        return Err("cancelled".into());
    }
    let client = reqwest::blocking::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|_| "icon_unreachable")?;
    let mut response = client.get(u).send().map_err(|_| "icon_unreachable")?;
    if cancel() {
        return Err("cancelled".into());
    }
    if !response.status().is_success() {
        return Err("icon_unavailable".into());
    }
    let mime = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    if mime != "image/svg+xml" {
        return Err("invalid_registry_icon_content".into());
    }
    if response.content_length().is_some_and(|n| n > MAX_BYTES) {
        return Err("icon_too_large".into());
    }
    let mut data = Vec::new();
    let mut chunk = [0; 8192];
    loop {
        if cancel() {
            return Err("cancelled".into());
        }
        let n = response.read(&mut chunk).map_err(|_| "icon_unavailable")?;
        if n == 0 {
            break;
        }
        if data.len() + n > MAX_BYTES as usize {
            return Err("icon_too_large".into());
        }
        data.extend_from_slice(&chunk[..n]);
    }
    validate_content(&data)?;
    if cancel() {
        return Err("cancelled".into());
    }
    Ok(AssetBytes { bytes: data, mime })
}
pub fn validate_content(data: &[u8]) -> Result<()> {
    let s = std::str::from_utf8(data).map_err(|_| "invalid_registry_icon_content")?;
    let lower = s
        .trim_start_matches('\u{feff}')
        .trim_start()
        .to_ascii_lowercase();
    // SVG remains untrusted image data; hosts must disable scripting/external resources.
    if !(lower.starts_with("<svg") || lower.starts_with("<?xml"))
        || !lower.contains("<svg")
        || !lower.contains("</svg>")
    {
        return Err("invalid_registry_icon_content".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };
    fn fixture(
        status: &str,
        mime: &str,
        body: Vec<u8>,
    ) -> (reqwest::Url, thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = reqwest::Url::parse(&format!("http://{}/icon", listener.local_addr().unwrap()))
            .unwrap();
        let header=format!("HTTP/1.1 {status}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nLocation: https://evil.test/\r\nConnection: close\r\n\r\n",body.len());
        let join = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = vec![];
            let mut c = [0];
            while !request.ends_with(b"\r\n\r\n") {
                stream.read_exact(&mut c).unwrap();
                request.push(c[0]);
            }
            let _ = stream.write_all(header.as_bytes());
            let _ = stream.write_all(&body);
            String::from_utf8(request).unwrap()
        });
        (url, join)
    }
    #[test]
    fn public_icon_has_no_credentials() {
        let (url, join) = fixture("200 OK", "image/svg+xml", b"<svg></svg>".to_vec());
        assert_eq!(fetch(url, || false).unwrap().bytes, b"<svg></svg>");
        let request = join.join().unwrap().to_lowercase();
        assert!(!request.contains("authorization:"));
        assert!(!request.contains("cookie:"));
    }
    #[test]
    fn redirects_are_not_followed() {
        let (url, join) = fixture("302 Found", "image/svg+xml", vec![]);
        assert_eq!(fetch(url, || false).err().unwrap(), "icon_unavailable");
        join.join().unwrap();
    }
    #[test]
    fn icon_content_and_length_are_bounded() {
        for (mime, body, error) in [
            (
                "text/html",
                b"<svg></svg>".to_vec(),
                "invalid_registry_icon_content",
            ),
            (
                "image/svg+xml",
                vec![b'x'; MAX_BYTES as usize + 1],
                "icon_too_large",
            ),
        ] {
            let (url, join) = fixture("200 OK", mime, body);
            assert_eq!(fetch(url, || false).err().unwrap(), error);
            join.join().unwrap();
        }
    }
}
