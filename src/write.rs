//! T3 Effect RPC and signed attachment uploads. Never retries a mutation implicitly.
use crate::*;
use serde_json::{json, Value};
use std::{fs::OpenOptions, os::unix::fs::OpenOptionsExt};

/// Raw dispatcher by wire tag, with the write token. ot3 itself sends every RPC through
/// `protocol::api`; this stays for tags a future server adds before the registry does.
pub fn rpc(e: &Environment, tag: &str, payload: Value) -> Result<Value> {
    e.connection(true)?.rpc(tag,payload)
}
/// `orchestration.dispatchCommand` through the typed API, with the write token like `rpc`.
pub fn dispatch_command(e: &Environment, command: &Value) -> Result<Value> {
    send_command(&e.connection(true)?,command)
}
/// The command JSON a caller built (and an outbox may hold) is parsed into the typed
/// `Command`, which sends it unchanged; a malformed command fails before any socket opens.
pub fn send_command(c: &protocol::Connection, command: &Value) -> Result<Value> {
    protocol::api::orchestration::dispatch(c,&protocol::api::Command::from_json(command)?)
}
/// `server.updateSettings` for a whole `{patch, providerInstanceMutation?}` payload.
pub fn update_settings(c: &protocol::Connection, payload: &Value) -> Result<Value> {
    let mutation=payload.get("providerInstanceMutation").map(protocol::api::server::ProviderInstanceMutation::from_json).transpose()?;
    protocol::api::server::update_settings(c,&payload["patch"],mutation.as_ref())
}
/// Outbox tags of `manage` sent through the typed API; the envelope keeps the tag as before.
const DISPATCH: &str = protocol::api::orchestration::DISPATCH_COMMAND.name;
const LAUNCH: &str = protocol::api::orchestration::LAUNCH_THREAD.name;
const MUTATE: &str = protocol::api::projects::MUTATE.name;
const CREATE_NEW: &str = protocol::api::projects::CREATE_NEW.name;
const CLONE: &str = protocol::api::project_clone::START.name;
/// What `manage` sends, typed before the outbox keeps it.
enum Managed {
    Dispatch(protocol::api::Command),
    Launch(protocol::api::ThreadLaunch),
    Mutate(protocol::api::projects::ProjectMutation),
    CreateNew(String),
    Clone(protocol::api::project_clone::CloneStart),
}
fn interaction_mode(input:&Value)->Result<&str>{
    match input.get("interactionMode"){None|Some(Value::Null)=>Ok("default"),Some(v)=>v.as_str().filter(|s|matches!(*s,"default"|"plan")).ok_or("invalid_interaction_mode".into())}
}
fn runtime_mode(input:&Value)->Result<&str>{
    let mode=match input.get("runtimeMode"){None|Some(Value::Null)=>"approval-required",Some(value)=>value.as_str().ok_or("invalid_runtime_mode")?};
    if protocol::models::valid_runtime_mode(mode){Ok(mode)}else{Err("invalid_runtime_mode".into())}
}
pub fn catalog(e: &Environment) -> Result<Value> {
    let config = protocol::api::server::config(&e.connection(true)?)?;
    let mut result=protocol::models::catalog(&config);
    for provider in result["providers"].as_array_mut().unwrap(){
        if let Some(native)=config["providers"].as_array().into_iter().flatten().find(|p|p["instanceId"]==provider["instanceId"]){provider["showInteractionModeToggle"]=json!(native["showInteractionModeToggle"]!=false);provider["supportsConversationRollback"]=json!(native["supportsConversationRollback"]!=false);provider["update"]=crate::provider_updates::advisory(native);}
    }
    // T3 resolveEnvironmentMachineKind: the user's icon choice, else what the server detected.
    const MACHINES:[&str;7]=["server","cloud","linux","desktop","laptop","mac-mini","mac-studio"];
    result["machine"]=json!([&config["settings"]["environmentIcon"],&config["environment"]["platform"]["machine"]].into_iter().filter_map(Value::as_str).find(|k|MACHINES.contains(k)).unwrap_or("server"));
    // The server's own version (environment.serverVersion), shown against the T3 revision ot3 is validated with.
    result["serverVersion"]=json!(config["environment"]["serverVersion"].as_str().filter(|v|v.len()<=80&&v.chars().all(|c|c.is_ascii_alphanumeric()||".-+_".contains(c))));
    let permission=Client::shared()?.get(e,"/api/auth/session",false);
    result["canWrite"]=json!(permission.as_ref().is_ok_and(|s|s["authenticated"]==true&&s.get("permissions").or_else(||s.get("scopes")).and_then(Value::as_array).is_some_and(|s|s.iter().any(|v|v=="orchestration:operate"))));
    if let Err(error)=permission{result["permissionError"]=json!(error);}
    Ok(result)
}
pub fn project_sources(e:&Environment)->Result<Value>{
    let config=protocol::api::server::config(&e.connection(true)?)?;
    let discovery=protocol::api::server::discover_source_control(&e.connection(true)?)?;
    let providers=discovery["sourceControlProviders"].as_array().into_iter().flatten().map(|p|json!({"kind":p["kind"],"status":p["status"],"auth":{"status":p["auth"]["status"]}})).collect::<Vec<_>>();
    Ok(json!({"newProjectsRoot":config["newProjectsRoot"],"baseDirectory":config["settings"]["addProjectBaseDirectory"],"cloneTracking":config["environment"]["capabilities"]["projectCloneTracking"]==true,"providers":providers}))
}
pub fn workspace_preference(e:&Environment,input:&Value)->Result<Value>{
    let project=text(input,"projectId");if !valid_id(project){return Err("invalid_project".into())}
    let raw=if input["action"]=="workspace-update" {
        if !matches!(input["mode"].as_str(),Some("local"|"worktree")){return Err("invalid_settings".into())}
        protocol::api::server::update_settings(&e.connection(true)?,&json!({"defaultThreadEnvMode":input["mode"]}),None)?
    }else if input["action"]=="workspace-read"{protocol::api::server::settings(&e.connection(true)?)?}else{return Err("invalid_settings".into())};
    let overrides=&raw["projectSettingsOverrides"][project];
    let mode=overrides["defaultThreadEnvMode"].as_str().or(raw["defaultThreadEnvMode"].as_str()).filter(|s|matches!(*s,"local"|"worktree"));
    let origin=overrides["newWorktreesStartFromOrigin"].as_bool().or(raw["newWorktreesStartFromOrigin"].as_bool()).unwrap_or(true);
    let mut result=crate::native_settings::thread_defaults(&raw,project);
    result["mode"]=json!(mode);result["startFromOrigin"]=json!(origin);Ok(result)
}
pub fn history(e: &Environment,input:&Value)->Result<Value>{
    let thread=text(input,"thread");if !valid_id(thread){return Err("invalid_id".into())}
    let path=protocol::history::path(thread,text(input,"cursor"))?;
    let page=Client::shared()?.get(e,&path,true)?;
    normalize_messages(&protocol::history::envelope(&page,thread)?)
}

