//! The pinned native FilesystemGetMetadataResult codec boundary.
use crate::{file_metadata_cache::Metadata, Result};
use serde_json::Value;

fn javascript_space(c: char) -> bool {
    matches!(c,'\u{9}'..='\u{d}'|' '|'\u{a0}'|'\u{1680}'|'\u{2000}'..='\u{200a}'|'\u{2028}'|'\u{2029}'|'\u{202f}'|'\u{205f}'|'\u{3000}'|'\u{feff}')
}
pub fn decode(value: &Value) -> Result<Vec<Option<Metadata>>> {
    value["entries"]
        .as_array()
        .ok_or("malformed_metadata")?
        .iter()
        .map(|entry| {
            if entry.is_null() {
                return Ok(None);
            }
            let kind = entry["kind"]
                .as_str()
                .filter(|kind| matches!(*kind, "file" | "directory" | "other"))
                .ok_or("malformed_metadata")?;
            let byte_length = entry
                .get("byteLength")
                .map(|value| {
                    value
                        .as_f64()
                        .filter(|value| {
                            value.is_finite()
                                && *value >= 0.0
                                && *value <= 9_007_199_254_740_991.0
                                && value.fract() == 0.0
                        })
                        .map(|value| value as u64)
                        .ok_or("malformed_metadata")
                })
                .transpose()?;
            let mime_type = entry
                .get("mimeType")
                .map(|value| {
                    value
                        .as_str()
                        .map(|value| value.trim_matches(javascript_space))
                        .filter(|value| !value.is_empty())
                        .map(str::to_owned)
                        .ok_or("malformed_metadata")
                })
                .transpose()?;
            Ok(Some(Metadata {
                kind: kind.into(),
                byte_length,
                mime_type,
            }))
        })
        .collect()
}

fn non_negative_int(value: &Value) -> Result<u64> {
    value
        .as_f64()
        .filter(|value| {
            value.is_finite()
                && *value >= 0.0
                && *value <= 9_007_199_254_740_991.0
                && value.fract() == 0.0
        })
        .map(|value| value as u64)
        .ok_or_else(|| "malformed_project_read".into())
}
fn trimmed_non_empty(value: &Value) -> Result<String> {
    value
        .as_str()
        .map(|value| value.trim_matches(javascript_space))
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| "malformed_project_read".into())
}
fn optional_bool(input: &Value, output: &mut Value, name: &str) -> Result<()> {
    if let Some(value) = input.get(name) {
        output[name] = Value::Bool(value.as_bool().ok_or("malformed_project_read")?);
    }
    Ok(())
}
/// Decode the full native read/search/list result before using it to seed a
/// positive cache entry. A malformed successful wire reply must not seed.
pub fn decode_project(value: &Value, file: bool) -> Result<Value> {
    let truncated = value["truncated"]
        .as_bool()
        .ok_or("malformed_project_read")?;
    if file {
        let mut output = serde_json::json!({
            "relativePath":trimmed_non_empty(&value["relativePath"])?,
            "contents":value["contents"].as_str().ok_or("malformed_project_read")?,
            "byteLength":non_negative_int(&value["byteLength"])?,
            "truncated":truncated,
        });
        optional_bool(value, &mut output, "exists")?;
        Ok(output)
    } else {
        let entries = value["entries"]
            .as_array()
            .ok_or("malformed_project_read")?
            .iter()
            .map(|entry| {
                let kind = entry["kind"]
                    .as_str()
                    .filter(|kind| matches!(*kind, "file" | "directory"))
                    .ok_or("malformed_project_read")?;
                let mut output =
                    serde_json::json!({"path":trimmed_non_empty(&entry["path"])?,"kind":kind});
                optional_bool(entry, &mut output, "ignored")?;
                Ok(output)
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(serde_json::json!({"entries":entries,"truncated":truncated}))
    }
}
