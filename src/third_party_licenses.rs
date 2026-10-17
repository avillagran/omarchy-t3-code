//! Application-owned static notices, matching native T3's manifest contract.
//! Metadata and lazy notice reads need no environment, credentials or network.
use crate::{json, text, Result};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{collections::HashSet, sync::OnceLock};

static MANIFEST: OnceLock<Result<Value>> = OnceLock::new();
const RAW: &str = include_str!("../licenses/third-party-licenses.json");

/// Original encodeURIComponent(JSON.stringify([kind,name,version])) identity.
pub fn entry_key(entry: &Value) -> String {
    let raw = json!([entry["kind"], entry["name"], entry["version"]]).to_string();
    let mut out = String::new();
    for byte in raw.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&byte) {
            out.push(byte as char);
        } else {
            use std::fmt::Write;
            let _ = write!(out, "%{byte:02X}");
        }
    }
    out
}

/// Native decoding rules, plus bounded local IPC/resource sizes.
pub fn decode(value: &Value) -> Result<Value> {
    let entries = value["entries"].as_array().filter(|e| e.len() <= 4096);
    if value["schemaVersion"] != 1 || entries.is_none() {
        return Err("invalid_license_manifest".into());
    }
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for e in entries.unwrap() {
        if !matches!(text(e, "kind"), "custom" | "package")
            || !["name", "license", "noticeText"].iter().all(|k| e[k].is_string())
            || !e["bundles"].as_array().is_some_and(|a| !a.is_empty() && a.len() <= 64 && a.iter().all(Value::is_string))
            || !e.get("version").is_some_and(|v| v.is_null() || v.is_string())
            || !e.get("sourceUrl").is_some_and(|v| v.is_null() || v.as_str().is_some_and(|s| reqwest::Url::parse(s).is_ok_and(|u| matches!(u.scheme(), "http" | "https"))))
            || text(e,"noticeText").len() > 2 * 1024 * 1024
            || ["name", "license", "version", "sourceUrl"].iter().any(|k| text(e,k).len() > 4096)
        {
            return Err("invalid_license_manifest".into());
        }
        if !seen.insert(entry_key(e)) { return Err("duplicate_license_entry".into()); }
        out.push(json!({"kind":e["kind"],"name":e["name"],"version":e["version"],"license":e["license"],"bundles":e["bundles"],"noticeText":e["noticeText"],"sourceUrl":e["sourceUrl"]}));
    }
    Ok(json!({"schemaVersion":1,"entries":out}))
}
fn manifest() -> Result<&'static Value> {
    MANIFEST.get_or_init(|| {
        if RAW.len() > 8*1024*1024 { return Err("license_manifest_too_large".into()); }
        decode(&serde_json::from_str(RAW).map_err(|_| "invalid_license_manifest")?)
    }).as_ref().map_err(Clone::clone)
}
fn revision() -> String { format!("{:x}", Sha256::digest(RAW)) }
pub fn read() -> Result<Value> {
    let entries=manifest()?["entries"].as_array().unwrap().iter().map(|e| {
        let mut metadata=e.clone();metadata.as_object_mut().unwrap().remove("noticeText");metadata["entryKey"]=json!(entry_key(e));metadata
    }).collect::<Vec<_>>();
    Ok(json!({"manifestId":revision(),"scope":"application","entries":entries}))
}
pub fn notice(input: &Value) -> Result<Value> {
    if input["manifestId"] != revision() { return Err("license_manifest_changed".into()); }
    let key=text(input,"entryKey");
    if key.is_empty() || key.len()>32*1024 { return Err("invalid_license_entry".into()); }
    let e=manifest()?["entries"].as_array().unwrap().iter().find(|e| entry_key(e)==key).ok_or("unknown_license_entry")?;
    Ok(json!({"manifestId":revision(),"entryKey":key,"noticeText":e["noticeText"]}))
}
