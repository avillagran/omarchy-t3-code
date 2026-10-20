//! This device's OmaBridge, managed exactly as the Omarchy Control Panel does:
//! each request goes unchanged to `omabridge manage`, with OmaBridge's own state
//! directory and publishing unit, so every front end shows and changes the same
//! device. OmaBridge validates, asks for `confirmed` and keeps the secrets; the
//! request travels on stdin, never on the command line.
use crate::*;
use std::{io::Write,os::unix::fs::PermissionsExt,process::{Command,Stdio}};
/// Same order as the Control Panel's helper, so both run the same binary: an
/// explicit override, the one on PATH, `~/.local/bin`, then the bundled copy.
fn candidates()->Vec<PathBuf>{
 let mut out=vec![];
 if let Some(raw)=std::env::var_os("OT3_OMABRIDGE_BIN"){let p=PathBuf::from(raw);if p.is_absolute(){out.push(p)}return out}
 if let Some(path)=std::env::var_os("PATH"){for dir in std::env::split_paths(&path){out.push(dir.join("omabridge"))}}
 out.push(home().join(".local/bin/omabridge"));
 if let Some(dir)=std::env::current_exe().ok().and_then(|e|e.parent().map(Path::to_path_buf)){out.push(dir.join("omabridge"))}
 out
}
fn executable(p:&Path)->bool{fs::metadata(p).is_ok_and(|m|m.is_file()&&m.permissions().mode()&0o111!=0)}
/// One request; the answer is OmaBridge's own JSON (the snapshot, or
/// `{"error":code}`) and its exit status. Adds `omabridge_missing` (no binary)
/// and `omabridge_outdated` (no binary knows `manage` yet), as the panel does.
pub fn manage(request:&Value,seconds:u32)->(Value,i32){
 let Ok(body)=serde_json::to_vec(request) else {return (json!({"error":"invalid_command"}),1)};
 if body.len()>64*1024{return (json!({"error":"invalid_command"}),1)}
 let mut seen=std::collections::HashSet::new();let mut any=false;
 for candidate in candidates(){
  if !executable(&candidate){continue}
  let Ok(real)=fs::canonicalize(&candidate) else {continue};
  if !seen.insert(real.clone()){continue}
  any=true;
  let Ok(mut child)=Command::new("timeout").arg("--kill-after=5").arg(format!("{seconds}s")).arg(&real).arg("manage").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn() else {continue};
  if let Some(mut stdin)=child.stdin.take(){let _=stdin.write_all(&body);}
  let Ok(out)=child.wait_with_output() else {continue};
  let status=out.status.code().unwrap_or(1);
  // A release without `manage` rejects the subcommand before doing anything.
  if status==2&&out.stdout.iter().all(u8::is_ascii_whitespace){continue}
  return match serde_json::from_slice::<Value>(&out.stdout){
   Ok(doc) if doc.is_object()=>(doc,status),
   _=>(json!({"error":if status==124||status==137{"timeout"}else{"invalid_response"}}),if status==0{1}else{status}),
  };
 }
 (json!({"error":if any{"omabridge_outdated"}else{"omabridge_missing"}}),1)
}
/// The `omabridge-manage` operation: `{"request":{…}}` in, `{"output","status"}` out.
pub fn execute(input:&Value)->Value{
 let request=&input["request"];
 if !request.is_object()||!request["action"].is_string(){return json!({"output":{"error":"invalid_command"},"status":1})}
 let (output,status)=manage(request,120);
 // Pairing and approval change which peers can carry connections.
 if status==0&&matches!(request["action"].as_str(),Some("approve"|"revoke"|"pair"|"pair-retry")){refresh_connections();}
 json!({"output":output,"status":status})
}
