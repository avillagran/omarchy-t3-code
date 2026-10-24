//! T3 server updates, after T3's `t3 update`: which connected servers are behind
//! the newest release on their channel and when an automatic update may run.
//! Fetching the release index and running the install stay with the host; the
//! install itself is always T3's own `t3 update`.
use serde_json::{json, Value};

/// Agent activity this recent keeps a server from updating by itself.
pub const QUIET_MS: i64 = 10 * 60 * 1000;
/// A failed automatic update of one version is tried again only after this long.
pub const RETRY_MS: i64 = 6 * 60 * 60 * 1000;
/// An update claimed this recently may still run (the host allows it 15 minutes).
pub const CLAIM_MS: i64 = 20 * 60 * 1000;

fn numeric(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) && (s == "0" || !s.starts_with('0'))
}
fn identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}
/// T3 isExactServiceVersion: strict semver, no leading zeros in numbers.
pub fn exact(version: &str) -> bool {
    let (rest, build) = version.split_once('+').map_or((version, None), |(a, b)| (a, Some(b)));
    if build.is_some_and(|b| !b.split('.').all(identifier)) {
        return false;
    }
    let (core, pre) = rest.split_once('-').map_or((rest, None), |(a, b)| (a, Some(b)));
    let parts: Vec<&str> = core.split('.').collect();
    parts.len() == 3
        && parts.iter().all(|p| numeric(p))
        && pre.is_none_or(|p| p.split('.').all(|i| identifier(i) && (numeric(i) || !i.bytes().all(|b| b.is_ascii_digit()))))
}
/// T3 cliReleaseChannelOf.
pub fn channel(version: &str) -> &'static str {
    let Some((_, pre)) = version.split_once('-') else { return "stable" };
    let mut parts = pre.split('.');
    match (parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some(c @ ("nightly" | "preview")), Some(date), Some(n), None)
            if date.len() == 8 && date.bytes().all(|b| b.is_ascii_digit()) && !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) =>
        {
            if c == "nightly" { "nightly" } else { "preview" }
        }
        _ => "stable",
    }
}
/// T3 compareExactServiceVersions.
pub fn compare(left: &str, right: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering::*;
    let parse = |v: &str| {
        let v = v.split('+').next().unwrap_or(v);
        let (core, pre) = v.split_once('-').map_or((v, ""), |(a, b)| (a, b));
        let core: Vec<u128> = core.split('.').map(|n| n.parse().unwrap_or(0)).chain([0, 0, 0]).take(3).collect();
        let pre: Vec<String> = if pre.is_empty() { vec![] } else { pre.split('.').map(String::from).collect() };
        (core, pre)
    };
    let (a, b) = (parse(left), parse(right));
    if a.0 != b.0 {
        return a.0.cmp(&b.0);
    }
    if a.1.is_empty() || b.1.is_empty() {
        return b.1.len().min(1).cmp(&a.1.len().min(1));
    }
    for i in 0..a.1.len().max(b.1.len()) {
        let (Some(x), Some(y)) = (a.1.get(i), b.1.get(i)) else {
            return if a.1.get(i).is_none() { Less } else { Greater };
        };
        if x == y {
            continue;
        }
        let (xn, yn) = (x.bytes().all(|c| c.is_ascii_digit()), y.bytes().all(|c| c.is_ascii_digit()));
        return match (xn, yn) {
            (true, true) => x.parse::<u128>().unwrap_or(0).cmp(&y.parse().unwrap_or(0)),
            (true, false) => Less,
            (false, true) => Greater,
            _ => x.cmp(y),
        };
    }
    Equal
}
/// T3 newestCliReleaseVersion: the first published release on the channel.
pub fn newest(releases: &Value, channel_name: &str) -> Option<String> {
    releases.as_array()?.iter().filter(|r| r["draft"] != true).find_map(|r| {
        let version = r["tag_name"].as_str()?.strip_prefix('v')?;
        (exact(version) && channel(version) == channel_name).then(|| version.to_string())
    })
}
/// One environment's update advisory. `latest` is null when it is current.
pub fn advisory(current: &str, latest: Option<&str>) -> Value {
    let newer = latest.filter(|l| exact(current) && compare(l, current).is_gt());
    json!({"current":current,"channel":channel(current),"latest":newer})
}
/// Whether an environment is quiet enough to restart its server unasked: no
/// agent working or paused on a question there, and nothing on it changed in
/// the last ten minutes. Threads only awaiting review, a plan or a retry run
/// nothing a restart could interrupt.
pub fn quiet(snapshot: &Value, environment: &str, now: i64) -> bool {
    snapshot["threads"].as_array().into_iter().flatten().filter(|t| t["environmentId"] == environment).all(|t| {
        t["working"] != true && t["reason"] != "request" && t["updatedAt"].as_i64().is_none_or(|at| now - at >= QUIET_MS)
    })
}
/// Whether the automatic update may run now for this environment, given its
/// last recorded attempt (`{target,status,at}`).
pub fn due(mode: &str, advisory: &Value, attempt: &Value, quiet: bool, now: i64) -> bool {
    let Some(target) = advisory["latest"].as_str() else { return false };
    let age = attempt["at"].as_i64().map(|at| now - at);
    mode == "auto"
        && advisory["canUpdate"] == true
        && quiet
        // Another ot3 process may still be running a claimed update.
        && !(attempt["status"] == "running" && age.is_some_and(|a| a < CLAIM_MS))
        && !(attempt["target"] == target && attempt["status"] != "running" && age.is_some_and(|a| a < RETRY_MS))
}
