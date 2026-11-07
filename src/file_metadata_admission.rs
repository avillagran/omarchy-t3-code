//! Native 4288 metadata-query admission and workspace/link path composition.
use crate::file_metadata_cache::{normalize, ThreadRef};
use serde_json::{json, Value};
fn windows(path: &str) -> bool {
    let b = path.as_bytes();
    b.len() >= 2
        && b[0].is_ascii_alphabetic()
        && b[1] == b':'
        && (b.len() == 2 || matches!(b[2], b'/' | b'\\'))
}
fn absolute(path: &str) -> bool {
    path.starts_with('/') || path.starts_with("\\\\") || windows(path)
}
fn windows_style(path: &str) -> bool {
    absolute_windows(path)
        || path
            .as_bytes()
            .windows(3)
            .any(|b| b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'\\')
}
fn absolute_windows(path: &str) -> bool {
    windows(path) || path.starts_with("\\\\")
}
fn position(path: &str) -> (&str, Option<f64>, Option<f64>) {
    let Some((prefix, last)) = path.rsplit_once(':') else {
        return (path, None, None);
    };
    let digits = |value: &str| !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit());
    if !digits(last) {
        return (path, None, None);
    }
    if let Some((parent, line)) = prefix.rsplit_once(':') {
        if digits(line) {
            return (parent, line.parse().ok(), last.parse().ok());
        }
    }
    (prefix, last.parse().ok(), None)
}

fn number(value: f64) -> String {
    if value.is_infinite() {
        return "Infinity".into();
    }
    let text = value.to_string();
    if value < 1e21 {
        return text;
    }
    if let Some((coefficient, exponent)) = text.split_once('e') {
        return format!("{coefficient}e+{}", exponent.trim_start_matches('+'));
    }
    let digits = text.trim_end_matches('0');
    let exponent = text.len() - 1;
    if digits.len() == 1 {
        format!("{digits}e+{exponent}")
    } else {
        format!("{}.{}e+{exponent}", &digits[..1], &digits[1..])
    }
}
fn home(cwd: &str) -> Option<&str> {
    for prefix in ["/Users/", "/home/"] {
        if let Some(rest) = cwd.strip_prefix(prefix) {
            let length = rest.find('/').unwrap_or(rest.len());
            return (length > 0).then_some(&cwd[..prefix.len() + length]);
        }
    }
    let bytes = cwd.as_bytes();
    if bytes.len() > 9
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'/' | b'\\')
        && cwd
            .get(3..8)
            .is_some_and(|value| value.eq_ignore_ascii_case("Users"))
        && matches!(bytes[8], b'/' | b'\\')
    {
        let rest = &cwd[9..];
        let length = rest.find(['/', '\\']).unwrap_or(rest.len());
        return (length > 0).then_some(&cwd[..9 + length]);
    }
    None
}
fn join(base: &str, next: &str, backslash: bool) -> String {
    let base = base.trim_end_matches(['/', '\\']);
    if backslash {
        format!("{base}\\{}", next.replace('/', "\\"))
    } else {
        format!("{base}/{}", next.trim_start_matches('/'))
    }
}
pub fn resolve_link(raw: &str, cwd: &str) -> String {
    let (path, line, column) = position(raw);
    let mut resolved = path.to_owned();
    if path.starts_with("~/") || (windows_style(cwd) && path.starts_with("~\\")) {
        if let Some(home) = home(cwd) {
            resolved = join(home, &path[2..], windows_style(home));
        }
    } else if !absolute(path) {
        resolved = join(cwd, path, windows_style(cwd));
    }
    if let Some(line) = line.filter(|value| *value > 0.0) {
        resolved.push(':');
        resolved.push_str(&number(line));
        if let Some(column) = column.filter(|value| *value > 0.0) {
            resolved.push(':');
            resolved.push_str(&number(column));
        }
    }
    resolved
}
pub fn workspace(path: &str, cwd: &str) -> String {
    if absolute(path) || path.starts_with("~/") || path.starts_with("~\\") {
        path.into()
    } else {
        join(cwd, path, absolute_windows(cwd))
    }
}
/// Admission after the query helper has normalized its physical path. Numeric
/// filename suffixes remain literal at this boundary, as in the cache atom.
pub fn admitted(path: &str) -> Option<String> {
    let path = normalize(path);
    let network =
        path.chars().take(2).count() == 2 && path.chars().take(2).all(|c| matches!(c, '/' | '\\'));
    ((absolute(&path) || path.starts_with("~/")) && !network && path.encode_utf16().count() <= 512)
        .then_some(path)
}
pub fn query(
    environment: &str,
    path: &str,
    cwd: Option<&str>,
    reference: Option<&ThreadRef>,
) -> Option<Value> {
    let host = position(path).0;
    let target = if host.starts_with("~/") || cwd.is_none_or(str::is_empty) {
        host.to_owned()
    } else {
        resolve_link(host, cwd.unwrap())
    };
    admitted(&target).map(|path| {
        let mut input = json!({"path":path});
        if let Some(reference) = reference.filter(|r| r.environment == environment) {
            input["threadId"] = json!(reference.thread);
        }
        json!({"environmentId":environment,"input":input})
    })
}
