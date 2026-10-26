//! Read-only compatibility with T3's Linux Electron connection catalog.
use super::*;
use aes::cipher::{block_padding::Pkcs7,BlockDecryptMut,KeyIvInit};
use base64::Engine;
use std::process::Command;
use zeroize::Zeroizing;
#[path="connections_management.rs"]
mod management;
pub use management::execute as manage_connections;
pub(crate) use management::native_primary;
pub fn t3_home() -> PathBuf {
    let Some(raw)=std::env::var_os("T3CODE_HOME") else {return home().join(".t3")};
    let value=raw.to_string_lossy();
    if value.trim().is_empty(){return home().join(".t3")}
    if let Some(rest)=value.strip_prefix("~/"){return home().join(rest)}
    let path=PathBuf::from(raw);
    if path.is_absolute(){path}else{std::env::current_dir().unwrap_or_else(|_|home()).join(path)}
}
pub fn decrypt_catalog(bytes:&[u8], password:&[u8]) -> Result<Value> {
    let plain=decrypt_storage(bytes,password)?;
    serde_json::from_slice(&plain).map_err(|_|"invalid_t3_catalog".into())
}
pub(super) fn decrypt_storage(bytes:&[u8], password:&[u8]) -> Result<Zeroizing<Vec<u8>>> {
    if bytes.len()<19 || !matches!(&bytes[..3], b"v10"|b"v11") {return Err("unsupported_catalog_encryption".into())}
    let mut key=Zeroizing::new([0u8;16]);
    pbkdf2::pbkdf2_hmac::<sha1::Sha1>(password,b"saltysalt",1,key.as_mut());
    let plain=Zeroizing::new(cbc::Decryptor::<aes::Aes128>::new_from_slices(key.as_ref(), &[b' ';16]).map_err(|_|"catalog_locked")?.decrypt_padded_vec_mut::<Pkcs7>(&bytes[3..]).map_err(|_|"catalog_locked")?);
    Ok(plain)
}
pub fn read_catalog() -> Result<Value> {
    let path=t3_home().join("userdata/connection-catalog.json");
    if !path.exists() {return Ok(json!({"profiles":[]}))}
    let doc=read_json(&path)?;
    if doc["version"]!=1{return Err("unsupported_t3_catalog".into())}
    let bytes=base64::engine::general_purpose::STANDARD.decode(text(&doc,"encryptedCatalog")).map_err(|_|"invalid_t3_catalog")?;
    if bytes.starts_with(b"v10") {return validate_catalog(decrypt_catalog(&bytes,b"peanuts")?)}
    if !bytes.starts_with(b"v11") {return Err("unsupported_catalog_encryption".into())}
    // T3's Electron application attribute; never query other applications' secrets.
    let output=Command::new("timeout").args(["4s","secret-tool","lookup","application","t3code"]).output().map_err(|_|"catalog_locked")?;
    if !output.status.success() {return Err("catalog_locked".into())}
    let mut password=Zeroizing::new(output.stdout);
    while password.last()==Some(&b'\n') || password.last()==Some(&b'\r') {password.pop();}
    validate_catalog(decrypt_catalog(&bytes,&password)?)
}
fn validate_catalog(c:Value)->Result<Value>{
    if c["schemaVersion"]!=1 || !["targets","profiles","credentials"].iter().all(|k|c[*k].is_array()){return Err("unsupported_t3_catalog".into())}
    Ok(c)
}

