//! T3's process diagnostics: the server's child processes with CPU and memory, and
//! interrupting or killing one (server.getProcessDiagnostics / server.signalProcess).
use crate::{json, text, write::rpc, Environment, Result};
use serde_json::Value;

pub fn run(e: &Environment, input: &Value) -> Result<Value> {
    if text(input, "action") == "signal" {
        let signal = match text(input, "signal") {
            "SIGINT" => "SIGINT",
            "SIGKILL" => "SIGKILL",
            _ => return Err("invalid_command".into()),
        };
        let pid = input["pid"].as_u64().filter(|p| *p > 1).ok_or("invalid_command")?;
        let start = input["startTimeMs"].as_u64().ok_or("invalid_command")?;
        rpc(e, "server.signalProcess", json!({"pid":pid,"startTimeMs":start,"signal":signal}))?;
    }
    let d = rpc(e, "server.getProcessDiagnostics", json!({}))?;
    let processes: Vec<Value> = d["processes"]
        .as_array()
        .into_iter()
        .flatten()
        .take(200)
        .map(|p| json!({"pid":p["pid"],"startTimeMs":p["startTimeMs"],"depth":p["depth"],"cpu":p["cpuPercent"],"rss":p["rssBytes"],"elapsed":p["elapsed"],"command":text(p,"command").chars().take(300).collect::<String>(),"status":p["status"]}))
        .collect();
    let error = d["error"]["value"]["message"].as_str().or_else(|| d["error"]["message"].as_str());
    // The last half hour per minute (server.getResourceTelemetryHistory), when the server keeps it.
    let history: Vec<Value> = rpc(e, "server.getResourceTelemetryHistory", json!({"windowMs":1_800_000,"bucketMs":60_000}))
        .map(|h| h["buckets"].as_array().into_iter().flatten().map(|b| json!({"cpu":b["avgCpuPercent"],"rss":b["maxRssBytes"]})).collect())
        .unwrap_or_default();
    Ok(json!({"history":history,"serverPid":d["serverPid"],"count":d["processCount"],"rss":d["totalRssBytes"],"cpu":d["totalCpuPercent"],"processes":processes,"error":error}))
}
