//! Native project metadata mutations shared with Omi. No host filesystem actions.
use crate::*;
pub fn command(input:&Value)->Result<Value>{
 let id=field(input,"projectId")?;if id.is_empty()||id.len()>2048||id.chars().any(char::is_control){return Err("invalid_project".into())}
 let mut c=json!({"projectId":id});
 match field(input,"action")?{
  "project-rename"=>{let title=field(input,"title")?.trim();if title.is_empty()||title.chars().count()>255||title.chars().any(char::is_control){return Err("invalid_title".into())}c["type"]=json!("project.update");c["title"]=json!(title)},
  "project-settings"=>{
   c["type"]=json!("project.update");let patch=&input["patch"];if !patch.is_object(){return Err("invalid_project".into())}
   for (k,v) in patch.as_object().unwrap(){match k.as_str(){
    "title"=>{let title=v.as_str().map(str::trim).filter(|s|!s.is_empty()&&s.chars().count()<=255&&!s.chars().any(char::is_control)).ok_or("invalid_title")?;c[k]=json!(title)},
    "defaultModelSelection" if v.is_null()||v.is_object()=>{c[k]=v.clone()},
    "autoPull" if v.is_boolean()=>{c[k]=v.clone()},
    "defaultThreadEnvMode" if v.is_null()||matches!(v.as_str(),Some("local"|"worktree"))=>{c[k]=v.clone()},
    _=>return Err("invalid_project_settings".into())
   }}
  },
  "project-remove"=>{if input["confirmed"]!=true{return Err("confirmation_required".into())}c["type"]=json!("project.delete");c["force"]=json!(input["force"]==true)},
  _=>return Err("invalid_command".into())
 }
 Ok(c)
}
