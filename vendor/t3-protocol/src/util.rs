//! Small JSON helpers shared by the domain modules and re-exported by hosts.
//! Byte-for-byte the same as the crate-root helpers Omarchy used before.
use chrono::DateTime;
use serde_json::Value;

pub use crate::Result;

/// The string at `v[k]`, or "" when missing or not a string.
pub fn text<'a>(v: &'a Value, k: &str) -> &'a str {
    v[k].as_str().unwrap_or("")
}
/// JavaScript-like truthiness: null, false, "", [], {} and 0 are false.
pub fn truth(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
        Value::Number(n) => n.as_f64().unwrap_or(0.) != 0.,
    }
}
/// T3 entity ids: ASCII alphanumeric first, then alphanumerics or `_:%.-`.
pub fn valid_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 2048
        && s.as_bytes()[0].is_ascii_alphanumeric()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_:%.-".contains(&b))
}
/// Milliseconds since the epoch from an RFC 3339 string or a number in
/// seconds (below 1e11) or milliseconds; 0 when absent or invalid.
pub fn epoch(v: &Value) -> i64 {
    if let Some(s) = v.as_str() {
        return DateTime::parse_from_rfc3339(s)
            .map(|d| d.timestamp_millis().max(0))
            .unwrap_or(0);
    }
    v.as_f64()
        .filter(|n| n.is_finite() && *n >= 0.)
        .map(|n| {
            if n < 100_000_000_000. {
                (n * 1000.) as i64
            } else {
                n as i64
            }
        })
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn helpers_keep_their_contracts() {
        assert_eq!(text(&json!({"a":"x","b":1}), "a"), "x");
        assert_eq!(text(&json!({"b":1}), "b"), "");
        for v in [json!(null), json!(false), json!(""), json!([]), json!({}), json!(0)] {
            assert!(!truth(&v));
        }
        for v in [json!(true), json!("x"), json!([0]), json!({"a":0}), json!(0.5)] {
            assert!(truth(&v));
        }
        assert!(valid_id("thread-1:a.b_c%2"));
        assert!(!valid_id("-x") && !valid_id("") && !valid_id("a b") && !valid_id(&"a".repeat(2049)));
        assert_eq!(epoch(&json!("1970-01-01T00:00:01Z")), 1000);
        assert_eq!(epoch(&json!(1)), 1000);
        assert_eq!(epoch(&json!(100_000_000_000u64)), 100_000_000_000);
        assert_eq!(epoch(&json!(-1)), 0);
        assert_eq!(epoch(&json!("nope")), 0);
    }
}