use std::{net::{TcpListener,TcpStream},process::{Child,Stdio},sync::{Mutex,OnceLock},time::{Duration,Instant},io::Write,os::unix::process::CommandExt};
/// A local route: the SSH/OmaBridge child plus, for SSH carried over OmaBridge, its forward.
struct Tunnel(Child,Option<Child>);
impl Tunnel{fn alive(&mut self)->bool{self.0.try_wait().ok()==Some(None)&&self.1.as_mut().is_none_or(|c|c.try_wait().ok()==Some(None))}}
struct ChildGuard(Option<Child>);
impl Drop for ChildGuard {fn drop(&mut self){if let Some(c)=self.0.as_mut(){let _=c.kill();let _=c.wait();}}}
impl Drop for Tunnel {fn drop(&mut self){let _=self.0.kill();let _=self.0.wait();if let Some(c)=self.1.as_mut(){let _=c.kill();let _=c.wait();}}}
/// `retry`: failed environments, with the next attempt, the failures in a row
/// and the route that failed.
struct Cache { signature:String, remote_signature:String, cloud_signature:String, until:Instant, environments:Vec<Environment>, routes:HashMap<String,String>, tunnels:HashMap<String,Tunnel>, catalog:Result<Value>, retry:HashMap<String,(Instant,u32,String)> }
static CACHE:OnceLock<Mutex<Option<Cache>>>=OnceLock::new();
// One rebuild at a time. A rebuild can take a while (an unreachable SSH host
// waits for its timeout), so other callers meanwhile get the last list
// (`LAST`) instead of waiting: a broken remote never stalls the local server.
static REBUILD:Mutex<()>=Mutex::new(());
static LAST:Mutex<Option<Vec<Environment>>>=Mutex::new(None);
// Invalidation never waits for a rebuild either: a flag, and the environments
// whose requests failed, read when the next rebuild starts.
static INVALIDATED:std::sync::atomic::AtomicBool=std::sync::atomic::AtomicBool::new(false);
static FAILED:Mutex<Vec<String>>=Mutex::new(Vec::new());
// Environments to drop and connect again from scratch (Reconnect).
static RECONNECT:Mutex<Vec<String>>=Mutex::new(Vec::new());
pub(crate) fn request_reconnect(id:&str){if let Ok(mut r)=RECONNECT.lock(){if !r.iter().any(|x|x==id){r.push(id.to_string());}}}
/// Wait before retrying an environment that failed `count` times in a row: 5 s, doubling, at most 2 min.
fn retry_delay(count:u32)->Duration{Duration::from_secs((5u64<<count.saturating_sub(1).min(5)).min(120))}
fn token_expiry_ms(token:&str)->Option<i64> {
    // T3 SessionStore uses payload.signature (not JWT) and millisecond exp.
    let parts=token.split('.').collect::<Vec<_>>();
    let payload=match parts.len(){2=>parts[0],3=>parts[1],_=>return None};
    let bytes=base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(payload).ok()?;
    let claims:Value=serde_json::from_slice(&bytes).ok()?;
    let expiry=epoch(&claims["exp"]);(expiry>0).then_some(expiry)
}
fn has_scope(token:&str,scope:&str)->bool {
    let parts=token.split('.').collect::<Vec<_>>();let payload=match parts.len(){2=>parts[0],3=>parts[1],_=>return false};
    base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(payload).ok().and_then(|b|serde_json::from_slice::<Value>(&b).ok()).is_some_and(|v|v["scopes"].as_array().is_some_and(|a|a.contains(&json!(scope))))
}
fn has_filesystem_read(token:&str)->bool {has_scope(token,"filesystem:read")}
fn fresh_token(token:&str)->bool {
    token_expiry_ms(token).is_some_and(|t|t>Utc::now().timestamp_millis()+3600*1000)
}
fn local_environment()->Result<Environment> {
    let runtime=read_json(&t3_home().join("userdata/server-runtime.json")).map_err(|_|"t3_not_running")?;
    let origin=text(&runtime,"origin");validate_base(origin)?;
    let url=crate::Url::parse(origin).map_err(|_|"invalid_t3_runtime")?;
    if !matches!(url.host_str(),Some("127.0.0.1"|"localhost"|"[::1]"|"::1")) {return Err("invalid_t3_runtime".into())}
    if runtime["pid"].as_i64().is_none_or(|pid|pid<=0 || unsafe{libc::kill(pid as i32,0)}!=0) {return Err("t3_not_running".into())}
    let token=xdg("XDG_DATA_HOME",".local/share").join("local.write.token");
    if private_token(&token).is_err() || (!fresh_token(&private_token(&token)?) || !has_filesystem_read(&private_token(&token)?) || !has_scope(&private_token(&token)?,"settings:write") || !has_scope(&private_token(&token)?,"terminal:read") || !has_scope(&private_token(&token)?,"terminal:operate")) {
        let cli=home().join(".t3/bin/t3");
        let mut cmd=Command::new("timeout");cmd.arg("12s").arg(if cli.exists(){cli.into_os_string()}else{"t3".into()});
        let out=cmd.args(["auth","session","issue","--base-dir"]).arg(t3_home()).args(["--scope","orchestration:read","--scope","orchestration:operate","--scope","filesystem:read","--scope","settings:write","--scope","terminal:read","--scope","terminal:operate","--ttl","30d","--label","omarchy-t3-code","--token-only"]).output().map_err(|_|"t3_auth_unavailable")?;
        if !out.status.success(){return Err("t3_auth_unavailable".into())}
        let value=Zeroizing::new(out.stdout);let value=std::str::from_utf8(&value).map_err(|_|"t3_auth_unavailable")?.trim();
        if value.len()<16 || value.contains(char::is_whitespace){return Err("t3_auth_unavailable".into())}
        let parent=token.parent().ok_or("local_error")?;fs::create_dir_all(parent).map_err(|_|"local_error")?;
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(parent,fs::Permissions::from_mode(0o700)).map_err(|_|"local_error")?;
        let temp=parent.join(format!(".token-{}",uuid::Uuid::new_v4()));
        let mut file=OpenOptions::new().write(true).create_new(true).mode(0o600).open(&temp).map_err(|_|"local_error")?;
        file.write_all(value.as_bytes()).map_err(|_|"local_error")?;file.sync_all().map_err(|_|"local_error")?;
        fs::rename(temp,&token).map_err(|_|"local_error")?;
    }
    Ok(Environment{id:"local".into(),name:"T3 Code".into(),base_url:origin.into(),token_file:token.clone(),write_token_file:Some(token),bearer:None,connection_error:None,connection:None})
}
fn remote_base(target:&Value)->Environment {
    Environment{id:text(target,"environmentId").into(),name:text(target,"label").into(),base_url:String::new(),token_file:PathBuf::new(),write_token_file:None,bearer:None,connection_error:None,connection:None}
}
fn ssh_args(target:&Value)->Result<Vec<String>> {
    let destination=if text(target,"alias").trim().is_empty(){text(target,"hostname")}else{text(target,"alias")}.trim();
    if destination.is_empty() || destination.len()>255 || !destination.bytes().all(|b|b.is_ascii_alphanumeric()||b".@_-:[]".contains(&b)) || destination.starts_with('-'){return Err("invalid_t3_ssh_profile".into())}
    let mut args=vec!["-o".into(),"BatchMode=yes".into(),"-o".into(),"ConnectTimeout=5".into()];
    // A private OpenSSH master keeps an explicitly authenticated session reusable.
    use std::os::unix::fs::{DirBuilderExt,MetadataExt};
    let runtime=std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(||PathBuf::from(format!("/run/user/{}",unsafe{libc::geteuid()})));
    let control=runtime.join("ot3-ssh");
    match fs::DirBuilder::new().mode(0o700).create(&control){Ok(())=>{},Err(error) if error.kind()==std::io::ErrorKind::AlreadyExists=>{},Err(_)=>return Err("ssh_runtime_unavailable".into())}
    let metadata=fs::symlink_metadata(&control).map_err(|_|"ssh_runtime_unavailable")?;
    if !metadata.is_dir()||metadata.uid()!=unsafe{libc::geteuid()}||metadata.mode()&0o077!=0{return Err("ssh_runtime_unavailable".into())}
    let path=control.join("%C");if path.as_os_str().len()+38>100{return Err("ssh_runtime_unavailable".into())}
    args.extend(["-o".into(),"ControlMaster=auto".into(),"-o".into(),"ControlPersist=600".into(),"-o".into(),format!("ControlPath={}",path.display())]);
    if let Some(port)=target["port"].as_u64(){if port==0||port>65535{return Err("invalid_t3_ssh_profile".into())}args.extend(["-p".into(),port.to_string()]);}
    if let Some(user)=target["username"].as_str(){if user.is_empty()||!user.bytes().all(|b|b.is_ascii_alphanumeric()||b"._-".contains(&b)){return Err("invalid_t3_ssh_profile".into())}args.extend(["-l".into(),user.into()]);}
    args.extend(["--".into(),destination.into()]);Ok(args)
}
/// SSH arguments (ending in `-- <destination>`) for an inherited SSH environment.
pub fn ssh_args_for(environment:&str)->Result<Vec<String>>{
    let catalog=read_catalog()?;
    let target=catalog["targets"].as_array().into_iter().flatten().find(|t|t["environmentId"]==environment&&t["_tag"]=="SshConnectionTarget").ok_or("ssh_environment_required")?;
    let profile=catalog["profiles"].as_array().into_iter().flatten().find(|p|p["connectionId"]==target["connectionId"]).ok_or("invalid_t3_ssh_profile")?;
    ssh_args(&profile["target"])
}
const REMOTE_DISCOVERY:&str=include_str!("remote-bootstrap.sh");
const REMOTE_RESTART:&str=include_str!("remote-restart.sh");
/// Kill an SSH environment's T3 server on its host and start it again
/// (remote-restart.sh), then reconnect. Only after the user confirmed it: any
/// run on that server stops.
pub fn restart_remote(environment:&str)->Result<String>{
    uuid::Uuid::parse_str(environment).map_err(|_|"invalid_t3_environment_id")?;
    let args=ssh_args_for(environment)?;
    let mut child=Command::new("timeout").args(["120s","ssh"]).args(&args).arg(format!("sh -s -- '{environment}'"))
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().map_err(|_|"ssh_unavailable")?;
    child.stdin.take().ok_or("ssh_unavailable")?.write_all(REMOTE_RESTART.as_bytes()).map_err(|_|"ssh_unavailable")?;
    let output=child.wait_with_output().map_err(|_|"ssh_unavailable")?;
    request_reconnect(environment);
    if !output.status.success(){return Err(match output.status.code(){Some(7)=>"t3_environment_mismatch",Some(8)=>"service_profile_mismatch",Some(9)=>"t3_remote_start_failed",Some(124)=>"ssh_timeout",Some(255)=>"ssh_unavailable",Some(127)=>"t3_cli_unavailable",_=>"t3_remote_unavailable"}.into())}
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
/// Stable, non-identifying id for this device and user: the session subject's suffix.
fn client_id()->String{
    let mut seed=fs::read("/etc/machine-id").unwrap_or_default();seed.extend(unsafe{libc::geteuid()}.to_le_bytes());
    format!("{:x}",Sha256::digest(&seed))[..16].to_string()
}
/// Sessions ot3 issued earlier from this device (or, before subjects, under its label)
/// that are idle and not the caller's own. Live and recently used sessions are kept,
/// so another ot3 process on this device is never cut off.
pub fn stale_ot3_sessions(clients:&Value,subject:&str,now:DateTime<Utc>)->Vec<String>{
    let list=clients.as_array().or_else(||clients["clientSessions"].as_array());
    list.into_iter().flatten().filter(|c|c["current"]!=true&&c["connected"]!=true).filter_map(|c|{
        let ours=c["subject"]==subject;
        let legacy=c["subject"]=="cli-issued-session"&&c["client"]["label"]=="omarchy-t3-code";
        let idle=if ours{120}else if legacy{3600}else{return None};
        let last=c["lastConnectedAt"].as_str().or(c["issuedAt"].as_str()).and_then(|t|DateTime::parse_from_rfc3339(t).ok())?;
        (now.signed_duration_since(last).num_seconds()>idle).then(||text(c,"sessionId").to_string()).filter(|id|!id.is_empty()&&id.len()<=128)
    }).collect()
}
fn prune_sessions(base:&str,token:&str){
    let options=protocol::ConnectionOptions::default().with_request_timeout(Duration::from_secs(10));
    let Ok(connection)=protocol::Credential::bearer(token).and_then(|c|protocol::Connection::with_options(base,None,c,&options)) else {return};
    let subject=format!("omarchy-t3-code:{}",client_id());
    let _=connection.revoke_stale_sessions(|clients|stale_ot3_sessions(clients,&subject,Utc::now()));
}
fn connect_ssh(profile:&Value, env:&mut Environment)->Result<Tunnel> {connect_ssh_via(profile,env,None)}
/// Resolve the host key name OpenSSH would use for this profile, so a bridged
/// connection is still verified against the user's existing known_hosts entry.
fn host_key_alias(args:&[String])->Result<String>{
    let out=Command::new("timeout").args(["4s","ssh","-G"]).args(args).stdin(Stdio::null()).stderr(Stdio::null()).output().map_err(|_|"ssh_unavailable")?;
    if !out.status.success(){return Err("ssh_unavailable".into())}
    let config=String::from_utf8(out.stdout).map_err(|_|"ssh_unavailable")?;
    let value=|key:&str|config.lines().find_map(|l|l.strip_prefix(key).and_then(|v|v.strip_prefix(' ')).map(str::trim).map(str::to_owned));
    let host=value("hostname").filter(|h|!h.is_empty()&&h.len()<=255&&h.bytes().all(|b|b.is_ascii_alphanumeric()||b".:_-[]%".contains(&b))).ok_or("invalid_t3_ssh_profile")?;
    let port=value("port").and_then(|p|p.parse::<u16>().ok()).unwrap_or(22);
    Ok(if port==22{host}else{format!("[{host}]:{port}")})
}
/// Same destination and identity, but TCP goes to a loopback OmaBridge forward.
pub(crate) fn bridged_ssh_args(target:&Value,alias:&str,local_port:u16)->Result<Vec<String>>{
    let mut target=target.clone();target["port"]=Value::Null;
    let mut args=ssh_args(&target)?;let split=args.iter().position(|a|a=="--").ok_or("invalid_t3_ssh_profile")?;
    let extra=["HostName=127.0.0.1".to_string(),format!("Port={local_port}"),format!("HostKeyAlias={alias}"),"CheckHostIP=no".into(),"ProxyJump=none".into(),"ProxyCommand=none".into()];
    for (i,o) in extra.into_iter().enumerate(){args.insert(split+i*2,"-o".into());args.insert(split+i*2+1,o);}
    Ok(args)
}
fn connect_ssh_via(profile:&Value, env:&mut Environment, bridge:Option<&crate::omabridge_route::Route>)->Result<Tunnel> {
    uuid::Uuid::parse_str(&env.id).map_err(|_|"invalid_t3_environment_id")?;
    let mut forward=None;
    let args=match bridge{
        None=>ssh_args(&profile["target"])?,
        Some(route)=>{
            let alias=host_key_alias(&ssh_args(&profile["target"])?)?;
            let (child,url)=crate::omabridge_route::connect(route,Duration::from_secs(15))?;forward=Some(child);
            let port=crate::Url::parse(&url).ok().and_then(|u|u.port()).ok_or("omabridge_route_failed")?;
            bridged_ssh_args(&profile["target"],&alias,port)?
        }
    };
    // Until the tunnel owns it, any early return kills the forward.
    let mut guard=ChildGuard(forward);
    let mut cmd=Command::new("timeout");cmd.args(["60s","ssh"]).args(&args).arg(format!("sh -s -- '{}' '{}'",env.id,client_id())).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
    let mut child=cmd.spawn().map_err(|_|"ssh_unavailable")?;
    child.stdin.take().ok_or("ssh_unavailable")?.write_all(REMOTE_DISCOVERY.as_bytes()).map_err(|_|"ssh_unavailable")?;
    let output=child.wait_with_output().map_err(|_|"ssh_unavailable")?;
    if !output.status.success(){return Err(match output.status.code(){Some(7)=>"t3_environment_mismatch",Some(8)=>"service_profile_mismatch",Some(9)=>"t3_remote_start_failed",Some(124)=>"ssh_timeout",Some(255)=>"ssh_unavailable",Some(127)=>"t3_cli_unavailable",_=>"t3_remote_unavailable"}.into())}
    let output=Zeroizing::new(output.stdout);let raw=std::str::from_utf8(&output).map_err(|_|"invalid_t3_runtime")?;
    let (runtime,token)=raw.split_once("\nT3_AUTH\n").ok_or("invalid_t3_runtime")?;
    let runtime:Value=serde_json::from_str(runtime).map_err(|_|"invalid_t3_runtime")?;
    let remote_port=runtime["port"].as_u64().filter(|p|*p>0&&*p<=65535).ok_or("invalid_t3_runtime")?;
    let token=token.trim();if !fresh_token(token){return Err("t3_auth_unavailable".into())}
    let listener=TcpListener::bind("127.0.0.1:0").map_err(|_|"local_error")?;
    let port=listener.local_addr().map_err(|_|"local_error")?.port();drop(listener);
    let mut command=Command::new("ssh");
    command.args(["-N","-L",&format!("127.0.0.1:{port}:127.0.0.1:{remote_port}"),"-o","ExitOnForwardFailure=yes","-o","ServerAliveInterval=30","-o","ServerAliveCountMax=2"]).args(&args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    let parent=unsafe{libc::getpid()};
    unsafe{command.pre_exec(move ||{if libc::prctl(libc::PR_SET_PDEATHSIG,libc::SIGTERM)!=0{return Err(std::io::Error::last_os_error())}if libc::getppid()!=parent{libc::raise(libc::SIGTERM);}Ok(())});}
    let mut tunnel=Tunnel(command.spawn().map_err(|_|"ssh_unavailable")?,guard.0.take());
    let deadline=Instant::now()+Duration::from_secs(6);
    while Instant::now()<deadline {
        if !tunnel.alive(){return Err("ssh_unavailable".into())}
        if TcpStream::connect_timeout(&format!("127.0.0.1:{port}").parse().unwrap(),Duration::from_millis(50)).is_ok(){
            env.base_url=format!("http://127.0.0.1:{port}");env.bearer=Some(token.into());
            // Each (re)connection issues a session; retire this device's previous ones.
            let (url,token)=(env.base_url.clone(),Zeroizing::new(token.to_string()));std::thread::spawn(move||prune_sessions(&url,&token));
            return Ok(tunnel)
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Err("ssh_unavailable".into())
}
pub fn inherited_environments()->Result<Vec<Environment>> {
    let _rebuild=match REBUILD.try_lock(){
        Ok(guard)=>guard,
        Err(std::sync::TryLockError::WouldBlock)=>{
            if let Some(last)=LAST.lock().ok().and_then(|l|l.clone()){return Ok(last)}
            REBUILD.lock().map_err(|_|"local_error")?
        }
        Err(std::sync::TryLockError::Poisoned(p))=>p.into_inner(),
    };
    let result=build_environments();
    if let Ok(list)=&result{if let Ok(mut last)=LAST.lock(){*last=Some(list.clone());}}
    result
}
fn build_environments()->Result<Vec<Environment>> {
    let mut cache=CACHE.get_or_init(||Mutex::new(None)).lock().map_err(|_|"local_error")?;
    // A refresh asked for by the user retries failed environments at once.
    let forced=INVALIDATED.swap(false,std::sync::atomic::Ordering::SeqCst);
    let failed:Vec<String>=FAILED.lock().map(|mut f|std::mem::take(&mut *f)).unwrap_or_default();
    if let Some(c)=cache.as_mut(){
        if forced||!failed.is_empty(){c.until=Instant::now();}
        for e in c.environments.iter_mut().filter(|e|failed.contains(&e.id)){e.connection_error=Some("retry".into());}
        if forced{c.retry.clear();}
        for id in RECONNECT.lock().map(|mut r|std::mem::take(&mut *r)).unwrap_or_default(){
            c.tunnels.remove(&id);c.environments.retain(|e|e.id!=id);c.routes.remove(&id);c.retry.remove(&id);c.until=Instant::now();
        }
    }
    let bridge_config=crate::omabridge_route::config_bytes();
    let signature=["userdata/server-runtime.json","userdata/connection-catalog.json","userdata/desktop-settings.json","userdata/environment-id"].iter().map(|name|fs::read(t3_home().join(name)).unwrap_or_default()).flatten().chain(bridge_config.iter().copied()).collect::<Vec<_>>();
    let signature=format!("{:x}",Sha256::digest(&signature));
    let cloud_signature=format!("{:x}",Sha256::digest(fs::read(t3_home().join("userdata/clerk-tokens.json")).unwrap_or_default()));
    if let Some(c)=cache.as_mut(){if c.signature==signature && c.cloud_signature==cloud_signature && Instant::now()<c.until && c.tunnels.values_mut().all(Tunnel::alive){return Ok(c.environments.clone())}}
    let remote_signature=format!("{:x}",Sha256::digest(["userdata/connection-catalog.json","userdata/desktop-settings.json","userdata/environment-id"].iter().flat_map(|name|fs::read(t3_home().join(name)).unwrap_or_default()).collect::<Vec<_>>()));
    let mut previous=cache.take();
    let mut result=vec![];let mut tunnels=HashMap::new();let mut routes=HashMap::new();let mut retry=HashMap::new();
    let catalog=previous.as_ref().filter(|c|c.remote_signature==remote_signature).and_then(|c|c.catalog.as_ref().ok()).cloned().map(Ok).unwrap_or_else(read_catalog);
    let primary_id=fs::read_to_string(t3_home().join("userdata/environment-id")).unwrap_or_default();
    // A malformed route file disables OmaBridge routing; inherited T3 routes still work.
    let bridge_config=crate::omabridge_route::config().unwrap_or_default();
    // Paired peers per service, listed at most once per rebuild and only when needed.
    let mut auto_routes:HashMap<&str,Vec<crate::omabridge_route::Route>>=HashMap::new();
    let local_disabled=catalog.as_ref().ok().and_then(|c|c["disabledEnvironmentIds"].as_array()).is_some_and(|ids|ids.contains(&json!(primary_id.trim()))||ids.contains(&json!("local")));
    let local_disabled=local_disabled || read_json(&t3_home().join("userdata/desktop-settings.json")).ok().is_some_and(|c|c["localEnvironmentEnabled"]==false);
    if !local_disabled {
        match local_environment(){Ok(e)=>result.push(e),Err(error)=>{let mut e=remote_base(&json!({"environmentId":"local","label":"T3 Code"}));e.connection_error=Some(error);result.push(e);}}
    }
    match &catalog {
        Ok(catalog)=>{
            let disabled=catalog["disabledEnvironmentIds"].as_array().cloned().unwrap_or_default();
            for target in catalog["targets"].as_array().into_iter().flatten().filter(|t|!disabled.contains(&t["environmentId"])) {
                let mut e=remote_base(target);
                if e.id.is_empty() || e.id=="local" || e.id==primary_id.trim(){continue}
                if result.iter().any(|known| known.id==e.id && known.connection_error.is_none()){continue}
                let service=match text(target,"_tag"){"SshConnectionTarget"=>"ssh","BearerConnectionTarget"=>"t3",_=>""};
                let plan=match bridge_config.plan(&e.id){
                    crate::omabridge_route::Plan::Pinned(r) if !crate::omabridge_route::carries(text(target,"_tag"),&r.service)=>crate::omabridge_route::Plan::Off,
                    crate::omabridge_route::Plan::Auto if service.is_empty()=>crate::omabridge_route::Plan::Off,
                    plan=>plan,
                };
                let connection=target["connectionId"].as_str().unwrap_or("relay");
                let route_id=match &plan{crate::omabridge_route::Plan::Pinned(r)=>format!("{connection}|{}",r.signature()),crate::omabridge_route::Plan::Auto=>format!("{connection}|omabridge:auto"),crate::omabridge_route::Plan::Off=>connection.to_string()};
                if let Some(prior)=previous.as_mut(){
                    let unchanged=prior.catalog.as_ref().ok().is_some_and(|old|{let id=&target["connectionId"];let profile=|c:&Value|c["profiles"].as_array().into_iter().flatten().find(|p|p["connectionId"]==*id).map(|p|json!({"target":p["target"],"url":p["httpBaseUrl"]}));let credentials=|c:&Value|c["credentials"].as_array().into_iter().flatten().find(|p|p["connectionId"]==*id).cloned();old["targets"].as_array().into_iter().flatten().any(|t|t["environmentId"]==target["environmentId"]&&t["connectionId"]==target["connectionId"]&&t["_tag"]==target["_tag"])&&profile(old)==profile(catalog)&&credentials(old)==credentials(catalog)});
                    if let Some(known)=prior.environments.iter().filter(|_|unchanged&&prior.routes.get(&e.id).is_some_and(|id|*id==route_id)).find(|known|known.id==e.id && known.connection_error.is_none()) {
                        let alive=prior.tunnels.get_mut(&e.id).is_none_or(Tunnel::alive);
                        if alive && (known.connection.is_none() || prior.cloud_signature==cloud_signature) && known.connection.as_ref().is_none_or(|c|c.expires_at().is_none_or(|t|t>Utc::now().timestamp()+300)) && known.bearer.as_ref().is_none_or(|t|fresh_token(t)) {
                            let mut known=known.clone();known.name=e.name.clone();routes.insert(e.id.clone(),route_id.clone());result.push(known);if let Some(t)=prior.tunnels.remove(&e.id){tunnels.insert(e.id.clone(),t);}continue
                        }
                    }
                }
                // Failed recently over the same route: keep the error until its retry time.
                if let Some(prior)=previous.as_ref().filter(|p|p.remote_signature==remote_signature){
                    if let (Some(&(at,count,ref failed_route)),Some(known))=(prior.retry.get(&e.id),prior.environments.iter().find(|k|k.id==e.id&&k.connection_error.is_some())){
                        if Instant::now()<at&&*failed_route==route_id{
                            let mut known=known.clone();known.name=e.name.clone();retry.insert(e.id.clone(),(at,count,route_id.clone()));
                            result.retain(|r:&Environment|r.id!=known.id);result.push(known);continue
                        }
                    }
                }
                let profile=catalog["profiles"].as_array().into_iter().flatten().find(|p|p["connectionId"]==target["connectionId"]);
                // Pinned routes are tried alone; automatic routing tries every granting peer.
                let candidates:Vec<(crate::omabridge_route::Route,bool)>=match &plan{
                    crate::omabridge_route::Plan::Pinned(r)=>vec![(r.clone(),false)],
                    crate::omabridge_route::Plan::Auto=>auto_routes.entry(service).or_insert_with(||crate::omabridge_route::auto_routes(&bridge_config,service)).iter().map(|r|(r.clone(),true)).collect(),
                    crate::omabridge_route::Plan::Off=>vec![],
                };
                let mut carried=None;
                let status=match (text(target,"_tag"),profile) {
                    ("BearerConnectionTarget",Some(p))=>{
                        e.base_url=text(p,"httpBaseUrl").into();
                        e.bearer=catalog["credentials"].as_array().into_iter().flatten().find(|c|c["connectionId"]==target["connectionId"]).and_then(|c|c["credential"]["token"].as_str()).map(String::from);
                        validate_base(&e.base_url).and_then(|_|if e.bearer.is_some(){Ok(())}else{Err("t3_credentials_unavailable".into())}).and_then(|_|{
                            // Same T3 bearer session, carried over the paired OmaBridge peer once
                            // the peer's T3 descriptor names this environment.
                            for (route,auto) in &candidates{
                                let attempt=crate::omabridge_route::connect(route,Duration::from_secs(15)).and_then(|(child,url)|{let mut guard=ChildGuard(Some(child));crate::omabridge_route::verify_environment(&url,&e.id)?;Ok((guard.0.take().ok_or("omabridge_route_failed")?,url))});
                                match attempt{
                                    Ok((child,url))=>{e.base_url=url;tunnels.insert(e.id.clone(),Tunnel(child,None));carried=Some((route.clone(),*auto));return Ok(())},
                                    Err(error) if route.only=>return Err(error),
                                    Err(_)=>{},
                                }
                            }
                            Ok(())
                        })
                    },
                    ("SshConnectionTarget",Some(p))=>(||{
                        // The bridged hop keeps host-key verification and the remote
                        // environment check, so a wrong peer simply fails.
                        for (route,auto) in &candidates{
                            match connect_ssh_via(p,&mut e,Some(route)){
                                Ok(t)=>{tunnels.insert(e.id.clone(),t);carried=Some((route.clone(),*auto));return Ok(())},
                                Err(error) if route.only=>return Err(error),
                                Err(_)=>{},
                            }
                        }
                        connect_ssh(p,&mut e).map(|t|{tunnels.insert(e.id.clone(),t);})
                    })(),
                    ("RelayConnectionTarget",_)=>crate::desktop_cloud::connect(&mut e),
                    _=>Err("invalid_t3_catalog".into())
                };
                crate::omabridge_route::set_active(&e.id,carried.as_ref().map(|(r,auto)|(r,*auto)));
                if let Err(error)=status{
                    let count=previous.as_ref().and_then(|p|p.retry.get(&e.id)).map_or(0,|r|r.1)+1;
                    retry.insert(e.id.clone(),(Instant::now()+retry_delay(count),count,route_id));
                    e.connection_error=Some(error);
                }else{routes.insert(e.id.clone(),route_id);}
                result.retain(|prior| prior.id!=e.id);
                result.push(e);
            }
        },
        Err(error)=>{let mut e=remote_base(&json!({"environmentId":"t3-catalog","label":"T3 connections"}));e.connection_error=Some(error.clone());result.push(e);}
    }
    let has_error=result.iter().any(|e|e.connection_error.is_some());
    let lifetime=result.iter().filter_map(|e|e.bearer.clone().or_else(||private_token(&e.token_file).ok())).filter_map(|t|token_expiry_ms(&t)).chain(result.iter().filter_map(|e|e.connection.as_ref().and_then(|c|c.expires_at()).map(|s|s*1000))).map(|exp|((exp-Utc::now().timestamp_millis()-300_000)/1000).max(30) as u64).min().unwrap_or(3600*20).min(3600*20);
    // Errors outside the retry schedule (the local server, the catalog) are checked again in 3 s.
    let until=if !has_error{Instant::now()+Duration::from_secs(lifetime)}
        else if result.iter().any(|e|e.connection_error.is_some()&&!retry.contains_key(&e.id)){Instant::now()+Duration::from_secs(3)}
        else{retry.values().map(|r|r.0).min().unwrap_or_else(Instant::now).max(Instant::now()+Duration::from_secs(3))};
    *cache=Some(Cache{signature,remote_signature,cloud_signature,until,environments:result.clone(),routes,tunnels,catalog,retry});
    Ok(result)
}

pub fn refresh_connections() {
    INVALIDATED.store(true,std::sync::atomic::Ordering::SeqCst);
}

pub fn connection_failed(id:&str) {
    if let Ok(mut failed)=FAILED.lock(){if !failed.iter().any(|f|f==id){failed.push(id.to_string());}}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_environments_back_off_without_blocking_refreshes() {
        let secs:Vec<u64>=(1..=8).map(|n|retry_delay(n).as_secs()).collect();
        assert_eq!(secs,[5,10,20,40,80,120,120,120]);
        refresh_connections();connection_failed("a");connection_failed("a");
        assert!(INVALIDATED.load(std::sync::atomic::Ordering::SeqCst));
        assert_eq!(*FAILED.lock().unwrap(),["a"]);
    }
    use aes::cipher::BlockEncryptMut;
    #[test]
    fn electron_catalog_compatibility_and_session_expiry() {
        let plain=br#"{"schemaVersion":1,"targets":[],"profiles":[],"credentials":[]}"#;
        for (tag,password) in [(b"v10",b"peanuts".as_slice()),(b"v11",b"synthetic-t3-key".as_slice())] {
            let mut key=[0u8;16];pbkdf2::pbkdf2_hmac::<sha1::Sha1>(password,b"saltysalt",1,&mut key);
            let mut bytes=tag.to_vec();bytes.extend(cbc::Encryptor::<aes::Aes128>::new_from_slices(&key,&[b' ';16]).unwrap().encrypt_padded_vec_mut::<Pkcs7>(plain));
            let decoded=decrypt_catalog(&bytes,password).unwrap();assert_eq!(decoded["schemaVersion"],1);
            assert!(decrypt_catalog(&bytes,b"wrong key").is_err());
        }
        assert_eq!(decrypt_catalog(b"v99-unsupported-format",b"key").unwrap_err(),"unsupported_catalog_encryption");
        let claims=json!({"kind":"session","exp":Utc::now().timestamp_millis()+86_400_000});
        let token=format!("{}.synthetic-signature",base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(claims.to_string()));
        assert!(fresh_token(&token));assert!(!has_filesystem_read(&token));
        let fs_token=format!("{}.synthetic-signature",base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json!({"scopes":["filesystem:read"]}).to_string()));assert!(has_filesystem_read(&fs_token));assert!(!has_scope(&fs_token,"settings:write"));
        let full=format!("{}.synthetic-signature",base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json!({"scopes":["filesystem:read","settings:write"]}).to_string()));assert!(has_scope(&full,"settings:write"));assert!(!fresh_token("invalid"));
        assert!(validate_catalog(json!({"schemaVersion":2})).is_err());
        assert!(ssh_args(&json!({"alias":"-oProxyCommand=bad"})).is_err());
        assert!(ssh_args(&json!({"alias":"node","username":"dev","port":22})).is_ok());
        // Bridged SSH keeps the user/destination, drops the profile port and pins host-key verification.
        let bridged=bridged_ssh_args(&json!({"alias":"devbox.example","username":"dev","port":2222}),"[devbox.example]:2222",41000).unwrap();
        let tail=bridged.iter().position(|a|a=="--").unwrap();
        assert_eq!(&bridged[tail..],["--","devbox.example"]);assert!(!bridged.contains(&"-p".to_string()));
        for option in ["HostName=127.0.0.1","Port=41000","HostKeyAlias=[devbox.example]:2222","CheckHostIP=no","ProxyJump=none","ProxyCommand=none"]{assert!(bridged[..tail].contains(&option.to_string()),"{option}");}
        assert!(bridged.windows(2).any(|w|w[0]=="-l"&&w[1]=="dev"));assert!(bridged.windows(2).any(|w|w[0]=="-o"&&w[1]=="BatchMode=yes"));
    }
}
