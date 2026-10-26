//! Bounded thread projection reducer, shared with Omi. Unknown events advance the cursor safely.
use serde_json::{json,Value};
use crate::util::text;
fn upsert(p:&mut Value,key:&str,v:&Value,cap:usize){
 if !p[key].is_array(){p[key]=json!([])}let list=p[key].as_array_mut().unwrap();
 if let Some(i)=list.iter().position(|a|a["id"]==v["id"]){list[i]=v.clone()}else{list.push(v.clone());if list.len()>cap{list.drain(..list.len()-cap);}}
}
pub fn try_apply(p: &mut Value, e: &Value) -> crate::Result<bool> {
 let mut event=e.clone();
 if event["type"]=="turn-item.updated" {crate::wire::restore_item(&mut event["payload"],p)?;}
 if event["type"]=="subagent.updated" {let prior=p["subagents"].as_array().and_then(|l|l.iter().rev().find(|r|r["id"]==event["payload"]["id"])).cloned();crate::wire::restore_subagent(&mut event["payload"],prior.as_ref())?;}
 Ok(apply(p,&event))
}
pub fn apply(p:&mut Value,e:&Value)->bool{
 let typ=text(e,"type");let mut restored=e["payload"].clone();
 if typ=="turn-item.updated" && crate::wire::restore_item(&mut restored,p).is_err(){return false}
 if typ=="subagent.updated" {let previous=p["subagents"].as_array().and_then(|l|l.iter().rev().find(|r|r["id"]==restored["id"]));if crate::wire::restore_subagent(&mut restored,previous).is_err(){return false}}
 let v=&restored;if !v.is_object(){return false}
 match typ{
  // Payloads that are not whole rows: apply them to the row they name.
  "thread.pull-request-link-updated"=>{let same=|l:&Value|l["host"]==v["host"]&&l["repository"]==v["repository"]&&l["number"]==v["number"];match p["thread"]["pullRequests"].as_array_mut(){Some(links)=>{for l in links.iter_mut().filter(|l|same(l)){*l=v.clone()}}None=>return false}return true}
  "provider-session.detached"=>{let id=&v["providerSessionId"];if let Some(list)=p["providerSessions"].as_array_mut(){list.retain(|s|&s["id"]!=id)}return true}
  "run.background-work-cancelled"=>{let id=&v["runId"];if let Some(list)=p["runs"].as_array_mut(){for r in list.iter_mut().filter(|r|&r["id"]==id){r["restartCancelledBackgroundWork"]=v["restartCancelledBackgroundWork"].clone()}}return true}
  "checkpoint.rollback-requested"=>return true,
  _=>{}
 }
 if typ.starts_with("thread.") && v["id"]==p["thread"]["id"]{p["thread"]=v.clone();return true}
 let (key,cap)=match typ{
  "message.updated"=>("messages",160),"turn-item.updated"=>("turnItems",600),"plan.updated"=>("plans",128),"run.created"|"run.updated"=>("runs",64),"run-attempt.created"|"run-attempt.updated"=>("attempts",64),"node.updated"=>("nodes",128),"subagent.updated"=>("subagents",128),"provider-thread.updated"=>("providerThreads",64),"provider-turn.updated"=>("providerTurns",128),"provider-session.attached"|"provider-session.updated"=>("providerSessions",64),"runtime-request.updated"=>("runtimeRequests",128),"checkpoint-scope.created"=>("checkpointScopes",64),"checkpoint.captured"=>("checkpoints",64),"context-handoff.updated"=>("contextHandoffs",64),"context-transfer.created"|"context-transfer.updated"=>("contextTransfers",64),_=>return false};
 if text(v,"id").is_empty(){return false}
 let mut compact=v.clone();
 if key=="turnItems"{// Outputs are dropped to keep the mirror small, but an html_render result is only a reference the timeline must keep.
 let html=crate::documents::html_render(v);let app=if html.is_none(){crate::documents::mcp_app(v)}else{None};if let Some(o)=compact.as_object_mut(){for k in ["output","diffStr","oldStr","newStr"]{o.remove(k);}if let Some(a)=html{let name=a["name"].as_str().unwrap_or("");o.insert("output".into(),json!({"htmlRender":{"attachmentId":a["id"],"title":name.strip_suffix(".html").unwrap_or(name)}}));}if let Some(a)=&app{let m=&a["mcpApp"];o.insert("output".into(),json!({"t3McpApp":{"attachmentId":m["attachmentId"],"server":m["server"],"tool":m["tool"],"resourceUri":m["resourceUri"]}}));}if o.get("type").and_then(Value::as_str)==Some("reasoning"){o.remove("text");}if let Some(input)=o.get_mut("input"){if let Some(raw)=input.as_str().filter(|s|s.len()>8192){*input=json!(raw.chars().take(2048).collect::<String>());}else if let Some(fields)=input.as_object_mut(){fields.retain(|k,_|matches!(k.as_str(),"command"|"cmd"|"title"));}}}}
 // A turn update without token usage keeps the usage it already had.
 if key=="providerTurns"&&compact["tokenUsage"].is_null(){if let Some(prev)=p[key].as_array().and_then(|l|l.iter().find(|t|t["id"]==v["id"])){if !prev["tokenUsage"].is_null(){compact["tokenUsage"]=prev["tokenUsage"].clone()}}}
 upsert(p,key,&compact,cap);
 if key=="runs"{
  if matches!(text(v,"status"),"preparing"|"starting"|"running"|"waiting"){p["thread"]["activeRunId"]=v["id"].clone();}else if p["thread"]["activeRunId"]==v["id"]{p["thread"]["activeRunId"]=Value::Null;}
 }true
}
#[cfg(test)]mod tests{use super::*;#[test]fn live_html_render_keeps_its_reference(){let mut p=json!({"thread":{"id":"one"}});assert!(apply(&mut p,&json!({"type":"turn-item.updated","payload":{"id":"t1","type":"dynamic_tool","toolName":"mcp__t3-code__html_render","input":{"html":"<p>x</p>"},"output":{"content":"{}","structuredContent":{"htmlRender":{"attachmentId":"a-html","title":"Ideas","height":800},"message":"long"}}}})));let item=&p["turnItems"][0];assert_eq!(crate::documents::html_render(item).unwrap()["id"],"a-html");assert_eq!(crate::documents::html_render(item).unwrap()["name"],"Ideas.html");assert!(item["output"]["structuredContent"].is_null());}#[test]fn messages_replace_and_mirror_stays_bounded(){let mut p=json!({"thread":{"id":"one"},"messages":[]});for i in 0..200{assert!(apply(&mut p,&json!({"type":"message.updated","payload":{"id":format!("m{i}"),"text":"x"}})))}assert_eq!(p["messages"].as_array().unwrap().len(),160);apply(&mut p,&json!({"type":"message.updated","payload":{"id":"m199","text":"updated"}}));assert_eq!(p["messages"][159]["text"],"updated");assert!(!apply(&mut p,&json!({"type":"future.event","payload":{}})));apply(&mut p,&json!({"type":"run.updated","payload":{"id":"r","status":"running"}}));assert_eq!(p["thread"]["activeRunId"],"r");apply(&mut p,&json!({"type":"run.updated","payload":{"id":"r","status":"completed"}}));assert!(p["thread"]["activeRunId"].is_null());}#[test]fn partial_payload_events_apply_to_their_rows(){let mut p=json!({"thread":{"id":"one","pullRequests":[{"host":"github.com","repository":"a/b","number":3,"url":"u"}]},"providerSessions":[{"id":"s1"},{"id":"s2"}],"runs":[{"id":"r"}],"providerTurns":[{"id":"t","tokenUsage":{"usedTokens":5}}]});
assert!(apply(&mut p,&json!({"type":"thread.pull-request-link-updated","payload":{"host":"github.com","repository":"a/b","number":3,"url":"u2"}})));assert_eq!(p["thread"]["pullRequests"][0]["url"],"u2");assert_eq!(p["thread"]["id"],"one");
assert!(apply(&mut p,&json!({"type":"provider-session.detached","payload":{"providerSessionId":"s1","detachedAt":"x"}})));assert_eq!(p["providerSessions"],json!([{"id":"s2"}]));
assert!(apply(&mut p,&json!({"type":"run.background-work-cancelled","payload":{"runId":"r","restartCancelledBackgroundWork":[{"k":1}]}})));assert_eq!(p["runs"][0]["restartCancelledBackgroundWork"][0]["k"],1);
assert!(apply(&mut p,&json!({"type":"context-transfer.created","payload":{"id":"c"}})));assert_eq!(p["contextTransfers"][0]["id"],"c");
assert!(apply(&mut p,&json!({"type":"provider-turn.updated","payload":{"id":"t","status":"done"}})));assert_eq!(p["providerTurns"][0]["tokenUsage"]["usedTokens"],5);}}
