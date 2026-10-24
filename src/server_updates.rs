//! T3 server updates, after T3's `t3 update`: which connected servers are behind
//! the newest release on their channel, and running the native update for one.
//! The install itself (download, checksum, pinned runtime, service restart) is
//! always T3's own `t3 update`; ot3 only picks the moment and the exact version.
use crate::*;
use serde_json::{json, Value};
use std::{
    io::Write,
    process::{Command, Stdio},
    time::Duration,
};

const RELEASE_INDEX: &str = "https://api.github.com/repos/pingdotgg/t3code/releases";
const REMOTE_UPDATE: &str = include_str!("remote-update.sh");
/// Agent activity this recent keeps a server from updating by itself.
pub const QUIET_MS: i64 = 10 * 60 * 1000;
/// A failed automatic update of one version is tried again only after this long.
pub const RETRY_MS: i64 = 6 * 60 * 60 * 1000;

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
/// The newest release on a channel, walking the index like `t3 update`.
pub fn latest(channel_name: &str) -> Result<String> {
    let base = std::env::var("OMARCHY_T3_RELEASE_INDEX").unwrap_or_else(|_| RELEASE_INDEX.into());
    let http = reqwest::blocking::Client::builder().timeout(Duration::from_secs(30)).user_agent("omarchy-t3-code").build().map_err(|_| "release_index_unavailable")?;
    for page in 1..=10 {
        let mut request = http.get(format!("{base}?per_page=100&page={page}")).header("Accept", "application/vnd.github+json");
        if let Some(token) = ["GH_TOKEN", "GITHUB_TOKEN"].iter().find_map(|k| std::env::var(k).ok().filter(|t| !t.trim().is_empty())) {
            request = request.bearer_auth(token.trim());
        }
        let releases: Value = request.send().and_then(|r| r.error_for_status()).and_then(|r| r.json()).map_err(|_| "release_index_unavailable")?;
        if let Some(version) = newest(&releases, channel_name) {
            return Ok(version);
        }
        if releases.as_array().is_none_or(|r| r.is_empty()) {
            break;
        }
    }
    Err("release_not_found".into())
}
/// One environment's update advisory. `latest` is null when it is current.
pub fn advisory(current: &str, latest: Option<&str>) -> Value {
    let newer = latest.filter(|l| exact(current) && compare(l, current).is_gt());
    json!({"current":current,"channel":channel(current),"latest":newer})
}
/// Whether an environment is quiet enough to restart its server unasked: no
/// agent working there and nothing on it changed in the last ten minutes.
pub fn quiet(snapshot: &Value, environment: &str, now: i64) -> bool {
    snapshot["threads"].as_array().into_iter().flatten().filter(|t| t["environmentId"] == environment).all(|t| {
        t["working"] != true && t["pending"] != true && t["updatedAt"].as_i64().is_none_or(|at| now - at >= QUIET_MS)
    })
}
/// Whether the automatic update may run now for this environment, given its
/// last recorded attempt (`{target,status,at}`).
pub fn due(mode: &str, advisory: &Value, attempt: &Value, quiet: bool, now: i64) -> bool {
    let Some(target) = advisory["latest"].as_str() else { return false };
    mode == "auto"
        && advisory["canUpdate"] == true
        && quiet
        && !(attempt["target"] == target && attempt["at"].as_i64().is_some_and(|at| now - at < RETRY_MS))
}
/// How ot3 can reach this environment's host to update it, or why it cannot.
pub fn route(environment: &str) -> std::result::Result<&'static str, &'static str> {
    if environment == "local" {
        let s = backend_service::status();
        return match (s["owner"].as_str(), s["installed"] == true, s["canControl"] == true) {
            (Some("gui"), ..) => Err("backend_owned_by_gui"),
            (_, false, _) => Err("service_required"),
            (_, true, false) => Err("service_profile_mismatch"),
            _ => Ok("local"),
        };
    }
    connections::ssh_args_for(environment).map(|_| "ssh").map_err(|_| "ssh_environment_required")
}
/// Runs T3's own update for one environment, pinned to `target`, then waits
/// for the server to answer again. Interrupts what runs on that server.
pub fn run(environment: &str, target: &str) -> Result<String> {
    if !exact(target) {
        return Err("invalid_version".into());
    }
    let mut command = Command::new("timeout");
    match route(environment)? {
        "local" => {
            let id = fs::read_to_string(t3_home().join("userdata/environment-id")).map_err(|_| "t3_environment_mismatch")?;
            let id = id.trim();
            uuid::Uuid::parse_str(id).map_err(|_| "t3_environment_mismatch")?;
            command.args(["900s", "sh", "-s", "--", id, target]);
        }
        _ => {
            uuid::Uuid::parse_str(environment).map_err(|_| "invalid_t3_environment_id")?;
            // ssh joins its trailing words into one remote command line.
            command.args(["900s", "ssh"]).args(connections::ssh_args_for(environment)?).arg(format!("sh -s -- '{environment}' '{target}'"));
        }
    }
    command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
    let mut child = command.spawn().map_err(|_| "t3_update_unavailable")?;
    child.stdin.take().ok_or("t3_update_unavailable")?.write_all(REMOTE_UPDATE.as_bytes()).map_err(|_| "t3_update_unavailable")?;
    let output = child.wait_with_output().map_err(|_| "t3_update_unavailable")?;
    connections::request_reconnect(environment);
    refresh_connections();
    if !output.status.success() {
        return Err(match output.status.code() {
            Some(7) => "t3_environment_mismatch",
            Some(8) => "service_profile_mismatch",
            Some(9) => "t3_remote_start_failed",
            Some(10) => "t3_update_failed",
            Some(11) => "service_required",
            Some(124) => "t3_update_timeout",
            Some(255) => "ssh_unavailable",
            Some(127) => "t3_cli_unavailable",
            _ => "t3_update_failed",
        }
        .into());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
