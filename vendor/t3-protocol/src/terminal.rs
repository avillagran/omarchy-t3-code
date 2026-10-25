//! Native terminal protocol shared by desktop hosts and Omi. Hosts own rendering and TTYs.
//! `session`, `dimensions`, `write` and `resize` are built by `api::terminal` and keep
//! their own error codes (`invalid_id`, `invalid_terminal`, `invalid_terminal_size`,
//! `invalid_terminal_input`).
use crate::{api,Result,Connection};
use serde_json::{json,Value};
fn id(s:&str)->Result<&str>{if s.is_empty()||s.len()>2048||s.chars().any(char::is_control){Err("invalid_id".into())}else{Ok(s)}}
fn legacy(error:String)->String{match error.as_str(){"invalid_terminal_id"=>"invalid_terminal","invalid_cols"|"invalid_rows"=>"invalid_terminal_size","invalid_data"=>"invalid_terminal_input",_=>"invalid_id"}.into()}
pub fn session(thread:&str,terminal:&str)->Result<Value>{api::terminal::session_payload(thread,terminal).map_err(legacy)}
pub fn dimensions(cols:u16,rows:u16)->Result<Value>{let (cols,rows)=api::terminal::size(cols.into(),rows.into()).map_err(legacy)?;Ok(json!({"cols":cols,"rows":rows}))}
/// Resolve paths on the selected server. Never check a remote path on the host filesystem.
pub fn context(shell:&Value,thread:&str,fallback:&str)->Result<Value>{
 id(thread)?;let t=shell["threads"].as_array().and_then(|ts|ts.iter().find(|t|t["id"]==thread)).ok_or("thread_not_found")?;
 let project=shell["projects"].as_array().and_then(|ps|ps.iter().find(|p|p["id"]==t["projectId"]));
 let worktree=t["worktreePath"].as_str().filter(|s|!s.is_empty());
 let root=project.and_then(|p|p["workspaceRoot"].as_str()).filter(|s|!s.is_empty());
 let cwd=worktree.or(root).unwrap_or(fallback);
 if cwd.is_empty()||cwd.len()>4096||cwd.chars().any(char::is_control){return Err("terminal_directory_unavailable".into())}
 let mut context=json!({"cwd":cwd});if let Some(p)=worktree{context["worktreePath"]=json!(p)}
 if let Some(p)=t["modelSelection"]["instanceId"].as_str().filter(|s|!s.is_empty()){context["providerInstanceId"]=json!(id(p)?)}
 Ok(context)
}
pub fn attach(thread:&str,terminal:&str,context:&Value,cols:u16,rows:u16)->Result<Value>{
 let mut p=session(thread,terminal)?;let size=dimensions(cols,rows)?;
 let cwd=context["cwd"].as_str().filter(|s|!s.is_empty()&&s.len()<=4096&&!s.chars().any(char::is_control)).ok_or("terminal_directory_unavailable")?;
 p["cwd"]=json!(cwd);p["cols"]=size["cols"].clone();p["rows"]=size["rows"].clone();
 for key in ["worktreePath","providerInstanceId"]{if let Some(v)=context.get(key){p[key]=v.clone()}}
 // An exited shell stays exited until the user explicitly opens it again.
 p["restartIfNotRunning"]=json!(true);Ok(p)
}
pub fn write(thread:&str,terminal:&str,data:&str)->Result<Value>{api::terminal::write_payload(thread,terminal,data).map_err(legacy)}
pub fn resize(thread:&str,terminal:&str,cols:u16,rows:u16)->Result<Value>{api::terminal::resize_payload(thread,terminal,cols.into(),rows.into()).map_err(legacy)}
/// A stream snapshot already contains history; ignore replayed output at/before its cursor.
#[derive(Default)]pub struct Output{sequence:Option<u64>,pub finished:bool}
impl Output{
 pub fn apply(&mut self,event:&Value)->Result<Option<String>>{
  match event["type"].as_str().unwrap_or(""){
   "snapshot"|"started"|"restarted"=>{let s=&event["snapshot"];self.sequence=s["sequence"].as_u64().or(event["sequence"].as_u64());self.finished=matches!(s["status"].as_str(),Some("exited"|"error"));Ok(s["history"].as_str().map(str::to_string))},
   "output"=>{let seq=event["sequence"].as_u64();if seq.zip(self.sequence).is_some_and(|(a,b)|a<=b){return Ok(None)}self.sequence=seq.or(self.sequence);Ok(event["data"].as_str().map(str::to_string))},
   "exited"|"closed"=>{self.finished=true;Ok(None)},
   "error"=>{self.finished=true;Err("terminal_remote_error".into())},
   "cleared"=>{self.sequence=event["sequence"].as_u64().or(self.sequence);Ok(Some("\x1b[2J\x1b[H".into()))},
   _=>Ok(None)
  }
 }
}
impl Connection{
 /// `api::terminal::write` with this module's error codes.
 pub fn terminal_write(&self,thread:&str,terminal:&str,data:&str)->Result<Value>{api::send(self,&api::terminal::WRITE,write(thread,terminal,data)?)}
 /// `api::terminal::resize` with this module's error codes.
 pub fn terminal_resize(&self,thread:&str,terminal:&str,cols:u16,rows:u16)->Result<Value>{api::send(self,&api::terminal::RESIZE,resize(thread,terminal,cols,rows)?)}
}
#[cfg(test)]mod tests{use super::*;
 #[test]fn native_context_and_input_stay_on_selected_server(){let shell=json!({"projects":[{"id":"p","workspaceRoot":"/remote/root"}],"threads":[{"id":"t","projectId":"p","worktreePath":"/remote/tree with spaces","modelSelection":{"instanceId":"provider"}}]});let c=context(&shell,"t","/scratch").unwrap();let a=attach("t","term-1",&c,80,24).unwrap();assert_eq!(a["cwd"],"/remote/tree with spaces");assert_eq!(a["worktreePath"],a["cwd"]);assert_eq!(a["providerInstanceId"],"provider");assert_eq!(a["restartIfNotRunning"],true);assert_eq!(write("t","term-1","\u{3}").unwrap()["data"],"\u{3}");assert!(write("t","term-1","").is_err());assert!(resize("t","term-1",1001,25).is_err());assert!(context(&shell,"missing","/scratch").is_err());}
 #[test]fn snapshots_do_not_duplicate_live_output(){let mut s=Output::default();assert_eq!(s.apply(&json!({"type":"snapshot","snapshot":{"history":"hello","sequence":3,"status":"running"}})).unwrap(),Some("hello".into()));assert_eq!(s.apply(&json!({"type":"output","data":"duplicate","sequence":3})).unwrap(),None);assert_eq!(s.apply(&json!({"type":"output","data":"new","sequence":4})).unwrap(),Some("new".into()));s.apply(&json!({"type":"exited","exitCode":0})).unwrap();assert!(s.finished);}
}
