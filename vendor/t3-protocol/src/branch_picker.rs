//! Thread branch picker: the composer's branch toolbar from T3 BranchToolbarBranchSelector.
//! Pure ref and branch rules. The host lists refs, switches or creates a branch through
//! the server, then records the branch (and worktree) on the thread.
use crate::util::text;
use serde_json::{json, Value};

/// Port of dedupeRemoteBranchesWithLocalMatches: an origin ref is hidden when its
/// local branch is listed.
pub fn dedupe(refs: &[Value]) -> Vec<Value> {
    let local: std::collections::HashSet<&str> = refs
        .iter()
        .filter(|r| r["isRemote"] != true)
        .filter_map(|r| r["name"].as_str())
        .collect();
    refs.iter()
        .filter(|r| {
            if r["isRemote"] != true || r["remoteName"] != "origin" {
                return true;
            }
            let name = text(r, "name");
            !local.contains(derive_local(name))
        })
        .cloned()
        .collect()
}
/// Port of deriveLocalBranchNameFromRemoteRef.
pub fn derive_local(name: &str) -> &str {
    match name.find('/') {
        Some(i) if i > 0 && i < name.len() - 1 => &name[i + 1..],
        _ => name,
    }
}
/// Port of sanitizeNewRefName.
pub fn sanitize(name: &str) -> String {
    name.split([' ', '\t', '\n', '\r', '\u{c}', '\u{b}'])
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}
/// Port of resolveBranchSelectionTarget.
pub fn selection_target(project_cwd: &str, worktree: Option<&str>, r: &Value) -> (String, Option<String>, bool) {
    if let Some(path) = r["worktreePath"].as_str().filter(|s| !s.is_empty()) {
        return (path.into(), (path != project_cwd).then(|| path.to_string()), true);
    }
    let next = worktree.filter(|_| r["isDefault"] != true).map(str::to_string);
    (next.clone().unwrap_or_else(|| project_cwd.into()), next, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn helpers_follow_native_branch_toolbar() {
        assert_eq!(derive_local("origin/feature/x"), "feature/x");
        assert_eq!(derive_local("main"), "main");
        assert_eq!(derive_local("origin/"), "origin/");
        assert_eq!(sanitize("  my new\tbranch "), "my-new-branch");
        let refs = vec![
            json!({"name":"main","current":true,"isDefault":true}),
            json!({"name":"origin/main","isRemote":true,"remoteName":"origin"}),
            json!({"name":"origin/feat","isRemote":true,"remoteName":"origin"}),
            json!({"name":"upstream/main","isRemote":true,"remoteName":"upstream"}),
        ];
        let names: Vec<_> = dedupe(&refs).iter().map(|r| text(r, "name").to_string()).collect();
        assert_eq!(names, ["main", "origin/feat", "upstream/main"]);
        assert_eq!(selection_target("/p", None, &json!({"worktreePath":"/w"})), ("/w".into(), Some("/w".into()), true));
        assert_eq!(selection_target("/p", None, &json!({"worktreePath":"/p"})), ("/p".into(), None, true));
        assert_eq!(selection_target("/p", Some("/w"), &json!({"isDefault":true})), ("/p".into(), None, false));
        assert_eq!(selection_target("/p", Some("/w"), &json!({"isDefault":false})), ("/w".into(), Some("/w".into()), false));
    }
}

/// Port of isTemporaryWorktreeBranch: t3-/t3/ + 8 hex, t3code-/t3code/ + 8 hex, t3code/ + uuid v4.
pub fn temporary_branch(name: &str) -> bool {
    let n = name.trim().to_lowercase();
    let hex8 = |s: &str| s.len() == 8 && s.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase());
    let uuid4 = |s: &str| {
        let parts: Vec<&str> = s.split('-').collect();
        parts.len() == 5
            && [8, 4, 4, 4, 12].iter().zip(&parts).all(|(l, p)| p.len() == *l && p.bytes().all(|b| b.is_ascii_hexdigit()))
            && parts[2].starts_with('4')
            && matches!(parts[3].as_bytes()[0], b'8' | b'9' | b'a' | b'b')
    };
    if let Some(rest) = n.strip_prefix("t3code") {
        return rest.strip_prefix('-').is_some_and(hex8) || rest.strip_prefix('/').is_some_and(|r| hex8(r) || uuid4(r));
    }
    n.strip_prefix("t3-").or_else(|| n.strip_prefix("t3/")).is_some_and(hex8)
}
/// Port of resolveCheckoutBranchMismatch for a started thread.
pub fn mismatch(worktree: Option<&str>, thread_branch: Option<&str>, current: Option<&str>) -> Value {
    let (Some(thread_branch), Some(current)) = (thread_branch, current) else { return Value::Null };
    if thread_branch == current || (worktree.is_some() && !temporary_branch(thread_branch) && temporary_branch(current)) {
        return Value::Null;
    }
    // Restoring a worktree checkout could switch the branch for sibling threads.
    json!({"threadBranch":thread_branch,"currentBranch":current,"canRestore":worktree.is_none()})
}
#[cfg(test)]
mod mismatch_tests {
    use super::*;
    #[test]
    fn follows_native_mismatch_rules() {
        assert!(temporary_branch("t3/1a2b3c4d") && temporary_branch("t3code-1a2b3c4d") && temporary_branch("t3code/1a2b3c4d-1111-4111-8111-111111111111"));
        assert!(!temporary_branch("t3/feature") && !temporary_branch("feature/x"));
        assert!(mismatch(None, Some("main"), Some("main")).is_null());
        assert_eq!(mismatch(None, Some("main"), Some("dev"))["canRestore"], true);
        assert_eq!(mismatch(Some("/w"), Some("t3code/fix"), Some("feature/checkout"))["canRestore"], false);
        assert!(mismatch(Some("/w"), Some("t3code/fix-sidebar"), Some("t3code/1a2b3c4d")).is_null());
        assert!(mismatch(None, None, Some("dev")).is_null());
    }
}
