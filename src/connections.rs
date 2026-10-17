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
struct Tunnel(Child);
impl Drop for Tunnel {fn drop(&mut self){let _=self.0.kill();let _=self.0.wait();}}
struct Cache { signature:String, remote_signature:String, cloud_signature:String, until:Instant, environments:Vec<Environment>, routes:HashMap<String,String>, tunnels:HashMap<String,Tunnel>, catalog:Result<Value> }
static CACHE:OnceLock<Mutex<Option<Cache>>>=OnceLock::new();
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
    let url=reqwest::Url::parse(origin).map_err(|_|"invalid_t3_runtime")?;
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
    if let Some(port)=target["port"].as_u64(){if port==0||port>65535{return Err("invalid_t3_ssh_profile".into())}args.extend(["-p".into(),port.to_string()]);}
    if let Some(user)=target["username"].as_str(){if user.is_empty()||!user.bytes().all(|b|b.is_ascii_alphanumeric()||b"._-".contains(&b)){return Err("invalid_t3_ssh_profile".into())}args.extend(["-l".into(),user.into()]);}
    args.extend(["--".into(),destination.into()]);Ok(args)
}
const REMOTE_DISCOVERY:&str=include_str!("remote-bootstrap.sh");
fn connect_ssh(profile:&Value, env:&mut Environment)->Result<Tunnel> {
    uuid::Uuid::parse_str(&env.id).map_err(|_|"invalid_t3_environment_id")?;
    let args=ssh_args(&profile["target"])?;
    let mut cmd=Command::new("timeout");cmd.args(["60s","ssh"]).args(&args).arg(format!("sh -s -- '{}'",env.id)).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
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
    let mut tunnel=Tunnel(command.spawn().map_err(|_|"ssh_unavailable")?);
    let deadline=Instant::now()+Duration::from_secs(6);
    while Instant::now()<deadline {
        if tunnel.0.try_wait().map_err(|_|"ssh_unavailable")?.is_some(){return Err("ssh_unavailable".into())}
        if TcpStream::connect_timeout(&format!("127.0.0.1:{port}").parse().unwrap(),Duration::from_millis(50)).is_ok(){
            env.base_url=format!("http://127.0.0.1:{port}");env.bearer=Some(token.into());return Ok(tunnel)
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Err("ssh_unavailable".into())
}
pub fn inherited_environments()->Result<Vec<Environment>> {
    let mut cache=CACHE.get_or_init(||Mutex::new(None)).lock().map_err(|_|"local_error")?;
    let signature=["userdata/server-runtime.json","userdata/connection-catalog.json","userdata/desktop-settings.json","userdata/environment-id"].iter().map(|name|fs::read(t3_home().join(name)).unwrap_or_default()).flatten().collect::<Vec<_>>();
    let signature=format!("{:x}",Sha256::digest(&signature));
    let cloud_signature=format!("{:x}",Sha256::digest(fs::read(t3_home().join("userdata/clerk-tokens.json")).unwrap_or_default()));
    if let Some(c)=cache.as_mut(){if c.signature==signature && c.cloud_signature==cloud_signature && Instant::now()<c.until && c.tunnels.values_mut().all(|t|t.0.try_wait().ok()==Some(None)){return Ok(c.environments.clone())}}
    let remote_signature=format!("{:x}",Sha256::digest(["userdata/connection-catalog.json","userdata/desktop-settings.json","userdata/environment-id"].iter().flat_map(|name|fs::read(t3_home().join(name)).unwrap_or_default()).collect::<Vec<_>>()));
    let mut previous=cache.take();
    let mut result=vec![];let mut tunnels=HashMap::new();let mut routes=HashMap::new();
    let catalog=previous.as_ref().filter(|c|c.remote_signature==remote_signature).and_then(|c|c.catalog.as_ref().ok()).cloned().map(Ok).unwrap_or_else(read_catalog);
    let primary_id=fs::read_to_string(t3_home().join("userdata/environment-id")).unwrap_or_default();
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
                if let Some(prior)=previous.as_mut(){
                    let unchanged=prior.catalog.as_ref().ok().is_some_and(|old|{let id=&target["connectionId"];let profile=|c:&Value|c["profiles"].as_array().into_iter().flatten().find(|p|p["connectionId"]==*id).map(|p|json!({"target":p["target"],"url":p["httpBaseUrl"]}));let credentials=|c:&Value|c["credentials"].as_array().into_iter().flatten().find(|p|p["connectionId"]==*id).cloned();old["targets"].as_array().into_iter().flatten().any(|t|t["environmentId"]==target["environmentId"]&&t["connectionId"]==target["connectionId"]&&t["_tag"]==target["_tag"])&&profile(old)==profile(catalog)&&credentials(old)==credentials(catalog)});
                    let route_id=target["connectionId"].as_str().unwrap_or("relay");
                    if let Some(known)=prior.environments.iter().filter(|_|unchanged&&prior.routes.get(&e.id).is_some_and(|id|id==route_id)).find(|known|known.id==e.id && known.connection_error.is_none()) {
                        let alive=prior.tunnels.get_mut(&e.id).is_none_or(|t|t.0.try_wait().ok()==Some(None));
                        if alive && (known.connection.is_none() || prior.cloud_signature==cloud_signature) && known.connection.as_ref().is_none_or(|c|c.expires_at().is_none_or(|t|t>Utc::now().timestamp()+300)) && known.bearer.as_ref().is_none_or(|t|fresh_token(t)) {
                            let mut known=known.clone();known.name=e.name.clone();routes.insert(e.id.clone(),route_id.to_string());result.push(known);if let Some(t)=prior.tunnels.remove(&e.id){tunnels.insert(e.id.clone(),t);}continue
                        }
                    }
                }
                let profile=catalog["profiles"].as_array().into_iter().flatten().find(|p|p["connectionId"]==target["connectionId"]);
                let status=match (text(target,"_tag"),profile) {
                    ("BearerConnectionTarget",Some(p))=>{
                        e.base_url=text(p,"httpBaseUrl").into();
                        e.bearer=catalog["credentials"].as_array().into_iter().flatten().find(|c|c["connectionId"]==target["connectionId"]).and_then(|c|c["credential"]["token"].as_str()).map(String::from);
                        validate_base(&e.base_url).and_then(|_|if e.bearer.is_some(){Ok(())}else{Err("t3_credentials_unavailable".into())})
                    },
                    ("SshConnectionTarget",Some(p))=>connect_ssh(p,&mut e).map(|t|{tunnels.insert(e.id.clone(),t);}),
                    ("RelayConnectionTarget",_)=>crate::desktop_cloud::connect(&mut e),
                    _=>Err("invalid_t3_catalog".into())
                };
                if let Err(error)=status{e.connection_error=Some(error);}else{routes.insert(e.id.clone(),target["connectionId"].as_str().unwrap_or("relay").to_string());}
                result.retain(|prior| prior.id!=e.id);
                result.push(e);
            }
        },
        Err(error)=>{let mut e=remote_base(&json!({"environmentId":"t3-catalog","label":"T3 connections"}));e.connection_error=Some(error.clone());result.push(e);}
    }
    let has_error=result.iter().any(|e|e.connection_error.is_some());
    let lifetime=result.iter().filter_map(|e|e.bearer.clone().or_else(||private_token(&e.token_file).ok())).filter_map(|t|token_expiry_ms(&t)).chain(result.iter().filter_map(|e|e.connection.as_ref().and_then(|c|c.expires_at()).map(|s|s*1000))).map(|exp|((exp-Utc::now().timestamp_millis()-300_000)/1000).max(30) as u64).min().unwrap_or(3600*20).min(3600*20);
    *cache=Some(Cache{signature,remote_signature,cloud_signature,until:Instant::now()+Duration::from_secs(if has_error{3}else{lifetime}),environments:result.clone(),routes,tunnels,catalog});
    Ok(result)
}

pub fn refresh_connections() {
    if let Some(lock)=CACHE.get(){if let Ok(mut c)=lock.lock(){if let Some(c)=c.as_mut(){c.until=Instant::now();}}}
}

pub fn connection_failed(id:&str) {
    if let Some(lock)=CACHE.get(){if let Ok(mut cache)=lock.lock(){if let Some(c)=cache.as_mut(){
        if let Some(e)=c.environments.iter_mut().find(|e|e.id==id){e.connection_error=Some("retry".into());}
        c.until=Instant::now();
    }}}
}

#[cfg(test)]
mod tests {
    use super::*;
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
        assert!(ssh_args(&json!({"alias":"node","username":"node","port":22})).is_ok());
    }
}
