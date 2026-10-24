//! Provider instances use T3's atomic mutation contract. The native view is a
//! bounded, non-secret projection of the upstream provider client definitions.
use crate::util::*;
use serde_json::{json, Value};
use std::collections::HashSet;

fn drivers() -> Value {
    serde_json::from_str(include_str!("../data/provider-drivers.json")).expect("pinned provider definitions")
}
fn driver<'a>(registry: &'a Value, kind: &str) -> Option<&'a Value> {
    registry["drivers"].as_array()?.iter().find(|d| text(d,"id")==kind)
}
fn visible_instance(id:&str,instance:&Value,config:&Value,definition:Option<&Value>,is_default:bool) -> Value {
    let kind=text(instance,"driver");
    let status=config["providers"].as_array().into_iter().flatten().find(|p|p["instanceId"]==id);
    let label=instance["displayName"].as_str().filter(|s|!s.is_empty())
        .or_else(||definition.and_then(|d|d["label"].as_str())).unwrap_or(kind);
    let mut fields=json!({});let mut secrets=json!({});
    for field in definition.into_iter().flat_map(|d|d["fields"].as_array().into_iter().flatten()) {
        let key=text(field,"key");
        let value=&instance["config"][key];
        if matches!(text(field,"control"),"secret"|"password") {
            secrets[key]=json!(value.as_str().is_some_and(|s|!s.is_empty()));
        } else if value.is_string() || value.is_boolean() {fields[key]=value.clone();}
    }
    let env=instance["environment"].as_array().into_iter().flatten().filter_map(|v|{
        let name=text(v,"name");if name.is_empty(){return None}
        let sensitive=v["sensitive"]==true;
        Some(json!({"name":name,"value":if sensitive {""} else {text(v,"value")},
            "sensitive":sensitive,"configured":sensitive&&(v["valueRedacted"]==true||!text(v,"value").is_empty())}))
    }).collect::<Vec<_>>();
    let enabled=if instance["enabled"]==false||instance["config"]["enabled"]==false {false}
        else {instance["enabled"].as_bool().or(instance["config"]["enabled"].as_bool()).unwrap_or_else(||definition.is_none_or(|d|d["defaultEnabled"]==true))};
    json!({"id":id,"driver":kind,"label":label,"enabled":enabled,"config":fields,"secrets":secrets,"environment":env,
        "accentColor":instance["accentColor"].as_str().unwrap_or(""),"isDefault":is_default,
        "installed":status.map(|p|&p["installed"]).unwrap_or(&Value::Null),
        "authStatus":status.map(|p|&p["auth"]["status"]).unwrap_or(&Value::Null),
        "status":status.map(|p|&p["status"]).unwrap_or(&Value::Null),
        "canAuthenticate":status.is_some_and(|p|p["setup"]["canAuthenticate"]==true),
        "canInstall":status.is_some_and(|p|p["setup"]["canInstall"]==true),
        "canLogout":status.is_some_and(|p|p["auth"]["canLogout"]==true),
        "versionStatus":status.map(|p|&p["versionAdvisory"]["status"]).unwrap_or(&Value::Null),
        "currentVersion":status.map(|p|&p["versionAdvisory"]["currentVersion"]).unwrap_or(&Value::Null),
        "latestVersion":status.map(|p|&p["versionAdvisory"]["latestVersion"]).unwrap_or(&Value::Null),
        "canUpdate":status.is_some_and(|p|p["versionAdvisory"]["canUpdate"]==true),
        "updateStatus":status.map(|p|&p["updateState"]["status"]).unwrap_or(&Value::Null)})
}

fn instance_id(id: &str) -> bool {
    id.len() <= 64
        && id.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
        && id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
}

fn safe_string(value: &Value, max: usize) -> bool {
    value.as_str().is_some_and(|s| {
        s.len() <= max && s.trim() == s && !s.chars().any(char::is_control)
    })
}

pub fn projection(settings: &Value, config: &Value, writable: bool) -> Value {
    let registry=drivers();
    let mut instances = Vec::new();
    for (id, instance) in settings["providerInstances"].as_object().into_iter().flatten() {
        let kind=text(instance,"driver");
        instances.push(visible_instance(id,instance,config,driver(&registry,kind),id==kind&&kind!="acpRegistry"));
    }
    for definition in registry["drivers"].as_array().into_iter().flatten() {
        let id=text(definition,"id");
        if definition["hasDefaultInstance"]==false||instances.iter().any(|v|v["id"]==id){continue}
        let mut fallback=json!({"driver":id,"enabled":matches!(id,"codex"|"claudeAgent")});
        if definition["defaultEnabled"]==false {fallback["enabled"]=json!(false)}
        instances.push(visible_instance(id,&fallback,config,Some(definition),true));
    }
    instances.sort_by(|a,b| text(a,"id").cmp(text(b,"id")));
    json!({"instances":instances,"drivers":registry["drivers"],"writable":writable})
}

