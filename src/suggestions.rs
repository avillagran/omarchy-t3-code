//! Server-owned workspace lookup; shared portable presentation lives in t3-protocol.
use crate::*;
use std::{sync::{Mutex,OnceLock},time::Instant};
static CACHE:OnceLock<Mutex<HashMap<String,(Instant,Value)>>>=OnceLock::new();
pub fn query(e:&Environment,input:&Value)->Result<Value>{
 let kind=text(input,"kind");let query=text(input,"query");let thread=text(input,"thread");let instance=text(input,"instanceId");
 if !valid_id(thread)||query.chars().count()>256||!["path","skill","command"].contains(&kind){return Err("invalid_suggestion_query".into())}
 let key=format!("{}/{}/{}",e.id,thread,instance);
 let cache=CACHE.get_or_init(||Mutex::new(HashMap::new()));
 let saved=cache.lock().unwrap().get(&key).filter(|(t,_)|t.elapsed()<Duration::from_secs(300)).map(|(_,v)|v.clone());
 let context=if let Some(v)=saved{v}else{
  let shell=Client::shared()?.get(e,"/api/orchestration/shell",true)?;
  let t=shell["threads"].as_array().and_then(|ts|ts.iter().find(|t|t["id"]==thread)).ok_or("thread_not_found")?;
  let project=shell["projects"].as_array().and_then(|ps|ps.iter().find(|p|p["id"]==t["projectId"]));
  let cwd=t["worktreePath"].as_str().filter(|s|!s.is_empty()).or_else(||project.and_then(|p|p["workspaceRoot"].as_str())).unwrap_or("");
  let value=json!({"cwd":cwd});
  let mut c=cache.lock().unwrap();if c.len()>=16{c.clear()}c.insert(key.clone(),(Instant::now(),value.clone()));value
 };
 let cwd=text(&context,"cwd");
 if kind=="path"{
  if cwd.is_empty(){return Err("workspace_unavailable".into())}
  let result=write::rpc(e,"projects.searchEntries",json!({"cwd":cwd,"query":query.trim(),"limit":40}))?;
  return Ok(json!({"items":protocol::composer::paths(&result),"truncated":result["truncated"]}));
 }
 let config=if context["config"].is_object(){context["config"].clone()}else{
  let result=if !cwd.is_empty()&&!instance.is_empty(){write::rpc(e,"server.refreshProviders",json!({"instanceId":instance,"cwd":cwd}))?}else{protocol::api::server::config(&e.connection(true)?)?};
  // Retain only provider metadata needed for composer suggestions; never auth/config secrets.
  let providers:Vec<Value>=result["providers"].as_array().into_iter().flatten().filter(|p|p["instanceId"]==instance).map(|p|{let scoped=p["workspaceSnapshots"].as_array().and_then(|a|a.iter().find(|s|s["cwd"]==cwd)).unwrap_or(p);json!({"instanceId":p["instanceId"],"enabled":p["enabled"],"skills":scoped["skills"],"slashCommands":scoped["slashCommands"]})}).collect();
  let value=json!({"providers":providers});let mut c=cache.lock().unwrap();let mut next=context.clone();next["config"]=value.clone();c.insert(key,(Instant::now(),next));value
 };
 let provider=config["providers"].as_array().and_then(|ps|ps.iter().find(|p|p["instanceId"]==instance&&p["enabled"]==true)).ok_or("provider_unavailable")?;
 Ok(json!({"items":protocol::composer::catalog(provider,cwd,kind,query)}))
}
