//! Safe projection/validation of native client-local browser preferences.
//! Source constants are generated from pinned T3 contracts; no browser/profile
//! lifecycle is emulated here. T3/Qt runtimes own actual partitions and data.
use crate::{json, Result};
use serde_json::Value;
use std::sync::OnceLock;
pub const KEYS: &[&str] = &[
    "browserDefaultViewport",
    "browserDefaultZoomFactor",
    "browserDefaultAppearance",
    "browserRecordingFrameRate",
    "browserRecordingShowKeyPresses",
    "browserRecordingShowMousePresses",
    "browserLinkTarget",
    "browserAutoShowFloatingPreview",
    "browserProfiles",
    "browserDefaultProfileId",
    "browserSearchEngine",
    "browserCustomSearchUrl",
];
static CONTRACT: OnceLock<Value> = OnceLock::new();
pub fn contract() -> &'static Value {
    CONTRACT.get_or_init(|| {
        serde_json::from_str(include_str!("../data/browser-preferences.json"))
            .expect("checked native browser preference contract")
    })
}
fn trimmed(v: &Value, max: usize) -> Result<&str> {
    v.as_str()
        .filter(|s| {
            !s.is_empty() && s.trim() == *s && s.encode_utf16().count() <= max && !s.contains('\0')
        })
        .ok_or("invalid_client_preference".into())
}
fn profile_id(v: &Value) -> Result<&str> {
    let s = trimmed(v, 64)?;
    if s.chars().any(char::is_control) {
        return Err("invalid_client_preference".into());
    }
    Ok(s)
}
pub fn validate(key: &str, value: &Value) -> Result<Value> {
    let c = contract();
    let bad = || "invalid_client_preference".to_string();
    Ok(match key {
        "browserDefaultViewport" => {
            let tag = value["_tag"].as_str().ok_or_else(bad)?;
            if tag == "fill" {
                json!({"_tag":"fill"})
            } else {
                if !matches!(tag, "freeform" | "preset") {
                    return Err(bad());
                }
                let dim = |v: &Value| {
                    v.as_f64()
                        .filter(|v| {
                            v.is_finite()
                                && v.fract() == 0.0
                                && *v >= c["minDimension"].as_f64().unwrap()
                                && *v <= c["maxDimension"].as_f64().unwrap()
                        })
                        .map(|v| v as u64)
                        .ok_or_else(bad)
                };
                let w = dim(&value["width"])?;
                let h = dim(&value["height"])?;
                if w * h > c["maxArea"].as_u64().unwrap() {
                    return Err(bad());
                }
                let mut out = json!({"_tag":tag,"width":w,"height":h});
                if tag == "preset" {
                    let id = value["presetId"].as_str().ok_or_else(bad)?;
                    if !c["presetIds"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .chain(c["legacyPresetIds"].as_array().unwrap())
                        .any(|v| v == id)
                    {
                        return Err(bad());
                    }
                    out["presetId"] = json!(id)
                }
                out
            }
        }
        "browserDefaultZoomFactor" => {
            let number = value.as_f64().ok_or_else(bad)?;
            if !c["zoomLevels"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v.as_f64() == Some(number))
            {
                return Err(bad());
            }
            value.clone()
        }
        "browserDefaultAppearance" => {
            if !matches!(value.as_str(), Some("system" | "light" | "dark")) {
                return Err(bad());
            }
            value.clone()
        }
        "browserLinkTarget" => {
            if !matches!(value.as_str(), Some("system" | "app")) {
                return Err(bad());
            }
            value.clone()
        }
        "browserRecordingFrameRate" => {
            if !c["recordingFrameRates"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v.as_f64() == value.as_f64())
            {
                return Err(bad());
            }
            value.clone()
        }
        "browserRecordingShowKeyPresses"
        | "browserRecordingShowMousePresses"
        | "browserAutoShowFloatingPreview" => json!(value.as_bool().ok_or_else(bad)?),
        "browserDefaultProfileId" => json!(profile_id(value)?),
        "browserSearchEngine" => {
            if !value
                .as_str()
                .is_some_and(|s| crate::browser_address::SEARCH_ENGINES.contains(&s))
            {
                return Err(bad());
            }
            value.clone()
        }
        // Kept while another engine is chosen, so switching back restores it.
        "browserCustomSearchUrl" => {
            let url = value
                .as_str()
                .filter(|s| s.encode_utf16().count() <= 2048)
                .ok_or_else(bad)?;
            json!(url.trim())
        }
        "browserProfiles" => {
            // The native client schema accepts arrays beyond the UI create limit24;
            // preserve existing identities, while bounding IPC resource use.
            let rows = value
                .as_array()
                .filter(|v| v.len() <= 512)
                .ok_or_else(bad)?;
            let mut profiles = vec![];
            for row in rows {
                let id = profile_id(&row["id"])?;
                let name = trimmed(
                    &row["name"],
                    c["profileNameMaxLength"].as_u64().unwrap() as usize,
                )?;
                if !matches!(row["kind"].as_str(), Some("persistent" | "incognito")) {
                    return Err(bad());
                }
                profiles.push(json!({"id":id,"name":name,"kind":row["kind"]}));
            }
            json!(profiles)
        }
        _ => return Err(bad()),
    })
}
pub fn projection(raw: &Value) -> Value {
    let defaults = json!({"browserDefaultViewport":{"_tag":"fill"},"browserDefaultZoomFactor":1.0,"browserDefaultAppearance":"system","browserRecordingFrameRate":30,"browserRecordingShowKeyPresses":false,"browserRecordingShowMousePresses":false,"browserLinkTarget":"system","browserAutoShowFloatingPreview":true,"browserProfiles":[],"browserDefaultProfileId":"default","browserSearchEngine":"duckduckgo","browserCustomSearchUrl":""});
    let mut out = json!({});
    for key in KEYS {
        out[*key] = validate(key, &raw[*key]).unwrap_or_else(|_| defaults[*key].clone())
    }
    out
}

/// Original browserDefaults.ts:toBrowserDefaults at the pinned T3 revision.
/// Deleted defaults and Incognito fall back to Default; user profile kinds are
/// normalized to persistent by resolveBrowserProfiles, and built-ins win.
pub fn default_profile(settings: &Value) -> String {
    let id = settings["browserDefaultProfileId"]
        .as_str()
        .unwrap_or("default");
    if id != "incognito"
        && profile_id(&json!(id)).is_ok()
        && (id == "default"
            || settings["browserProfiles"]
                .as_array()
                .is_some_and(|rows| rows.iter().any(|p| p["id"] == id)))
    {
        id.into()
    } else {
        "default".into()
    }
}
