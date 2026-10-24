//! Argument validators for the typed API. Each rejects before any socket opens
//! and names the field it rejected: `invalid_<field>` (camelCase → snake_case).
//! They are stricter than the server only where ot3 already is (`id`); the rest
//! follow the contract's own bounds so a valid server id is never refused.
use crate::Result;

/// `invalid_<field>` for a camelCase field name: `threadId` → `invalid_thread_id`.
pub fn invalid(field: &str) -> String {
    let mut code = String::from("invalid_");
    for c in field.chars() {
        if c.is_ascii_uppercase() {
            code.push('_');
            code.push(c.to_ascii_lowercase());
        } else {
            code.push(c);
        }
    }
    code
}
fn check<'a>(field: &str, value: &'a str, ok: bool) -> Result<&'a str> {
    if ok {
        Ok(value)
    } else {
        Err(invalid(field))
    }
}
/// Ids T3 mints (thread, project, command, run, message, item, node, checkpoint):
/// non-empty, ≤ 2048 bytes, `[A-Za-z0-9][A-Za-z0-9_:%.-]*`. Same rule as ot3's `valid_id`.
pub fn id<'a>(field: &str, value: &'a str) -> Result<&'a str> {
    let b = value.as_bytes();
    check(
        field,
        value,
        !b.is_empty()
            && b.len() <= 2048
            && b[0].is_ascii_alphanumeric()
            && b.iter().all(|c| c.is_ascii_alphanumeric() || b"_:%.-".contains(c)),
    )
}
/// Provider instance ids and driver kinds: `^[a-zA-Z][a-zA-Z0-9_-]*$`, ≤ 64.
pub fn slug<'a>(field: &str, value: &'a str) -> Result<&'a str> {
    let b = value.as_bytes();
    check(
        field,
        value,
        !b.is_empty()
            && b.len() <= 64
            && b[0].is_ascii_alphabetic()
            && b.iter().all(|c| c.is_ascii_alphanumeric() || b"_-".contains(c)),
    )
}
/// Bounded ids without an alphabet in the contract (tab, profile, attachment…):
/// non-empty, no control characters, at most `max` characters.
pub fn token<'a>(field: &str, value: &'a str, max: usize) -> Result<&'a str> {
    check(
        field,
        value,
        !value.trim().is_empty() && value.chars().count() <= max && !value.chars().any(char::is_control),
    )
}
/// Provider request ids: may exceed 256 and carry `%253A`. Never trimmed or capped,
/// but must already be trimmed and free of control characters.
pub fn opaque<'a>(field: &str, value: &'a str) -> Result<&'a str> {
    check(
        field,
        value,
        !value.is_empty() && value.trim() == value && !value.chars().any(char::is_control),
    )
}
/// Host paths (`cwd`, `relativePath`, `worktreePath`…): non-empty, ≤ 4096 bytes,
/// no control characters. Never checked against the caller's own disk.
pub fn path<'a>(field: &str, value: &'a str) -> Result<&'a str> {
    check(
        field,
        value,
        !value.trim().is_empty() && value.len() <= 4096 && !value.chars().any(char::is_control),
    )
}
/// Human text (`title`, `query`, `refName`…): non-empty once trimmed, ≤ `max`
/// characters, no NUL. Returns the trimmed text, as T3's `TrimmedNonEmptyString` decodes it.
pub fn text<'a>(field: &str, value: &'a str, max: usize) -> Result<&'a str> {
    let trimmed = value.trim();
    check(
        field,
        trimmed,
        !trimmed.is_empty() && trimmed.chars().count() <= max && !trimmed.contains('\0'),
    )
}
/// An id the host may use as a `commandId` prefix target: `<prefix>-<uuid v4>`.
/// The library never calls this itself; the host keeps the id to resend it.
pub fn command_id(prefix: &str) -> Result<String> {
    let id = format!("{prefix}-{}", uuid::Uuid::new_v4());
    self::id("commandId", &id).map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validators_name_the_field_and_follow_their_rules() {
        assert_eq!(invalid("threadId"), "invalid_thread_id");
        assert_eq!(invalid("cwd"), "invalid_cwd");
        assert!(id("threadId", "omarchy-1:a%2.b_c").is_ok());
        for bad in ["", "-x", "a/b", "a b", &"x".repeat(2049)] {
            assert_eq!(id("threadId", bad).unwrap_err(), "invalid_thread_id", "{bad}");
        }
        assert!(slug("instanceId", "codex_2-b").is_ok());
        assert!(slug("instanceId", "2codex").is_err());
        assert!(slug("instanceId", &"a".repeat(65)).is_err());
        assert!(token("tabId", "tab with spaces/ok", 128).is_ok());
        assert!(token("tabId", &"t".repeat(129), 128).is_err());
        assert!(token("tabId", "a\nb", 128).is_err());
        let long = format!("call_{}", "%253A".repeat(80));
        assert!(opaque("requestId", &long).is_ok());
        assert!(opaque("requestId", " padded").is_err());
        assert!(path("cwd", "/home/user/My Repo").is_ok());
        assert!(path("cwd", "   ").is_err());
        assert!(path("cwd", &"p".repeat(4097)).is_err());
        assert_eq!(text("title", "  Hi  ", 255).unwrap(), "Hi");
        assert!(text("title", &"t".repeat(256), 255).is_err());
        assert!(command_id("omarchy").unwrap().starts_with("omarchy-"));
        assert!(command_id("bad prefix").is_err());
    }
}
