//! T3 server updates, after T3's `t3 update`: which connected servers are behind
//! the newest release on their channel, and running the native update for one.
//! The install itself (download, checksum, pinned runtime, service restart) is
//! always T3's own `t3 update`; ot3 only picks the moment and the exact version.
use crate::*;
use serde_json::Value;
use std::{
    io::Write,
    process::{Command, Stdio},
    time::Duration,
};
pub use t3_protocol::server_updates::{advisory, channel, compare, due, exact, newest, quiet, CLAIM_MS, QUIET_MS, RETRY_MS};

const RELEASE_INDEX: &str = "https://api.github.com/repos/pingdotgg/t3code/releases";
const REMOTE_UPDATE: &str = include_str!("remote-update.sh");

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
            Some(12) => "t3_update_running",
            Some(124) => "t3_update_timeout",
            Some(255) => "ssh_unavailable",
            Some(127) => "t3_cli_unavailable",
            _ => "t3_update_failed",
        }
        .into());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
