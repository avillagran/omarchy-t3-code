//! Local thread metadata and sticky model state; prompt bodies stay in draft documents.
use crate::{native_settings, valid_id};
use serde_json::{json, Map, Value};
pub fn metadata(input: &Value) -> Value {
    let mut out = Map::new();
    for (key, row) in input.as_object().into_iter().flatten().take(256) {
        let Some((environment, thread)) = key.split_once('/') else {
            continue;
        };
        let Some(creation) = row["threadReservationId"]
            .as_str()
            .filter(|id| uuid::Uuid::parse_str(id).is_ok())
        else {
            continue;
        };
        if !valid_id(environment)
            || thread != format!("omarchy-thread-{creation}")
            || row["environmentId"] != environment
            || row["threadId"] != thread
            || !row["projectId"].as_str().is_some_and(valid_id)
            || !row["draftId"]
                .as_str()
                .is_some_and(|id| uuid::Uuid::parse_str(id).is_ok())
        {
            continue;
        }
        if !row["createdAt"]
            .as_str()
            .is_some_and(|s| chrono::DateTime::parse_from_rfc3339(s).is_ok())
        {
            continue;
        }
        let mut clean = json!({"environmentId":environment,"threadId":thread,"threadReservationId":creation,"draftId":row["draftId"],"projectId":row["projectId"],"createdAt":row["createdAt"],"title":row["title"].as_str().unwrap_or("New thread").chars().take(255).collect::<String>(),"runtimeMode":row["runtimeMode"].as_str().filter(|s|t3_protocol::models::valid_runtime_mode(s)).unwrap_or("approval-required"),"interactionMode":if row["interactionMode"]=="plan"{"plan"}else{"default"}});
        if let Some(explicit) = row["workspaceExplicit"].as_bool() {
            clean["workspaceExplicit"] = json!(explicit);
        }
        for field in ["branch", "worktreePath"] {
            if row[field].is_null()
                || row[field]
                    .as_str()
                    .is_some_and(|s| s.len() <= 4096 && !s.contains('\0'))
            {
                clean[field] = row[field].clone();
            }
        }
        if let Ok(selection) = native_settings::model_selection(&row["modelSelection"]) {
            clean["modelSelection"] = selection;
        }
        if row["workspaceStrategy"].is_object() {
            let strategy = &row["workspaceStrategy"];
            if matches!(
                strategy["type"].as_str(),
                Some("root" | "worktree" | "existing_worktree")
            ) {
                let mut value = json!({"type":strategy["type"]});
                for field in ["baseRef", "branch", "worktreePath"] {
                    if let Some(text) = strategy[field]
                        .as_str()
                        .filter(|s| s.len() <= 4096 && !s.chars().any(char::is_control))
                    {
                        value[field] = json!(text);
                    }
                }
                if let Some(origin) = strategy["startFromOrigin"].as_bool() {
                    value["startFromOrigin"] = json!(origin);
                }
                clean["workspaceStrategy"] = value;
            }
        }
        out.insert(key.clone(), clean);
    }
    Value::Object(out)
}
pub fn sticky(input: &Value) -> Value {
    let mut out = Map::new();
    for (provider, selection) in input.as_object().into_iter().flatten().take(128) {
        if let Ok(clean) = native_settings::model_selection(selection) {
            if clean["instanceId"] == provider.as_str() {
                out.insert(provider.clone(), clean);
            }
        }
    }
    Value::Object(out)
}

pub fn sticky_options(input: &Value) -> Value {
    let mut out = Map::new();
    for (provider, models) in input.as_object().into_iter().flatten().take(128) {
        let mut options = Map::new();
        for (model, traits) in models.as_object().into_iter().flatten().take(64) {
            if let Ok(clean) = native_settings::model_selection(
                &json!({"instanceId":provider,"model":model,"options":traits}),
            ) {
                if clean["options"]
                    .as_array()
                    .is_some_and(|items| !items.is_empty())
                {
                    options.insert(model.clone(), clean["options"].clone());
                }
            }
        }
        if !options.is_empty() {
            out.insert(provider.clone(), Value::Object(options));
        }
    }
    Value::Object(out)
}

/// A draft without a local owner may only be claimed when it contains no intent.
pub fn empty_document(value: &Value) -> bool {
    value["text"].as_str().unwrap_or("").trim().is_empty()
        && value["attachments"]
            .as_array()
            .is_none_or(|items| items.is_empty())
        && value["context"]["records"]
            .as_array()
            .is_none_or(|items| items.is_empty())
        && [
            "modelSelection",
            "attempt",
            "queueEdit",
            "sourcePlanRef",
            "runtimeMode",
            "interactionMode",
        ]
        .iter()
        .all(|key| value[*key].is_null())
        && value["delivery"] != "queue"
        && value["recoveryIds"]
            .as_array()
            .is_none_or(|items| items.is_empty())
        && ["questionnaires", "approvals"]
            .iter()
            .all(|key| value[*key].as_object().is_none_or(|items| items.is_empty()))
}

/// Read-only new-thread defaults, using the published shared settings resolver.
pub fn workspace_defaults(raw: &Value, project: Option<&Value>, file: Option<&Value>) -> Value {
    let (effective, sources, _) = t3_protocol::scoped_settings::effective(raw, project, file);
    let id = project
        .and_then(|project| project["id"].as_str())
        .unwrap_or("");
    let mut defaults = native_settings::thread_defaults(&effective, id);
    defaults["mode"] = effective["defaultThreadEnvMode"].clone();
    defaults["modeSource"] = sources["defaultThreadEnvMode"].clone();
    defaults["startFromOrigin"] = json!(effective["newWorktreesStartFromOrigin"]
        .as_bool()
        .unwrap_or(true));
    let stored = &raw["projectSettingsOverrides"][id];
    let mut configured = raw.get("defaultThreadEnvMode");
    if raw["projectSettingsFolded"] != true {
        if let Some(value) = project
            .and_then(|project| project.get("defaultThreadEnvMode"))
            .filter(|value| !value.is_null())
        {
            configured = Some(value);
        }
    }
    if let Some(value) = stored.get("defaultThreadEnvMode") {
        configured = Some(value);
    }
    defaults["consultProjectFile"] = json!(configured.is_none_or(Value::is_null));
    defaults["modelOverrideConfigured"] = json!(stored.get("defaultModelSelection").is_some());
    defaults["modeOverrideConfigured"] = json!(stored.get("defaultThreadEnvMode").is_some());
    defaults
}
