//! Portable sidebar metadata and native thread lifecycle commands.
use super::*;
pub mod view;
fn stamp(v:&Value)->i64 { v.as_str().and_then(|s|chrono::DateTime::parse_from_rfc3339(s).ok()).map(|d|d.timestamp_millis()).or_else(||v.as_i64()).unwrap_or(0) }
/// Native inbox/working/history anchors, matching client-runtime/state/threadInbox.ts
/// and threadSort.ts. Shells use flat run stamps; bounded projections use latestRun.
pub fn sort_timestamps(t:&Value)->Value {
 let run=&t["latestRun"];
 let requested=stamp(&t["latestRunRequestedAt"]).max(stamp(&run["requestedAt"]));
 let completed=stamp(&t["latestRunCompletedAt"]).max(stamp(&run["completedAt"]));
 let created=stamp(&t["createdAt"]);
 let anchor=created.max(stamp(&t["unsettledAt"]));
 let authored=if t.get("latestUserAuthoredMessageAt").is_some(){stamp(&t["latestUserAuthoredMessageAt"])}else{requested};
 let settled=stamp(&t["settledAt"]);
 let history=stamp(&t["latestUserMessageAt"]).max(requested).max(completed).max(stamp(&t["latestRunStartedAt"])).max(stamp(&run["startedAt"]));
 json!({"inboxAt":anchor.max(requested).max(completed),"activeAnchorAt":anchor,"lastSentAt":created.max(authored),"settledSortAt":if settled>0{settled}else if history>0{history}else{stamp(&t["updatedAt"])}})
}
/// T3 resolveSidebarThreadStatus over presentThreadShell's runtime (client-runtime models.ts):
/// approval, input, waiting, working, goal, limited, failed or ready.
pub fn background_work_holds(tasks: &Value) -> bool {
 tasks.as_array().is_some_and(|tasks|tasks.iter().any(|task|task["awaited"]==true||matches!(task["kind"].as_str(),Some("subagent"|"background_task"))))
}
pub fn sidebar_status(t:&Value)->&'static str {
 let req=&t["pendingRuntimeRequest"];let kind=req["kind"].as_str().unwrap_or("");
 let sub=t["pendingSubagentRequest"].as_str().or(t["pendingSubagentRequest"]["kind"].as_str()).unwrap_or("");
 // Work that wakes the agent parks the thread at idle; a command left running holds nothing.
 let park=(background_work_holds(&t["pendingBackgroundTasks"])||t["pendingBackgroundTasks"].as_array().is_some_and(|a|a.iter().any(|task|task.get("kind").is_none())))&&t["status"]!="failed";
 let legacy=if t["runtime"].is_object(){&t["runtime"]}else{&t["session"]};
 let status=if park{"idle"}else{legacy["status"].as_str().or(t["activityRunStatus"].as_str()).or(t["status"].as_str()).unwrap_or("")};
 let has_runtime=legacy.is_object()||!t["latestRunId"].is_null()&&t.get("latestRunId").is_some()||!t["activeProviderThreadId"].is_null()&&t.get("activeProviderThreadId").is_some()||park;
 let running=matches!(status,"preparing"|"starting"|"running");
 if (req.is_object()&&kind!="user_input"&&kind!="auth_refresh")||t["hasPendingApprovals"]==true||sub=="approval"{return "approval"}
 if (kind=="user_input"&&(req["blocking"]!=false||!running))||t["hasPendingUserInput"]==true||sub=="user_input"{return "input"}
 if !has_runtime{return "ready"}
 if status=="running"&&t["blockedOnThreadWait"]==true{return "waiting"}
 if matches!(status,"preparing"|"queued"|"starting"|"running"|"waiting"){return if t["goal"]["status"]=="active"{"goal"}else{"working"}}
 if status=="idle"{return "waiting"}
 if matches!(status,"failed"|"error"){let class=legacy["lastErrorClass"].as_str().or(t["lastErrorClass"].as_str());return if class==Some("usage_limit"){"limited"}else{"failed"}}
 "ready"
}
pub fn metadata(t:&Value,now:i64)->Value {
 let session=&t["session"];let runtime=if t["runtime"].is_object(){&t["runtime"]}else{session};
 // V2 shells carry no runtime/session: the run status and failure live on the thread.
 let status=runtime["status"].as_str().or(t["activityRunStatus"].as_str()).or(t["status"].as_str()).unwrap_or("");
 let message=stamp(&t["latestUserMessageAt"]);
 let run=if t["latestRun"].is_object(){&t["latestRun"]}else{&t["latestTurn"]};
 let queued=matches!(status,"preparing"|"queued"|"starting") || (message>0 && session["status"]!="error" && t["status"]!="failed" && now.abs_diff(message)<=120_000 && [("requestedAt","latestRunRequestedAt"),("startedAt","latestRunStartedAt"),("completedAt","latestRunCompletedAt")].iter().all(|(k,v2)|stamp(&run[*k]).max(stamp(&t[*v2]))<message));
 // An async question (blocking:false) never holds the thread.
 let request=t["pendingRuntimeRequest"].is_object()&&t["pendingRuntimeRequest"]["blocking"]!=false;
 let blocked=t["hasPendingApprovals"]==true || t["hasPendingUserInput"]==true || t["pendingSubagentRequest"].is_object() || request;
 let snoozed_at=stamp(&t["snoozedAt"]);
 // T3 threadRaisedHandWhileSnoozed: only a request or a fresh failure wakes a snoozed thread.
 // failureEdgeAt prefers the failed run's completion; updatedAt also moves on metadata writes.
 let runtime_updated=if runtime.is_object(){stamp(&runtime["updatedAt"])}else{stamp(&t["updatedAt"])};
 let run_failed=run["status"]=="failed"||t["status"]=="failed";
 let failed_at=if !matches!(status,"error"|"failed"){None}else{let done=stamp(&run["completedAt"]).max(stamp(&t["latestRunCompletedAt"]));Some(if run_failed&&done>0{done}else{runtime_updated})};
 let fresh_failure=failed_at.is_some_and(|at|snoozed_at==0||at>snoozed_at);
 let raised=blocked || fresh_failure;
 // threadWokeAt: an early wake keeps its own time; otherwise the timer's.
 let until=stamp(&t["snoozedUntil"]);
 let woke_at=if until==0{0}else if raised{if fresh_failure{failed_at.unwrap_or(0)}else if runtime_updated>0{runtime_updated}else{snoozed_at}}else if until<=now{until}else{0};
 let visited=stamp(&t["lastVisitedAt"]);
 let woke=woke_at>0&&(visited==0||visited<woke_at)&&t["settledOverride"]!="settled";
 let last_error=runtime["lastError"].as_str().or(t["lastError"].as_str()).is_some_and(|s|!s.trim().is_empty());
 json!({"sidebarStatus":sidebar_status(t),"wokeAt":if woke{json!(woke_at)}else{Value::Null},"woke":woke,"snoozed":until>now&&!raised,"canSnooze":!blocked&&!queued,"snoozedUntil":t["snoozedUntil"],"hasRuntimeError":last_error,"runtimeErrorClass":if runtime["lastErrorClass"].is_null(){t["lastErrorClass"].clone()}else{runtime["lastErrorClass"].clone()},"providerHistory":t["providerInstanceHistory"],"pullRequests":t["pullRequests"],"linkedPullRequest":t["linkedPullRequest"],"branchPullRequest":t["branchPullRequest"],"pullRequestBadge":super::pull_requests::sidebar_badge(t)})
}
pub fn command(action:&str,thread:&str,until:Option<&str>)->Result<Value> {
 let mut c=json!({"type":match action{"settle"=>"thread.settle","unsettle"=>"thread.unsettle","snooze"=>"thread.snooze","unsnooze"=>"thread.unsnooze",_=>return Err("invalid_command".into())},"threadId":thread});
 if matches!(action,"unsettle"|"unsnooze"){c["reason"]=json!("user")}
 if action=="snooze" {let date=until.and_then(|s|chrono::DateTime::parse_from_rfc3339(s).ok()).ok_or("invalid_snooze_time")?;if date.timestamp_millis()<=chrono::Utc::now().timestamp_millis(){return Err("invalid_snooze_time".into())}c["snoozedUntil"]=json!(date.to_rfc3339());}
 Ok(c)
}
#[cfg(test)]mod tests{use super::*;#[test]fn snooze_never_hides_blocked_or_queued_work(){let now=1_000_000;assert_eq!(metadata(&json!({}),now)["canSnooze"],true);for blocked in [json!({"hasPendingApprovals":true}),json!({"hasPendingUserInput":true}),json!({"pendingSubagentRequest":{}}),json!({"runtime":{"status":"queued"}}),json!({"latestUserMessageAt":now-100})]{assert_eq!(metadata(&blocked,now)["canSnooze"],false)}assert_eq!(metadata(&json!({"latestUserMessageAt":now-120_001}),now)["canSnooze"],true);assert_eq!(metadata(&json!({"latestUserMessageAt":now,"latestRun":{"requestedAt":now}}),now)["canSnooze"],true);assert_eq!(metadata(&json!({"latestUserMessageAt":now,"session":{"status":"error"}}),now)["canSnooze"],true);let snoozed=json!({"snoozedUntil":now+1000,"snoozedAt":now-100});assert_eq!(metadata(&snoozed,now)["snoozed"],true);let mut wake=snoozed.clone();wake["hasPendingApprovals"]=json!(true);assert_eq!(metadata(&wake,now)["snoozed"],false);wake=snoozed.clone();wake["latestRun"]=json!({"state":"completed","completedAt":now});assert_eq!(metadata(&wake,now)["snoozed"],true,"completed runs never end a snooze early");wake=snoozed.clone();wake["runtime"]=json!({"status":"error","updatedAt":now-200});assert_eq!(metadata(&wake,now)["snoozed"],true);wake["runtime"]["updatedAt"]=json!(now);assert_eq!(metadata(&wake,now)["snoozed"],false);assert_eq!(command("unsnooze","t",None).unwrap()["reason"],"user");assert!(command("snooze","t",Some("2000-01-01T00:00:00Z")).is_err());assert!(command("snooze","t",Some("2099-01-01T00:00:00Z")).is_ok());}}

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