pub fn mutation(input: &Value, settings: &Value) -> Result<Value> {
    let id=text(input,"instanceId");if !instance_id(id){return Err("invalid_provider_instance_id".into())}
    let registry=drivers();let existing=settings["providerInstances"].get(id);
    let action=text(input,"action");
    if action=="remove" {
        if id=="codex"||id=="claudeAgent"||existing.is_none()||existing.is_some_and(|v|v["driver"]==id&&id!="acpRegistry"){
            return Err("provider_instance_unavailable".into())
        }
        return Ok(json!({"patch":{},"providerInstanceMutation":{"operation":"remove","instanceId":id}}))
    }
    if !matches!(action,"create"|"update"){return Err("invalid_provider_action".into())}
    let kind=if action=="create"{input["driver"].as_str().ok_or("provider_driver_unavailable")?}
        else{existing.and_then(|v|v["driver"].as_str()).unwrap_or(id)};
    let definition=driver(&registry,kind);
    if action=="create"&&(existing.is_some()||matches!(id,"codex"|"claudeAgent")){return Err("provider_instance_exists".into())}
    if action=="create"&&definition.is_none(){return Err("provider_driver_unavailable".into())}
    if action=="update"&&existing.is_none()&&(id!=kind||definition.is_none_or(|d|d["hasDefaultInstance"]==false)){
        return Err("provider_instance_unavailable".into())
    }
    if input.get("driver").is_some_and(|v|v!=kind){return Err("provider_driver_mismatch".into())}
    let mut instance=existing.cloned().unwrap_or_else(||json!({"driver":kind,"enabled":true}));
    if action=="create"{instance=json!({"driver":kind,"enabled":true})}
    if !instance.is_object(){return Err("invalid_provider_instance".into())}
    if let Some(label)=input.get("label"){
        if !safe_string(label,128)||text(input,"label").is_empty(){return Err("invalid_provider_label".into())}
        instance["displayName"]=label.clone();
    }
    if let Some(color)=input.get("accentColor"){
        let hex=color.as_str().ok_or("invalid_provider_color")?;
        if !hex.is_empty()&&(hex.len()!=7||!hex.starts_with('#')||!hex[1..].bytes().all(|c|c.is_ascii_hexdigit())){return Err("invalid_provider_color".into())}
        if hex.is_empty(){instance.as_object_mut().unwrap().remove("accentColor");}
        else{instance["accentColor"]=color.clone()}
    }
    if let Some(enabled)=input.get("enabled"){
        if !enabled.is_boolean(){return Err("invalid_provider_enabled".into())}
        instance["enabled"]=enabled.clone();
        if instance["config"]["enabled"].is_boolean(){instance["config"]["enabled"]=enabled.clone()}
    }
    let incoming=input.get("config").cloned().unwrap_or_else(||json!({}));
    let changes=incoming.as_object().ok_or("invalid_provider_config")?;
    if !changes.is_empty()&&definition.is_none(){return Err("provider_driver_unavailable".into())}
    if !instance["config"].is_object(){instance["config"]=json!({})}
    for (key,value) in changes {
        let field=definition.and_then(|d|d["fields"].as_array()).and_then(|fs|fs.iter().find(|f|text(f,"key")==key));
        let hidden=matches!((kind,key.as_str()),("codex","setupMode")|("acpRegistry","commandArgs")|("acpRegistry","distribution"));
        if field.is_none()&&!hidden{return Err("invalid_provider_config".into())}
        if key=="commandArgs"{
            let args=value.as_array().filter(|a|a.len()<=64&&a.iter().all(|s|safe_string(s,4096))).ok_or("invalid_provider_config")?;
            instance["config"][key]=json!(args);continue
        }
        let options=field.and_then(|f|f["options"].as_array());
        if let Some(options)=options {
            if !options.iter().any(|o|o["id"]==*value){return Err("invalid_provider_config".into())}
        } else if key=="setupMode"{
            if value!="existing"{return Err("invalid_provider_config".into())}
        } else if key=="distribution"{
            if !matches!(value.as_str(),Some("auto"|"binary"|"npx"|"uvx")){return Err("invalid_provider_config".into())}
        } else if !safe_string(value,4096){return Err("invalid_provider_config".into())}
        let s=value.as_str().unwrap_or("");
        if key=="autoCompactWindow"&&!s.is_empty()&&
            (s.len()!=6&&s!="1000000"||s.starts_with('0')||!s.bytes().all(|c|c.is_ascii_digit())||s.parse::<u32>().ok().is_none_or(|n|!(100000..=1000000).contains(&n))){return Err("invalid_provider_config".into())}
        let persist_empty=matches!((kind,key.as_str()),("acpRegistry","agentId")|("antigravity","binaryPath"));
        if s.is_empty()&&!persist_empty{instance["config"].as_object_mut().unwrap().remove(key);}else{instance["config"][key]=value.clone()}
    }
    if kind=="codex"&&action=="create" {instance["config"]["setupMode"]=json!("existing")}
    if kind=="acpRegistry"&&action=="create" {
        let c=&instance["config"];
        if c["source"]=="local"{
            if text(c,"commandPath").is_empty(){return Err("invalid_provider_config".into())}
            if c.get("commandArgs").is_none(){instance["config"]["commandArgs"]=json!([])}
        }else if c["source"]=="registry" {
            if text(c,"agentId").is_empty(){return Err("invalid_provider_config".into())}
        }else{return Err("invalid_provider_config".into())}
    }
    if let Some(environment)=input.get("providerEnvironment"){
        let rows=environment.as_array().filter(|r|r.len()<=64).ok_or("invalid_provider_environment")?;
        let mut seen=HashSet::new();let mut env=Vec::new();
        for row in rows {
            let name=text(row,"name");
            if name.is_empty()||name.len()>128||!name.as_bytes()[0].is_ascii_alphabetic()&&name.as_bytes()[0]!=b'_'
                ||!name.bytes().all(|c|c.is_ascii_alphanumeric()||c==b'_')||!seen.insert(name){return Err("invalid_provider_environment".into())}
            let sensitive=row["sensitive"].as_bool().ok_or("invalid_provider_environment")?;
            let supplied=row["value"].as_str().filter(|v|v.len()<=8192&&!v.chars().any(char::is_control)).ok_or("invalid_provider_environment")?;
            let saved=instance["environment"].as_array().into_iter().flatten().find(|v|v["name"]==name);
            let prior=saved.filter(|v|v["sensitive"]==true&&v["valueRedacted"]==true);
            let next=if sensitive&&supplied.is_empty()&&row["configured"]==true {
                prior.cloned().ok_or("invalid_provider_environment")?
            }else{json!({"name":name,"value":supplied,"sensitive":sensitive})};
            env.push(next);
        }
        instance["environment"]=json!(env);
    }
    if instance["config"].as_object().is_some_and(|c|c.is_empty()){instance.as_object_mut().unwrap().remove("config");}
    Ok(json!({"patch":{},"providerInstanceMutation":{"operation":if action=="create"{"create"}else{"upsert"},"instanceId":id,"instance":instance}}))
}

