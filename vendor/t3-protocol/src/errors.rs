//! Classification of a failed RPC exit. Authorization is decided by the error's
//! `_tag` (`EnvironmentAuthorizationError`, T3 `auth.ts`), never by message text:
//! a git failure that says "Author identity unknown" or "Authentication failed"
//! is a rejection, not a missing permission.
use serde_json::Value;

pub const WRITE_NOT_AUTHORIZED: &str = "write_not_authorized";
pub const RPC_REJECTED: &str = "rpc_rejected";

/// A classified failure: a stable code, the server's one-line reason, and for
/// `write_not_authorized` the scope the token lacked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rejection {
    pub code: &'static str,
    pub reason: Option<String>,
    pub required_scope: Option<String>,
}
impl Rejection {
    /// The library's error string: `code`, `code: reason`, or for a known scope
    /// `write_not_authorized: [scope] reason`. `required_scope` and `message` read it back.
    pub fn to_error(&self) -> String {
        let reason = self.reason.as_deref().unwrap_or("");
        let error = crate::with_reason(self.code, reason);
        match &self.required_scope {
            Some(scope) if error.len() > self.code.len() => format!("{}: [{scope}] {}", self.code, &error[self.code.len() + 2..]),
            Some(scope) => format!("{}: [{scope}]", self.code),
            None => error,
        }
    }
}

/// Classify the `cause` of an Effect `Failure` exit.
pub fn classify(cause: &Value) -> Rejection {
    match authorization_error(cause, 0) {
        Some(error) => Rejection {
            code: WRITE_NOT_AUTHORIZED,
            reason: error["message"].as_str().map(str::to_string).filter(|m| !m.trim().is_empty()),
            required_scope: error["requiredScope"].as_str().filter(|s| valid_scope(s)).map(str::to_string),
        },
        None => Rejection {
            code: RPC_REJECTED,
            reason: crate::failure_reason(cause, 0),
            required_scope: None,
        },
    }
}
/// The typed failure of a cause, searched where Effect encodes failures.
/// Defects (`Die`) are untyped and never count as authorization errors.
fn authorization_error(cause: &Value, depth: usize) -> Option<&Value> {
    if depth > 8 {
        return None;
    }
    match cause {
        Value::Array(items) => items.iter().find_map(|i| authorization_error(i, depth + 1)),
        Value::Object(o) => {
            if o.get("_tag").and_then(Value::as_str) == Some("EnvironmentAuthorizationError") {
                return Some(cause);
            }
            if o.get("_tag").and_then(Value::as_str) == Some("Die") {
                return None;
            }
            ["error", "cause", "failures", "reasons", "left", "right"]
                .iter()
                .filter_map(|k| o.get(*k))
                .find_map(|v| authorization_error(v, depth + 1))
        }
        _ => None,
    }
}
/// T3 scopes look like `settings:write` or `source-control:write`.
fn valid_scope(s: &str) -> bool {
    s.len() <= 64
        && s.split_once(':').is_some_and(|(a, b)| {
            [a, b].iter().all(|p| !p.is_empty() && p.bytes().all(|c| c.is_ascii_lowercase() || c == b'-'))
        })
}
/// The scope a `write_not_authorized` error names, if the server sent one.
pub fn required_scope(error: &str) -> Option<&str> {
    let rest = error.strip_prefix(WRITE_NOT_AUTHORIZED)?.strip_prefix(": [")?;
    let (scope, _) = rest.split_once(']')?;
    Some(scope).filter(|s| valid_scope(s))
}
/// The server's reason of an error, without its code (or scope marker).
pub fn message(error: &str) -> Option<&str> {
    let code = crate::error_code(error);
    let reason = error.get(code.len() + 2..).filter(|_| code.len() < error.len())?;
    let reason = match required_scope(error) {
        Some(scope) => reason[scope.len() + 2..].trim_start(),
        None => reason,
    };
    Some(reason).filter(|r| !r.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn authorization_comes_from_the_tag_and_carries_the_scope() {
        let cause = json!([{"_tag":"Fail","error":{"_tag":"EnvironmentAuthorizationError",
            "message":"The authenticated token is missing required scope: settings:write.","requiredScope":"settings:write"}}]);
        let r = classify(&cause);
        assert_eq!(r.code, WRITE_NOT_AUTHORIZED);
        assert_eq!(r.required_scope.as_deref(), Some("settings:write"));
        let error = r.to_error();
        assert_eq!(error, "write_not_authorized: [settings:write] The authenticated token is missing required scope: settings:write.");
        assert_eq!(crate::error_code(&error), WRITE_NOT_AUTHORIZED);
        assert_eq!(required_scope(&error), Some("settings:write"));
        assert_eq!(message(&error), Some("The authenticated token is missing required scope: settings:write."));
        let bare = Rejection { code: WRITE_NOT_AUTHORIZED, reason: None, required_scope: Some("orchestration:operate".into()) };
        assert_eq!(bare.to_error(), "write_not_authorized: [orchestration:operate]");
        assert_eq!(message(&bare.to_error()), None);
    }
    #[test]
    fn auth_words_in_a_message_are_a_rejection() {
        for text in ["Author identity unknown\n*** Please tell me who you are.", "Authentication failed for 'https://github.com/x/y'", "Forbidden: scope of the branch"] {
            let cause = json!([{"_tag":"Fail","error":{"_tag":"GitCommandError","message":text}}]);
            let error = classify(&cause).to_error();
            assert_eq!(crate::error_code(&error), RPC_REJECTED, "{text}");
            assert_eq!(required_scope(&error), None);
        }
        let defect = json!({"_tag":"Die","defect":{"_tag":"EnvironmentAuthorizationError","requiredScope":"settings:write"}});
        assert_eq!(classify(&defect).code, RPC_REJECTED);
        let odd = json!({"_tag":"Fail","error":{"_tag":"EnvironmentAuthorizationError","message":"no","requiredScope":"NOT A SCOPE"}});
        assert_eq!(classify(&odd).to_error(), "write_not_authorized: no");
    }
}
