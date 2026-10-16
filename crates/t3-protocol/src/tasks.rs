//! Active-run task presentation, matching T3's composer task badge.
use serde_json::{json,Value};
pub fn derive(p:&Value)->Option<Value>{
 let run=p["runs"].as_array()?.iter().rev().find(|r|matches!(r["status"].as_str(),Some("preparing"|"starting"|"running"|"waiting")))?;
 let plan=p["plans"].as_array()?.iter().rev().find(|a|a["kind"]=="todo_list"&&a["runId"]==run["id"])?;
 let items:Vec<Value>=plan["steps"].as_array()?.iter().map(|s|{let mut item=json!({"id":s["id"],"text":s["text"],"status":s["status"]});if let Some(d)=s["durationMs"].as_u64(){item["durationMs"]=json!(d)}item}).collect();
 if items.is_empty(){return None}
 let completed=items.iter().filter(|i|i["status"]=="completed").count();
 let current=items.iter().find(|i|i["status"]=="running").or_else(||items.iter().find(|i|i["status"]=="pending")).or_else(||items.last()).unwrap();
 Some(json!({"planId":plan["id"],"runId":run["id"],"current":current["text"],"completed":completed,"total":items.len(),"items":items}))
}
#[cfg(test)]mod tests{use super::*;
 #[test]fn badge_uses_only_current_runs_plan_and_preserves_repeated_steps(){
 let mut p=json!({"runs":[{"id":"current","status":"running"}],"plans":[{"id":"old","runId":"old","kind":"todo_list","steps":[{"id":"a","text":"Old task","status":"running"}]}]});assert!(derive(&p).is_none());
 p["plans"].as_array_mut().unwrap().push(json!({"id":"new","runId":"current","kind":"todo_list","steps":[{"id":"a","text":"Verify","status":"completed","durationMs":3000},{"id":"b","text":"Verify","status":"completed","durationMs":4000},{"id":"c","text":"Report","status":"running"},{"id":"d","text":"Finish","status":"pending"}]}));
 let t=derive(&p).unwrap();assert_eq!(t["current"],"Report");assert_eq!(t["completed"],2);assert_eq!(t["total"],4);assert_eq!(t["items"][1]["durationMs"],4000);assert_ne!(t["items"][0]["id"],t["items"][1]["id"]);
 p["runs"][0]["status"]=json!("completed");assert!(derive(&p).is_none());
 }
}
