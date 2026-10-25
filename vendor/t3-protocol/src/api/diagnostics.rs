//! `diagnostics`: host resources, process diagnostics, telemetry, usage and
//! storage cleanup (all `server.*` tags). The scopes are mixed on purpose:
//! `getHostResources` is an operating read, `signalProcess` needs
//! `environment:maintain`, and the OTLP probe and cleanup need `settings:write`.
use super::reader::{int, one_of, put};
use super::{api_fn, ids, send, Function};
use crate::{Connection, Result};
use serde_json::{json, Value};

pub const SIGNALS: &[&str] = &["SIGINT", "SIGKILL"];
pub const OTLP_SIGNALS: &[&str] = &["traces", "metrics", "logs"];
pub const USAGE_RESOLUTIONS: &[&str] = &["day", "hour"];

pub const GET_HOST_RESOURCES: Function = api_fn! {
    name: "server.getHostResources", module: "diagnostics", tag: Some("server.getHostResources"),
    kind: Read, scope: Fixed("orchestration:read"),
    args: [], payload: |_| Ok(json!({})),
};
pub const GET_PROCESS_DIAGNOSTICS: Function = api_fn! {
    name: "server.getProcessDiagnostics", module: "diagnostics", tag: Some("server.getProcessDiagnostics"),
    kind: Read, scope: Fixed("diagnostics:read"),
    args: [], payload: |_| Ok(json!({})),
};
pub const GET_RESOURCE_TELEMETRY_HISTORY: Function = api_fn! {
    name: "server.getResourceTelemetryHistory", module: "diagnostics", tag: Some("server.getResourceTelemetryHistory"),
    kind: Read, scope: Fixed("diagnostics:read"),
    args: [(windowMs, Integer, required), (bucketMs, Integer, required)],
    payload: |a| Ok(telemetry_history_payload(a.req_u64("windowMs")?, a.req_u64("bucketMs")?)),
};
pub const SIGNAL_PROCESS: Function = api_fn! {
    name: "server.signalProcess", module: "diagnostics", tag: Some("server.signalProcess"),
    kind: IdempotentMutation, scope: Fixed("environment:maintain"),
    args: [(pid, Integer, required), (startTimeMs, Integer, required), (signal, Enum(SIGNALS), required)],
    payload: |a| signal_process_payload(a.req_u64("pid")?, a.req_u64("startTimeMs")?, a.req("signal")?),
};
pub const CHECK_OTLP_ENDPOINT: Function = api_fn! {
    name: "server.checkOtlpEndpoint", module: "diagnostics", tag: Some("server.checkOtlpEndpoint"),
    kind: Read, scope: Fixed("settings:write"),
    args: [(signal, Enum(OTLP_SIGNALS), required), (url, Text, required)],
    payload: |a| check_otlp_endpoint_payload(a.req("signal")?, a.req("url")?),
};
pub const GET_USAGE_SUMMARY: Function = api_fn! {
    name: "server.getUsageSummary", module: "diagnostics", tag: Some("server.getUsageSummary"),
    kind: Read, scope: Fixed("diagnostics:read"),
    args: [
        (sinceDay, Text, required), (untilDay, Text, required), (timeZone, Text, required),
        (resolution, Enum(USAGE_RESOLUTIONS), optional), (sinceTime, Text, optional), (untilTime, Text, optional),
        (awaitRefresh, Bool, optional),
    ],
    payload: |a| UsageQuery {
        since_day: a.req("sinceDay")?.into(),
        until_day: a.req("untilDay")?.into(),
        time_zone: a.req("timeZone")?.into(),
        resolution: a.str("resolution")?.map(Into::into),
        since_time: a.str("sinceTime")?.map(Into::into),
        until_time: a.str("untilTime")?.map(Into::into),
        await_refresh: a.bool("awaitRefresh")?,
    }
    .payload(),
};
pub const REFRESH_USAGE_RATES: Function = api_fn! {
    name: "server.refreshUsageRates", module: "diagnostics", tag: Some("server.refreshUsageRates"),
    kind: Read, scope: Fixed("diagnostics:read"),
    args: [], payload: |_| Ok(json!({})),
};
pub const RUN_STORAGE_CLEANUP: Function = api_fn! {
    name: "server.runStorageCleanup", module: "diagnostics", tag: Some("server.runStorageCleanup"),
    kind: UnrepeatableMutation, scope: Fixed("settings:write"),
    args: [], payload: |_| Ok(json!({})),
};
pub const FUNCTIONS: &[Function] = &[
    GET_HOST_RESOURCES, GET_PROCESS_DIAGNOSTICS, GET_RESOURCE_TELEMETRY_HISTORY, SIGNAL_PROCESS,
    CHECK_OTLP_ENDPOINT, GET_USAGE_SUMMARY, REFRESH_USAGE_RATES, RUN_STORAGE_CLEANUP,
];

