//! Scalar-string port of nightly 4340's inline-code file candidate classifier.
//! Classification performs no filesystem access or path authorization.

fn whitespace(c: char) -> bool {
    matches!(c, '\u{0009}'..='\u{000d}' | ' ' | '\u{00a0}' | '\u{1680}' |
        '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' |
        '\u{205f}' | '\u{3000}' | '\u{feff}')
}
fn windows_absolute(value: &str) -> bool {
    let b = value.as_bytes();
    value.starts_with("\\\\") || (b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' &&
        (b.len() == 2 || matches!(b[2], b'/' | b'\\')))
}
fn without_position(value: &str) -> (&str, bool) {
    let mut end = value.len();
    let mut found = false;
    for _ in 0..2 {
        let Some((prefix, number)) = value[..end].rsplit_once(':') else { break };
        if number.is_empty() || !number.bytes().all(|c| c.is_ascii_digit()) { break }
        found = true;
        end = prefix.len();
    }
    (&value[..end], found)
}
fn hostname(value: &str, positioned: bool) -> bool {
    if value.starts_with('.') { return false }
    let lower = value.to_lowercase();
    if lower == "localhost" { return true }
    if lower.contains('.') && lower.split('.').all(|s| !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit())) {
        return true;
    }
    let Some((_, tld)) = lower.rsplit_once('.') else { return false };
    if matches!(tld, "com"|"net"|"org"|"io"|"dev"|"app"|"ai"|"co"|"edu"|"gov"|"mil"|
        "info"|"biz"|"xyz"|"me"|"tv"|"gg"|"chat"|"cloud"|"site"|"online"|"tech"|"store"|"link") {
        return true;
    }
    !positioned && matches!(tld, "uk"|"de"|"fr"|"nl"|"se"|"no"|"fi"|"dk"|"pl"|"ch"|"at"|
        "be"|"es"|"it"|"pt"|"eu"|"us"|"ca"|"au"|"nz"|"jp"|"kr"|"cn"|"br"|"ru"|"mx"|"ie"|
        "cz"|"tr"|"sg"|"hk"|"cc")
}
/// Equivalent to the original classifier for Rust-representable scalar strings.
/// Consumers must separately authorize the resulting file-link destination.
pub fn inline_code_path(code: &str) -> Option<String> {
    let trimmed = code.trim_matches(whitespace);
    if trimmed.is_empty() || trimmed.chars().any(|c| whitespace(c) || c == '`') { return None }
    let candidate = if windows_absolute(trimmed) { trimmed.to_owned() } else { trimmed.replace('\\', "/") };
    let (body, positioned) = without_position(&candidate);
    if !positioned && !candidate.contains(['/', '\\']) { return None }
    if body.split(['/', '\\']).any(|s| s == "…" || s.len() >= 3 && s.bytes().all(|c| c == b'.')) { return None }
    let explicit = candidate.starts_with("~/") || candidate.starts_with("./") ||
        candidate.starts_with("../") || candidate.starts_with('/') || windows_absolute(&candidate);
    if !explicit {
        if body.ends_with('/') || hostname(body.split('/').next().unwrap_or(body), positioned) { return None }
        let basename = body.trim_end_matches(['/', '\\']).rsplit(['/', '\\']).next().unwrap_or("");
        if let Some((left, right)) = basename.rsplit_once('.') {
            if left.as_bytes().last().is_some_and(u8::is_ascii_digit) && right.as_bytes().first().is_some_and(u8::is_ascii_digit) { return None }
        }
        if !positioned && !basename.rsplit_once('.').is_some_and(|(_, ext)|
            !ext.is_empty() && ext.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-'))) { return None }
    }
    Some(candidate)
}
