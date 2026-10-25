//! Storage cleanup run and its last report (server.runStorageCleanup /
//! server.getStorageCleanupReport), as on T3's storage settings page.
use crate::{json, protocol::api::{diagnostics, streams}, text, Environment, Result};
use serde_json::Value;
use std::{cell::Cell, time::{Duration, Instant}};

fn summary(report: &Value) -> Value {
    if report.is_null() {
        return Value::Null;
    }
    let entries: Vec<Value> = report["entries"].as_array().into_iter().flatten().take(200).cloned().collect();
    json!({"trigger":report["trigger"],"finishedAt":report["finishedAt"],"counts":report["counts"],"bytesFreed":report["bytesFreed"],"omittedCount":report["omittedCount"],"entries":entries})
}
fn last_report(e: &Environment) -> Result<Value> {
    let c = e.connection(true)?;
    let done = Cell::new(false);
    let started = Instant::now();
    let mut report = Value::Null;
    // Nothing within 10 s is no report yet, not an error (unlike `streams::storage_cleanup_report`).
    streams::subscribe_storage_cleanup_report(
        &c,
        || done.get() || started.elapsed() > Duration::from_secs(10),
        || false,
        |item| {
            if let Some(v) = item {
                report = v;
                done.set(true);
            }
            Ok(())
        },
    )?;
    Ok(report)
}
pub fn run(e: &Environment, input: &Value) -> Result<Value> {
    let report = match text(input, "action") {
        "run" => diagnostics::run_storage_cleanup(&e.connection(true)?)?,
        "report" => last_report(e)?,
        _ => return Err("invalid_command".into()),
    };
    Ok(json!({"report":summary(&report)}))
}
