//! Checkpoint file summaries for the app run, not nested provider scopes.
use serde_json::{json,Value};
fn text<'a>(v:&'a Value,k:&str)->&'a str{v[k].as_str().unwrap_or("")}
pub fn summaries(p:&Value)->Vec<Value>{
 let mut out=vec![];
 for c in p["checkpoints"].as_array().into_iter().flatten(){
  let Some(ordinal)=c["appRunOrdinal"].as_u64().filter(|n|*n>0) else{continue};
  if c["status"]!="ready"||text(c,"runId").is_empty(){continue}
  let files:Vec<Value>=c["files"].as_array().into_iter().flatten().filter(|f|!text(f,"path").is_empty()).map(|f|json!({"path":f["path"],"kind":f["kind"],"additions":f["additions"].as_u64(),"deletions":f["deletions"].as_u64()})).collect();
  if files.is_empty(){continue}
  let additions:u64=files.iter().map(|f|f["additions"].as_u64().unwrap_or(0)).sum();let deletions:u64=files.iter().map(|f|f["deletions"].as_u64().unwrap_or(0)).sum();
  let summary=json!({"id":c["id"],"runId":c["runId"],"capturedAt":c["capturedAt"],"fromTurnCount":ordinal-1,"toTurnCount":ordinal,"files":files,"additions":additions,"deletions":deletions});
  if let Some(i)=out.iter().position(|r:&Value|r["runId"]==c["runId"]){out[i]=summary}else{out.push(summary)}
 }out
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn summaries_exclude_child_and_unready_and_keep_counts(){let p=json!({"checkpoints":[{"id":"root","status":"ready","runId":"run","appRunOrdinal":4,"files":[{"path":"a.rs","kind":"modified","additions":2,"deletions":1}]},{"id":"child","status":"ready","runId":"run","appRunOrdinal":null,"files":[{"path":"child.rs"}]},{"id":"bad","status":"error","runId":"bad","appRunOrdinal":5,"files":[{"path":"bad.rs"}]}]});let rows=summaries(&p);assert_eq!(rows.len(),1);assert_eq!(rows[0]["fromTurnCount"],3);assert_eq!(rows[0]["additions"],2);assert_eq!(rows[0]["files"][0]["path"],"a.rs");}
}
