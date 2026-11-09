//! Minimal native fallback-selection inputs. Provider secrets and arbitrary
//! configuration never cross into the QML presentation layer.
use serde_json::{json, Map, Value};
fn instances(raw: &Value, include_driver: bool) -> Value {
    Value::Object(
        raw.as_object()
            .into_iter()
            .flatten()
            .map(|(id, config)| {
                let mut row = Map::new();
                if include_driver {
                    if let Some(driver) = config["driver"].as_str() {
                        row.insert("driver".into(), json!(driver));
                    }
                    if let Some(enabled) = config["config"]["enabled"].as_bool() {
                        row.insert("config".into(), json!({"enabled":enabled}));
                    }
                }
                if let Some(enabled) = config["enabled"].as_bool() {
                    row.insert("enabled".into(), json!(enabled));
                }
                (id.clone(), Value::Object(row))
            })
            .collect(),
    )
}
pub fn present(config: &Value, project: &str) -> Value {
    let raw = &config["settings"];
    let providers = config["providers"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|provider| {
            let mut row = Map::new();
            for key in [
                "instanceId",
                "driver",
                "displayName",
                "enabled",
                "installed",
                "availability",
                "status",
            ] {
                if let Some(value) = provider.get(key) {
                    row.insert(key.into(), value.clone());
                }
            }
            row.insert("models".into(),Value::Array(provider["models"].as_array().into_iter().flatten().map(|model| {
                let mut value=Map::new();
                for key in ["slug","isDefault","isCustom"] {if let Some(field)=model.get(key){value.insert(key.into(),field.clone());}}
                Value::Object(value)
            }).collect()));
            if let Some(limits) = provider.get("usageLimits") {
                let windows = limits["windows"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|window| {
                        Value::Object(
                            ["id", "usedPercent", "resetsAt"]
                                .into_iter()
                                .filter_map(|key| {
                                    window.get(key).map(|value| (key.into(), value.clone()))
                                })
                                .collect(),
                        )
                    })
                    .collect::<Vec<_>>();
                row.insert("usageLimits".into(), json!({"windows":windows}));
            }
            row.insert("auth".into(), json!({"status":provider["auth"]["status"]}));
            Value::Object(row)
        })
        .collect::<Vec<_>>();
    let fallback = crate::native_settings::model_selection(&raw["fallbackModelSelection"]).ok();
    json!({"providers":providers,"fallbackSelection":fallback,"settings":{
        "providerInstances":instances(&raw["providerInstances"],true),
        "projectSettingsOverrides":{project:{"providerInstances":instances(&raw["projectSettingsOverrides"][project]["providerInstances"],false)}}
    }})
}

/// The same observed configuration that supplies the model catalog can open a
/// local draft offline. Only model/workspace defaults and enablement cross to QML.
pub fn catalog_context(config:&Value)->Value {
    let raw=&config["settings"];
    let mut context=present(config,"");
    let mut defaults=Map::new();
    let mut overrides=Map::new();
    for (project,entry) in raw["projectSettingsOverrides"].as_object().into_iter().flatten() {
        defaults.insert(project.clone(),crate::local_drafts::workspace_defaults(raw,Some(&json!({"id":project})),None));
        overrides.insert(project.clone(),json!({"providerInstances":instances(&entry["providerInstances"],false)}));
    }
    context["settings"]["projectSettingsOverrides"]=Value::Object(overrides);
    context["defaults"]=crate::local_drafts::workspace_defaults(raw,None,None);
    context["projectDefaults"]=Value::Object(defaults);
    context["projectSettingsFolded"]=json!(raw["projectSettingsFolded"]==true);
    context["newProjectsRoot"]=json!(config["newProjectsRoot"].as_str());
    context
}
