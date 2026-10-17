//! Durable per-thread prompt drafts. Independent from presentation preferences.
use omarchy_t3_client::*;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{fs, io::Read, os::unix::fs::PermissionsExt};
fn path(key: &str) -> Result<std::path::PathBuf> {
    let (env, id) = key.split_once('/').ok_or("invalid_draft_key")?;
    if !valid_id(env) || !valid_id(id) {
        return Err("invalid_draft_key".into());
    }
    Ok(xdg("XDG_STATE_HOME", ".local/state")
        .join("drafts")
        .join(format!("{:x}.json", Sha256::digest(key.as_bytes()))))
}
pub fn clean(v: &Value) -> Result<Value> {
    if !v.is_object() || (v.get("text").is_some() && !v["text"].is_string()) {return Err("invalid_draft".into())}
    let raw=v["text"].as_str().unwrap_or("");
    let repaired=if v["attempt"].is_object(){None}else{message_text::plain_qt(raw)};
    let text=repaired.as_deref().unwrap_or(raw);
    if text.chars().count() > 120000 {
        return Err("draft_too_large".into());
    }
    let mut out = json!({"text":text,"attachments":[],"modelSelection":null,"delivery":if v["delivery"]=="queue"{"queue"}else{"auto"},"attempt":null});
    if let Some(mode)=v.get("runtimeMode").filter(|v|!v.is_null()){
        if !omarchy_t3_client::protocol::models::valid_runtime_mode(mode.as_str().unwrap_or("")){return Err("invalid_runtime_mode".into())}
        out["runtimeMode"]=mode.clone();
    }
    if let Some(s) = v["modelSelection"].as_object() {
        let options = s
            .get("options")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if options.len() > 64 {
            return Err("invalid_draft".into());
        }
        out["modelSelection"] = json!({"instanceId":s.get("instanceId"),"model":s.get("model"),"options":options.iter().map(|o|json!({"id":o["id"],"value":o["value"]})).collect::<Vec<_>>()});
    }
    for f in v["attachments"].as_array().into_iter().flatten().take(100) {
        let a = &f["attachment"];
        let mut file = json!({"uploadId":f["uploadId"],"name":f["name"],"uploading":false,"retained":f["retained"]==true});
        if a.is_object() {
            file["attachment"] = json!({"type":a["type"],"id":a["id"],"name":a["name"],"mimeType":a["mimeType"],"sizeBytes":a["sizeBytes"]});
        }
        if f["uploading"] == true {
            file["error"] = json!("upload_interrupted")
        } else if f["error"].is_string() {
            file["error"] = f["error"].clone()
        }
        out["attachments"].as_array_mut().unwrap().push(file);
    }
    if v["queueEdit"].is_object(){
        let q=&v["queueEdit"];if !valid_id(text_field(q,"runId")){return Err("invalid_draft".into())}
        let b=&q["backup"];
        let backup=clean(&json!({"text":b["text"],"attachments":b["attachments"],"modelSelection":b["modelSelection"],"delivery":b["delivery"]}))?;
        out["queueEdit"]=json!({"runId":q["runId"],"messageId":q["messageId"],"context":q["context"],"backup":backup});
    }
    if v["attempt"].is_object() {
        let a = &v["attempt"];
        uuid::Uuid::parse_str(text_field(a, "requestId")).map_err(|_| "invalid_draft")?;
        if !(a["op"]=="send" || (a["op"]=="manage"&&a["action"]=="new-thread"&&valid_id(text_field(a,"projectId"))) || (a["op"]=="queue"&&a["action"]=="edit"&&a["runId"]==out["queueEdit"]["runId"])) || a["text"] != out["text"] {
            return Err("invalid_draft".into());
        }
        if a["op"]=="manage" {uuid::Uuid::parse_str(text_field(a,"sendRequestId")).map_err(|_| "invalid_draft")?;}
        let mut attempt = json!({});
        for key in [
            "op",
            "action",
            "runId",
            "context",
            "environment",
            "thread",
            "requestId",
            "text",
            "attachments",
            "modelSelection",
            "delivery",
            "nextThread",
            "sendRequestId",
            "moveSource",
            "creationRequestId",
            "workspaceStrategy",
            "launchWithMessage",
            "projectId",
            "title",
            "runtimeMode",
            "branch",
            "worktreePath",
        ] {
            if let Some(value) = a.get(key) {
                attempt[key] = value.clone();
            }
        }
        out["attempt"] = attempt;
    }
    if let Some(questions)=v["questionnaires"].as_object() {
        if questions.len()>32 || v["questionnaires"].to_string().len()>480000 {return Err("invalid_draft".into())}
        out["questionnaires"]=v["questionnaires"].clone();
    }
    if let Some(approvals)=v["approvals"].as_object() {
        if approvals.len()>32 || v["approvals"].to_string().len()>96000 {return Err("invalid_draft".into())}
        out["approvals"]=v["approvals"].clone();
    }
    if out.to_string().len() > 750000 {
        return Err("draft_too_large".into());
    }
    Ok(out)
}
fn text_field<'a>(v: &'a Value, k: &str) -> &'a str {
    v[k].as_str().unwrap_or("")
}
pub fn load(key: &str) -> Result<Value> {
    let path = path(key)?;
    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return clean(&json!({})),
        Err(_) => return Err("draft_read_failed".into()),
    };
    let mut bytes = vec![];
    file.take(800001)
        .read_to_end(&mut bytes)
        .map_err(|_| "draft_read_failed")?;
    if bytes.len() > 800000 {
        return Err("draft_read_failed".into());
    }
    let doc: Value = serde_json::from_slice(&bytes).map_err(|_| "draft_read_failed")?;
    if doc["key"] != key || doc["version"] != 1 || !doc["draft"].is_object() {
        return Err("draft_read_failed".into());
    }
    clean(&doc["draft"])
}
pub fn save(key: &str, value: &Value) -> Result<()> {
    let path = path(key)?;
    let value = clean(value)?;
    if value["attempt"].is_object()
        && format!(
            "{}/{}",
            text_field(&value["attempt"], "environment"),
            text_field(&value["attempt"], "thread")
        ) != key
    {
        return Err("invalid_draft_key".into());
    }
    let dir = path.parent().ok_or("draft_save_failed")?;
    fs::create_dir_all(dir).map_err(|_| "draft_save_failed")?;
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700)).map_err(|_| "draft_save_failed")?;
    save_json(&path, &json!({"version":1,"key":key,"draft":value}))
        .map_err(|_| "draft_save_failed")?;
    fs::File::open(dir)
        .and_then(|d| d.sync_all())
        .map_err(|_| "draft_save_failed".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn creation_and_transfer_keep_distinct_retry_ids() {
        let creation=uuid::Uuid::new_v4().to_string();
        let send=uuid::Uuid::new_v4().to_string();
        let draft=json!({"text":"Move me","attempt":{"op":"manage","action":"new-thread","environment":"local","thread":"source","requestId":creation,"sendRequestId":send,"projectId":"p","text":"Move me","attachments":[],"branch":"main","runtimeMode":"full-access","workspaceStrategy":{"type":"worktree","baseRef":"main"},"launchWithMessage":true}});
        let saved=clean(&draft).unwrap();
        assert_eq!(saved["attempt"]["sendRequestId"],send);
        assert_eq!(saved["attempt"]["projectId"],"p");
        assert_eq!(saved["attempt"]["branch"],"main");
        assert_eq!(saved["attempt"]["workspaceStrategy"],json!({"type":"worktree","baseRef":"main"}));
        assert_eq!(saved["attempt"]["launchWithMessage"],true);
        let mut destination=saved.clone();
        destination["attempt"]["op"]=json!("send");
        destination["attempt"]["thread"]=json!("destination");
        destination["attempt"]["requestId"]=json!(send);
        destination["attempt"]["moveSource"]=json!("local/source");
        destination["attempt"]["creationRequestId"]=json!(creation);
        let saved=clean(&destination).unwrap();
        assert_eq!(saved["attempt"]["moveSource"],"local/source");
        assert_eq!(saved["attempt"]["creationRequestId"],creation);
        let mut invalid=draft;invalid["attempt"]["sendRequestId"]=json!("bad");
        assert!(clean(&invalid).is_err());
    }
}
