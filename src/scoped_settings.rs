//! Native settings scope planning. No raw settings/provider configuration enters the UI.
use crate::*;
use std::collections::HashMap;
pub use t3_protocol::scoped_settings::{aggregate, base_key, effective, project_catalog_config, project_file, project_patch, validated_targets, write_was_dropped, PROJECT_KEYS};
pub fn execute(input:&Value)->Result<Value>{execute_in(&environments()?,input)}
pub fn execute_in(environments:&[Environment],input:&Value)->Result<Value>{
    let requested=validated_targets(input)?;let key=text(input,"key");let action=text(input,"action");
    if !matches!(action,"read"|"update"|"clear"){return Err("invalid_settings".into())}
    // Resolve every selected environment before any write: stale selections never broaden.
    for t in &requested{if !environments.iter().any(|e|e.id==text(t,"environment")){return Err("settings_scope_unavailable".into())}}
    let mut views=vec![];let mut errors=vec![];let mut saved=vec![];
    let mut configs=HashMap::new();let mut permissions=HashMap::new();let client=Client::shared()?;
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
            if !configs.contains_key(&e.id){let config=protocol::api::server::config(&e.connection(true)?)?;configs.insert(e.id.clone(),config);}
            let config=configs.get(&e.id).unwrap();
            let mut raw=protocol::api::server::settings(&e.connection(true)?)?;
            let project=if let Some(id)=target["projectId"].as_str(){
                let shell=client.get(e,"/api/orchestration/shell",true)?;
                Some(shell["projects"].as_array().into_iter().flatten().find(|p|text(p,"id")==id).cloned().ok_or("settings_scope_unavailable")?)
            }else{None};
            let project_capable=config["environment"]["capabilities"]["projectSettingsOverrides"]==true;
            if action!="read"&&project.is_none()&&key=="enableAgentDeviceAccess" {return Err("device_configuration_required".into())}
            if action!="read"{
                if project.is_some()&&(!project_capable||key=="worktreeCleanup"&&(config["environment"]["capabilities"]["projectWorktreeCleanup"]!=true||config["environment"]["capabilities"]["storageCleanup"]!=true)){return Err("project_settings_unsupported".into())}
                let validation_config=project_catalog_config(config,&raw,project.as_ref());
                let validated=if action=="clear"{json!({})}else{crate::native_settings::patch(input,Some(&validation_config))?};
                let patch=if let Some(p)=&project{project_patch(&raw,p,key,&validated,action=="clear")?}else{if action=="clear"{return Err("invalid_settings".into())}if key=="idleCompaction"{crate::native_settings::idle_compaction_patch(&raw,&validated)?}else{crate::storage_settings::environment_patch(&raw,key,&validated)?}};
                let before=raw.clone();
                crate::write::update_settings(&e.connection(true)?,&patch)?;
                raw=protocol::api::server::settings(&e.connection(true)?)?;
                if write_was_dropped(&patch["patch"],&before,&raw){return Err("settings_write_dropped".into())}
                saved.push(target.clone());
            }
            let file=if let Some(p)=&project{e.connection(true).and_then(|c|protocol::api::projects::read_file(&c,text(p,"workspaceRoot"),"t3.json",None)).ok().filter(|v|v["truncated"]==false).and_then(|v|project_file(text(&v,"contents")))}else{None};
            let (effective,sources,overrides)=if project.is_some(){effective(&raw,project.as_ref(),file.as_ref())}else{(raw.clone(),json!({}),json!({}))};
            let mut view=crate::native_settings::snapshot(&effective);
            view["fields"].as_array_mut().unwrap().retain(|f| f.get("capability").and_then(Value::as_str).is_none_or(|cap| config["environment"]["capabilities"][cap]==true));
            for field in view["fields"].as_array_mut().unwrap(){let base=base_key(text(field,"key")).to_string();let scopable=PROJECT_KEYS.contains(&base.as_str());
                field["source"]=if project.is_some(){sources[&base].clone()}else{json!("environment")};
                field["scopeWritable"]=json!(write_allowed&&(project.is_none()||(scopable&&project_capable)));
                if project.is_none()&&base=="enableAgentDeviceAccess" {field["scopeWritable"]=json!(false);}
                field["overridden"]=json!(project.is_some()&&overrides.get(&base).is_some());
                field["projectScoped"]=json!(project.is_some());
            }
            Ok(json!({"target":target,"fields":view["fields"],"writeAllowed":write_allowed,"storageCapabilities":crate::storage_settings::capabilities(config),"catalog":protocol::models::catalog(&project_catalog_config(config,&raw,project.as_ref()))}))
        })();
        match result{Ok(view)=>views.push(view),Err(error)=>errors.push(json!({"target":target,"error":error}))}
    }
    let mut result=aggregate(&views);result["storageScopes"]=json!(views.iter().map(|v|json!({"target":v["target"],"capabilities":v["storageCapabilities"]})).collect::<Vec<_>>());result["scope"]=input["scope"].clone();result["modelCatalog"]=views.first().map(|v|v["catalog"].clone()).unwrap_or(Value::Null);result["targets"]=json!(views.iter().map(|v|v["target"].clone()).collect::<Vec<_>>());result["errors"]=json!(errors);result["savedTargets"]=json!(saved);result["complete"]=json!(errors.is_empty()&&views.len()==requested.len());result["writable"]=json!(errors.is_empty()&&!views.is_empty()&&views.iter().all(|v|v["writeAllowed"]==true));
    Ok(result)
}
