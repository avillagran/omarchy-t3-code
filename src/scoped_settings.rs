//! Native settings scope planning. No raw settings/provider configuration enters the UI.
use crate::*;
use std::collections::{HashMap,HashSet};
pub const PROJECT_KEYS:&[&str]=&["worktreeCleanup","defaultModelSelection","defaultRuntimeMode","defaultThreadEnvMode","newWorktreesStartFromOrigin","worktreeSubmodules","defaultAutoPull","defaultProjectScripts","enableAgentBrowserAccess","enableAgentDeviceAccess","textGenerationModelSelection","sourceControlWriterModelSelection","reviewGuideModelSelection","reviewGuideInstructions","sourceControlWritingStyle","removeAgentCreditsOnMerge","branchNamingMode","branchNamePrefix","branchNameInstructions","pullRequestMergeMethod","sidebarAutoSettleOnMerge","sidebarAutoSettleAfterDays","sidebarAutoSettleKeepPullRequests","continueThreadsAfterServerUpdate","responseStreamingMode"];
fn base_key(key:&str)->&str{key.split('.').next().unwrap_or(key)}
fn nullable(key:&str)->bool{matches!(key,"defaultModelSelection"|"sourceControlWriterModelSelection"|"reviewGuideModelSelection"|"pullRequestMergeMethod"|"sidebarAutoSettleAfterDays")}
/// Missing/invalid files are absent, like native parseT3ProjectFile. JSON comments
/// and trailing commas are allowed; strings are never rewritten.
pub fn project_file(contents:&str)->Option<Value>{
    if contents.len()>256*1024{return None}
    let b=contents.as_bytes();let mut out=Vec::with_capacity(b.len());let(mut i,mut quoted,mut escaped)=(0,false,false);
    while i<b.len(){let c=b[i];if quoted{out.push(c);if escaped{escaped=false}else if c==b'\\'{escaped=true}else if c==b'"'{quoted=false}i+=1;continue}
        if c==b'"'{quoted=true;out.push(c);i+=1;continue}
        if c==b'/'&&b.get(i+1)==Some(&b'/'){i+=2;while i<b.len()&&b[i]!=b'\n'{i+=1}out.push(b' ');continue}
        if c==b'/'&&b.get(i+1)==Some(&b'*'){i+=2;while i+1<b.len()&&!(b[i]==b'*'&&b[i+1]==b'/'){i+=1}if i+1>=b.len(){return None}i+=2;out.push(b' ');continue}
        out.push(c);i+=1;
    }
    let mut cleaned=vec![];quoted=false;escaped=false;
    for (i,&c) in out.iter().enumerate(){if !quoted&&c==b','{let next=out[i+1..].iter().find(|c|!c.is_ascii_whitespace());if matches!(next,Some(b'}'|b']')){continue}}
        cleaned.push(c);if quoted{if escaped{escaped=false}else if c==b'\\'{escaped=true}else if c==b'"'{quoted=false}}else if c==b'"'{quoted=true}
    }
    let raw:Value=serde_json::from_slice(&cleaned).ok()?;if !raw.is_object(){return None}
    if raw.get("defaultThreadEnvMode").is_some_and(|v|!matches!(v.as_str(),Some("local"|"worktree")))||raw.get("worktreeSubmodules").is_some_and(|v|!matches!(v.as_str(),Some("recursive"|"top-level"|"none"))){return None}
    for key in ["$schema","iconPath"]{if raw.get(key).is_some_and(|v|!v.is_string()||(key=="iconPath"&&v.as_str().is_some_and(|s|s.trim().is_empty()||s.chars().count()>512))){return None}}
    if let Some(scripts)=raw.get("scripts"){
        let scripts=scripts.as_array().filter(|v|v.len()<=50)?;
        for script in scripts{
            if !script.is_object(){return None}
            for key in ["name","command"]{if !script[key].as_str().is_some_and(|s|!s.trim().is_empty()){return None}}
            for key in ["runOnWorktreeCreate","runOnSettle","async","autoOpenPreview"]{if script.get(key).is_some_and(|v|!v.is_boolean()){return None}}
            if script.get("icon").is_some_and(|v|!matches!(v.as_str(),Some("play"|"test"|"lint"|"configure"|"build"|"debug"))){return None}
            if script.get("previewUrl").is_some_and(|v|!v.as_str().is_some_and(|s|!s.trim().is_empty())){return None}
        }
    }
    Some(raw)
}
pub fn effective(raw:&Value,project:Option<&Value>,file:Option<&Value>)->(Value,Value,Value){
    let mut resolved=raw.clone();let mut sources=json!({});let mut overrides=json!({});
    if let Some(project)=project{
        if raw["projectSettingsFolded"]!=true{for key in ["defaultModelSelection","defaultThreadEnvMode"]{if let Some(v)=project.get(key).filter(|v|!v.is_null()){overrides[key]=v.clone()}}}
        if let Some(stored)=raw["projectSettingsOverrides"][text(project,"id")].as_object(){for (key,v)in stored{overrides[key]=v.clone()}}
    }
    for key in PROJECT_KEYS{
        sources[*key]=json!("environment");
        if let Some(v)=overrides.get(*key){
            let disabled=matches!(*key,"defaultModelSelection"|"textGenerationModelSelection")&&!v.is_null()&&overrides["providerInstances"][text(v,"instanceId")]["enabled"].as_bool().or(raw["providerInstances"][text(v,"instanceId")]["enabled"].as_bool())==Some(false);
            if !disabled{resolved[*key]=v.clone();sources[*key]=json!("project")}
        }
    }
    for (key,builtin) in [("defaultThreadEnvMode","local"),("worktreeSubmodules","recursive")]{if resolved[key].is_null(){if let Some(v)=file.and_then(|f|f.get(key)){resolved[key]=v.clone();sources[key]=json!("t3.json")}else{resolved[key]=json!(builtin);sources[key]=json!("environment")}}}
    (resolved,sources,overrides)
}
/// The native patch replaces each whole project entry. Preserve unrelated keys;
/// complete nested values from the effective settings before replacing them.
pub fn project_patch(raw:&Value,project:&Value,key:&str,patch:&Value,clear:bool)->Result<Value>{
    let base=base_key(key);if !PROJECT_KEYS.contains(&base){return Err("setting_environment_only".into())}
    let (resolved,_,mut entry)=effective(raw,Some(project),None);
    if clear{entry.as_object_mut().ok_or("invalid_settings")?.remove(base);}else{
        let next=patch["patch"].get(base).ok_or("invalid_settings")?;
        if next.is_null()&&!nullable(base){entry.as_object_mut().ok_or("invalid_settings")?.remove(base);}else if let (Some(next),Some(current))=(next.as_object(),resolved[base].as_object()){
            let mut merged=current.clone();for(k,v)in next{merged.insert(k.clone(),v.clone());}entry[base]=json!(merged);
        }else{entry[base]=next.clone()}
    }
    let entry=if entry.as_object().is_some_and(|e|e.is_empty()){Value::Null}else{entry};
    Ok(json!({"patch":{"projectSettingsOverrides":{text(project,"id"):entry}}}))
}
pub fn aggregate(targets:&[Value])->Value{
    let mut fields=targets.first().map(|t|t["fields"].as_array().cloned().unwrap_or_default()).unwrap_or_default();
    fields.retain(|f|targets.iter().all(|t|t["fields"].as_array().is_some_and(|fs|fs.iter().any(|o|o["key"]==f["key"]))));
    for f in &mut fields{
        let others=targets.iter().flat_map(|t|t["fields"].as_array().into_iter().flatten().filter(|o|o["key"]==f["key"])).collect::<Vec<_>>();
        f["mixed"]=json!(others.iter().any(|o|o["value"]!=f["value"]));
        f["source"]=if others.iter().any(|o|o["source"]!=f["source"]){json!("mixed")}else{f["source"].clone()};
        f["overridden"]=json!(others.iter().any(|o|o["overridden"]==true));
        f["scopeWritable"]=json!(others.iter().all(|o|o["scopeWritable"]==true));
    }
    json!({"fields":fields})
}
fn validated_targets(input:&Value)->Result<Vec<Value>>{
    let targets=input["scope"]["targets"].as_array().filter(|v|!v.is_empty()&&v.len()<=64).ok_or("settings_scope_unavailable")?;
    let mut seen=HashSet::new();let mut project_scope=None;
    for t in targets{let env=text(t,"environment");let project=t.get("projectId").ok_or("settings_scope_unavailable")?;
        if !valid_id(env)||(!project.is_null()&&!project.as_str().is_some_and(valid_id))||!seen.insert((env.to_string(),project.clone().to_string())){return Err("settings_scope_unavailable".into())}
        if project_scope.is_some_and(|p|p!=!project.is_null()){return Err("settings_scope_unavailable".into())}project_scope=Some(!project.is_null());
    }
    Ok(targets.clone())
}
fn write_was_dropped(next:&Value,before:&Value,saved:&Value)->bool{
    if next.is_null(){return !before.is_null()&&before==saved}
    if let Some(m)=next.as_object(){if !before.is_array()&&!saved.is_array(){return m.iter().any(|(k,v)|write_was_dropped(v,&before[k],&saved[k]))}}
    let unchanged=if let (Some(n),Some(b))=(next.as_str(),before.as_str()){n.trim()==b}else{next==before};
    !unchanged&&saved==before
}
fn project_catalog_config(config:&Value,raw:&Value,project:Option<&Value>)->Value{
    let mut adjusted=config.clone();
    if let Some(project)=project{if let Some(providers)=adjusted["providers"].as_array_mut(){for provider in providers{
        let id=text(provider,"instanceId").to_string();
        if let Some(enabled)=raw["projectSettingsOverrides"][text(project,"id")]["providerInstances"][&id]["enabled"].as_bool(){provider["enabled"]=json!(enabled)}
    }}}
    adjusted
}
pub fn execute(input:&Value)->Result<Value>{execute_in(&environments()?,input)}
pub fn execute_in(environments:&[Environment],input:&Value)->Result<Value>{
    let requested=validated_targets(input)?;let key=text(input,"key");let action=text(input,"action");
    if !matches!(action,"read"|"update"|"clear"){return Err("invalid_settings".into())}
    // Resolve every selected environment before any write: stale selections never broaden.
    for t in &requested{if !environments.iter().any(|e|e.id==text(t,"environment")){return Err("settings_scope_unavailable".into())}}
    let mut views=vec![];let mut errors=vec![];let mut saved=vec![];
    let mut configs=HashMap::new();let mut permissions=HashMap::new();let client=Client::new()?;
    for target in &requested{
        let e=environments.iter().find(|e|e.id==text(target,"environment")).unwrap();
        let result=(||->Result<Value>{
            if !permissions.contains_key(&e.id){
                let session=client.get(e,"/api/auth/session",false)?;
                let granted=session.get("permissions").or_else(||session.get("scopes")).and_then(Value::as_array);
                let allowed=session["authenticated"]==true&&granted.is_some_and(|s|s.iter().any(|s|s=="settings:write"));
                permissions.insert(e.id.clone(),allowed);
            }
            let write_allowed=*permissions.get(&e.id).unwrap();
            if action!="read"&&!write_allowed{return Err("settings_write_forbidden".into())}
            if !configs.contains_key(&e.id){let config=crate::write::rpc(e,"server.getConfig",json!({}))?;configs.insert(e.id.clone(),config);}
            let config=configs.get(&e.id).unwrap();
            let mut raw=crate::write::rpc(e,"server.getSettings",json!({}))?;
            let project=if let Some(id)=target["projectId"].as_str(){
                let shell=client.get(e,"/api/orchestration/shell",true)?;
                Some(shell["projects"].as_array().into_iter().flatten().find(|p|text(p,"id")==id).cloned().ok_or("settings_scope_unavailable")?)
            }else{None};
            let project_capable=config["environment"]["capabilities"]["projectSettingsOverrides"]==true;
            if action!="read"&&project.is_none()&&key=="enableAgentDeviceAccess" {return Err("device_configuration_required".into())}
            if action!="read"{
                if project.is_some()&&!project_capable{return Err("project_settings_unsupported".into())}
                let validation_config=project_catalog_config(config,&raw,project.as_ref());
                let validated=if action=="clear"{json!({})}else{crate::native_settings::patch(input,Some(&validation_config))?};
                let patch=if let Some(p)=&project{project_patch(&raw,p,key,&validated,action=="clear")?}else{if action=="clear"{return Err("invalid_settings".into())}validated};
                let before=raw.clone();
                crate::write::rpc(e,"server.updateSettings",patch.clone())?;
                raw=crate::write::rpc(e,"server.getSettings",json!({}))?;
                if write_was_dropped(&patch["patch"],&before,&raw){return Err("settings_write_dropped".into())}
                saved.push(target.clone());
            }
            let file=if let Some(p)=&project{crate::write::rpc(e,"projects.readFile",json!({"cwd":p["workspaceRoot"],"relativePath":"t3.json"})).ok().filter(|v|v["truncated"]==false).and_then(|v|project_file(text(&v,"contents")))}else{None};
            let (effective,sources,overrides)=if project.is_some(){effective(&raw,project.as_ref(),file.as_ref())}else{(raw.clone(),json!({}),json!({}))};
            let mut view=crate::native_settings::snapshot(&effective);
            for field in view["fields"].as_array_mut().unwrap(){let base=base_key(text(field,"key")).to_string();let scopable=PROJECT_KEYS.contains(&base.as_str());
                field["source"]=if project.is_some(){sources[&base].clone()}else{json!("environment")};
                field["scopeWritable"]=json!(write_allowed&&(project.is_none()||(scopable&&project_capable)));
                if project.is_none()&&base=="enableAgentDeviceAccess" {field["scopeWritable"]=json!(false);}
                field["overridden"]=json!(project.is_some()&&overrides.get(&base).is_some());
                field["projectScoped"]=json!(project.is_some());
            }
            Ok(json!({"target":target,"fields":view["fields"],"writeAllowed":write_allowed,"catalog":protocol::models::catalog(&project_catalog_config(config,&raw,project.as_ref()))}))
        })();
        match result{Ok(view)=>views.push(view),Err(error)=>errors.push(json!({"target":target,"error":error}))}
    }
    let mut result=aggregate(&views);result["scope"]=input["scope"].clone();result["modelCatalog"]=views.first().map(|v|v["catalog"].clone()).unwrap_or(Value::Null);result["targets"]=json!(views.iter().map(|v|v["target"].clone()).collect::<Vec<_>>());result["errors"]=json!(errors);result["savedTargets"]=json!(saved);result["complete"]=json!(errors.is_empty()&&views.len()==requested.len());result["writable"]=json!(errors.is_empty()&&!views.is_empty()&&views.iter().all(|v|v["writeAllowed"]==true));
    Ok(result)
}
