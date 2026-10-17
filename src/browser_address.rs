//! Port of pinned packages/shared/src/preview.ts address-bar helpers. Native
//! server normalization remains the final authority for lifecycle RPCs.
use crate::Result;
fn whitespace(c: char) -> bool {
    matches!(c,'\u{0009}'..='\u{000d}'|'\u{0020}'|'\u{00a0}'|'\u{1680}'|'\u{2000}'..='\u{200a}'|'\u{2028}'|'\u{2029}'|'\u{202f}'|'\u{205f}'|'\u{3000}'|'\u{feff}')
}
fn protocol(s: &str) -> Option<String> {
    let (p, _) = s.split_once(':')?;
    let mut bytes = p.bytes();
    if !bytes.next()?.is_ascii_alphabetic()
        || !bytes.all(|b| b.is_ascii_alphanumeric() || b"+.-".contains(&b))
    {
        return None;
    }
    Some(format!("{}:", p.to_ascii_lowercase()))
}
fn loopback(s: &str) -> bool {
    let s = s.to_ascii_lowercase();
    ["localhost", "127.0.0.1", "0.0.0.0", "[::1]", "[::]"]
        .iter()
        .any(|host| {
            s.strip_prefix(host).is_some_and(|tail| {
                tail.is_empty() || tail.starts_with(':') || tail.starts_with('/')
            })
        })
}
fn host_port(s: &str) -> bool {
    let Some((host, tail)) = s.split_once(':') else {
        return false;
    };
    if host.is_empty() || host.contains(['/', '?', '#', '@']) {
        return false;
    }
    let count = tail.bytes().take_while(u8::is_ascii_digit).count();
    count > 0 && (count == tail.len() || matches!(tail.as_bytes()[count], b'/' | b'?' | b'#'))
}
fn address_host(s: &str) -> bool {
    if s.eq_ignore_ascii_case("localhost") {
        return true;
    }
    let ipv4 = s.split('.').collect::<Vec<_>>();
    if ipv4.len() == 4
        && ipv4
            .iter()
            .all(|n| !n.is_empty() && n.len() <= 3 && n.bytes().all(|c| c.is_ascii_digit()))
    {
        return true;
    }
    s.strip_prefix('[')
        .and_then(|v| v.strip_suffix(']'))
        .is_some_and(|v| {
            !v.is_empty()
                && v.bytes()
                    .all(|c| c.is_ascii_hexdigit() || b":.".contains(&c))
        })
}
fn dotted_host(s: &str) -> bool {
    let s = s.strip_suffix('.').unwrap_or(s);
    let mut parts = s.split('.');
    parts.next().is_some_and(|p| !p.is_empty()) && {
        let tail = parts.collect::<Vec<_>>();
        !tail.is_empty() && tail.iter().all(|p| !p.is_empty())
    }
}
fn normalize(s: &str) -> Result<String> {
    let s = s.trim_matches(whitespace);
    if s.is_empty() {
        return Err("invalid_preview_address".into());
    }
    let candidate = if s.contains("://") {
        s.to_owned()
    } else {
        format!("{}://{s}", if loopback(s) { "http" } else { "https" })
    };
    let url = reqwest::Url::parse(&candidate).map_err(|_| "invalid_preview_address")?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("invalid_preview_address".into());
    }
    Ok(url.to_string())
}
fn encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
            out.push(b as char)
        } else {
            use std::fmt::Write;
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}
pub fn resolve(raw: &str) -> Result<String> {
    if raw.encode_utf16().count() > 2048 || raw.contains('\0') {
        return Err("invalid_preview_address".into());
    }
    let s = raw.trim_matches(whitespace);
    let p = protocol(s);
    if s.is_empty()
        || p.as_ref()
            .is_some_and(|p| s.get(p.len()..).is_some_and(|tail| tail.starts_with("//")))
    {
        return normalize(s);
    }
    if !s.chars().any(whitespace) {
        if let Some(p) = &p {
            let known = matches!(
                p.as_str(),
                "about:"
                    | "blob:"
                    | "chrome:"
                    | "data:"
                    | "file:"
                    | "ftp:"
                    | "javascript:"
                    | "mailto:"
                    | "sms:"
                    | "ssh:"
                    | "tel:"
                    | "view-source:"
                    | "ws:"
                    | "wss:"
            );
            if known || !host_port(s) {
                if p != "http:" && p != "https:" {
                    return Err("invalid_preview_address".into());
                }
                return normalize(s);
            }
        }
        let authority = s.split(['/', '?', '#']).next().unwrap_or("");
        let authority = authority
            .split_once('@')
            .map(|(_, rest)| rest)
            .unwrap_or(authority);
        let host = authority
            .rsplit_once(':')
            .filter(|(_, port)| !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()))
            .map(|(host, _)| host)
            .unwrap_or(authority);
        if host.len() < authority.len() || address_host(host) || dotted_host(host) {
            if let Ok(value) = normalize(s) {
                return Ok(value);
            }
        }
    }
    Ok(format!("https://duckduckgo.com/?q={}", encode(s)))
}