/// Verify a host's readback without comparing newly submitted secret bytes to
/// the redaction marker returned by `server.getSettings`.
pub fn confirm_target(action:&str,id:&str,payload:&Value,saved:&Value)->bool {
    let actual=saved["providerInstances"].get(id);
    if action=="remove"{return actual.is_none()}
    let Some(actual)=actual else{return false};
    let desired=&payload["providerInstanceMutation"]["instance"];
    for key in ["driver","enabled","displayName","accentColor"]{
        if let Some(value)=desired.get(key){if actual.get(key)!=Some(value){return false}}
    }
    let registry=drivers();let definition=driver(&registry,text(desired,"driver"));
    if let Some(config)=desired["config"].as_object(){for(key,value)in config{
        let secret=definition.and_then(|d|d["fields"].as_array()).and_then(|fs|fs.iter().find(|f|text(f,"key")==key))
            .is_some_and(|f|text(f,"control")=="secret");
        if secret&&value.as_str().is_some_and(|v|!v.is_empty()){
            if !actual["config"][key].as_str().is_some_and(|v|!v.is_empty()){return false}
        }else if actual["config"].get(key)!=Some(value){return false}
    }}
    if let Some(expected)=desired["environment"].as_array(){
        let Some(observed)=actual["environment"].as_array()else{return false};
        if expected.len()!=observed.len(){return false}
        for (want,have) in expected.iter().zip(observed){
            if want["name"]!=have["name"]||want["sensitive"]!=have["sensitive"]{return false}
            if want["sensitive"]==true&&(!text(want,"value").is_empty()||want["valueRedacted"]==true){
                if have["valueRedacted"]!=true&&text(have,"value").is_empty(){return false}
            }else if want["value"]!=have["value"]{return false}
        }
    }
    true
}
