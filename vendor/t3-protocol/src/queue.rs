//! T3 queued-run workflow, portable for desktop and Omi.
use serde_json::{json,Value};
use crate::util::text;
fn rows<'a>(v:&'a Value,k:&str)->impl DoubleEndedIterator<Item=&'a Value>{v[k].as_array().into_iter().flatten()}
pub fn derive(p:&Value)->Value{
 let active=rows(p,"runs").rev().find(|r|matches!(text(r,"status"),"preparing"|"starting"|"running"|"waiting"));
 let provider_id=active.and_then(|r|r["providerThreadId"].as_str()).or(p["thread"]["activeProviderThreadId"].as_str());
 let attached=rows(p,"providerThreads").find(|t|Some(text(t,"id"))==provider_id).or_else(||rows(p,"providerThreads").find(|t|t["appThreadId"]==p["thread"]["id"]&&t["providerSessionId"].is_string()));
 let session=attached.and_then(|t|rows(p,"providerSessions").find(|s|s["id"]==t["providerSessionId"])).or_else(||rows(p,"providerSessions").rev().find(|s|!matches!(text(s,"status"),"stopped"|"error")));
 let caps=session.map(|s|&s["capabilities"]["turns"]);
 let mut queued:Vec<&Value>=rows(p,"runs").filter(|r|r["status"]=="queued" && !rows(p,"messages").any(|m|m["id"]==r["userMessageId"]&&(m.get("delegatedCompletion").is_some()||m.get("notification").is_some()))).collect();
 queued.sort_by_key(|r|(r["queuePosition"].as_u64().or(r["ordinal"].as_u64()).unwrap_or(0),r["ordinal"].as_u64().unwrap_or(0)));
 let items:Vec<Value>=queued.iter().map(|r|{let m=rows(p,"messages").find(|m|m["id"]==r["userMessageId"]);json!({"available":m.is_some(),"runId":r["id"],"messageId":r["userMessageId"],"text":m.map(|m|text(m,"text")).unwrap_or(""),"attachments":m.map(|m|m["attachments"].clone()).filter(Value::is_array).unwrap_or(json!([])),"context":m.map(|m|m["context"].clone()).unwrap_or(Value::Null)})}).collect();
 // A promoted steer cancels its queued run but re-homes the message onto the
 // active run; only hide messages still owned by the queued/cancelled run.
 let owned=|r:&Value|rows(p,"messages").find(|m|m["id"]==r["userMessageId"]).map_or(true,|m|m["runId"].is_null()||m["runId"]==r["id"]);
 let hidden:Vec<Value>=rows(p,"runs").filter(|r|(r["status"]=="queued" || (r["status"]=="cancelled"&&r["startedAt"].is_null()))&&owned(r)).map(|r|r["userMessageId"].clone()).collect();
 let steerable=active.is_some_and(|a|a["status"]=="running"&&a["activeAttemptId"].is_string()&&rows(p,"providerTurns").any(|t|t["runAttemptId"]==a["activeAttemptId"]&&t["status"]=="running"));
 json!({"acknowledgedMessageIds":rows(p,"messages").map(|m|m["id"].clone()).chain(rows(p,"runs").map(|r|r["userMessageId"].clone())).collect::<Vec<_>>(),"items":items,"hiddenMessageIds":hidden,"held":rows(p,"runs").any(|r|r["status"]=="queued"&&r["queueHeld"]==true),"activeRunId":active.map(|a|a["id"].clone()),"canReorder":caps.is_some_and(|c|c["supportsQueuedMessages"]==true),"canSteer":steerable&&caps.is_some_and(|c|c["supportsActiveSteering"]==true||c["supportsSteeringByInterruptRestart"]==true)})
}
#[cfg(test)]mod tests{use super::*;
 #[test]fn queue_order_capabilities_and_timeline_visibility_follow_t3(){
 let mut p=json!({"thread":{"id":"thread"},"runs":[{"id":"active","status":"running","userMessageId":"sent","ordinal":1,"activeAttemptId":"attempt"},{"id":"later","status":"queued","userMessageId":"q2","ordinal":3},{"id":"first","status":"queued","userMessageId":"q1","ordinal":4,"queuePosition":1,"queueHeld":true},{"id":"cancel","status":"cancelled","userMessageId":"removed","startedAt":null},{"id":"auto","status":"queued","userMessageId":"notification"}],"messages":[{"id":"q1","text":"First","attachments":[{"id":"file"}]},{"id":"q2","text":"Later"},{"id":"notification","notification":{}}],"providerSessions":[{"id":"session","status":"ready","capabilities":{"turns":{"supportsQueuedMessages":true,"supportsActiveSteering":true}}}],"providerTurns":[{"status":"running","runAttemptId":"attempt"}]});
 let q=derive(&p);assert_eq!(q["items"].as_array().unwrap().len(),2);assert_eq!(q["items"][0]["runId"],"first");assert_eq!(q["items"][0]["attachments"][0]["id"],"file");assert_eq!(q["held"],true);assert_eq!(q["canSteer"],true);assert_eq!(q["canReorder"],true);assert!(!q["hiddenMessageIds"].as_array().unwrap().contains(&json!("sent")));p["runs"][1]["status"]=json!("running");p["runs"][1]["startedAt"]=json!(1);assert!(!derive(&p)["hiddenMessageIds"].as_array().unwrap().contains(&json!("q2")));p["providerTurns"]=json!([]);assert_eq!(derive(&p)["canSteer"],false);
 // Promoted to steer: the queued run is cancelled, its message moves to the active run.
 p["runs"][2]["status"]=json!("cancelled");p["messages"][0]["runId"]=json!("active");assert!(!derive(&p)["hiddenMessageIds"].as_array().unwrap().contains(&json!("q1")));
 p["messages"][0]["runId"]=json!("first");assert!(derive(&p)["hiddenMessageIds"].as_array().unwrap().contains(&json!("q1")));
 }
}
/// Native queue commands. The host supplies a validated thread and stable commandId.
pub fn command(input:&Value)->crate::Result<Value>{
 let action=text(input,"action");let run=text(input,"runId");
 let valid=|s:&str|!s.is_empty()&&s.len()<=2048&&!s.contains('/')&&!s.chars().any(char::is_control);
 if !valid(text(input,"thread"))|| (action!="resume"&&!valid(run)){return Err("invalid_queue_command".into())}
 let mut c=match action{
 "cancel"=>json!({"type":"queued-run.cancel","runId":run}),
 "reorder"=>{if !input["beforeRunId"].is_null()&&!valid(text(input,"beforeRunId")){return Err("invalid_queue_command".into())}json!({"type":"queued-run.reorder","runId":run,"beforeRunId":input["beforeRunId"]})},
 "steer"=>{if !valid(text(input,"targetRunId")){return Err("invalid_queue_command".into())}json!({"type":"queued-message.promote-to-steer","queuedRunId":run,"targetRunId":input["targetRunId"]})},
 "resume"=>json!({"type":"queue.resume"}),
 "edit"=>{let content=input["text"].as_str().ok_or("invalid_message")?;let files=input["attachments"].as_array().ok_or("invalid_attachments")?;if content.chars().count()>120000||files.len()>100||(content.trim().is_empty()&&files.is_empty()){return Err("invalid_message".into())}let mut c=json!({"type":"queued-run.edit","runId":run,"text":crate::composer::legacy_attachment_text(content,files),"attachments":files});if input["context"].is_object(){c["context"]=input["context"].clone()}c},
 _=>return Err("invalid_queue_command".into())};
 c["threadId"]=input["thread"].clone();Ok(c)
}
