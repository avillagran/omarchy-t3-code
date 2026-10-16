//! Context boundaries and native handoff identities, shared with Omi.
use serde_json::{json,Value};
fn text<'a>(v:&'a Value,k:&str)->&'a str{v[k].as_str().unwrap_or("")}
pub fn describe(row:&Value,runs:&[Value])->Option<Value>{
 let i=crate::activity::item(row);let kind=text(i,"type");
 if !matches!(kind,"compaction"|"handoff"){return None}
 let active=matches!(text(i,"status"),"running"|"pending"|"in_progress"|"queued");
 let mut out=json!({"kind":kind,"active":active,"failed":i["status"]=="failed"});
 if kind=="handoff"{
  let target=runs.iter().find(|r|r["id"]==i["runId"]);
  let before=target.and_then(|r|r["ordinal"].as_i64());
  let stamped=i["fromModelSelections"].as_array().filter(|a|!a.is_empty());
  let from=stamped.cloned().unwrap_or_else(||i["fromProviderInstanceIds"].as_array().into_iter().flatten().map(|id|{
   let model=runs.iter().filter(|r|r["providerInstanceId"]==*id&&before.is_none_or(|n|r["ordinal"].as_i64().unwrap_or(0)<n)).max_by_key(|r|r["ordinal"].as_i64().unwrap_or(0)).map(|r|r["modelSelection"]["model"].clone()).unwrap_or(Value::Null);
   json!({"instanceId":id,"model":model})
  }).collect());
  let model=i["toModel"].as_str().map(Value::from).or_else(||target.filter(|r|r["providerInstanceId"]==i["toProviderInstanceId"]).map(|r|r["modelSelection"]["model"].clone())).unwrap_or(Value::Null);
  out["from"]=json!(from);out["to"]=json!({"instanceId":i["toProviderInstanceId"],"model":model});
 }
 Some(out)
}
