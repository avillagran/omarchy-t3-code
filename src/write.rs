//! T3 Effect RPC and signed attachment uploads. Never retries a mutation implicitly.
use crate::*;
use serde_json::{json, Value};
use std::{fs::OpenOptions, os::unix::fs::OpenOptionsExt};

pub fn rpc(e: &Environment, tag: &str, payload: Value) -> Result<Value> {
    if let Some(error)=&e.connection_error {return Err(error.clone())}
    if let Some(connection)=&e.connection {return connection.rpc(tag,payload)}
    protocol::Connection::new(&e.base_url,None,protocol::Credential::bearer(&environment_token(e,true)?)?)?.rpc(tag,payload)
}
fn interaction_mode(input:&Value)->Result<&str>{
    match input.get("interactionMode"){None|Some(Value::Null)=>Ok("default"),Some(v)=>v.as_str().filter(|s|matches!(*s,"default"|"plan")).ok_or("invalid_interaction_mode".into())}
}
fn runtime_mode(input:&Value)->Result<&str>{
    let mode=match input.get("runtimeMode"){None|Some(Value::Null)=>"approval-required",Some(value)=>value.as_str().ok_or("invalid_runtime_mode")?};
    if protocol::models::valid_runtime_mode(mode){Ok(mode)}else{Err("invalid_runtime_mode".into())}
}
pub fn catalog(e: &Environment) -> Result<Value> {
    let config = rpc(e,"server.getConfig",json!({}))?;
    let mut result=protocol::models::catalog(&config);
    for provider in result["providers"].as_array_mut().unwrap(){
        if let Some(native)=config["providers"].as_array().into_iter().flatten().find(|p|p["instanceId"]==provider["instanceId"]){provider["showInteractionModeToggle"]=json!(native["showInteractionModeToggle"]!=false);provider["supportsConversationRollback"]=json!(native["supportsConversationRollback"]!=false);}
    }
    let permission=Client::new()?.get(e,"/api/auth/session",false);
    result["canWrite"]=json!(permission.as_ref().is_ok_and(|s|s["authenticated"]==true&&s.get("permissions").or_else(||s.get("scopes")).and_then(Value::as_array).is_some_and(|s|s.iter().any(|v|v=="orchestration:operate"))));
    if let Err(error)=permission{result["permissionError"]=json!(error);}
    Ok(result)
}
pub fn project_sources(e:&Environment)->Result<Value>{
    let config=rpc(e,"server.getConfig",json!({}))?;
    let discovery=rpc(e,"server.discoverSourceControl",json!({}))?;
    let providers=discovery["sourceControlProviders"].as_array().into_iter().flatten().map(|p|json!({"kind":p["kind"],"status":p["status"],"auth":{"status":p["auth"]["status"]}})).collect::<Vec<_>>();
    Ok(json!({"newProjectsRoot":config["newProjectsRoot"],"baseDirectory":config["settings"]["addProjectBaseDirectory"],"cloneTracking":config["environment"]["capabilities"]["projectCloneTracking"]==true,"providers":providers}))
}
pub fn workspace_preference(e:&Environment,input:&Value)->Result<Value>{
    let project=text(input,"projectId");if !valid_id(project){return Err("invalid_project".into())}
    let raw=if input["action"]=="workspace-update" {
        if !matches!(input["mode"].as_str(),Some("local"|"worktree")){return Err("invalid_settings".into())}
        rpc(e,"server.updateSettings",json!({"patch":{"defaultThreadEnvMode":input["mode"]}}))?
    }else if input["action"]=="workspace-read"{rpc(e,"server.getSettings",json!({}))?}else{return Err("invalid_settings".into())};
    let overrides=&raw["projectSettingsOverrides"][project];
    let mode=overrides["defaultThreadEnvMode"].as_str().or(raw["defaultThreadEnvMode"].as_str()).filter(|s|matches!(*s,"local"|"worktree"));
    let origin=overrides["newWorktreesStartFromOrigin"].as_bool().or(raw["newWorktreesStartFromOrigin"].as_bool()).unwrap_or(true);
    let mut result=crate::native_settings::thread_defaults(&raw,project);
    result["mode"]=json!(mode);result["startFromOrigin"]=json!(origin);Ok(result)
}
pub fn history(e: &Environment,input:&Value)->Result<Value>{
    let thread=text(input,"thread");if !valid_id(thread){return Err("invalid_id".into())}
    let path=protocol::history::path(thread,text(input,"cursor"))?;
    let page=Client::new()?.get(e,&path,true)?;
    normalize_messages(&protocol::history::envelope(&page,thread)?)
}

