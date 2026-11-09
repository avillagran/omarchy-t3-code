//! Search payloads stay validated raw JSON across Unicode-scalar host metadata.
//! Packed strings contain JSON syntax, never repaired native UTF-16 strings.
use crate::{
    protocol::{wtf16::Wtf16String, RawJson},
    Result,
};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

pub type Object = BTreeMap<String, Box<RawJson>>;
pub fn encode(value: &impl Serialize) -> Result<Box<RawJson>> {
    serde_json::value::to_raw_value(value).map_err(|_| "invalid_search_json".into())
}
pub fn object(raw: &RawJson) -> Result<Object> {
    serde_json::from_str(raw.get()).map_err(|_| "invalid_search_json".into())
}
pub fn array(raw: &RawJson) -> Result<Vec<Box<RawJson>>> {
    serde_json::from_str(raw.get()).map_err(|_| "invalid_search_json".into())
}
pub fn field<'a>(map: &'a Object, name: &str) -> Result<&'a RawJson> {
    map.get(name)
        .map(Box::as_ref)
        .ok_or_else(|| "invalid_search_json".into())
}
pub fn scalar<T: serde::de::DeserializeOwned>(raw: &RawJson) -> Result<T> {
    serde_json::from_str(raw.get()).map_err(|_| "invalid_search_json".into())
}
pub fn request(input: &Value) -> Result<Object> {
    if let Some(packed) = input.get("payloadJson") {
        let packed = packed.as_str().ok_or("invalid_search_json")?;
        let raw = RawJson::from_string(packed.to_owned()).map_err(|_| "invalid_search_json")?;
        object(&raw)
    } else {
        object(&encode(input)?)
    }
}
/// Scalar routing fields are separate from the complete native row. A field
/// outside Rust's scalar string domain stays exclusively in the raw channel.
fn routing_metadata(raw: &RawJson) -> Result<Option<Value>> {
    match raw.get().as_bytes().first() {
        Some(b'"') => {
            let units = Wtf16String::from_json(raw).map_err(|_| "invalid_search_json")?;
            Ok(units.to_utf8().ok().map(Value::String))
        }
        Some(b'{') => {
            let mut fields = serde_json::Map::new();
            for (key, value) in object(raw)? {
                if let Some(value) = routing_metadata(&value)? {
                    fields.insert(key, value);
                }
            }
            Ok(Some(Value::Object(fields)))
        }
        Some(b'[') => {
            let values = array(raw)?
                .iter()
                .map(|value| routing_metadata(value).map(|value| value.unwrap_or(Value::Null)))
                .collect::<Result<Vec<_>>>()?;
            Ok(Some(Value::Array(values)))
        }
        _ => scalar(raw).map(Some),
    }
}
pub fn row_metadata(raw: &RawJson) -> Result<Value> {
    let source = object(raw)?;
    let id: String = scalar(field(&source, "id")?)?;
    Wtf16String::from_json(field(&source, "title")?).map_err(|_| "malformed_shell_page")?;
    if !crate::valid_id(&id) {
        return Err("malformed_shell_page".into());
    }
    let mut metadata = serde_json::Map::new();
    for (key, value) in &source {
        if let Some(value) = routing_metadata(value)? {
            metadata.insert(key.clone(), value);
        }
    }
    metadata.insert(
        "o3SearchRowJson".into(),
        Value::String(raw.get().to_owned()),
    );
    Ok(Value::Object(metadata))
}
pub fn decorate_item(row: &Value, environment: &str, item: &mut Value) -> Result<()> {
    let Some(packed) = row.get("o3SearchRowJson").and_then(Value::as_str) else {
        return Ok(());
    };
    let raw = RawJson::from_string(packed.to_owned()).map_err(|_| "malformed_shell_page")?;
    let source = object(&raw)?;
    let id: String = scalar(field(&source, "id")?)?;
    if row["id"].as_str() != Some(id.as_str()) {
        return Err("malformed_shell_page".into());
    }
    let mut shell = Object::new();
    shell.insert("environmentId".into(), encode(&environment)?);
    for key in [
        "id",
        "title",
        "branch",
        "createdAt",
        "updatedAt",
        "latestUserMessageAt",
        "latestUserAuthoredMessageAt",
        "lastVisitedAt",
        "opens",
        "pullRequests",
        "linkedPullRequest",
        "branchPullRequest",
    ] {
        if let Some(value) = source.get(key) {
            shell.insert(key.into(), value.clone());
        }
    }
    if let Some(title) = source.get("title") {
        Wtf16String::from_json(title).map_err(|_| "malformed_shell_page")?;
        item["searchTitleJson"] = Value::String(title.get().to_owned());
    }
    if let Some(branch) = source.get("branch") {
        // Branch is nullable in the native shell contract.
        if branch.get() != "null" {
            Wtf16String::from_json(branch).map_err(|_| "malformed_shell_page")?;
        }
        item["searchBranchJson"] = Value::String(branch.get().to_owned());
    }
    item["searchShellJson"] = Value::String(encode(&shell)?.get().to_owned());
    Ok(())
}

/// Recognize the three raw-capable pipe operations before Value decoding. A
/// legacy top-level request gets the same packed representation as Qt sends.
/// Control identifiers remain scalar metadata; data strings stay inside JSON.
pub fn pipe_input(line: &str) -> Result<Value> {
    let raw = RawJson::from_string(line.to_owned()).map_err(|_| "invalid_command")?;
    let fields = object(&raw)?;
    let op: String = scalar(field(&fields, "op")?)?;
    if !matches!(
        op.as_str(),
        "thread-search" | "thread-search-mount" | "thread-visit"
    ) || fields.contains_key("payloadJson")
    {
        return scalar(&raw);
    }
    let mut envelope = serde_json::Map::new();
    for key in ["op", "requestId", "environment", "thread"] {
        if let Some(value) = fields.get(key) {
            envelope.insert(key.into(), scalar(value)?);
        }
    }
    envelope.insert("payloadJson".into(), Value::String(line.to_owned()));
    Ok(Value::Object(envelope))
}

/// A native title may live in the raw channel when it has no Rust String form.
/// The source row must still have the same validated identifier and String type.
pub fn has_raw_title(row: &Value) -> Result<bool> {
    let Some(packed) = row.get("o3SearchRowJson").and_then(Value::as_str) else {
        return Ok(false);
    };
    let raw = RawJson::from_string(packed.to_owned()).map_err(|_| "malformed_shell_page")?;
    let fields = object(&raw)?;
    let id: String = scalar(field(&fields, "id")?)?;
    if row["id"].as_str() != Some(id.as_str()) {
        return Err("malformed_shell_page".into());
    }
    Wtf16String::from_json(field(&fields, "title")?).map_err(|_| "malformed_shell_page")?;
    Ok(true)
}
