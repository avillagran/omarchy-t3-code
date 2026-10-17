//! T3's account-guarded cross-environment PR read routing. Credentials stay put.
use crate::*;
use serde_json::{json,Value};
pub fn permission(catalog:&Value,primary:Option<&Value>,environment:&str)->String{
 let Some(key)=connection_preferences::github_connection_key(catalog,environment,primary)else{return "off".into()};
 catalog["githubRoutingPermissions"].as_array().into_iter().flatten().find(|r|r["connectionKey"]==key&&r["environmentId"]==environment).and_then(|r|r["permission"].as_str()).filter(|s|matches!(*s,"read"|"read-write")).unwrap_or("off").into()
}
pub fn identities_match(origin:&Value,destination:&Value)->bool{
 origin["provider"]=="github"&&destination["provider"]=="github"&&origin["accountId"].as_str().is_some_and(|s|!s.trim().is_empty())&&!text(origin,"host").is_empty()&&text(origin,"host").eq_ignore_ascii_case(text(destination,"host"))&&origin["accountId"]==destination["accountId"]
}
fn allowed(source:&str,target:&str,primary:Option<&Value>)->bool{
 let Ok(c)=read_catalog()else{return false};permission(&c,primary,source)!="off"&&permission(&c,primary,target)!="off"
}
pub fn read(origin:&Environment,tag:&str,payload:Value)->Result<Value>{
 if !matches!(tag,"pullRequests.detail"|"pullRequests.checks"|"pullRequests.activity"){return write::rpc(origin,tag,payload)}
 let primary=connections::native_primary();let native_id=|e:&Environment|if e.id=="local"{primary.as_ref().and_then(|p|p["environmentId"].as_str()).unwrap_or("local").to_string()}else{e.id.clone()};
 let source_id=native_id(origin);let mut alternatives=environments()?.into_iter().filter(|e|e.id!=origin.id&&e.connection_error.is_none()&&allowed(&source_id,&native_id(e),primary.as_ref())).collect::<Vec<_>>();
 if alternatives.is_empty(){return write::rpc(origin,tag,payload)}
 alternatives.sort_by_key(|e|e.id!="local");
 let identity=match write::rpc(origin,"pullRequests.routing",payload.clone()){
  Ok(v)if v["provider"]=="github"=>v,_=>return write::rpc(origin,tag,payload)
 };
 let mut guarded=payload;guarded["host"]=identity["host"].clone();guarded["expectedAccountId"]=identity["accountId"].clone();
 for destination in alternatives{
  if !allowed(&source_id,&native_id(&destination),primary.as_ref()){continue}
  let Ok(other)=write::rpc(&destination,"pullRequests.routingIdentity",json!({"host":identity["host"]}))else{continue};
  if !identities_match(&identity,&other)||!allowed(&source_id,&native_id(&destination),primary.as_ref()){continue}
  let fresh=environments()?.into_iter().find(|e|e.id==destination.id&&e.connection_error.is_none());let Some(fresh)=fresh else{continue};
  if let Ok(mut result)=write::rpc(&fresh,tag,guarded.clone()){
   if result.is_object(){result["projectId"]=guarded["projectId"].clone();if tag=="pullRequests.detail"{result["projectTitle"]=identity["projectTitle"].clone();result["workspaceRoot"]=identity["workspaceRoot"].clone();}}
   return Ok(result)
  }
 }
 write::rpc(origin,tag,guarded)
}
