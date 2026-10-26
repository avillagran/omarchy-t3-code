//! Nightly additions. Every request remains scoped to its environment and thread.
use crate::*;

pub fn shell(client: &Client, e: &Environment) -> Result<Value> {
    let c=client.connection(e)?;
    let caps=server_capabilities::get(&e.id,&c)?;
    let mut url=Url::parse("http://shell.invalid/api/orchestration/shell").unwrap();
    if caps.shell_thread_families {url.query_pairs_mut().append_pair("omitIdleSubagents","1");}
    if caps.shell_older_settled {url.query_pairs_mut().append_pair("omitOlderSettled","1");}
    let path=format!("{}{}",url.path(),url.query().map(|q|format!("?{q}")).unwrap_or_default());
    c.request_json("GET",&path,None,true)
}
pub fn execute(e: &Environment, input: &Value) -> Result<Value> {
    let action=text(input,"action");
    let c=e.connection(true)?;
    let thread=text(input,"thread");
    let args=input.get("args").cloned().unwrap_or_else(||json!({}));
    let (method,payload)=match action {
        "older-settled" | "older-counts" => ("orchestration.getOlderSettledThreads",args),
        "family" => ("orchestration.getThreadFamily",json!({"threadId":thread})),
        "worktree-threads" => ("orchestration.getWorktreeThreadIds",json!({"worktreePath":args["worktreePath"]})),
        "ignored-files" => ("projects.listIgnoredFiles",json!({"cwd":args["cwd"]})),
        "guest-create" => ("guestShare.create",json!({"threadId":thread})),
        "guest-close" | "guest-dismiss" | "guest-send" | "limit-recovery" => {
            let id=text(input,"requestId");uuid::Uuid::parse_str(id).map_err(|_|"invalid_request_id")?;
            if action=="limit-recovery" {
                protocol::api::ids::id("runId",text(&args,"runId"))?;
                let reset=chrono::DateTime::parse_from_rfc3339(text(&args,"resetAt")).map_err(|_|"invalid_limit_recovery")?;
                if !args.get("autoResume").is_some_and(Value::is_boolean)&&!args.get("snooze").is_some_and(Value::is_boolean){return Err("invalid_limit_recovery".into())}
                if args["snooze"]==true&&reset.timestamp_millis()<=Utc::now().timestamp_millis(){return Err("usage_reset_passed".into())}
            }
            let mut command=match action {
                "limit-recovery"=>json!({"type":"thread.metadata.update","threadId":thread,"limitRecovery":args}),
                "guest-close"=>json!({"type":"thread.guest-share.close","threadId":thread}),
                "guest-dismiss"=>json!({"type":"thread.guest-message.dismiss","threadId":thread,"guestMessageId":args["guestMessageId"]}),
                _=>json!({"type":"message.dispatch","threadId":thread,"guestMessageId":args["guestMessageId"],"messageId":format!("omarchy-{id}"),"text":"","attachments":[],"createdBy":"user","creationSource":"web","dispatchMode":{"type":"start_immediately"},"deliveryIntent":"auto"}),
            };
            command["commandId"]=json!(format!("omarchy-{id}"));
            return write::dispatch_command(e,&command);
        }
        "connect-reachability"=>return c.request_json("POST","/api/connect/reachability",None,false),
        "eas-account" => ("device.easAccount",args),
        _=>return Err("invalid_nightly_action".into()),
    };
    let result=protocol::api::call(&c,method,&payload,None)?;
    if action=="older-counts" {shell_state::update_older_counts(&e.id,text(&payload["before"],"settledAt"),&result)?;}
    if matches!(action,"older-settled"|"family") {shell_state::supplement(&e.id,&result)?;}
    Ok(result)
}
