//! Native server preferences exposed to the QML settings views. Credentials and
//! arbitrary provider configuration are never projected into these descriptors.
use crate::util::*;
use serde_json::{json, Value};
pub const MODEL_KEYS: &[(&str, &str)] = &[
    ("defaultModelSelection", "Model"),
    ("textGenerationModelSelection", "Text generation model"),
    ("sourceControlWriterModelSelection", "Source control writer model"),
    ("reviewGuideModelSelection", "Review guide model"),
];
const TITLE_INSTRUCTIONS_LIMIT: usize = 16384;
fn identifier(value: &str) -> bool {
    !value.trim().is_empty() && value.trim()==value && value.len()<=2048 && !value.chars().any(char::is_control)
}
pub fn model_selection(value: &Value) -> Result<Value> {
    let instance=value["instanceId"].as_str().or(value["provider"].as_str()).ok_or("invalid_model")?;
    let model=value["model"].as_str().ok_or("invalid_model")?;
    if !identifier(instance)||!identifier(model){return Err("invalid_model".into())}
    let mut out=json!({"instanceId":instance,"model":model});
    if let Some(raw)=value.get("options") {
        let options=raw.as_array().filter(|v|v.len()<=64).ok_or("invalid_model_options")?;
        let mut ids=std::collections::HashSet::new();let mut selections=vec![];
        for option in options {
            let id=option["id"].as_str().filter(|s|identifier(s)&&ids.insert(s.to_string())).ok_or("invalid_model_options")?;
            let v=&option["value"];
            if !v.is_boolean()&&!v.as_str().is_some_and(identifier){return Err("invalid_model_options".into())}
            selections.push(json!({"id":id,"value":v}));
        }
        out["options"]=json!(selections);
    }
    Ok(out)
}
pub fn description(key: &str) -> &'static str {
    match key {
        "defaultModelSelection"=>"Default model for new threads. Projects can override it.",
        "observability.otlpTracesUrl"=>"Send traces, metrics, and logs to an OTLP HTTP receiver. Restart the server to apply.",
        "defaultRuntimeMode"=>"Default permissions for new threads. Projects can override them.",
        "defaultThreadEnvMode"=>"Where new threads start. Projects and their t3.json can override it.",
        "worktreeSubmodules"=>"How new worktrees populate git submodules. Projects can override it.",
        "autoResumeLimitedThreads"=>"Resume usage-limit stops at the reported reset time. Each thread can cancel its scheduled continuation.",
        "snoozeLimitedThreads"=>"Move usage-limit stops to Snoozed until the reset time. Combine with auto-resume to wake and continue them.",
        "sidebarAutoSettleOnMerge"=>"Automatically settle threads after their pull requests are merged.",
        "sidebarAutoSettleAfterDays"=>"Automatically settle threads after this many days of inactivity.",
        "sidebarAutoSettleKeepPullRequests"=>"Keep threads with open pull requests out of automatic settlement.",
        "responseStreamingMode"=>"Choose when responses become visible while the agent is working.",
        "enableProviderUpdateChecks"=>"Check installed providers for updates in the background.",
        "continueThreadsAfterServerUpdate"=>"Continue active threads after the server restarts for an update.",
        "backgroundActivityProfile"=>"Balance background refresh activity with performance and battery use.",
        "newWorktreesStartFromOrigin"=>"Start new worktrees from the remote base branch when available.",
        "defaultAutoPull"=>"Keeps the default branch current when the checkout has no local changes or commits. Projects can override it.",
        "removeAgentCreditsOnMerge"=>"Remove recognized agent credit lines from merge messages, keeping human co-authors.",
        "pullRequestMergeMethod"=>"Pull requests start with this method. Last used reuses the most recent choice on this device.",
        "branchNamingMode"=>"Choose how branches created for new worktrees are named.",
        "branchNamePrefix"=>"Prefix added to automatically generated worktree branch names.",
        "branchNameInstructions"=>"Instructions for generating worktree branch names.",
        "textGenerationModelSelection"=>"Model used for titles and other generated text.",
        "idleCompaction"=>"When to compact an idle Claude thread over 200,000 tokens: after you return, just before its prompt cache expires, or never. Each Claude provider can override this.",
        "threadTitleInstructions"=>"Appended to the prompt for new and regenerated thread titles.",
        "sourceControlWriterModelSelection"=>"Model used to write commit and pull request messages.",
        "reviewGuideModelSelection"=>"Model used to generate pull request review guides.",
        "enableAgentBrowserAccess"=>"Allow agents to use the shared browser. Projects can override it.",
        "enableAgentDeviceAccess"=>"Allow new agent sessions in this environment to start and control local and remote devices, with required tools set up automatically.",
        _=>"",
    }
}
const IDLE_COMPACTION_MODES: &[(&str, &str)] = &[
    ("onReturn", "When you return"),
    ("beforeCacheExpires", "Before the cache expires (beta)"),
    ("off", "Never"),
];
fn idle_compaction_mode(value: &Value) -> Option<&str> {
    value.as_str().filter(|mode| IDLE_COMPACTION_MODES.iter().any(|(m, _)| m == mode))
}
/// Whether the server stores modes; older ones keep only the on/off `compactIdleThreads`.
fn reads_idle_compaction_modes(raw: &Value) -> bool {
    raw.get("idleCompaction").is_some_and(|v| !v.is_null()) || raw["compactIdleThreads"].is_string()
}
/// T3 globalIdleCompaction: `idleCompaction`, else the legacy mode or switch.
pub fn idle_compaction(raw: &Value) -> Option<&str> {
    if let Some(mode) = idle_compaction_mode(&raw["idleCompaction"]) { return Some(mode) }
    match &raw["compactIdleThreads"] {
        Value::Bool(true) => Some("beforeCacheExpires"),
        Value::Bool(false) => Some("onReturn"),
        value => idle_compaction_mode(value),
    }
}
/// T3 resolveIdleCompaction: a Claude instance may override the environment's
/// mode in its config; "default" or no override falls back to the global mode.
pub fn resolve_idle_compaction<'a>(raw: &'a Value, instance: &str) -> Option<&'a str> {
    idle_compaction_mode(&raw["providerInstances"][instance]["config"]["idleCompaction"]).or_else(|| idle_compaction(raw))
}
/// T3 idleCompactionPatch: write the mode in the form this server reads. Shipped
/// clients decode `compactIdleThreads` as a boolean, so it never carries a mode
/// unless the server already stored one there.
pub fn idle_compaction_patch(raw: &Value, patch: &Value) -> Result<Value> {
    let mode = idle_compaction_mode(&patch["patch"]["idleCompaction"]).ok_or("invalid_settings")?;
    Ok(if raw.get("idleCompaction").is_some_and(|v| !v.is_null()) {
        json!({"patch":{"idleCompaction":mode}})
    } else if raw["compactIdleThreads"].is_string() {
        json!({"patch":{"compactIdleThreads":mode}})
    } else if mode == "off" {
        return Err("idle_compaction_unsupported".into())
    } else {
        json!({"patch":{"compactIdleThreads":mode=="beforeCacheExpires"}})
    })
}
pub fn snapshot(raw: &Value) -> Value {
    let mut view=crate::settings::snapshot(raw);
    let fields=view["fields"].as_array_mut().unwrap();
    for (key,label,default) in [("enableAgentBrowserAccess","Agent browser access",true),("enableAgentDeviceAccess","Agent device access",false)] {
        if let Some(value)=raw[key].as_bool() {
            fields.push(json!({"key":key,"label":label,"section":"integrations","kind":"bool","value":value,"resetValue":default}));
        }
    }
    for (key,label) in MODEL_KEYS {
        if let Some(v)=raw.get(*key) {
            let selection=if v.is_null(){Some(Value::Null)}else{model_selection(v).ok()};
            if let Some(selection)=selection {
                let mut field=json!({"key":key,"label":label,"section":if *key=="sourceControlWriterModelSelection"||*key=="reviewGuideModelSelection"{"source-control"}else{"general"},"kind":"model","value":selection});
                if *key!="textGenerationModelSelection"{field["resetValue"]=Value::Null;}
                fields.push(field);
            }
        }
    }
    // T3 threadTitleInstructions (0.1.3): only servers that report it can apply it.
    if let Some(v)=raw["threadTitleInstructions"].as_str() {
        fields.push(json!({"key":"threadTitleInstructions","label":"Thread title instructions","section":"general","kind":"text","multiline":true,"maximumLength":TITLE_INSTRUCTIONS_LIMIT,"placeholder":"Keep titles to a few words. Lead with the main subject.","value":v}));
    }
    if let Some(mode)=idle_compaction(raw) {
        // A server from before modes cannot store "Never".
        let choices=IDLE_COMPACTION_MODES.iter().filter(|(m,_)|*m!="off"||reads_idle_compaction_modes(raw)).map(|(id,name)|json!({"id":id,"name":name})).collect::<Vec<_>>();
        fields.push(json!({"key":"idleCompaction","label":"Compact idle threads","kind":"choice","section":"general","value":mode,"resetValue":"onReturn","choices":choices}));
    }
    if let Some(mode)=raw["defaultRuntimeMode"].as_str().filter(|s|crate::models::valid_runtime_mode(s)) {
        fields.push(json!({"key":"defaultRuntimeMode","label":"Permissions","kind":"choice","section":"general","value":mode,"resetValue":"full-access","choices":[{"id":"approval-required","name":"Supervised"},{"id":"auto-accept-edits","name":"Auto-accept edits"},{"id":"auto","name":"Automatic"},{"id":"full-access","name":"Full access"}]}));
    }
    if raw.get("defaultThreadEnvMode").is_none_or(|v|v.is_null()||matches!(v.as_str(),Some("local"|"worktree"))) {
        fields.push(json!({"key":"defaultThreadEnvMode","label":"Workspace","kind":"choice","section":"general","value":raw["defaultThreadEnvMode"],"resetValue":null,"choices":[{"id":null,"name":"Automatic"},{"id":"local","name":"Current checkout"},{"id":"worktree","name":"New worktree"}]}));
    }
    fields.extend(crate::storage_settings::fields(raw));
    // OTLP export endpoints (T3 telemetry export); applied when the server restarts.
    if raw["observability"].is_object() {
        for (member,label) in [("otlpTracesUrl","Traces endpoint"),("otlpMetricsUrl","Metrics endpoint"),("otlpLogsUrl","Logs endpoint")] {
            fields.push(json!({"key":format!("observability.{member}"),"label":label,"section":"general","kind":"text","commitOnBlur":true,"placeholder":"https://collector.example.com/v1/","value":raw["observability"][member].as_str().unwrap_or(""),"resetValue":""}));
        }
    }
    for field in fields {
        let detail=description(text(field,"key"));if !detail.is_empty(){field["description"]=json!(detail);}
    }
    view
}
pub fn patch(input:&Value,config:Option<&Value>)->Result<Value> {
    let key=text(input,"key");let value=input.get("value").ok_or("invalid_settings")?;
    if crate::storage_settings::is_key(key) {
        let config=config.ok_or("storage_settings_unsupported")?;
        let capability=crate::storage_settings::capability(key);
        if config["environment"]["capabilities"][capability]!=true
            || key=="storageCleanup.worktreeKeepWhen"&&config["environment"]["capabilities"]["storageCleanupRun"]!=true {
            return Err("storage_settings_unsupported".into());
        }
        let patch=crate::storage_settings::patch(input)?;
        return crate::storage_settings::environment_patch(&config["settings"],key,&patch);
    }
    if let Some(member)=key.strip_prefix("observability.") {
        if !matches!(member,"otlpTracesUrl"|"otlpMetricsUrl"|"otlpLogsUrl"){return Err("invalid_settings".into())}
        let url=value.as_str().map(str::trim).ok_or("invalid_settings")?;
        if !url.is_empty()&&(url.len()>2048||!(url.starts_with("http://")||url.starts_with("https://"))||url.chars().any(char::is_control)){return Err("invalid_settings".into())}
        return Ok(json!({"patch":{"observability":{member:url}}}))
    }
    if MODEL_KEYS.iter().any(|(k,_)|*k==key) {
        if value.is_null() {
            if key=="textGenerationModelSelection"{return Err("invalid_model".into())}
            return Ok(json!({"patch":{key:null}}))
        }
        let selection=model_selection(value)?;
        if let Some(config)=config {
            let catalog=crate::models::catalog(config);
            let model=catalog["models"].as_array().into_iter().flatten().find(|m|m["instanceId"]==selection["instanceId"]&&m["model"]==selection["model"]&&m["available"]!=false).ok_or("model_unavailable")?;
            for option in selection["options"].as_array().into_iter().flatten() {
                let descriptor=model["descriptors"].as_array().into_iter().flatten().find(|d|d["id"]==option["id"]).ok_or("invalid_model_options")?;
                let valid=match text(descriptor,"type") {"boolean"=>option["value"].is_boolean(),"select"=>descriptor["options"].as_array().is_some_and(|v|v.iter().any(|o|o["id"]==option["value"])),_=>false};
                if !valid{return Err("invalid_model_options".into())}
            }
        }
        return Ok(json!({"patch":{key:selection}}))
    }
    if key=="idleCompaction" {
        let mode=idle_compaction_mode(value).ok_or("invalid_settings")?;
        return Ok(json!({"patch":{key:mode}}))
    }
    if key=="threadTitleInstructions" {
        let text=value.as_str().filter(|s|s.chars().count()<=TITLE_INSTRUCTIONS_LIMIT&&!s.contains('\0')).ok_or("invalid_settings")?;
        return Ok(json!({"patch":{key:text.trim()}}))
    }
    let valid=match key {
        "enableAgentBrowserAccess"|"enableAgentDeviceAccess"=>value.is_boolean(),
        "defaultRuntimeMode"=>value.as_str().is_some_and(crate::models::valid_runtime_mode),
        "defaultThreadEnvMode"=>value.is_null()||matches!(value.as_str(),Some("local"|"worktree")),
        _=>return crate::settings::patch(input),
    };
    if !valid{return Err("invalid_settings".into())}
    Ok(json!({"patch":{key:value}}))
}
/// Native project-over-environment defaults for fresh threads. A nullable model
/// override means Automatic, not "inherit". Disabled provider overrides fall back.
pub fn thread_defaults(raw:&Value,project:&str)->Value {
    let overrides=&raw["projectSettingsOverrides"][project];
    let enabled=|selection:&Value|->bool{let id=text(selection,"instanceId");overrides["providerInstances"][id]["enabled"].as_bool().or(raw["providerInstances"][id]["enabled"].as_bool()).unwrap_or(true)};
    let global=raw.get("defaultModelSelection").filter(|v|v.is_null()||enabled(v));
    let chosen=overrides.get("defaultModelSelection").filter(|v|v.is_null()||enabled(v)).or(global);
    let model=chosen.filter(|v|!v.is_null()).and_then(|v|model_selection(v).ok());
    let mode=overrides["defaultRuntimeMode"].as_str().or(raw["defaultRuntimeMode"].as_str()).filter(|s|crate::models::valid_runtime_mode(s));
    json!({"defaultModelSelection":model,"modelDefaultConfigured":chosen.is_some(),"defaultRuntimeMode":mode})
}
