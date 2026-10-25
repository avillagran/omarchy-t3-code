//! Native checkpoint transport + draft recovery. Only explicit confirmation mutates T3.
use crate::*;
use crate::{checkpoint_journal as journal, checkpoint_revert as rewind};
use std::io::Cursor;
fn authorize(e: &Environment) -> Result<()> {
    let s = Client::shared()?.get(e, "/api/auth/session", false)?;
    if s["authenticated"] == true
        && s.get("permissions")
            .or_else(|| s.get("scopes"))
            .and_then(Value::as_array)
            .is_some_and(|p| p.iter().any(|v| v == "orchestration:operate"))
    {
        Ok(())
    } else {
        Err("revert_operate_required".into())
    }
}
fn projection(e: &Environment, thread: &str) -> Result<Value> {
    if !valid_id(thread) {
        return Err("revert_scope_mismatch".into());
    }
    let raw = protocol::api::orchestration::thread_projection(&e.connection(true)?, thread)?;
    let p = raw.get("projection").unwrap_or(&raw);
    if p["thread"]["id"] != thread {
        return Err("revert_scope_mismatch".into());
    }
    Ok(p.clone())
}
fn safety(e: &Environment, p: &Value, draft: &Value) -> Result<rewind::Safety> {
    let config = protocol::api::server::config(&e.connection(true)?)?;
    let instance = p["thread"]["providerInstanceId"]
        .as_str()
        .or(p["thread"]["modelSelection"]["instanceId"].as_str())
        .ok_or("revert_provider_unavailable")?;
    let provider = config["providers"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|v| v["instanceId"] == instance && v["enabled"] == true);
    // Native ChatView.tsx:3300: a present provider opts out only with explicit false.
    let active = !crate::active_run_id(p).is_empty();
    Ok(rewind::Safety {
        can_operate: true,
        connected: true,
        supports_conversation_rollback: provider
            .is_some_and(|p| p["supportsConversationRollback"] != false),
        working: active,
        send_busy: draft["attempt"].is_object() || draft["queueEdit"].is_object(),
        connecting: !crate::plan_workflow::latest_run_settled(p),
        pending_attachments: draft["attachments"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|a| a["uploading"] == true || a["error"].is_string()),
        rewinding: false,
    })
}
fn scope<'a>(e: &Environment, input: &'a Value) -> Result<(&'a str, String)> {
    let thread = text(input, "thread");
    if input["environment"] != e.id || !valid_id(thread) {
        return Err("revert_scope_mismatch".into());
    }
    Ok((thread, format!("{}/{thread}", e.id)))
}
fn request(input: &Value) -> Result<Value> {
    let r = json!({"environment":input["environment"],"thread":input["thread"],"requestId":input["requestId"],"confirmed":input["confirmed"],"restoreFiles":input["restoreFiles"],"reference":input["reference"]});
    rewind::confirmed_command(&r)?;
    Ok(r)
}
pub fn preview(e: &Environment, input: &Value) -> Result<Value> {
    let (thread, key) = scope(e, input)?;
    authorize(e)?;
    if let Some(r) = journal::read(&e.id, thread)? {
        if journal::pending(&r) {
            return Ok(
                json!({"environment":e.id,"thread":thread,"reference":r["request"]["reference"],"canRestoreFiles":false,"operation":journal::receipt(&r)}),
            );
        }
    }
    let p = projection(e, thread)?;
    let d = drafts::load(&key)?;
    let gates = safety(e, &p, &d)?;
    gates.check()?;
    let reference = rewind::reference(&p, text(input, "messageId"))?;
    Ok(
        json!({"environment":e.id,"thread":thread,"reference":reference,"canRestoreFiles":p["thread"]["worktreePath"].as_str().is_some_and(|s|!s.is_empty()),"operation":journal::read(&e.id,thread)?.as_ref().map(journal::receipt)}),
    )
}
fn observe_record(e: &Environment, record: &mut Value) -> Result<()> {
    if matches!(
        record["state"].as_str(),
        Some("complete" | "failed" | "reviewed" | "superseded")
    ) {
        return Ok(());
    }
    let p = projection(e, text(&record["request"], "thread"))?;
    let observation = rewind::observation(&record["request"], &p)?;
    match observation["state"].as_str() {
        Some("complete" | "failed" | "superseded") => {
            record["state"] = observation["state"].clone();
            record["error"] = observation.get("error").cloned().unwrap_or(json!(""));
        }
        _ => {
            if record["state"] == "prepared" {
                record["state"] = json!("uncertain");
            }
            let elapsed = Utc::now().timestamp_millis() - record["startedAt"].as_i64().unwrap_or(0);
            if elapsed > 120_000 {
                record["state"] = json!("uncertain");
            }
        }
    }
    journal::save(record)
}
pub fn observe(e: &Environment, input: &Value) -> Result<Value> {
    let (thread, _) = scope(e, input)?;
    let _guard = journal::lock(&e.id, thread)?;
    let Some(mut r) = journal::read(&e.id, thread)? else {
        return Ok(json!({"environment":e.id,"thread":thread,"state":"idle"}));
    };
    if input["operationId"].is_string() && input["operationId"] != r["request"]["requestId"] {
        return Err("revert_scope_mismatch".into());
    }
    if let Err(error) = observe_record(e, &mut r) {
        if journal::pending(&r) && r["state"] != "complete" {
            r["state"] = json!("uncertain");
            r["error"] = json!(error);
            journal::save(&r)?;
        } else {
            return Err(error);
        }
    }
    Ok(journal::receipt(&r))
}
/// Fetch fresh native assets and upload them before rollback. Source IDs alone
/// must not depend on the messages which the rollback will remove.
fn prepare_attachments(e: &Environment, source: &Value, current: &Value) -> Result<Vec<Value>> {
    let attachments = source.as_array().ok_or("revert_message_invalid")?;
    let current_files = current["attachments"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if current_files.len() + attachments.len() > 100 {
        return Err("invalid_attachments".into());
    }
    let mut all = current_files
        .iter()
        .map(|f| f["attachment"].clone())
        .collect::<Vec<_>>();
    all.extend(attachments.iter().cloned());
    write::validate_message(&json!({"text":"preflight","attachments":all}))?;
    if attachments.is_empty() {
        return Ok(vec![]);
    }
    let c = e.connection(true)?;
    let mut restored = vec![];
    for a in attachments {
        let expected = a["sizeBytes"].as_u64().ok_or("invalid_attachments")?;
        let resource = protocol::assets::attachment_resource(text(a, "id"), text(a, "name"), text(a, "mimeType"), false)?;
        // Exactly the recorded size: a longer body is cut at the limit, a shorter one differs.
        let bytes = c
            .asset_bytes(resource, expected)
            .map_err(|error| match error.as_str() {
                "asset_too_large" => "revert_attachment_changed".into(),
                "asset_unreachable" | "asset_unavailable" => "revert_attachment_unavailable".into(),
                _ => error,
            })?
            .bytes;
        if bytes.len() as u64 != expected {
            return Err("revert_attachment_changed".into());
        }
        let uploaded = c.upload_attachment(
            text(a, "name"),
            text(a, "mimeType"),
            expected,
            Cursor::new(bytes),
        )?;
        let descriptor = uploaded.get("attachment").unwrap_or(&uploaded);
        write::validate_message(&json!({"text":"preflight","attachments":[descriptor]}))?;
        if descriptor["type"] != a["type"]
            || descriptor["mimeType"] != a["mimeType"]
            || descriptor["sizeBytes"] != a["sizeBytes"]
        {
            return Err("revert_attachment_changed".into());
        }
        restored.push(json!({"uploadId":uuid::Uuid::new_v4().to_string(),"name":descriptor["name"],"uploading":false,"attachment":descriptor}));
    }
    Ok(restored)
}
pub fn dispatch(e: &Environment, input: &Value) -> Result<Value> {
    let (thread, key) = scope(e, input)?;
    let request = request(input)?;
    let _guard = journal::lock(&e.id, thread)?;
    authorize(e)?;
    if let Some(mut stored) = journal::action(text(&request, "requestId"))? {
        if stored["request"] != request
            || stored["envelope"]["environment"] != e.id
            || stored["envelope"]["command"] != rewind::confirmed_command(&request)?
        {
            return Err("retry_payload_changed".into());
        }
        if !journal::pending(&stored)
            || stored["state"] == "complete"
            || stored["state"] == "superseded"
        {
            return Ok(journal::receipt(&stored));
        }
        let current = journal::read(&e.id, thread)?;
        if current
            .as_ref()
            .is_none_or(|r| r["request"]["requestId"] != request["requestId"])
        {
            return Err("revert_superseded".into());
        }
        if input["retry"] != true {
            return Ok(journal::receipt(&stored));
        }
        // Read original outcome before an explicit, identical replay. Never rederive
        // the old reference: the source message may already have been rolled back.
        if let Err(error) = observe_record(e, &mut stored) {
            stored["state"] = json!("uncertain");
            stored["error"] = json!(error);
            journal::save(&stored)?;
            return Ok(journal::receipt(&stored));
        }

        if stored["state"] == "complete"
            || stored["state"] == "failed"
            || stored["state"] == "superseded"
        {
            return Ok(journal::receipt(&stored));
        }
        let outcome = write::dispatch_command(e, &stored["envelope"]["command"]);
        stored["state"] = json!(if outcome.is_ok() {
            "accepted"
        } else {
            "uncertain"
        });
        stored["error"] = json!(outcome.err().unwrap_or_default());
        journal::save(&stored)?;
        return Ok(journal::receipt(&stored));
    }
    if input["retry"] == true {
        return Err("revert_request_unavailable".into());
    }
    journal::ensure_available(&e.id, thread)?;
    let p = projection(e, thread)?;
    let current = drafts::load(&key)?;
    let gates = safety(e, &p, &current)?;
    let envelope = rewind::outbox_envelope(&e.id, &request, &p, &gates)?;
    let recovery = rewind::recovery_message(&p, &request["reference"])?;
    let restored = prepare_attachments(e, &recovery["attachments"], &current)?;
    let prompt = prompt_recall::recall(text(&recovery, "text"));
    let next = rewind::append_prompt(text(&current, "text"), &prompt);
    if next.chars().count() > 120000 {
        return Err("draft_too_large".into());
    }
    let mut record = json!({"request":request,"envelope":envelope,"recovery":recovery,"prompt":prompt,"attachments":restored,"state":"prepared","recovered":false,"error":"","startedAt":Utc::now().timestamp_millis()});
    journal::insert(&record)?;
    let result = write::dispatch_command(e, &record["envelope"]["command"]);
    record["state"] = json!(if result.is_ok() {
        "accepted"
    } else {
        "uncertain"
    });
    record["error"] = json!(result.err().unwrap_or_default());
    journal::save(&record)?;
    Ok(journal::receipt(&record))
}
/// Latest UI draft is frozen briefly for this local transaction. With no loaded
/// composer, restart recovery uses the latest disk draft. Recovery IDs prevent
/// late autosaves, duplicate receipts and restarts from appending twice.
pub fn recover(input: &Value) -> Result<Value> {
    let environment = text(input, "environment");
    let thread = text(input, "thread");
    let key = format!("{environment}/{thread}");
    let _guard = journal::lock(environment, thread)?;
    let mut r = journal::action(text(input, "operationId"))?.ok_or("revert_request_unavailable")?;
    if r["request"]["environment"] != environment
        || r["request"]["thread"] != thread
        || r["state"] != "complete"
    {
        return Err("revert_not_complete".into());
    }
    let preferred = input.get("draft").filter(|d| d.is_object());
    let saved = drafts::update(&key, |disk| {
        let mut next = drafts::clean(preferred.unwrap_or(disk))?;
        if next["attempt"].is_object() || next["queueEdit"].is_object() {
            return Err("revert_interrupt_first".into());
        }
        let mut markers = disk["recoveryIds"].as_array().cloned().unwrap_or_default();
        let id = r["request"]["requestId"].clone();
        if !markers.contains(&id) {
            markers.push(id)
        }
        let mut applied = next["recoveryIds"].as_array().cloned().unwrap_or_default();
        for id in markers {
            if applied.contains(&id) {
                continue;
            }
            let source = journal::action(id.as_str().ok_or("invalid_draft")?)?
                .ok_or("revert_request_unavailable")?;
            if source["state"] != "complete"
                || source["request"]["environment"] != environment
                || source["request"]["thread"] != thread
            {
                return Err("revert_scope_mismatch".into());
            }
            if next["attachments"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|a| a["uploading"] == true || a["error"].is_string())
            {
                return Err("revert_attachments_pending".into());
            }
            next["text"] = json!(rewind::append_prompt(
                text(&next, "text"),
                text(&source, "prompt")
            ));
            next["attachments"]
                .as_array_mut()
                .ok_or("invalid_draft")?
                .extend(
                    source["attachments"]
                        .as_array()
                        .ok_or("invalid_draft")?
                        .iter()
                        .cloned(),
                );
            let descriptors = next["attachments"]
                .as_array()
                .unwrap()
                .iter()
                .map(|a| a["attachment"].clone())
                .collect::<Vec<_>>();
            write::validate_message(&json!({"text":"preflight","attachments":descriptors}))?;
            applied.push(id);
        }
        next["recoveryIds"] = json!(applied);
        drafts::clean(&next)
    })?;
    r["recovered"] = json!(true);
    journal::save(&r)?;
    Ok(json!({"key":key,"draft":saved,"operation":journal::receipt(&r)}))
}
pub fn resolve(e: &Environment, input: &Value) -> Result<Value> {
    let (thread, _) = scope(e, input)?;
    if input["confirmed"] != true {
        return Err("revert_confirmation_required".into());
    }
    authorize(e)?;
    let _guard = journal::lock(&e.id, thread)?;
    let mut r = journal::read(&e.id, thread)?.ok_or("revert_request_unavailable")?;
    if input["operationId"] != r["request"]["requestId"] {
        return Err("revert_scope_mismatch".into());
    }
    if !matches!(r["state"].as_str(), Some("uncertain" | "superseded")) {
        return Err("revert_not_uncertain".into());
    }
    observe_record(e, &mut r)?;
    if r["state"] != "complete" {
        r["state"] = json!("reviewed");
        journal::save(&r)?
    }
    Ok(journal::receipt(&r))
}

/// Restart recovery is local and idempotent; never sends another native rollback.
pub fn recover_before_load(key: &str) -> Result<()> {
    let (environment, thread) = key.split_once('/').ok_or("invalid_draft_key")?;
    if let Some(r) = journal::read(environment, thread)? {
        if r["state"] == "complete" && r["recovered"] != true {
            recover(
                &json!({"environment":environment,"thread":thread,"operationId":r["request"]["requestId"]}),
            )?;
        }
    }
    Ok(())
}
