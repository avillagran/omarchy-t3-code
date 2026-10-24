//! T3's usage page data (server.getUsageSummary), summed in Rust so the panel only
//! draws totals, a daily series and the top models.
use crate::{json, text, write::rpc, Environment, Result};
use chrono::Local;
use serde_json::Value;
pub use t3_protocol::usage_summary::{aggregate, request, window};

/// IANA zone of this machine, as the server buckets days in it.
pub fn time_zone() -> String {
    if let Ok(tz) = std::env::var("TZ") {
        let tz = tz.trim_start_matches(':');
        if !tz.is_empty() && !tz.starts_with('/') {
            return tz.to_string();
        }
    }
    std::fs::read_link("/etc/localtime")
        .ok()
        .and_then(|p| p.to_str().and_then(|s| s.split("zoneinfo/").nth(1)).map(str::to_string))
        .unwrap_or_else(|| "UTC".into())
}
pub fn read(e: &Environment, input: &Value) -> Result<Value> {
    // Model prices come from a public table the server caches; refresh it on request.
    if input["refreshRates"] == true {
        rpc(e, "server.refreshUsageRates", json!({}))?;
    }
    let (since, until) = window(text(input, "period"), Local::now().date_naive())?;
    let summary = rpc(
        e,
        "server.getUsageSummary",
        json!({"sinceDay":since.format("%Y-%m-%d").to_string(),"untilDay":until.format("%Y-%m-%d").to_string(),"timeZone":time_zone(),"resolution":"day"}),
    )?;
    let mut out = aggregate(&summary, since, until);
    out["period"] = input["period"].clone();
    Ok(out)
}