pub fn upload(e: &Environment, file_url: &str) -> Result<Value> {
    let url=reqwest::Url::parse(file_url).map_err(|_| "invalid_file")?;
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
    if let Some(error)=&e.connection_error {return Err(error.clone())}
    let connection=if let Some(c)=&e.connection {c.as_ref().clone()} else {protocol::Connection::new(&e.base_url,None,protocol::Credential::bearer(&environment_token(e,true)?)?)?};
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
            let snapshot=Client::new()?.bounded_thread(e,thread)?;
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
        if input["delivery"]!="queue" {c["deliveryIntent"]=json!("auto");}
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
        Some(protocol::models::runtime_mode_command(thread,&format!("omarchy-{id}-runtime-mode"),runtime_mode(input)?)?)
    }else{None};
    let mut envelope=json!({"environment":e.id,"command":cmd});
    if permissions.is_some(){envelope["runtimeMode"]=input["runtimeMode"].clone();}
    let interaction=if cmd["type"]=="message.dispatch"&&input.get("interactionMode").is_some_and(|v|!v.is_null()){
        Some(json!({"type":"thread.interaction-mode.set","threadId":thread,"commandId":format!("omarchy-{id}-interaction-mode"),"interactionMode":interaction_mode(input)?}))
    }else{None};
    if interaction.is_some(){envelope["interactionMode"]=input["interactionMode"].clone();}
    if path.exists() {if read_json(&path)?!=envelope {return Err("retry_payload_changed".into())}}
    else { save_json(&path,&envelope)?; }
    if let Some(mode)=permissions{rpc(e,"orchestration.dispatchCommand",mode)?;}
    if let Some(mode)=interaction{rpc(e,"orchestration.dispatchCommand",mode)?;}
    rpc(e,"orchestration.dispatchCommand",cmd)
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
            ("projects.createNew",json!({"name":name}))
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
            ("projectClone.start",c)
        },
        "new-thread" => {
            let project=text(input,"projectId"); if !valid_id(project) || !input["modelSelection"].is_object() {return Err("invalid_project".into())}
            thread=format!("omarchy-thread-{id}");
            let mode=runtime_mode(input)?;
            if input["workspaceStrategy"].is_object(){("orchestration.launchThread",launch_command(input,&thread)?)}else{("orchestration.dispatchCommand",json!({"type":"thread.create","threadId":thread,"projectId":project,"title":text(input,"title").trim(),"modelSelection":input["modelSelection"],"runtimeMode":mode,"interactionMode":interaction_mode(input)?,"branch":input["branch"],"worktreePath":input["worktreePath"],"createdBy":"user","creationSource":"web"}))}
        },
        "add-project" => {
            let path=if input["workspaceRoot"].is_string(){
                let path=protocol::projects::folder_path(text(input,"workspaceRoot"))?;
                let meta=rpc(e,"filesystem.getMetadata",json!({"paths":[path]}))?;
                if meta["entries"][0]["kind"]!="directory"{return Err("invalid_folder".into())}path
            }else{
                if e.id!="local"{return Err("local_folder_only".into())}
                let path=reqwest::Url::parse(text(input,"url")).map_err(|_| "invalid_folder")?.to_file_path().map_err(|_| "invalid_folder")?.canonicalize().map_err(|_| "invalid_folder")?;
                if !path.is_dir(){return Err("invalid_folder".into())}path.to_str().ok_or("invalid_folder")?.to_string()
            };
            let title=path.trim_end_matches('/').rsplit('/').next().filter(|s|!s.is_empty()).unwrap_or("Project");
            ("projects.mutate",json!({"type":"project.create","projectId":format!("omarchy-project-{id}"),"title":title,"workspaceRoot":path,"createWorkspaceRootIfMissing":false}))
        },
        // Forks open as a new thread; merge-back carries a fork's finished run to its source.
        "fork" => {
            let source=text(input,"thread"); let run=text(input,"runId");
            if !valid_id(source)||!valid_id(run){return Err("invalid_id".into())}
            thread=format!("omarchy-thread-{id}");
            let title=text(input,"title").trim(); let title=if title.is_empty(){"Fork".to_string()}else{format!("{} fork",title.chars().take(250).collect::<String>())};
            ("orchestration.dispatchCommand",json!({"type":"thread.fork","createdBy":"user","creationSource":"web","sourceThreadId":source,"targetThreadId":thread,"sourcePoint":{"type":"run","runId":run},"title":title}))
        },
        "merge-back" => {
            let source=text(input,"thread"); let run=text(input,"runId"); let target=text(input,"targetThread");
            if !valid_id(source)||!valid_id(run)||!valid_id(target){return Err("invalid_id".into())}
            thread=target.to_string();
            ("orchestration.dispatchCommand",json!({"type":"thread.merge_back","createdBy":"user","creationSource":"web","sourceThreadId":source,"targetThreadId":target,"sourcePoint":{"type":"run","runId":run}}))
        },
        // Stop one subagent of a run (subagent.stop).
        "stop-subagent" => {
            thread=text(input,"thread").to_string();let run=text(input,"runId");let node=text(input,"subagentId");
            if !valid_id(&thread)||!valid_id(run)||node.is_empty()||node.len()>512{return Err("invalid_id".into())}
            ("orchestration.dispatchCommand",json!({"type":"subagent.stop","threadId":thread,"runId":run,"subagentId":node}))
        },
        "thread-rename"|"regenerate-title"|"pin"|"unpin"|"mark-unread"|"auto-settle"|"archive"|"delete-thread" => {thread=text(input,"thread").to_string();("orchestration.dispatchCommand",protocol::sidebar::thread_action(input)?)},
        "project-rename" | "project-settings" | "project-remove" => ("projects.mutate",protocol::projects::command(input)?),
        "settle" | "unsettle" | "snooze" | "unsnooze" => {
            thread=text(input,"thread").to_string(); if !valid_id(&thread) {return Err("invalid_id".into())}
            let c=protocol::sidebar::command(text(input,"action"),&thread,input["snoozedUntil"].as_str())?;
            ("orchestration.dispatchCommand",c)
        },
        _=>return Err("invalid_command".into())
    };
    if command["type"]=="thread.create" && (text(&command,"title").is_empty() || text(&command,"title").chars().count()>255) {return Err("invalid_title".into())}
    command["commandId"]=json!(format!("omarchy-{id}"));
    let path=xdg("XDG_STATE_HOME",".local/state").join("outbox").join(format!("{id}.json"));
    let envelope=json!({"environment":e.id,"command":command,"rpc":tag});
    let receipt=path.with_extension("receipt.json");
    let non_replayable=matches!(tag,"projects.createNew"|"projectClone.start");
    if path.exists() {
        if read_json(&path)?!=envelope {return Err("retry_payload_changed".into())}
        if non_replayable {return if receipt.exists(){read_json(&receipt)}else{Err("rpc_outcome_unknown".into())}}
    } else {save_json(&path,&envelope)?;}
    let command_project=command["projectId"].clone();
    let response=rpc(e,tag,command)?;
    let project=response.get("projectId").or_else(||response.get("id")).filter(|v|v.is_string()).unwrap_or(if command_project.is_string(){&command_project}else{&input["projectId"]});
    let result=json!({"threadId":thread,"projectId":project,"action":input["action"]});
    if non_replayable{save_json(&receipt,&result)?;}
    Ok(result)
}
pub fn execute(input:&Value) -> Result<Value> {
    if matches!(text(input,"op"),"browser-profiles-read"|"browser-profiles-action"){return crate::browser_profiles::execute(input)}
    if input["op"]=="licenses-read"{return crate::third_party_licenses::read()}
    if input["op"]=="thread-search"{return crate::thread_search::search(input)}
    // Generated pages (Mermaid diagrams) need no environment.
    if input["op"]=="document-open"&&input["attachment"]["generated"]=="mermaid"{return crate::documents::mermaid_page(input)}
    if input["op"]=="licenses-notice"{return crate::third_party_licenses::notice(input)}
    if input["op"]=="client-settings"{return match text(input,"action"){
        "read"=>crate::connection_preferences::read_client_preferences(),
        "update"=>crate::connection_preferences::update_client_preferences(text(input,"revision"),&input["patch"]),
        _=>Err("invalid_client_preference".into())
    }}
    if input["op"]=="settings"&&input["scope"].is_object(){return crate::scoped_settings::execute(input)}
    if input["op"]=="connection-route"{return crate::connection_route::choose(input)}
    if input["op"]=="connection-network"{return crate::connection_network::execute(input)}
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
        "pull-requests"=>{let(tag,payload)=protocol::pull_requests::request(input)?;if tag=="diff" {let c=if let Some(c)=&e.connection{c.as_ref().clone()}else{protocol::Connection::new(&e.base_url,None,protocol::Credential::bearer(&environment_token(&e,false)?)?)?};c.request_json("POST","/api/pull-requests/diff",Some(&payload),false)}else{crate::connection_github::read(&e,&tag,payload)}},
        "settings" if matches!(text(input,"action"),"workspace-read"|"workspace-update")=>workspace_preference(&e,input),
        "settings"=>crate::native_settings::execute(&e,input),
        "provider-instances"=>crate::provider_instances::execute(&e,input),
        "catalog"=>catalog(&e),
        "project-sources"=>project_sources(&e),
        "browse-folders"=>{
            let path=protocol::projects::folder_path(text(input,"path"))?;
            protocol::projects::folders(&rpc(&e,"filesystem.browse",json!({"partialPath":path}))?)
        },
        "history"=>history(&e,input),
        "model-favorite"=>{let desired=input["favorite"].as_bool().ok_or("invalid_model")?;let current=rpc(&e,"server.getSettings",json!({}))?;if !current["favorites"].is_array(){return Err("model_preferences_uninitialized".into())}rpc(&e,"server.updateSettings",protocol::models::favorite_payload(text(input,"provider"),text(input,"model"),desired)?).map(|v|protocol::models::preferences(&v))},
        "suggest"=>suggestions::query(&e,input),
        "plan-followup"=>prepare_plan_follow_up(&e,input),
        "rewind-preview"=>crate::checkpoint_operations::preview(&e,input),
        "rewind-dispatch"=>crate::checkpoint_operations::dispatch(&e,input),
        "rewind-observe"=>crate::checkpoint_operations::observe(&e,input),
        "rewind-resolve"=>crate::checkpoint_operations::resolve(&e,input),
        "keybindings-open"=>crate::keybinding_settings::open(&e,input),
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
        "usage-summary"=>crate::usage_summary::read(&e,input),
        "scheduled-tasks"=>crate::scheduled_tasks::run(&e,input),
        "storage-cleanup"=>crate::storage_cleanup::run(&e,input),
        "mcp-app"=>crate::mcp_apps::run(&e,input),
        "provider-setup"=>crate::provider_setup::run(&e,input),
        "agent-sessions"=>crate::agent_sessions::run(&e,input),
        "process-diagnostics"=>crate::process_diagnostics::run(&e,input),
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
        "upload"=>upload(&e,text(input,"url")),
        "delete-upload"=>rpc(&e,"attachments.delete",json!({"attachmentId":input["attachmentId"]})),
        "approval"|"question"|"send"|"stop"|"queue"=>dispatch(&e,input),
        _=>Err("invalid_command".into())
    }
}