pub fn upload(e: &Environment, file_url: &str) -> Result<Value> {
    let url=crate::Url::parse(file_url).map_err(|_| "invalid_file")?;
    if url.scheme()!="file" {return Err("invalid_file".into())}
    let path=url.to_file_path().map_err(|_| "invalid_file")?;
    let file=OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW|libc::O_NONBLOCK).open(&path).map_err(|_| "invalid_file")?;
    let metadata=file.metadata().map_err(|_| "invalid_file")?;
    let size=metadata.len();
    if !metadata.is_file() || size==0 || size>50*1024*1024 {return Err("file_limit_50MiB".into())}
    let name=path.file_name().and_then(|n|n.to_str()).ok_or("invalid_file")?;
    if name.chars().count()>255 {return Err("invalid_file".into())}
    let mime=mime_guess::from_path(&path).first_or_octet_stream().to_string();
    let image=matches!(mime.as_str(),"image/png"|"image/jpeg"|"image/gif"|"image/webp");
    if image && size>10*1024*1024 {return Err("image_limit_10MiB".into())}
    let connection=e.connection(true)?;
    connection.upload_attachment(name,&mime,size,file)
}
pub fn validate_message(input:&Value)->Result<()> {
    let content=text(input,"text");
    let attachments=input["attachments"].as_array().ok_or("invalid_attachments")?;
        if (content.trim().is_empty() && attachments.is_empty()) || content.chars().count()>120000 || attachments.len()>100 {return Err("invalid_message".into())}
        for a in attachments {
            let id = text(a,"id");
            let size = a["sizeBytes"].as_u64().ok_or("invalid_attachments")?;
            let image = a["type"] == "image";
            if (!image && a["type"] != "file") || id.is_empty() || id.len()>128
                || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
                || text(a,"name").is_empty() || text(a,"name").chars().count()>255
                || text(a,"mimeType").is_empty() || text(a,"mimeType").len()>100
                || size==0 || size>if image {10*1024*1024} else {50*1024*1024}
            { return Err("invalid_attachments".into()); }
        }
        let image_bytes:u64=attachments.iter().filter(|a|a["type"]=="image").map(|a|a["sizeBytes"].as_u64().unwrap_or(u64::MAX/100)).sum();
        if image_bytes>80*1024*1024 {return Err("image_total_limit_80MiB".into())}
    Ok(())
}
fn launch_command(input:&Value,thread:&str)->Result<Value>{
    let strategy=&input["workspaceStrategy"];
    let field=match text(strategy,"type") {"root"=>None,"existing_worktree"=>Some("worktreePath"),"worktree"=>Some("baseRef"),_=>return Err("invalid_workspace".into())};
    let mut workspace=json!({"type":strategy["type"]});
    if let Some(field)=field {
        let value=text(strategy,field);
        if value.trim().is_empty()||value.len()>4096||value.chars().any(char::is_control){return Err("invalid_workspace".into())}
        workspace[field]=json!(value);
    }
    if let Some(branch)=strategy["branch"].as_str(){
        if branch.trim().is_empty()||branch.len()>1024||branch.chars().any(char::is_control){return Err("invalid_workspace".into())}
        workspace["branch"]=json!(branch);
    }
    if let Some(origin)=strategy["startFromOrigin"].as_bool(){workspace["startFromOrigin"]=json!(origin);}
    let title=text(input,"title").trim();
    if title.is_empty()||title.chars().count()>255{return Err("invalid_title".into())}
    let mut command=json!({"threadId":thread,"projectId":input["projectId"],"title":title,"modelSelection":input["modelSelection"],"runtimeMode":runtime_mode(input)?,"interactionMode":interaction_mode(input)?,"workspaceStrategy":workspace,"creationSource":"web"});
    if input["launchWithMessage"]==true {
        validate_message(input)?;
        let message=uuid::Uuid::parse_str(text(input,"sendRequestId")).map_err(|_|"invalid_request_id")?;
        command["initialMessage"]=json!({"messageId":format!("omarchy-{message}"),"text":protocol::composer::legacy_attachment_text(text(input,"text"),input["attachments"].as_array().unwrap()),"attachments":input["attachments"]});
    }
    Ok(command)
}
pub fn dispatch(e: &Environment, input: &Value) -> Result<Value> {
    let id=text(input,"requestId");
    uuid::Uuid::parse_str(id).map_err(|_| "invalid_request_id")?;
    let thread=text(input,"thread");
    if !valid_id(thread) {return Err("invalid_id".into())}
    let _rewind_guard=crate::checkpoint_journal::lock(&e.id,thread)?;
    crate::checkpoint_journal::ensure_available(&e.id,thread)?;
    let mut cmd = if input["op"]=="approval" {
        let path=xdg("XDG_STATE_HOME",".local/state").join("outbox").join(format!("{id}.json"));
        let expected=json!({"environment":e.id,"command":{"type":"runtime-request.respond","threadId":thread,"requestId":input["runtimeRequestId"],"decision":input["decision"],"commandId":format!("omarchy-{id}")}});
        if path.exists() {
            // A lost acknowledgement may already have resolved the request. Replay
            // only the identical durable command, letting T3 deduplicate its ID.
            if read_json(&path)?!=expected {return Err("retry_payload_changed".into())}
            expected["command"].clone()
        } else {
            let snapshot=Client::shared()?.bounded_thread(e,thread)?;
            protocol::approvals::command(input,&snapshot["projection"])?
        }
    } else if input["op"]=="question" {
        protocol::questions::command(input)?
    } else if input["op"]=="queue" {
        protocol::queue::command(input)?
    } else if input["op"]=="stop" {
        if !valid_id(text(input,"runId")) {return Err("invalid_run_id".into())}
        json!({"type":"run.interrupt","threadId":thread,"runId":input["runId"],"holdQueue":true})
    } else {
        let content=text(input,"text");
        let attachments=input["attachments"].as_array().ok_or("invalid_attachments")?;
        validate_message(input)?;
        let mut c=json!({"type":"message.dispatch","threadId":thread,"messageId":format!("omarchy-{id}"),"text":protocol::composer::legacy_attachment_text(content,attachments),"attachments":attachments,"createdBy":"user","creationSource":"web","dispatchMode":{"type":if input["delivery"]=="queue" {"queue_after_active"} else {"start_immediately"}}});
        // T3's server-resolved dispatch: steer redirects the active run, auto starts or queues as the provider allows.
        if input["delivery"]!="queue" {c["deliveryIntent"]=json!(if input["delivery"]=="steer" {"steer"} else {"auto"});}
        if input["modelSelection"].is_object() {c["modelSelection"]=input["modelSelection"].clone();}
        c
    };
    if cmd["type"]=="message.dispatch"&&input.get("sourcePlanRef").is_some_and(|v|!v.is_null()){
        crate::plan_workflow::attach_source_ref(&mut cmd,input).map_err(str::to_string)?;
    }
    cmd["commandId"]=json!(format!("omarchy-{id}"));
    // Persist before sending. An explicit retry must reuse exactly the same command.
    let path=xdg("XDG_STATE_HOME",".local/state").join("outbox").join(format!("{id}.json"));
    let permissions=if cmd["type"]=="message.dispatch"&&input.get("runtimeMode").is_some_and(|v|!v.is_null()){
        Some(protocol::api::Command::runtime_mode(&format!("omarchy-{id}-runtime-mode"),thread,runtime_mode(input)?)?)
    }else{None};
    let mut envelope=json!({"environment":e.id,"command":cmd});
    if permissions.is_some(){envelope["runtimeMode"]=input["runtimeMode"].clone();}
    let interaction=if cmd["type"]=="message.dispatch"&&input.get("interactionMode").is_some_and(|v|!v.is_null()){
        Some(protocol::api::Command::InteractionModeSet{command_id:format!("omarchy-{id}-interaction-mode"),thread_id:thread.to_string(),interaction_mode:interaction_mode(input)?.to_string()})
    }else{None};
    if interaction.is_some(){envelope["interactionMode"]=input["interactionMode"].clone();}
    let command=protocol::api::Command::from_json(&cmd)?;
    if path.exists() {if read_json(&path)?!=envelope {return Err("retry_payload_changed".into())}}
    else { save_json(&path,&envelope)?; }
    let c=e.connection(true)?;
    if let Some(mode)=permissions{protocol::api::orchestration::dispatch(&c,&mode)?;}
    if let Some(mode)=interaction{protocol::api::orchestration::dispatch(&c,&mode)?;}
    protocol::api::orchestration::dispatch(&c,&command)
}
/// Explicit management actions. Native launches own workspace preparation and
/// may durably accept an initial message; legacy thread.create remains metadata-only.
pub fn manage(e: &Environment, input: &Value) -> Result<Value> {
    let id=text(input,"requestId"); uuid::Uuid::parse_str(id).map_err(|_| "invalid_request_id")?;
    let mut thread=String::new();
    let (tag,mut command)=match text(input,"action") {
        "create-project" => {
            let name=text(input,"name").trim();
            if name.is_empty()||name.chars().count()>200||name.chars().any(char::is_control){return Err("invalid_title".into())}
            (CREATE_NEW,json!({"name":name}))
        },
        "clone-project" => {
            let destination=protocol::projects::folder_path(text(input,"workspaceRoot"))?;
            let repository=text(input,"repository").trim();
            if repository.is_empty()||repository.len()>4096||repository.chars().any(char::is_control){return Err("invalid_project".into())}
            let mut c=json!({"projectId":format!("omarchy-project-{id}"),"title":destination.rsplit('/').next().unwrap_or("Project"),"createdAt":input["createdAt"],"destinationPath":destination});
            if input["source"]=="url"{
                let shorthand=repository.split('/').collect::<Vec<_>>();
                let url=if shorthand.len()==2&&shorthand.iter().all(|s|!s.is_empty()&&s.chars().all(|c|c.is_ascii_alphanumeric()||matches!(c,'-'|'_'|'.'))){format!("https://github.com/{repository}{}",if repository.ends_with(".git"){""}else{".git"})}else{repository.to_string()};
                c["remoteUrl"]=json!(url)
            }else{let provider=text(input,"source");if provider.is_empty()||provider.len()>64||!provider.chars().all(|c|c.is_ascii_lowercase()||c.is_ascii_digit()||c=='-'){return Err("invalid_project".into())}c["provider"]=json!(provider);c["repository"]=json!(repository)}
            (CLONE,c)
        },
        "new-thread" => {
            let project=text(input,"projectId"); if !valid_id(project) || !input["modelSelection"].is_object() {return Err("invalid_project".into())}
            thread=format!("omarchy-thread-{id}");
            let mode=runtime_mode(input)?;
            if input["workspaceStrategy"].is_object(){(LAUNCH,launch_command(input,&thread)?)}else{(DISPATCH,json!({"type":"thread.create","threadId":thread,"projectId":project,"title":text(input,"title").trim(),"modelSelection":input["modelSelection"],"runtimeMode":mode,"interactionMode":interaction_mode(input)?,"branch":input["branch"],"worktreePath":input["worktreePath"],"createdBy":"user","creationSource":"web"}))}
        },
        "add-project" => {
            let path=if input["workspaceRoot"].is_string(){
                let path=protocol::projects::folder_path(text(input,"workspaceRoot"))?;
                let meta=protocol::api::filesystem::metadata(&e.connection(true)?,std::slice::from_ref(&path))?;
                if meta["entries"][0]["kind"]!="directory"{return Err("invalid_folder".into())}path
            }else{
                if e.id!="local"{return Err("local_folder_only".into())}
                let path=crate::Url::parse(text(input,"url")).map_err(|_| "invalid_folder")?.to_file_path().map_err(|_| "invalid_folder")?.canonicalize().map_err(|_| "invalid_folder")?;
                if !path.is_dir(){return Err("invalid_folder".into())}path.to_str().ok_or("invalid_folder")?.to_string()
            };
            let title=path.trim_end_matches('/').rsplit('/').next().filter(|s|!s.is_empty()).unwrap_or("Project");
            (MUTATE,json!({"type":"project.create","projectId":format!("omarchy-project-{id}"),"title":title,"workspaceRoot":path,"createWorkspaceRootIfMissing":false}))
        },
        // Forks open as a new thread; merge-back carries a fork's finished run to its source.
        "fork" => {
            let source=text(input,"thread"); let run=text(input,"runId");
            if !valid_id(source)||!valid_id(run){return Err("invalid_id".into())}
            thread=format!("omarchy-thread-{id}");
            let title=text(input,"title").trim(); let title=if title.is_empty(){"Fork".to_string()}else{format!("{} fork",title.chars().take(250).collect::<String>())};
            (DISPATCH,json!({"type":"thread.fork","createdBy":"user","creationSource":"web","sourceThreadId":source,"targetThreadId":thread,"sourcePoint":{"type":"run","runId":run},"title":title}))
        },
        "merge-back" => {
            let source=text(input,"thread"); let run=text(input,"runId"); let target=text(input,"targetThread");
            if !valid_id(source)||!valid_id(run)||!valid_id(target){return Err("invalid_id".into())}
            thread=target.to_string();
            (DISPATCH,json!({"type":"thread.merge_back","createdBy":"user","creationSource":"web","sourceThreadId":source,"targetThreadId":target,"sourcePoint":{"type":"run","runId":run}}))
        },
        // Stop one subagent of a run (subagent.stop).
        "stop-subagent" => {
            thread=text(input,"thread").to_string();let run=text(input,"runId");let node=text(input,"subagentId");
            if !valid_id(&thread)||!valid_id(run)||node.is_empty()||node.len()>512{return Err("invalid_id".into())}
            (DISPATCH,json!({"type":"subagent.stop","threadId":thread,"runId":run,"subagentId":node}))
        },
        "thread-rename"|"regenerate-title"|"pin"|"unpin"|"mark-unread"|"auto-settle"|"archive"|"delete-thread" => {thread=text(input,"thread").to_string();(DISPATCH,protocol::sidebar::thread_action(input)?)},
        "project-rename" | "project-settings" | "project-remove" => (MUTATE,protocol::projects::command(input)?),
        "settle" | "unsettle" | "snooze" | "unsnooze" => {
            thread=text(input,"thread").to_string(); if !valid_id(&thread) {return Err("invalid_id".into())}
            let c=protocol::sidebar::command(text(input,"action"),&thread,input["snoozedUntil"].as_str())?;
            (DISPATCH,c)
        },
        _=>return Err("invalid_command".into())
    };
    if command["type"]=="thread.create" && (text(&command,"title").is_empty() || text(&command,"title").chars().count()>255) {return Err("invalid_title".into())}
    command["commandId"]=json!(format!("omarchy-{id}"));
    // Parsed before persisting: what the outbox keeps is what the typed API sends.
    // createNew and clone.start take no commandId: the envelope keeps it, the wire does not.
    let bare=||{let mut c=command.clone();if let Some(o)=c.as_object_mut(){o.remove("commandId");}c};
    let typed=match tag{
        DISPATCH=>Managed::Dispatch(protocol::api::Command::from_json(&command)?),
        LAUNCH=>Managed::Launch(protocol::api::ThreadLaunch::from_json(&command)?),
        MUTATE=>Managed::Mutate(protocol::api::projects::ProjectMutation::from_json(&command)?),
        CREATE_NEW=>Managed::CreateNew(text(&command,"name").to_string()),
        CLONE=>Managed::Clone(protocol::api::project_clone::CloneStart::from_json(&bare())?),
        _=>return Err("invalid_command".into()),
    };
    let path=xdg("XDG_STATE_HOME",".local/state").join("outbox").join(format!("{id}.json"));
    let envelope=json!({"environment":e.id,"command":command,"rpc":tag});
    let receipt=path.with_extension("receipt.json");
    let non_replayable=tag==CREATE_NEW||tag==CLONE;
    if path.exists() {
        if read_json(&path)?!=envelope {return Err("retry_payload_changed".into())}
        if non_replayable {return if receipt.exists(){read_json(&receipt)}else{Err("rpc_outcome_unknown".into())}}
    } else {save_json(&path,&envelope)?;}
    let command_project=command["projectId"].clone();
    let c=e.connection(true)?;
    let response=match &typed {
        Managed::Dispatch(command)=>protocol::api::orchestration::dispatch(&c,command)?,
        Managed::Launch(launch)=>protocol::api::orchestration::launch_thread(&c,launch)?,
        Managed::Mutate(mutation)=>protocol::api::projects::mutate(&c,mutation)?,
        Managed::CreateNew(name)=>protocol::api::projects::create_new(&c,name)?,
        Managed::Clone(clone)=>protocol::api::project_clone::start(&c,clone)?,
    };
    let project=response.get("projectId").or_else(||response.get("id")).filter(|v|v.is_string()).unwrap_or(if command_project.is_string(){&command_project}else{&input["projectId"]});
    let result=json!({"threadId":thread,"projectId":project,"action":input["action"]});
    if non_replayable{save_json(&receipt,&result)?;}
    Ok(result)
}
pub fn execute(input:&Value) -> Result<Value> {
    if matches!(text(input,"op"),"browser-profiles-read"|"browser-profiles-action"){return crate::browser_profiles::execute(input)}
    if input["op"]=="licenses-read"{return crate::third_party_licenses::read()}
    if input["op"]=="thread-search"{return crate::thread_search::search(input)}
    if input["op"]=="pin-reorder"{return pin_reorder(input)}
    // Generated pages (Mermaid diagrams, QML previews) need no environment.
    if input["op"]=="document-open"&&input["attachment"]["generated"]=="mermaid"{return crate::documents::mermaid_page(input)}
    if input["op"]=="document-open"&&input["attachment"]["generated"]=="qml"{return crate::qml_preview::page(input)}
    if input["op"]=="licenses-notice"{return crate::third_party_licenses::notice(input)}
    if input["op"]=="client-settings"{return match text(input,"action"){
        "read"=>crate::connection_preferences::read_client_preferences(),
        "update"=>crate::connection_preferences::update_client_preferences(text(input,"revision"),&input["patch"]),
        _=>Err("invalid_client_preference".into())
    }}
    if input["op"]=="settings"&&input["scope"].is_object(){return crate::scoped_settings::execute(input)}
    if input["op"]=="connection-route"{return crate::connection_route::choose(input)}
    if input["op"]=="connection-network"{return crate::connection_network::execute(input)}
    if input["op"]=="connection-omabridge"{return crate::omabridge_route::execute(input)}
    if input["op"]=="omabridge-manage"{return Ok(crate::omabridge_device::execute(input))}
    if input["op"]=="omabridge-install-prompt"{return Ok(json!({"prompt":crate::omabridge_install::prompt(input)?}))}
    if input["op"]=="omabridge-invite-handoff"{return crate::omabridge_install::handoff(input)}
    if input["op"]=="connections-manage"{return crate::connections::manage_connections(input)}
    if input["op"]=="window-layout" {return crate::window_layout::apply(input)}
    if input["op"]=="backend-control" {return crate::backend_service::control(text(input,"action"))}
    if input["op"]=="backend-start" {return crate::backend_service::start()}
    if input["op"]=="scroll-config" {return Ok(scroll::read())}
    if input["op"]=="rewind-list"{return Ok(json!({"operations":crate::checkpoint_journal::list()?}))}
    if input["op"]=="rewind-recover"{return crate::checkpoint_operations::recover(input)}
    if input["op"]=="clipboard" {return clipboard::paste(input)}
    if input["op"]=="copy" {return clipboard::copy(input)}
    let e=environments()?.into_iter().find(|e|e.id==text(input,"environment")).ok_or("unknown_environment")?;
    match text(input,"op") {
        "devices-read"=>crate::integration_devices::read(&e),
        "devices-action"=>crate::integration_devices::apply(&e,input),
        "connection-access"=>crate::connection_access::execute(&e,input),
        "terminal-open"=>{let command=match input.get("command"){None|Some(Value::Null)=>None,Some(value)=>Some(value.as_str().ok_or("invalid_terminal_input")?)};crate::terminal::launch_command(&e,text(input,"thread"),command)},
        "document-open"=>crate::documents::read(&e,input),
        "pull-requests"=>{let(tag,payload)=protocol::pull_requests::request(input)?;if tag=="diff" {let c=e.connection(false)?;c.request_json("POST","/api/pull-requests/diff",Some(&payload),false)}else{crate::connection_github::read(&e,&tag,payload)}},
        "settings" if matches!(text(input,"action"),"workspace-read"|"workspace-update")=>workspace_preference(&e,input),
        "settings"=>crate::native_settings::execute(&e,input),
        "provider-instances"=>crate::provider_instances::execute(&e,input),
        "catalog"=>catalog(&e),
        "project-sources"=>project_sources(&e),
        "browse-folders"=>{
            let path=protocol::projects::folder_path(text(input,"path"))?;
            protocol::projects::folders(&protocol::api::filesystem::browse(&e.connection(true)?,&path,None)?)
        },
        "history"=>history(&e,input),
        "model-favorite"=>{let desired=input["favorite"].as_bool().ok_or("invalid_model")?;let current=protocol::api::server::settings(&e.connection(true)?)?;if !current["favorites"].is_array(){return Err("model_preferences_uninitialized".into())}update_settings(&e.connection(true)?,&protocol::models::favorite_payload(text(input,"provider"),text(input,"model"),desired)?).map(|v|protocol::models::preferences(&v))},
        "suggest"=>suggestions::query(&e,input),
        "plan-followup"=>prepare_plan_follow_up(&e,input),
        "rewind-preview"=>crate::checkpoint_operations::preview(&e,input),
        "rewind-dispatch"=>crate::checkpoint_operations::dispatch(&e,input),
        "rewind-observe"=>crate::checkpoint_operations::observe(&e,input),
        "rewind-resolve"=>crate::checkpoint_operations::resolve(&e,input),
        "keybindings-open"=>crate::keybinding_settings::open(&e,input),
        "provider-update"=>crate::provider_updates::update(&e,input),
        "failure-context"=>crate::failure_context::run(&e,input),
        "tool-failure"=>crate::failure_context::tool(&e,input),
        "thread-details-read"=>crate::thread_details::read(&e,input),
        "thread-details-open-editor"=>crate::thread_details::open(&e,input),
        "keybindings-read"=>crate::keybinding_settings::read(&e),
        "keybindings-write"=>crate::keybinding_settings::apply(&e,input),
        "archive-read"=>crate::archive_management::read(&e,input),
        "archive-action"=>crate::archive_management::action(&e,input),
        "archive-state"=>crate::archive_management::observe(&e,input),
        "review-refs"=>crate::working_review::refs(&e,input),
        "branch-refs"=>crate::branch_picker::refs(&e,input),
        "secret-answer"=>secret_answer(&e,input),
        "pull-request-action"=>crate::pull_request_actions::run(&e,input),
        "thread-pull-request"=>crate::pull_request_actions::thread_link(&e,input),
        "pull-request-thread"=>crate::pull_request_actions::open_thread(&e,input),
        "usage-summary"=>crate::usage_summary::read(&e,input),
        "scheduled-tasks"=>crate::scheduled_tasks::run(&e,input),
        "storage-cleanup"=>crate::storage_cleanup::run(&e,input),
        "mcp-app"=>crate::mcp_apps::run(&e,input),
        "provider-setup"=>crate::provider_setup::run(&e,input),
        "agent-sessions"=>crate::agent_sessions::run(&e,input),
        "process-diagnostics"=>crate::process_diagnostics::run(&e,input),
        "local-servers"=>crate::local_servers::read(&e),
        "worktree-setup-cancel"=>protocol::api::worktree_setup::cancel(&e.connection(true)?,text(input,"thread")),
        "otlp-check"=>{let signal=text(input,"signal");if !matches!(signal,"traces"|"metrics"|"logs"){return Err("invalid_command".into())}let url=input["url"].as_str().filter(|u|(u.starts_with("http://")||u.starts_with("https://"))&&u.len()<=2048).ok_or("invalid_settings")?;let r=protocol::api::diagnostics::check_otlp_endpoint(&e.connection(true)?,signal,url)?;Ok(json!({"signal":signal,"result":r}))},
        "workflow-script"=>{let path=input["scriptPath"].as_str().filter(|p|p.starts_with('/')&&p.len()<=4096).ok_or("invalid_file")?;protocol::api::orchestration::workflow_script(&e.connection(true)?,text(input,"thread"),path)},
        "workflow-stop"=>{let node=input["subagentId"].as_str().filter(|n|!n.is_empty()&&n.len()<=512).ok_or("invalid_id")?;protocol::api::orchestration::stop_workflow(&e.connection(true)?,text(input,"thread"),node)?;Ok(json!({}))},
        "git-init"=>{let cwd=input["cwd"].as_str().filter(|c|c.starts_with('/')&&c.len()<=4096).ok_or("invalid_folder")?;protocol::api::vcs::init(&e.connection(true)?,cwd,None)},
        "acp-registry"=>crate::acp_registry::run(&e,input),
        "branch-switch"=>crate::branch_picker::switch(&e,input),
        "branch-check"=>crate::branch_picker::check(&e,input),
        "branch-resolve"=>crate::branch_picker::resolve(&e,input),
        "working-review"=>crate::working_review::execute(&e,input),
        "git-read"=>crate::git_operations::read(&e,input),
        "git-action"=>crate::git_operations::action(&e,input),
        "git-state"=>crate::git_operations::status(&e,input),
        "git-resolve"=>crate::git_operations::resolve(&e,input),
        "git-terminal"=>crate::git_operations::open_terminal(&e,input),
        "tool-inspector"=>crate::tool_inspector::execute(&e,input),
        "diff"=>crate::diff_review::execute(&e,input),
        "manage"=>manage(&e,input),
        // T3 ThreadVisitWatermark: the open thread is seen up to its updatedAt; Woke is cleared at wokeAt.
        "thread-visit"=>{let thread=text(input,"thread");let at=text(input,"visitedAt");if !valid_id(thread)||chrono::DateTime::parse_from_rfc3339(at).is_err(){return Err("invalid_command".into())}dispatch_command(&e,&json!({"type":"thread.visit","commandId":format!("omarchy-visit-{}",uuid::Uuid::new_v4()),"threadId":thread,"visitedAt":at}))},
        "upload"=>upload(&e,text(input,"url")),
        "delete-upload"=>protocol::api::assets::delete_attachment(&e.connection(true)?,text(input,"attachmentId")),
        "approval"|"question"|"send"|"stop"|"queue"=>dispatch(&e,input),
        _=>Err("invalid_command".into())
    }
}

