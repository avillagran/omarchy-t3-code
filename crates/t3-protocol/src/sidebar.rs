//! Portable sidebar metadata and native thread lifecycle commands.
use super::*;
fn stamp(v:&Value)->i64 { v.as_str().and_then(|s|chrono::DateTime::parse_from_rfc3339(s).ok()).map(|d|d.timestamp_millis()).or_else(||v.as_i64()).unwrap_or(0) }
pub fn metadata(t:&Value,now:i64)->Value {
 let session=&t["session"];let runtime=if t["runtime"].is_object(){&t["runtime"]}else{session};
 let message=stamp(&t["latestUserMessageAt"]);
 let run=if t["latestRun"].is_object(){&t["latestRun"]}else{&t["latestTurn"]};
 let queued=matches!(runtime["status"].as_str(),Some("preparing"|"queued"|"starting")) || (message>0 && session["status"]!="error" && now.abs_diff(message)<=120_000 && ["requestedAt","startedAt","completedAt"].iter().all(|k|stamp(&run[*k])<message));
 let blocked=t["hasPendingApprovals"]==true || t["hasPendingUserInput"]==true || t["pendingSubagentRequest"].is_object() || t["pendingRuntimeRequest"].is_object();
 let snoozed_at=stamp(&t["snoozedAt"]);
 let raised=blocked || (matches!(runtime["status"].as_str(),Some("error"|"failed"))&&(snoozed_at==0||stamp(&runtime["updatedAt"])>snoozed_at)) || (snoozed_at>0&&(run["state"]=="completed"||run["status"]=="completed")&&stamp(&run["completedAt"])>snoozed_at);
 json!({"snoozed":stamp(&t["snoozedUntil"])>now&&!raised,"canSnooze":!blocked&&!queued,"snoozedUntil":t["snoozedUntil"],"hasRuntimeError":runtime["lastError"].as_str().is_some_and(|s|!s.is_empty()),"runtimeErrorClass":runtime["lastErrorClass"],"providerHistory":t["providerInstanceHistory"]})
}
pub fn command(action:&str,thread:&str,until:Option<&str>)->Result<Value> {
 let mut c=json!({"type":match action{"settle"=>"thread.settle","unsettle"=>"thread.unsettle","snooze"=>"thread.snooze","unsnooze"=>"thread.unsnooze",_=>return Err("invalid_command".into())},"threadId":thread});
 if matches!(action,"unsettle"|"unsnooze"){c["reason"]=json!("user")}
 if action=="snooze" {let date=until.and_then(|s|chrono::DateTime::parse_from_rfc3339(s).ok()).ok_or("invalid_snooze_time")?;if date.timestamp_millis()<=chrono::Utc::now().timestamp_millis(){return Err("invalid_snooze_time".into())}c["snoozedUntil"]=json!(date.to_rfc3339());}
 Ok(c)
}
#[cfg(test)]mod tests{use super::*;#[test]fn snooze_never_hides_blocked_or_queued_work(){let now=1_000_000;assert_eq!(metadata(&json!({}),now)["canSnooze"],true);for blocked in [json!({"hasPendingApprovals":true}),json!({"hasPendingUserInput":true}),json!({"pendingSubagentRequest":{}}),json!({"runtime":{"status":"queued"}}),json!({"latestUserMessageAt":now-100})]{assert_eq!(metadata(&blocked,now)["canSnooze"],false)}assert_eq!(metadata(&json!({"latestUserMessageAt":now-120_001}),now)["canSnooze"],true);assert_eq!(metadata(&json!({"latestUserMessageAt":now,"latestRun":{"requestedAt":now}}),now)["canSnooze"],true);assert_eq!(metadata(&json!({"latestUserMessageAt":now,"session":{"status":"error"}}),now)["canSnooze"],true);let snoozed=json!({"snoozedUntil":now+1000,"snoozedAt":now-100});assert_eq!(metadata(&snoozed,now)["snoozed"],true);let mut wake=snoozed.clone();wake["hasPendingApprovals"]=json!(true);assert_eq!(metadata(&wake,now)["snoozed"],false);wake=snoozed.clone();wake["latestRun"]=json!({"state":"completed","completedAt":now});assert_eq!(metadata(&wake,now)["snoozed"],false);wake=snoozed.clone();wake["runtime"]=json!({"status":"error","updatedAt":now-200});assert_eq!(metadata(&wake,now)["snoozed"],true);wake["runtime"]["updatedAt"]=json!(now);assert_eq!(metadata(&wake,now)["snoozed"],false);assert_eq!(command("unsnooze","t",None).unwrap()["reason"],"user");assert!(command("snooze","t",Some("2000-01-01T00:00:00Z")).is_err());assert!(command("snooze","t",Some("2099-01-01T00:00:00Z")).is_ok());}}

pub fn thread_action(input:&Value)->Result<Value>{
 let thread=super::field(input,"thread")?;if thread.is_empty()||thread.len()>2048||thread.chars().any(char::is_control){return Err("invalid_id".into())}
 let action=super::field(input,"action")?;let mut c=json!({"threadId":thread});
 c["type"]=json!(match action{
  "thread-rename"|"regenerate-title"=>"thread.metadata.update",
  "pin"=>"thread.pin","unpin"=>"thread.unpin","mark-unread"=>"thread.mark-unread",
  "auto-settle"=>"thread.auto-settle.set","archive"=>"thread.archive",
  "delete-thread"=>{if input["confirmed"]!=true{return Err("confirmation_required".into())}"thread.delete"},
  _=>return Err("invalid_command".into())
 });
 if action=="thread-rename"{let title=super::field(input,"title")?.trim();if title.is_empty()||title.chars().count()>255||title.chars().any(char::is_control){return Err("invalid_title".into())}c["title"]=json!(title)}
 if action=="regenerate-title"{c["regenerateTitle"]=json!(true)}
 if action=="auto-settle"{c["enabled"]=json!(input["enabled"].as_bool().ok_or("invalid_command")?)}
 if action=="archive" && input["working"]==true{return Err("thread_is_working".into())}
 Ok(c)
}