/// `server.getHostResources`: `{sampledAt, cpuUtilization, cpuCount, availableMemoryBytes, totalMemoryBytes}`.
pub fn host_resources(conn: &Connection) -> Result<Value> {
    send(conn, &GET_HOST_RESOURCES, json!({}))
}
/// `server.getProcessDiagnostics`: `{serverPid, readAt, processCount, …, processes[], error}`.
/// `error` keeps Effect's `Option` encoding (`{_tag:"Some",value}` / `{_tag:"None"}`).
pub fn processes(conn: &Connection) -> Result<Value> {
    send(conn, &GET_PROCESS_DIAGNOSTICS, json!({}))
}
/// `server.getResourceTelemetryHistory`. The server clamps `window_ms` to
/// 1 s..1 h and `bucket_ms` to 1 s..`window_ms`.
pub fn telemetry_history(conn: &Connection, window_ms: u64, bucket_ms: u64) -> Result<Value> {
    send(conn, &GET_RESOURCE_TELEMETRY_HISTORY, telemetry_history_payload(window_ms, bucket_ms))
}
fn telemetry_history_payload(window_ms: u64, bucket_ms: u64) -> Value {
    json!({"windowMs":window_ms,"bucketMs":bucket_ms})
}
/// `server.signalProcess`: `{pid, signal, signaled, message}`. The `pid` +
/// `start_time_ms` pair keeps a recycled pid safe, so a repeat is harmless.
pub fn signal_process(conn: &Connection, pid: u64, start_time_ms: u64, signal: &str) -> Result<Value> {
    send(conn, &SIGNAL_PROCESS, signal_process_payload(pid, start_time_ms, signal)?)
}
fn signal_process_payload(pid: u64, start_time_ms: u64, signal: &str) -> Result<Value> {
    Ok(json!({"pid":int("pid", pid, 1, u32::MAX as u64)?,"startTimeMs":start_time_ms,"signal":one_of("signal", signal, SIGNALS)?}))
}
/// `server.checkOtlpEndpoint`: `{_tag:"Accepted",latencyMs}` / `{_tag:"Rejected",status}` /
/// `{_tag:"Unreachable",timedOut}`. A probe without effects, but guarded by `settings:write`.
pub fn check_otlp_endpoint(conn: &Connection, signal: &str, url: &str) -> Result<Value> {
    send(conn, &CHECK_OTLP_ENDPOINT, check_otlp_endpoint_payload(signal, url)?)
}
fn check_otlp_endpoint_payload(signal: &str, url: &str) -> Result<Value> {
    let url = ids::text("url", url, 2048)?;
    let lower = url.to_ascii_lowercase();
    let rest = lower.strip_prefix("https://").or_else(|| lower.strip_prefix("http://"));
    if rest.is_none_or(str::is_empty) || url.chars().any(char::is_control) {
        return Err(ids::invalid("url"));
    }
    Ok(json!({"signal":one_of("signal", signal, OTLP_SIGNALS)?,"url":url}))
}

/// `UsageSummaryInput`. Days are `YYYY-MM-DD` in `time_zone` (IANA; the server
/// falls back to UTC for an unknown zone). `since_time`/`until_time` (ISO instants)
/// only apply to `resolution: "hour"`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UsageQuery {
    pub since_day: String,
    pub until_day: String,
    pub time_zone: String,
    pub resolution: Option<String>,
    pub since_time: Option<String>,
    pub until_time: Option<String>,
    pub await_refresh: Option<bool>,
}
fn day<'a>(field: &str, value: &'a str) -> Result<&'a str> {
    let v = value.trim();
    let b = v.as_bytes();
    let ok = b.len() == 10 && b[4] == b'-' && b[7] == b'-' && b.iter().enumerate().all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit());
    if ok {
        Ok(v)
    } else {
        Err(ids::invalid(field))
    }
}
impl UsageQuery {
    pub fn payload(&self) -> Result<Value> {
        let since = day("sinceDay", &self.since_day)?;
        let until = day("untilDay", &self.until_day)?;
        if since > until {
            return Err(ids::invalid("untilDay"));
        }
        let mut v = json!({"sinceDay":since,"untilDay":until,"timeZone":ids::text("timeZone", &self.time_zone, 64)?});
        put(&mut v, "resolution", self.resolution.as_deref().map(|r| one_of("resolution", r, USAGE_RESOLUTIONS)).transpose()?);
        put(&mut v, "sinceTime", self.since_time.as_deref().map(|t| ids::text("sinceTime", t, 64)).transpose()?);
        put(&mut v, "untilTime", self.until_time.as_deref().map(|t| ids::text("untilTime", t, 64)).transpose()?);
        put(&mut v, "awaitRefresh", self.await_refresh);
        Ok(v)
    }
}
/// `server.getUsageSummary`: `{contractVersion, readAt, timeZone, sinceDay, untilDay, buckets[], sources[], pricing, scanDurationMs}`.
/// `usage_summary` derives the native view.
pub fn usage_summary(conn: &Connection, query: &UsageQuery) -> Result<Value> {
    send(conn, &GET_USAGE_SUMMARY, query.payload()?)
}
/// `server.refreshUsageRates`: `UsagePricing{status, source, fetchedAt, knownModels}`.
/// Refreshes a public price table on the server; no user state changes.
pub fn refresh_usage_rates(conn: &Connection) -> Result<Value> {
    send(conn, &REFRESH_USAGE_RATES, json!({}))
}
/// `server.runStorageCleanup`: `StorageCleanupReport`. Deletes worktrees and logs:
/// never repeat it blindly after `rpc_outcome_unknown`.
pub fn run_storage_cleanup(conn: &Connection) -> Result<Value> {
    send(conn, &RUN_STORAGE_CLEANUP, json!({}))
}
