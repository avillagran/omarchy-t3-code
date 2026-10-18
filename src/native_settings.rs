//! Native server preferences exposed to the QML settings views. Credentials and
//! arbitrary provider configuration are never projected into these descriptors.
use crate::*;
const MODEL_KEYS: &[(&str, &str)] = &[
    ("defaultModelSelection", "Model"),
    ("textGenerationModelSelection", "Text generation model"),
    ("sourceControlWriterModelSelection", "Source control writer model"),
    ("reviewGuideModelSelection", "Review guide model"),
];
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
        "sourceControlWriterModelSelection"=>"Model used to write commit and pull request messages.",
        "reviewGuideModelSelection"=>"Model used to generate pull request review guides.",
        "enableAgentBrowserAccess"=>"Allow agents to use the shared browser. Projects can override it.",
        "enableAgentDeviceAccess"=>"Allow new agent sessions in this environment to start and control local and remote devices, with required tools set up automatically.",
        _=>"",
    }
}
pub fn snapshot(raw: &Value) -> Value {
    let mut view=protocol::settings::snapshot(raw);
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
    if let Some(mode)=raw["defaultRuntimeMode"].as_str().filter(|s|protocol::models::valid_runtime_mode(s)) {
        fields.push(json!({"key":"defaultRuntimeMode","label":"Permissions","kind":"choice","section":"general","value":mode,"resetValue":"full-access","choices":[{"id":"approval-required","name":"Supervised"},{"id":"auto-accept-edits","name":"Auto-accept edits"},{"id":"auto","name":"Automatic"},{"id":"full-access","name":"Full access"}]}));
    }
    if raw.get("defaultThreadEnvMode").is_none_or(|v|v.is_null()||matches!(v.as_str(),Some("local"|"worktree"))) {
        fields.push(json!({"key":"defaultThreadEnvMode","label":"Workspace","kind":"choice","section":"general","value":raw["defaultThreadEnvMode"],"resetValue":null,"choices":[{"id":null,"name":"Automatic"},{"id":"local","name":"Current checkout"},{"id":"worktree","name":"New worktree"}]}));
    }
    fields.extend(crate::storage_settings::fields(raw));
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
    if MODEL_KEYS.iter().any(|(k,_)|*k==key) {
        if value.is_null() {
            if key=="textGenerationModelSelection"{return Err("invalid_model".into())}
            return Ok(json!({"patch":{key:null}}))
        }
        let selection=model_selection(value)?;
        if let Some(config)=config {
            let catalog=protocol::models::catalog(config);
            let model=catalog["models"].as_array().into_iter().flatten().find(|m|m["instanceId"]==selection["instanceId"]&&m["model"]==selection["model"]&&m["available"]!=false).ok_or("model_unavailable")?;
            for option in selection["options"].as_array().into_iter().flatten() {
                let descriptor=model["descriptors"].as_array().into_iter().flatten().find(|d|d["id"]==option["id"]).ok_or("invalid_model_options")?;
                let valid=match text(descriptor,"type") {"boolean"=>option["value"].is_boolean(),"select"=>descriptor["options"].as_array().is_some_and(|v|v.iter().any(|o|o["id"]==option["value"])),_=>false};
                if !valid{return Err("invalid_model_options".into())}
            }
        }
        return Ok(json!({"patch":{key:selection}}))
    }
    let valid=match key {
        "enableAgentBrowserAccess"|"enableAgentDeviceAccess"=>value.is_boolean(),
        "defaultRuntimeMode"=>value.as_str().is_some_and(protocol::models::valid_runtime_mode),
        "defaultThreadEnvMode"=>value.is_null()||matches!(value.as_str(),Some("local"|"worktree")),
        _=>return protocol::settings::patch(input),
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
    let mode=overrides["defaultRuntimeMode"].as_str().or(raw["defaultRuntimeMode"].as_str()).filter(|s|protocol::models::valid_runtime_mode(s));
    json!({"defaultModelSelection":model,"modelDefaultConfigured":chosen.is_some(),"defaultRuntimeMode":mode})
}
pub fn execute(e:&Environment,input:&Value)->Result<Value>{
    if text(input,"action")=="update"&&text(input,"key")=="enableAgentDeviceAccess" {
        return Err("device_configuration_required".into());
    }
    match text(input,"action") {
        "read"=>{},
        "update"=>{
            let config=if crate::storage_settings::is_key(text(input,"key"))||MODEL_KEYS.iter().any(|(key,_)|*key==text(input,"key"))&&!input["value"].is_null(){Some(crate::write::rpc(e,"server.getConfig",json!({}))?)}else{None};
            crate::write::rpc(e,"server.updateSettings",patch(input,config.as_ref())?)?;
        },
        _=>return Err("invalid_settings".into()),
    }
    crate::write::rpc(e,"server.getSettings",json!({})).map(|v|snapshot(&v))
}
