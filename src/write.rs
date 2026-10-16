//! T3 Effect RPC and signed attachment uploads. Never retries a mutation implicitly.
use crate::*;
use serde_json::{json, Value};
use std::{fs::OpenOptions, os::unix::fs::OpenOptionsExt};

pub fn rpc(e: &Environment, tag: &str, payload: Value) -> Result<Value> {
    if let Some(error)=&e.connection_error {return Err(error.clone())}
    if let Some(connection)=&e.connection {return connection.rpc(tag,payload)}
    protocol::Connection::new(&e.base_url,None,protocol::Credential::bearer(&environment_token(e,true)?)?)?.rpc(tag,payload)
}
pub fn catalog(e: &Environment) -> Result<Value> {
    let config = rpc(e,"server.getConfig",json!({}))?;
    let mut result=protocol::models::catalog(&config);result["canWrite"]=json!(e.write_token_file.is_some() || e.bearer.is_some() || e.connection.is_some());Ok(result)
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
pub fn dispatch(e: &Environment, input: &Value) -> Result<Value> {
    let id=text(input,"requestId");
    uuid::Uuid::parse_str(id).map_err(|_| "invalid_request_id")?;
    let thread=text(input,"thread");
    if !valid_id(thread) {return Err("invalid_id".into())}
    let mut cmd = if input["op"]=="approval" {
        let path=xdg("XDG_STATE_HOME",".local/state").join("outbox").join(format!("{id}.json"));
        let expected=json!({"environment":e.id,"command":{"type":"runtime-request.respond","threadId":thread,"requestId":input["runtimeRequestId"],"decision":input["decision"],"commandId":format!("omarchy-{id}")}});
        if path.exists() {
            // A lost acknowledgement may already have resolved the request. Replay
            // only the identical durable command, letting T3 deduplicate its ID.
            if read_json(&path)?!=expected {return Err("retry_payload_changed".into())}
            expected["command"].clone()
        } else {
            let mut url=reqwest::Url::parse(&e.base_url).map_err(|_|"invalid_endpoint")?;
            url.path_segments_mut().map_err(|_|"invalid_endpoint")?.extend(["api","orchestration","threads",thread,"bounded"]);
            let snapshot=Client::new()?.get(e,url.path(),true)?;
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
        let mut c=json!({"type":"message.dispatch","threadId":thread,"messageId":format!("omarchy-{id}"),"text":protocol::composer::legacy_attachment_text(content,attachments),"attachments":attachments,"createdBy":"user","creationSource":"web","dispatchMode":{"type":if input["delivery"]=="queue" {"queue_after_active"} else {"start_immediately"}}});
        if input["delivery"]!="queue" {c["deliveryIntent"]=json!("auto");}
        if input["modelSelection"].is_object() {c["modelSelection"]=input["modelSelection"].clone();}
        c
    };
    cmd["commandId"]=json!(format!("omarchy-{id}"));
    // Persist before sending. An explicit retry must reuse exactly the same command.
    let path=xdg("XDG_STATE_HOME",".local/state").join("outbox").join(format!("{id}.json"));
    let envelope=json!({"environment":e.id,"command":cmd});
    if path.exists() {if read_json(&path)?!=envelope {return Err("retry_payload_changed".into())}}
    else { save_json(&path,&envelope)?; }
    rpc(e,"orchestration.dispatchCommand",cmd)
}
/// Header actions create metadata only; never starts a provider or an agent run.
pub fn manage(e: &Environment, input: &Value) -> Result<Value> {
    let id=text(input,"requestId"); uuid::Uuid::parse_str(id).map_err(|_| "invalid_request_id")?;
    let mut thread=String::new();
    let (tag,mut command)=match text(input,"action") {
        "new-thread" => {
            let project=text(input,"projectId"); if !valid_id(project) || !input["modelSelection"].is_object() {return Err("invalid_project".into())}
            thread=format!("omarchy-thread-{id}");
            let mode=if input["runtimeMode"]=="full-access" {"full-access"} else {"approval-required"};
            ("orchestration.dispatchCommand",json!({"type":"thread.create","threadId":thread,"projectId":project,"title":text(input,"title").trim(),"modelSelection":input["modelSelection"],"runtimeMode":mode,"interactionMode":"default","branch":input["branch"],"worktreePath":input["worktreePath"],"createdBy":"user","creationSource":"web"}))
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
    if path.exists() {if read_json(&path)?!=envelope {return Err("retry_payload_changed".into())}} else {save_json(&path,&envelope)?;}
    rpc(e,tag,command).map(|_|json!({"threadId":thread,"projectId":input["projectId"],"action":input["action"]}))
}
pub fn execute(input:&Value) -> Result<Value> {
    if input["op"]=="window-layout" {return crate::window_layout::apply(input)}
    if input["op"]=="backend-control" {return crate::backend_service::control(text(input,"action"))}
    if input["op"]=="backend-start" {return crate::backend_service::start()}
    if input["op"]=="scroll-config" {return Ok(scroll::read())}
    if input["op"]=="clipboard" {return clipboard::paste(input)}
    if input["op"]=="copy" {return clipboard::copy(input)}
    let e=environments()?.into_iter().find(|e|e.id==text(input,"environment")).ok_or("unknown_environment")?;
    match text(input,"op") {
        "document-open"=>crate::documents::read(&e,input),
        "pull-requests"=>{let(tag,payload)=protocol::pull_requests::request(input)?;if tag=="diff" {let c=if let Some(c)=&e.connection{c.as_ref().clone()}else{protocol::Connection::new(&e.base_url,None,protocol::Credential::bearer(&environment_token(&e,false)?)?)?};c.request_json("POST","/api/pull-requests/diff",Some(&payload),false)}else{rpc(&e,&tag,payload)}},
        "settings"=>{if input["action"]=="update"{rpc(&e,"server.updateSettings",protocol::settings::patch(input)?)?;}else if input["action"]!="read"{return Err("invalid_settings".into())}rpc(&e,"server.getSettings",json!({})).map(|v|protocol::settings::snapshot(&v))},
        "catalog"=>catalog(&e),
        "browse-folders"=>{
            let path=protocol::projects::folder_path(text(input,"path"))?;
            protocol::projects::folders(&rpc(&e,"filesystem.browse",json!({"partialPath":path}))?)
        },
        "history"=>history(&e,input),
        "model-favorite"=>{let desired=input["favorite"].as_bool().ok_or("invalid_model")?;let current=rpc(&e,"server.getSettings",json!({}))?;if !current["favorites"].is_array(){return Err("model_preferences_uninitialized".into())}rpc(&e,"server.updateSettings",protocol::models::favorite_payload(text(input,"provider"),text(input,"model"),desired)?).map(|v|protocol::models::preferences(&v))},
        "suggest"=>suggestions::query(&e,input),
        "diff"=>{let from=input["fromTurnCount"].as_u64().ok_or("invalid_range")?;let to=input["toTurnCount"].as_u64().filter(|n|*n>=from).ok_or("invalid_range")?;let thread=text(input,"thread");if !valid_id(thread){return Err("invalid_id".into())}if from==0{rpc(&e,"orchestration.getFullThreadDiff",json!({"threadId":thread,"toTurnCount":to}))}else{rpc(&e,"orchestration.getTurnDiff",json!({"threadId":thread,"fromTurnCount":from,"toTurnCount":to}))}},
        "manage"=>manage(&e,input),
        "upload"=>upload(&e,text(input,"url")),
        "delete-upload"=>rpc(&e,"attachments.delete",json!({"attachmentId":input["attachmentId"]})),
        "approval"|"question"|"send"|"stop"|"queue"=>dispatch(&e,input),
        _=>Err("invalid_command".into())
    }
}