/// Re-read the native plan before preparing a user-triggered follow-up. This
/// operation only returns a prompt; the existing durable send path owns writes.
pub fn prepare_plan_follow_up(e:&Environment,input:&Value)->Result<Value>{
    let session=Client::new()?.get(e,"/api/auth/session",false)?;
    if session["authenticated"]!=true || !session.get("permissions").or_else(||session.get("scopes")).and_then(Value::as_array).is_some_and(|scopes|scopes.iter().any(|scope|scope=="orchestration:operate")){return Err("permission_denied".into())}
    let thread=text(input,"thread");if !valid_id(thread){return Err("invalid_plan_scope".into())}
    let config=rpc(e,"server.getConfig",json!({}))?;
    let selection=crate::native_settings::model_selection(&input["modelSelection"])?;
    let provider=config["providers"].as_array().into_iter().flatten().find(|p|p["instanceId"]==selection["instanceId"]&&p["enabled"]==true&&p["status"]!="error");
    let local=crate::connection_preferences::read_client_preferences()?;
    let gate=crate::plan_workflow::mode(local["planModeEnabled"]==true,provider,text(input,"interactionMode"));
    if gate["interactionMode"]!="plan"{return Err("plan_mode_unavailable".into())}
    crate::native_settings::patch(&json!({"key":"defaultModelSelection","value":selection}),Some(&config))?;
    let bounded=Client::new()?.bounded_thread(e,thread)?;let projection=&bounded["projection"];
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
    rpc(e,"secrets.answerRequest",json!({"threadId":thread,"turnItemId":item,"answer":answer}))?;
    Ok(json!({"turnItemId":item}))
}