#[cfg(test)]mod status_tests{
 use super::*;
 #[test]fn sidebar_status_follows_t3_order(){
  let base=json!({"latestRunId":"r","status":"completed","activeProviderThreadId":null});
  assert_eq!(sidebar_status(&base),"ready");
  assert_eq!(sidebar_status(&json!({"status":"idle","latestRunId":null,"activeProviderThreadId":null})),"ready","never run");
  let mut t=base.clone();t["status"]=json!("running");assert_eq!(sidebar_status(&t),"working");
  t["goal"]=json!({"status":"active"});assert_eq!(sidebar_status(&t),"goal");
  t["blockedOnThreadWait"]=json!(true);assert_eq!(sidebar_status(&t),"waiting");
  t["pendingRuntimeRequest"]=json!({"kind":"user_input","blocking":false});assert_eq!(sidebar_status(&t),"waiting","async question keeps the run going");
  t["pendingRuntimeRequest"]=json!({"kind":"user_input"});assert_eq!(sidebar_status(&t),"input");
  t["pendingRuntimeRequest"]=json!({"kind":"command_approval"});assert_eq!(sidebar_status(&t),"approval");
  let mut f=base.clone();f["status"]=json!("failed");assert_eq!(sidebar_status(&f),"failed");
  f["lastErrorClass"]=json!("usage_limit");assert_eq!(sidebar_status(&f),"limited");
  let mut bg=base.clone();bg["pendingBackgroundTasks"]=json!([{"kind":"command","taskId":"dev"}]);assert_eq!(sidebar_status(&bg),"ready","a dev server holds nothing");
  bg["pendingBackgroundTasks"]=json!([{"kind":"subagent","taskId":"a"}]);assert_eq!(sidebar_status(&bg),"waiting");
 }
 #[test]fn woke_follows_the_timer_or_a_fresh_failure_until_visited(){
  let now=10_000_000;let iso=|ms:i64|chrono::DateTime::from_timestamp_millis(ms).unwrap().to_rfc3339();
  let mut t=json!({"latestRunId":"r","status":"completed","snoozedAt":iso(now-5000),"snoozedUntil":iso(now-1000)});
  let m=metadata(&t,now);assert_eq!(m["woke"],true);assert_eq!(m["wokeAt"],now-1000);assert_eq!(m["snoozed"],false);
  t["lastVisitedAt"]=json!(iso(now-1000));assert_eq!(metadata(&t,now)["woke"],false,"a visit at wokeAt clears it");
  let mut early=json!({"latestRunId":"r","status":"failed","snoozedAt":iso(now-5000),"snoozedUntil":iso(now+60_000),"latestRunCompletedAt":iso(now-2000)});
  let m=metadata(&early,now);assert_eq!(m["snoozed"],false);assert_eq!(m["woke"],true);assert_eq!(m["wokeAt"],now-2000);
  early["latestRunCompletedAt"]=json!(iso(now-9000));early["updatedAt"]=json!(iso(now));assert_eq!(metadata(&early,now)["snoozed"],true,"an old failure keeps the snooze");
  let mut settled=t.clone();settled["lastVisitedAt"]=Value::Null;settled["settledOverride"]=json!("settled");assert_eq!(metadata(&settled,now)["woke"],false);
 }
}
#[cfg(test)]mod ordering_tests{
 use super::*;
 #[test]fn sidebar_returns_use_completion_not_metadata_updates(){
  let old=json!({"createdAt":100,"updatedAt":9999,"latestRunRequestedAt":200,"latestRunCompletedAt":300});
  let finished=json!({"createdAt":10,"updatedAt":20,"latestRunRequestedAt":50,"latestRunCompletedAt":1000});
  assert!(sort_timestamps(&finished)["inboxAt"].as_i64()>sort_timestamps(&old)["inboxAt"].as_i64());
  assert_eq!(sort_timestamps(&finished)["activeAnchorAt"],10);
  let nested=json!({"createdAt":10,"latestRun":{"requestedAt":50,"completedAt":1000}});
  assert_eq!(sort_timestamps(&nested)["inboxAt"],1000);
  let awakened=json!({"createdAt":10,"unsettledAt":1200,"latestRunCompletedAt":1000});
  assert_eq!(sort_timestamps(&awakened)["inboxAt"],1200);
  assert_eq!(sort_timestamps(&json!({"createdAt":10,"latestUserAuthoredMessageAt":100,"latestRunRequestedAt":2000,"latestRunCompletedAt":3000}))["lastSentAt"],100);
  assert_eq!(sort_timestamps(&json!({"settledAt":400,"latestRunCompletedAt":5000}))["settledSortAt"],400);
  assert_eq!(sort_timestamps(&json!({"createdAt":"invalid","latestRunCompletedAt":null}))["inboxAt"],0);
 }
}
