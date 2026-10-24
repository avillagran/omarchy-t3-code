//! Dev servers the environment sees listening (subscribeDiscoveredLocalServers), offered
//! under the browser's address bar as in T3.
use crate::{json, Environment, Result};
use serde_json::Value;
use std::{cell::Cell, time::{Duration, Instant}};

pub fn read(e: &Environment) -> Result<Value> {
    let c = e.connection(true)?;
    let done = Cell::new(false);
    let started = Instant::now();
    let mut list = Value::Null;
    // The first list can predate the scan; wait briefly for one with servers.
    let result = c.subscribe("subscribeDiscoveredLocalServers", json!({}), || done.get() || started.elapsed() > Duration::from_secs(4), |item| {
        if let Some(v) = item {
            if v["servers"].as_array().is_some_and(|s| !s.is_empty()) {
                done.set(true);
            }
            list = v;
        }
        Ok(())
    });
    if list.is_null() {
        result?;
    }
    let servers: Vec<Value> = list["servers"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|s| s["url"].as_str().is_some_and(|u| u.starts_with("http://") || u.starts_with("https://")))
        .take(20)
        .map(|s| json!({"url":s["url"],"port":s["port"],"processName":s["processName"]}))
        .collect();
    Ok(json!({"servers":servers}))
}
