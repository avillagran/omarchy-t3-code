//! OmaBridge device management from ot3, so `omabridge-gui` is not needed: this
//! device's identity, one-use invitations, pairing from an invitation file,
//! pending approvals, paired peers and revocation. Everything goes through the
//! bundled CLI and its JSON output, against OmaBridge's private state directory.
//! Keys, tokens and invitation contents are never returned, stored or logged:
//! only peer ids, names, services and expiry times leave this module. A created
//! invitation is a file path plus an optional QR image, both treated as secrets.
use crate::*;
use crate::omabridge_route::{binary,valid_peer,valid_service};
use std::{io::Write,process::{Command,Stdio}};
fn state_dir()->PathBuf{crate::omabridge_route::config().map(|c|c.state_dir()).unwrap_or_else(|_|crate::omabridge_route::default_state_dir())}
/// Invitations being paired from, copied here (0700/0600) until approved.
fn awaiting_dir()->PathBuf{xdg("XDG_STATE_HOME",".local/state").join("omabridge-pairing")}
/// Run one CLI command and parse its first stdout line. stderr is only matched
/// against known messages and never surfaced, since it may echo private input.
fn run(args:&[&std::ffi::OsStr],seconds:u32)->Result<(Value,String)>{
 let bin=binary().ok_or("omabridge_unavailable")?;
 let out=Command::new("timeout").arg(format!("{seconds}s")).arg(bin).arg("--state-dir").arg(state_dir()).args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).output().map_err(|_|"omabridge_unavailable")?;
 let stdout=String::from_utf8_lossy(&out.stdout).into_owned();
 if !out.status.success(){return Err(error_code(&String::from_utf8_lossy(&out.stderr)).into())}
 let (first,rest)=stdout.split_once('\n').unwrap_or((&stdout,""));
 let doc=serde_json::from_str(first.trim()).map_err(|_|"omabridge_unavailable")?;
 Ok((doc,rest.to_string()))
}
fn os(values:&[&str])->Vec<std::ffi::OsString>{values.iter().map(Into::into).collect()}
fn call(args:&[&str],seconds:u32)->Result<(Value,String)>{let owned=os(args);run(&owned.iter().map(|s|s.as_os_str()).collect::<Vec<_>>(),seconds)}
pub fn error_code(stderr:&str)->&'static str{
 if stderr.contains("server not running"){"omabridge_not_published"}
 else if stderr.contains("grant only currently published services"){"omabridge_service_not_published"}
 else if stderr.contains("peer not found")||stderr.contains("peer not paired"){"omabridge_peer_not_found"}
 else if stderr.contains("another server"){"omabridge_already_running"}
 else if stderr.contains("expired"){"omabridge_invite_expired"}
 else {"omabridge_command_failed"}
}
/// Device name for a new identity: the host name when it is a plain label.
fn device_name()->String{
 let raw=fs::read_to_string("/etc/hostname").unwrap_or_default();let name=raw.trim();
 if !name.is_empty()&&name.len()<=63&&name.bytes().all(|b|b.is_ascii_alphanumeric()||b"-_.".contains(&b)){name.into()}else{"ot3".into()}
}
pub fn identity_ready()->bool{crate::omabridge_route::identity_ready(&state_dir())}
/// `init` is idempotent once an identity exists; it is only run to create one
/// on an explicit request, or to read the public id and name of an existing one.
pub fn identity()->Option<Value>{
 if !identity_ready(){return None}
 let (doc,_)=call(&["init"],5).ok()?;
 valid_peer(text(&doc,"id")).then(||json!({"id":doc["id"],"name":text(&doc,"name")}))
}
fn ensure_identity()->Result<Value>{
 if let Some(id)=identity(){return Ok(id)}
 let (doc,_)=call(&["init","--name",&device_name()],10)?;
 if !valid_peer(text(&doc,"id")){return Err("omabridge_command_failed".into())}
 Ok(json!({"id":doc["id"],"name":text(&doc,"name")}))
}
fn services(v:&Value)->Vec<Value>{v.as_array().into_iter().flatten().filter(|s|s.as_str().is_some_and(valid_service)).cloned().collect()}
/// Pending claims on this device's invitations, without addresses or invitation ids.
pub fn pending()->Vec<Value>{
 if !identity_ready(){return vec![]}
 call(&["pending"],5).map(|(doc,_)|doc["pending"].as_array().into_iter().flatten().filter(|p|valid_peer(text(p,"id"))).map(|p|json!({"id":p["id"],"name":text(p,"name"),"services":services(&p["services"]),"expires":p["expires"].as_i64()})).collect()).unwrap_or_default()
}
/// Public fields of an invitation file: never its token or address.
fn invitation_summary(path:&Path)->Result<Value>{
 let meta=fs::symlink_metadata(path).map_err(|_|"omabridge_invite_unreadable")?;
 if !meta.is_file()||meta.len()>64*1024{return Err("omabridge_invite_unreadable".into())}
 let doc:Value=serde_json::from_slice(&fs::read(path).map_err(|_|"omabridge_invite_unreadable")?).map_err(|_|"omabridge_invite_invalid")?;
 if doc["version"]!=1||!doc["token"].is_string()||!doc["address"].is_object(){return Err("omabridge_invite_invalid".into())}
 let expires=doc["expires"].as_i64().ok_or("omabridge_invite_invalid")?;
 let name:String=text(&doc,"name").chars().filter(|c|!c.is_control()).take(64).collect();
 Ok(json!({"name":name,"expires":expires}))
}
/// Pairings sent from this device that the other side has not approved yet.
pub fn awaiting()->Vec<Value>{
 let now=Utc::now().timestamp();
 let mut out:Vec<Value>=fs::read_dir(awaiting_dir()).into_iter().flatten().flatten().filter_map(|entry|{
  let path=entry.path();let id=path.file_stem()?.to_str()?.to_string();
  if uuid::Uuid::parse_str(&id).is_err()||path.extension()?.to_str()?!="json"{return None}
  match invitation_summary(&path){
   Ok(mut s) if s["expires"].as_i64().is_some_and(|e|e>now)=>{s["id"]=json!(id);Some(s)},
   // Expired or damaged copies are secrets with no further use.
   _=>{let _=fs::remove_file(&path);None}
  }
 }).collect();
 out.sort_by_key(|s|s["expires"].as_i64());out
}
fn private_dir(path:&Path)->Result<()>{
 fs::create_dir_all(path).map_err(|_|"local_error")?;
 let meta=fs::symlink_metadata(path).map_err(|_|"local_error")?;
 if !meta.is_dir()||meta.uid()!=unsafe{libc::geteuid()}{return Err("local_error".into())}
 fs::set_permissions(path,std::os::unix::fs::PermissionsExt::from_mode(0o700)).map_err(|_|"local_error".into())
}
/// Pair with an invitation file copy; the copy is kept only while approval is pending.
fn pair_copy(id:&str)->Result<Value>{
 let path=awaiting_dir().join(format!("{id}.json"));
 let summary=invitation_summary(&path)?;
 if summary["expires"].as_i64().is_some_and(|e|e<=Utc::now().timestamp()){let _=fs::remove_file(&path);return Err("omabridge_invite_expired".into())}
 ensure_identity()?;
 let owned=[std::ffi::OsString::from("pair"),"--invite-file".into(),path.clone().into_os_string()];
 match run(&owned.iter().map(|s|s.as_os_str()).collect::<Vec<_>>(),30){
  Ok((doc,_)) if doc["status"]=="approved"=>{let _=fs::remove_file(&path);Ok(json!({"status":"approved","peer":doc["peer"],"name":summary["name"]}))},
  Ok((doc,_)) if doc["status"]=="pending"=>Ok(json!({"status":"pending","id":id,"name":summary["name"],"expires":summary["expires"]})),
  Ok(_)=>{let _=fs::remove_file(&path);Err("omabridge_command_failed".into())},
  // Kept for "Check again" unless it expired; a network failure is often temporary.
  Err(e)=>{if e=="omabridge_invite_expired"{let _=fs::remove_file(&path);}Err(e)},
 }
}
fn pair(source:&str)->Result<Value>{
 let source=Path::new(source.strip_prefix("file://").unwrap_or(source));
 if !source.is_absolute(){return Err("omabridge_invite_unreadable".into())}
 invitation_summary(source)?;
 let dir=awaiting_dir();private_dir(&dir)?;
 let id=uuid::Uuid::new_v4().to_string();let copy=dir.join(format!("{id}.json"));
 let body=fs::read(source).map_err(|_|"omabridge_invite_unreadable")?;
 let mut f=OpenOptions::new().write(true).create_new(true).mode(0o600).custom_flags(libc::O_NOFOLLOW).open(&copy).map_err(|_|"local_error")?;
 if f.write_all(&body).and_then(|_|f.sync_all()).is_err(){let _=fs::remove_file(&copy);return Err("local_error".into())}
 pair_copy(&id)
}
fn invite_directory(input:&Value)->Result<PathBuf>{
 let requested=text(input,"directory");
 let dir=if requested.is_empty(){[home().join("Downloads"),home()].into_iter().find(|p|p.is_dir()).ok_or("local_error")?}else{PathBuf::from(requested.strip_prefix("file://").unwrap_or(requested))};
 if !dir.is_absolute()||!fs::metadata(&dir).is_ok_and(|m|m.is_dir()){return Err("omabridge_invite_directory".into())}
 Ok(dir)
}
/// Convert the CLI's half-block terminal QR into an SVG data URI, dark on light.
pub fn qr_svg(text:&str)->Option<String>{
 let lines:Vec<Vec<char>>=text.lines().map(|l|l.trim_end().chars().collect::<Vec<_>>()).filter(|l|!l.is_empty()).collect();
 let width=lines.iter().map(Vec::len).max()?;
 if lines.len()<10||width<20||lines.iter().flatten().any(|c|!matches!(c,' '|'█'|'▀'|'▄')){return None}
 let mut rects=String::new();
 for (y,line) in lines.iter().enumerate(){for (x,c) in line.iter().enumerate(){
  let (top,bottom)=match c{'█'=>(true,true),'▀'=>(true,false),'▄'=>(false,true),_=>(false,false)};
  if top{rects.push_str(&format!("<rect x='{x}' y='{}' width='1' height='1'/>",y*2));}
  if bottom{rects.push_str(&format!("<rect x='{x}' y='{}' width='1' height='1'/>",y*2+1));}
 }}
 let height=lines.len()*2;
 let svg=format!("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 {width} {height}' shape-rendering='crispEdges'><rect width='{width}' height='{height}' fill='#fff'/><g fill='#000'>{rects}</g></svg>");
 use base64::Engine;Some(format!("data:image/svg+xml;base64,{}",base64::engine::general_purpose::STANDARD.encode(svg)))
}
fn invite(input:&Value)->Result<Value>{
 let wanted:Vec<&str>=input["services"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
 if wanted.is_empty()||wanted.iter().any(|s|!valid_service(s)){return Err("invalid_command".into())}
 identity().ok_or("omabridge_identity_missing")?;
 let ttl=input["ttl"].as_u64().unwrap_or(600);if !(60..=3600).contains(&ttl){return Err("invalid_command".into())}
 let path=invite_directory(input)?.join(format!("omabridge-invite-{}.json",Utc::now().format("%Y%m%d-%H%M%S")));
 let mut args:Vec<std::ffi::OsString>=vec!["invite".into(),"--ttl".into(),ttl.to_string().into(),"--output".into(),path.clone().into_os_string()];
 for s in &wanted{args.push("--service".into());args.push(s.into());}
 if input["qr"]==true{args.push("--qr".into());}
 let (doc,rest)=run(&args.iter().map(|s|s.as_os_str()).collect::<Vec<_>>(),15)?;
 // OmaBridge writes the file itself (0600, never over an existing path); confirm that.
 if !fs::symlink_metadata(&path).is_ok_and(|m|m.is_file()&&m.mode()&0o077==0){let _=fs::remove_file(&path);return Err("omabridge_command_failed".into())}
 let qr=if input["qr"]==true{qr_svg(&rest)}else{None};
 Ok(json!({"path":path.display().to_string(),"expires":doc["expires"].as_i64(),"services":services(&doc["services"]),"qrImage":qr}))
}
/// Delete an invitation file this user created: only an `omabridge-invite-*.json`
/// owned by us, private (0600) and still a valid invitation. Works across processes,
/// so other front ends (the control panel) can discard what they created.
fn invite_discard(input:&Value)->Result<()>{
 let path=PathBuf::from(text(input,"path"));
 let named=path.file_name().and_then(|n|n.to_str()).is_some_and(|n|n.starts_with("omabridge-invite-")&&n.ends_with(".json"));
 if !path.is_absolute()||!named{return Err("invalid_command".into())}
 let meta=match fs::symlink_metadata(&path){Ok(m)=>m,Err(e) if e.kind()==std::io::ErrorKind::NotFound=>return Ok(()),Err(_)=>return Err("local_error".into())};
 if !meta.is_file()||meta.uid()!=unsafe{libc::geteuid()}||meta.mode()&0o077!=0||invitation_summary(&path).is_err(){return Err("invalid_command".into())}
 fs::remove_file(&path).map_err(|_|"local_error".into())
}
fn approve(input:&Value)->Result<()>{
 let peer=text(input,"peer");if !valid_peer(peer){return Err("invalid_command".into())}
 let (doc,_)=call(&["pending"],5)?;
 let claims:Vec<&Value>=doc["pending"].as_array().into_iter().flatten().filter(|p|p["id"]==peer).collect();
 // A peer that claimed several invitations is approved for the one the user saw.
 let claim=match claims.as_slice(){[]=>return Err("omabridge_peer_not_found".into()),[one]=>*one,many=>*many.iter().find(|p|p["expires"]==input["expires"]).ok_or("catalog_changed_reload")?};
 if claims.len()==1{call(&["approve",peer],10)?;}else{
  let invitation=text(claim,"invitation_id");if invitation.is_empty()||invitation.len()>128||!invitation.bytes().all(|b|b.is_ascii_alphanumeric()||b"-_".contains(&b)){return Err("omabridge_command_failed".into())}
  call(&["approve","--invitation-id",invitation,peer],10)?;
 }
 Ok(())
}
/// Publishing runs `omabridge serve` as a user unit owned by ot3, off by default.
pub const UNIT:&str="omarchy-t3-omabridge.service";
fn unit_path()->PathBuf{std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).filter(|p|p.is_absolute()).unwrap_or_else(||home().join(".config")).join("systemd/user").join(UNIT)}
fn t3_port()->u16{read_json(&t3_home().join("userdata/server-runtime.json")).ok().and_then(|r|r["port"].as_u64()).and_then(|p|u16::try_from(p).ok()).filter(|p|*p>0).unwrap_or(3773)}
fn ssh_listening()->bool{std::net::TcpStream::connect_timeout(&"127.0.0.1:22".parse().unwrap(),Duration::from_millis(300)).is_ok()}
fn systemctl(args:&[&str])->bool{Command::new("timeout").arg("20s").arg("systemctl").arg("--user").args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s|s.success())}
/// Services named in the unit ot3 wrote, as `name=127.0.0.1:port`.
fn unit_services()->Option<Vec<(String,u16)>>{
 let unit=fs::read_to_string(unit_path()).ok()?;
 let line=unit.lines().find(|l|l.starts_with("ExecStart="))?;
 Some(line.split_whitespace().filter_map(|w|w.split_once("=127.0.0.1:")).filter(|(n,_)|valid_service(n)).filter_map(|(n,p)|Some((n.to_string(),p.parse().ok()?))).collect())
}
pub fn published()->Value{
 let services=unit_services();
 let active=services.is_some()&&systemctl(&["is-active","--quiet",UNIT]);
 let port=t3_port();
 json!({
  "enabled":services.is_some(),"active":active,
  "services":services.as_ref().map(|s|s.iter().map(|(n,_)|n.clone()).collect::<Vec<_>>()).unwrap_or_default(),
  // T3 moved to another port since publishing; publishing again follows it.
  "stale":services.as_ref().is_some_and(|s|s.iter().any(|(n,p)|n=="t3"&&*p!=port)),
  "sshAvailable":ssh_listening(),
 })
}
pub fn unit_text(bin:&Path,state:&Path,ssh:bool)->Result<String>{
 // Quoted for systemd; refuse anything that would need further escaping.
 let quote=|p:&Path|->Result<String>{let s=p.to_str().ok_or("local_error")?;if !p.is_absolute()||s.chars().any(|c|c.is_control()||matches!(c,'"'|'\\'|'%'|'$')){return Err("omabridge_path_unsupported".into())}Ok(format!("\"{s}\""))};
 let mut exec=format!("ExecStart={} --state-dir {} serve --service t3=127.0.0.1:{}",quote(bin)?,quote(state)?,t3_port());
 if ssh{exec.push_str(" --service ssh=127.0.0.1:22");}
 Ok(format!("# Managed by Omarchy T3 Code: publishes this device to paired OmaBridge peers.\n# Turn it off in Preferences, Connections, OmaBridge.\n[Unit]\nDescription=OmaBridge for Omarchy T3 Code\nAfter=network-online.target\nWants=network-online.target\n\n[Service]\n{exec}\nRestart=on-failure\nRestartSec=5\n\n[Install]\nWantedBy=default.target\n"))
}
fn publish(input:&Value)->Result<()>{
 let ssh=input["ssh"]==true;
 if ssh&&!ssh_listening(){return Err("omabridge_ssh_unavailable".into())}
 let bin=binary().ok_or("omabridge_unavailable")?;
 ensure_identity()?;
 let body=unit_text(&bin,&state_dir(),ssh)?;
 let path=unit_path();let dir=path.parent().ok_or("local_error")?;fs::create_dir_all(dir).map_err(|_|"local_error")?;
 if let Ok(existing)=fs::read_to_string(&path){if !existing.starts_with("# Managed by Omarchy T3 Code"){return Err("omabridge_unit_foreign".into())}}
 let tmp=dir.join(format!(".{UNIT}.{}",uuid::Uuid::new_v4()));
 let written=(||{let mut f=OpenOptions::new().write(true).create_new(true).mode(0o644).custom_flags(libc::O_NOFOLLOW).open(&tmp)?;f.write_all(body.as_bytes())?;f.sync_all()?;fs::rename(&tmp,&path)})();
 if written.is_err(){let _=fs::remove_file(&tmp);return Err("local_error".into())}
 if !systemctl(&["daemon-reload"])||!systemctl(&["enable",UNIT])||!systemctl(&["restart",UNIT]){return Err("omabridge_publish_failed".into())}
 Ok(())
}
fn unpublish()->Result<()>{
 let path=unit_path();
 match fs::read_to_string(&path){
  Ok(existing) if !existing.starts_with("# Managed by Omarchy T3 Code")=>return Err("omabridge_unit_foreign".into()),
  Err(_)=>return Ok(()),
  _=>{}
 }
 let _=systemctl(&["disable","--now",UNIT]);
 fs::remove_file(&path).map_err(|_|"local_error")?;
 let _=systemctl(&["daemon-reload"]);
 Ok(())
}
/// Actions on OmaBridge itself; each changes device trust, so each needs confirmation.
pub fn execute(action:&str,input:&Value)->Result<Option<Value>>{
 if !matches!(action,"identity-create"|"invite"|"invite-discard"|"pair"|"pair-retry"|"pair-discard"|"approve"|"revoke"|"publish"|"unpublish"){return Ok(None)}
 if input["confirmed"]!=true{return Err("confirmation_required".into())}
 if binary().is_none()&&action!="unpublish"{return Err("omabridge_unavailable".into())}
 let outcome=match action{
  "identity-create"=>{ensure_identity()?;json!({})},
  "invite"=>json!({"created":invite(input)?}),
  "invite-discard"=>{invite_discard(input)?;json!({})},
  "pair"=>json!({"paired":pair(text(input,"path"))?}),
  "pair-retry"=>{let id=text(input,"id");uuid::Uuid::parse_str(id).map_err(|_|"invalid_command")?;json!({"paired":pair_copy(id)?})},
  "pair-discard"=>{let id=text(input,"id");uuid::Uuid::parse_str(id).map_err(|_|"invalid_command")?;let _=fs::remove_file(awaiting_dir().join(format!("{id}.json")));json!({})},
  "approve"=>{approve(input)?;json!({})},
  "publish"=>{publish(input)?;json!({})},
  "unpublish"=>{unpublish()?;json!({})},
  _=>{let peer=text(input,"peer");if !valid_peer(peer){return Err("invalid_command".into())}call(&["revoke",peer],10)?;json!({})},
 };
 if matches!(action,"approve"|"revoke"|"pair"|"pair-retry"){refresh_connections();}
 let mut snapshot=crate::omabridge_route::snapshot()?;
 if let (Some(s),Some(o))=(snapshot.as_object_mut(),outcome.as_object()){for (k,v) in o{s.insert(k.clone(),v.clone());}}
 Ok(Some(snapshot))
}