/// Re-read the native plan before preparing a user-triggered follow-up. This
/// operation only returns a prompt; the existing durable send path owns writes.
pub fn prepare_plan_follow_up(e:&Environment,input:&Value)->Result<Value>{
    let session=Client::shared()?.get(e,"/api/auth/session",false)?;
    if session["authenticated"]!=true || !session.get("permissions").or_else(||session.get("scopes")).and_then(Value::as_array).is_some_and(|scopes|scopes.iter().any(|scope|scope=="orchestration:operate")){return Err("permission_denied".into())}
    let thread=text(input,"thread");if !valid_id(thread){return Err("invalid_plan_scope".into())}
    let config=protocol::api::server::config(&e.connection(true)?)?;
    let selection=crate::native_settings::model_selection(&input["modelSelection"])?;
    let provider=config["providers"].as_array().into_iter().flatten().find(|p|p["instanceId"]==selection["instanceId"]&&p["enabled"]==true&&p["status"]!="error");
    let local=crate::connection_preferences::read_client_preferences()?;
    let gate=crate::plan_workflow::mode(local["planModeEnabled"]==true,provider,text(input,"interactionMode"));
    if gate["interactionMode"]!="plan"{return Err("plan_mode_unavailable".into())}
    crate::native_settings::patch(&json!({"key":"defaultModelSelection","value":selection}),Some(&config))?;
    let bounded=Client::shared()?.bounded_thread(e,thread)?;let projection=&bounded["projection"];
    let view=crate::plan_workflow::view(projection,crate::rendered_markdown);
    let plan=&view["ready"];
    if view.get("error").is_some()||plan.is_null()||plan["id"]!=input["planId"]||input["hasAttachments"]!=false{return Err("plan_no_longer_ready".into())}
    let mut result=crate::plan_workflow::follow_up(plan,thread,text(input,"draftText")).map_err(str::to_string)?;
    result["planId"]=plan["id"].clone();result["thread"]=json!(thread);result["environment"]=json!(&e.id);
    result["projectId"]=projection["thread"]["projectId"].clone();result["branch"]=projection["thread"]["branch"].clone();result["worktreePath"]=projection["thread"]["worktreePath"].clone();
    result["title"]=json!(crate::plan_workflow::implementation_title(text(plan,"markdown")));
    result["modelSelection"]=selection;Ok(result)
}
/// secrets.answerRequest: the value goes to the server once and is not journaled.
fn secret_answer(e:&Environment,input:&Value)->Result<Value>{
    let thread=text(input,"thread");let item=text(input,"turnItemId");
    if !valid_id(thread)||item.is_empty()||item.len()>512{return Err("invalid_id".into())}
    let answer=match text(&input["answer"],"type"){
        "decline"=>json!({"type":"decline"}),
        "save"=>{let secret=text(&input["answer"],"secret").trim();if secret.is_empty()||secret.len()>64*1024{return Err("invalid_secret".into())}json!({"type":"save","secret":secret})},
        _=>return Err("invalid_secret".into()),
    };
    let answer=protocol::api::secrets::SecretAnswer::from_json(&answer).map_err(|_|"invalid_secret")?;
    protocol::api::secrets::answer(&e.connection(true)?,thread,item,&answer)?;
    Ok(json!({"turnItemId":item}))
}
/// New pin order keys for pinned threads (thread.pin.reorder), on each thread's environment.
fn pin_reorder(input:&Value)->Result<Value>{
    let rows=input["assignments"].as_array().filter(|a|!a.is_empty()&&a.len()<=200).ok_or("invalid_command")?;
    let envs=environments()?;
    for a in rows{
        let thread=text(a,"thread");let key=text(a,"orderKey");
        if !valid_id(thread)||key.is_empty()||key.len()>64||!key.bytes().all(|b|b.is_ascii_lowercase()){return Err("invalid_command".into())}
        let e=envs.iter().find(|e|e.id==text(a,"environment")).ok_or("unknown_environment")?;
        dispatch_command(e,&json!({"type":"thread.pin.reorder","commandId":format!("omarchy-pin-{}",uuid::Uuid::new_v4()),"threadId":thread,"orderKey":key}))?;
    }
    Ok(json!({"count":rows.len()}))
}
