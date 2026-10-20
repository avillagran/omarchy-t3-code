//! OmaBridge transport for inherited SSH and pairing-link connections.
//! OmaBridge owns device identity, grants and encryption; ot3 only chooses which
//! already-paired peer carries an environment and spawns a loopback forward.
//! By default ("auto") every paired peer that grants the matching service is
//! tried first, and a peer is used only once it proves it serves that
//! environment (SSH host key plus T3 environment id, or the T3 descriptor).
//! T3 credentials stay in T3's catalog and are never written here.
use crate::*;
use std::{collections::HashSet,io::{BufRead,BufReader,Write},os::unix::process::CommandExt,process::{Child,Command,Stdio},sync::{mpsc,Mutex},time::Instant};
pub fn config_path()->PathBuf{xdg("XDG_CONFIG_HOME",".config").join("omabridge-routes.json")}
pub(crate) fn default_state_dir()->PathBuf{
 std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).filter(|p|p.is_absolute()).unwrap_or_else(||home().join(".local/share")).join("omabridge")
}
/// The OmaBridge release ot3 bundles, from the single pinned file.
pub fn bundled_version()->&'static str{
 include_str!("../integrations/omabridge/release").lines().find_map(|l|l.strip_prefix("version=")).unwrap_or("")
}
/// The bundled `bin/omabridge` next to this client; never resolved from PATH.
/// `OT3_OMABRIDGE_BIN` is an explicit override for tests and development.
pub fn binary()->Option<PathBuf>{
 if let Some(raw)=std::env::var_os("OT3_OMABRIDGE_BIN"){let p=PathBuf::from(raw);return (p.is_absolute()&&p.is_file()).then_some(p)}
 std::env::current_exe().ok()?.parent().map(|d|d.join("omabridge")).filter(|p|p.is_file())
}
fn bytes(path:&Path)->Result<Option<Vec<u8>>>{match fs::read(path){Ok(v)=>Ok(Some(v)),Err(e) if e.kind()==std::io::ErrorKind::NotFound=>Ok(None),Err(_)=>Err("omabridge_routes_read_failed".into())}}
fn revision_of(raw:&Option<Vec<u8>>)->String{format!("{:x}",Sha256::digest(raw.as_deref().unwrap_or_default()))}
pub fn valid_peer(id:&str)->bool{id.len()==64&&id.bytes().all(|b|b.is_ascii_digit()||(b'a'..=b'f').contains(&b))}
pub fn valid_service(name:&str)->bool{!name.is_empty()&&name.len()<=64&&name.bytes().all(|b|b.is_ascii_alphanumeric()||b"-_.".contains(&b))}
fn valid_environment(id:&str)->bool{!id.is_empty()&&id!="local"&&id.len()<=128&&id.bytes().all(|b|b.is_ascii_alphanumeric()||b"-_".contains(&b))}
fn valid_relay(url:&str)->bool{reqwest::Url::parse(url).is_ok_and(|u|u.scheme()=="https"&&u.host_str().is_some()&&u.username().is_empty()&&u.password().is_none()&&u.query().is_none())}
/// SSH profiles are carried by a peer's `ssh` service; pairing links by any other
/// service (normally `t3`). A route never applies to T3 Connect relay targets.
pub fn carries(tag:&str,service:&str)->bool{
 match tag{"SshConnectionTarget"=>service=="ssh","BearerConnectionTarget"=>service!="ssh",_=>false}
}
#[derive(Clone,Debug,PartialEq)]
pub struct Route{pub peer:String,pub service:String,pub only:bool,pub relay_url:Option<String>,pub state_dir:PathBuf}
impl Route{
 /// Stable identity of the transport choice, used to reuse or replace a live connection.
 pub fn signature(&self)->String{format!("omabridge:{}:{}:{}:{}",self.peer,self.service,self.relay_url.as_deref().unwrap_or(""),self.state_dir.display())}
}
/// Per-environment choice: a pinned peer, automatic discovery, or the saved T3 route.
#[derive(Clone,Debug,PartialEq)]
pub enum Plan{Pinned(Route),Auto,Off}
#[derive(Clone,Debug,Default)]
pub struct Config{pub state_dir:Option<PathBuf>,pub auto:bool,pub routes:HashMap<String,Route>,pub off:HashSet<String>}
impl Config{
 pub fn state_dir(&self)->PathBuf{self.state_dir.clone().unwrap_or_else(default_state_dir)}
 pub fn plan(&self,env:&str)->Plan{
  if let Some(r)=self.routes.get(env){return Plan::Pinned(r.clone())}
  if self.auto&&!self.off.contains(env){Plan::Auto}else{Plan::Off}
 }
}
/// Validate the whole document; one malformed entry disables every OmaBridge route
/// rather than silently routing a different environment.
pub fn parse_config(doc:&Value)->Result<Config>{
 if doc["version"]!=1||!doc["routes"].is_object(){return Err("invalid_omabridge_routes".into())}
 let state_dir=match doc["stateDir"].as_str(){Some(s)=>{let p=PathBuf::from(s);if !p.is_absolute()||s.contains('\0'){return Err("invalid_omabridge_routes".into())}Some(p)},None if doc["stateDir"].is_null()=>None,None=>return Err("invalid_omabridge_routes".into())};
 let auto=match doc["default"].as_str(){Some("auto")=>true,Some("off")=>false,None if doc["default"].is_null()=>true,_=>return Err("invalid_omabridge_routes".into())};
 let mut config=Config{state_dir:state_dir.clone(),auto,..Default::default()};
 for (env,r) in doc["routes"].as_object().into_iter().flatten(){
  if !valid_environment(env){return Err("invalid_omabridge_routes".into())}
  if r["mode"]=="off"{
   if !r["peer"].is_null(){return Err("invalid_omabridge_routes".into())}
   config.off.insert(env.clone());continue
  }
  let (peer,service)=(text(r,"peer"),r["service"].as_str().unwrap_or("t3"));
  let relay=r["relayUrl"].as_str().filter(|s|!s.is_empty()).map(String::from);
  if !valid_peer(peer)||!valid_service(service)||relay.as_deref().is_some_and(|u|!valid_relay(u))||!matches!(r["mode"].as_str().unwrap_or("prefer"),"prefer"|"only"){return Err("invalid_omabridge_routes".into())}
  config.routes.insert(env.clone(),Route{peer:peer.into(),service:service.into(),only:r["mode"]=="only",relay_url:relay,state_dir:state_dir.clone().unwrap_or_else(default_state_dir)});
 }
 Ok(config)
}
pub fn parse(doc:&Value)->Result<(Option<PathBuf>,HashMap<String,Route>)>{parse_config(doc).map(|c|(c.state_dir,c.routes))}
fn load()->Result<(Option<Vec<u8>>,Value)>{
 let raw=bytes(&config_path())?;
 let doc=match &raw{Some(b)=>serde_json::from_slice::<Value>(b).map_err(|_|"invalid_omabridge_routes")?,None=>json!({"version":1,"routes":{}})};
 Ok((raw,doc))
}
/// Routing configuration for connection setup. A missing file means automatic routing.
pub fn config()->Result<Config>{let (_,doc)=load()?;parse_config(&doc)}
pub fn routes()->Result<HashMap<String,Route>>{config().map(|c|c.routes)}
/// Inputs that change OmaBridge routing: the route file and the paired-peer state.
pub fn config_bytes()->Vec<u8>{
 let mut out=fs::read(config_path()).unwrap_or_default();
 let state=config().map(|c|c.state_dir()).unwrap_or_else(|_|default_state_dir());
 out.extend(fs::read(state.join("state.json")).unwrap_or_default());out
}
/// Automatic candidates for one service: every paired peer that grants it.
pub fn auto_routes(config:&Config,service:&str)->Vec<Route>{
 let state_dir=config.state_dir();
 let Some(bin)=binary() else {return vec![]};
 peers(&bin,&state_dir).unwrap_or_default().into_iter()
  .filter(|p|p["remoteServices"].as_array().is_some_and(|s|s.contains(&json!(service))))
  .map(|p|Route{peer:text(&p,"id").into(),service:service.into(),only:false,relay_url:None,state_dir:state_dir.clone()}).collect()
}
/// The peer must serve this environment before any T3 credential is sent to it.
pub fn verify_environment(url:&str,environment:&str)->Result<()>{
 let http=reqwest::blocking::Client::builder().timeout(Duration::from_secs(5)).build().map_err(|_|"local_error")?;
 let descriptor:Value=http.get(format!("{url}/.well-known/t3/environment")).send().and_then(|r|r.error_for_status()).and_then(|r|r.json()).map_err(|_|"omabridge_route_failed")?;
 if text(&descriptor,"environmentId")!=environment{return Err("t3_environment_mismatch".into())}
 Ok(())
}
/// Environments currently carried by OmaBridge in this process, for the UI.
static ACTIVE:Mutex<Option<HashMap<String,Value>>>=Mutex::new(None);
pub fn set_active(environment:&str,route:Option<(&Route,bool)>){
 if let Ok(mut active)=ACTIVE.lock(){
  let map=active.get_or_insert_with(HashMap::new);
  match route{Some((r,auto))=>{map.insert(environment.into(),json!({"peer":r.peer,"service":r.service,"auto":auto}));},None=>{map.remove(environment);}}
 }
}
fn active()->Value{ACTIVE.lock().ok().and_then(|a|a.clone()).map(|m|json!(m)).unwrap_or_else(||json!({}))}
pub(crate) fn identity_ready(state_dir:&Path)->bool{
 fs::symlink_metadata(state_dir).is_ok_and(|m|m.is_dir()&&m.mode()&0o077==0)&&fs::symlink_metadata(state_dir.join("state.json")).is_ok_and(|m|m.is_file())
}
fn base(bin:&Path,state_dir:&Path,relay:Option<&str>)->Command{
 let mut c=Command::new(bin);c.arg("--state-dir").arg(state_dir).args(["--name","ot3"]);
 if let Some(url)=relay{c.args(["--relay-url",url]);}
 c.stdin(Stdio::null()).stderr(Stdio::null());c
}
/// Public projection of paired peers: never their addresses or any grant secret.
pub fn peers(bin:&Path,state_dir:&Path)->Result<Vec<Value>>{
 if !identity_ready(state_dir){return Err("omabridge_identity_missing".into())}
 let mut cmd=Command::new("timeout");cmd.arg("5s").arg(bin).arg("--state-dir").arg(state_dir).args(["--name","ot3","peers"]).stdin(Stdio::null()).stderr(Stdio::null());
 let out=cmd.output().map_err(|_|"omabridge_unavailable")?;
 if !out.status.success(){return Err("omabridge_unavailable".into())}
 let doc:Value=serde_json::from_slice(&out.stdout).map_err(|_|"omabridge_unavailable")?;
 Ok(project_peers(&doc))
}
pub fn project_peers(doc:&Value)->Vec<Value>{
 doc["peers"].as_array().into_iter().flatten().filter(|p|valid_peer(text(p,"id"))).map(|p|json!({
  "id":p["id"],"name":text(p,"name"),
  "services":p["services"].as_array().cloned().unwrap_or_default().into_iter().filter(|s|s.as_str().is_some_and(valid_service)).collect::<Vec<_>>(),
  "remoteServices":p["remote_services"].as_array().cloned().unwrap_or_default().into_iter().filter(|s|s.as_str().is_some_and(valid_service)).collect::<Vec<_>>(),
 })).collect()
}
/// Parse the forward's readiness line; only a loopback listener is accepted.
pub fn forwarding_url(line:&str,peer:&str,service:&str)->Result<String>{
 let v:Value=serde_json::from_str(line.trim()).map_err(|_|"omabridge_route_failed")?;
 if v["event"]!="forwarding"||v["peer"]!=peer||v["service"]!=service{return Err("omabridge_route_failed".into())}
 let addr:std::net::SocketAddr=text(&v,"listen").parse().map_err(|_|"omabridge_route_failed")?;
 if !addr.ip().is_loopback()||addr.port()==0{return Err("omabridge_route_failed".into())}
 Ok(format!("http://{addr}"))
}
/// Start a private loopback forward tied to this process' lifetime. The child is
/// killed by the caller's tunnel guard, or by PDEATHSIG if ot3 dies first.
pub fn connect(route:&Route,deadline:Duration)->Result<(Child,String)>{
 let bin=binary().ok_or("omabridge_unavailable")?;
 if !identity_ready(&route.state_dir){return Err("omabridge_identity_missing".into())}
 let mut cmd=base(&bin,&route.state_dir,route.relay_url.as_deref());
 cmd.args(["forward","--peer",&route.peer,"--service",&route.service,"--listen","127.0.0.1:0"]).stdout(Stdio::piped());
 let parent=unsafe{libc::getpid()};
 unsafe{cmd.pre_exec(move ||{if libc::prctl(libc::PR_SET_PDEATHSIG,libc::SIGTERM)!=0{return Err(std::io::Error::last_os_error())}if libc::getppid()!=parent{libc::raise(libc::SIGTERM);}Ok(())});}
 let mut child=cmd.spawn().map_err(|_|"omabridge_unavailable")?;
 let stdout=child.stdout.take().ok_or("omabridge_unavailable")?;
 let (tx,rx)=mpsc::channel();
 // Drain stdout for the forward's whole life so it never blocks on a full pipe.
 std::thread::spawn(move||{let mut lines=BufReader::new(stdout).lines();if let Some(Ok(first))=lines.next(){let _=tx.send(first);}for _ in lines{}});
 let started=Instant::now();
 let result=loop{
  match rx.recv_timeout(Duration::from_millis(100)){
   Ok(line)=>break forwarding_url(&line,&route.peer,&route.service),
   Err(mpsc::RecvTimeoutError::Disconnected)=>break Err("omabridge_route_failed".into()),
   Err(mpsc::RecvTimeoutError::Timeout)=>{
    if child.try_wait().ok().flatten().is_some(){break Err("omabridge_route_failed".into())}
    if started.elapsed()>=deadline{break Err("omabridge_route_timeout".into())}
   }
  }
 };
 match result{Ok(url)=>Ok((child,url)),Err(e)=>{let _=child.kill();let _=child.wait();Err(e)}}
}
pub fn snapshot()->Result<Value>{
 let (raw,doc)=load()?;
 let parsed=parse_config(&doc);
 let state_dir=parsed.as_ref().map(Config::state_dir).unwrap_or_else(|_|default_state_dir());
 let bin=binary();
 let peers=match &bin{Some(b)=>peers(b,&state_dir),None=>Err("omabridge_unavailable".into())};
 let mut routes=parsed.as_ref().map(|c|c.routes.iter().map(|(env,r)|(env.clone(),json!({"peer":r.peer,"service":r.service,"mode":if r.only{"only"}else{"prefer"},"relayUrl":r.relay_url}))).collect::<serde_json::Map<_,_>>()).unwrap_or_default();
 for env in parsed.as_ref().map(|c|c.off.clone()).unwrap_or_default(){routes.insert(env,json!({"mode":"off"}));}
 Ok(json!({
  "revision":revision_of(&raw),"available":bin.is_some(),"version":bundled_version(),"identity":identity_ready(&state_dir),"stateDir":state_dir.display().to_string(),
  "default":if parsed.as_ref().is_ok_and(|c|!c.auto){"off"}else{"auto"},
  "peers":peers.as_ref().cloned().unwrap_or_default(),"routes":routes,"active":active(),
   "error":parsed.err().or(peers.err().filter(|e|e!="omabridge_identity_missing"&&e!="omabridge_unavailable")).unwrap_or_default(),
 }))
}
fn write_private(path:&Path,doc:&Value)->Result<()>{
 let parent=path.parent().ok_or("local_error")?;fs::create_dir_all(parent).map_err(|_|"omabridge_routes_write_failed")?;
 let tmp=parent.join(format!(".omabridge-routes-{}",uuid::Uuid::new_v4()));
 let body=serde_json::to_vec_pretty(doc).map_err(|_|"local_error")?;
 let result:Result<()>=(||{let mut f=fs::OpenOptions::new().write(true).create_new(true).mode(0o600).custom_flags(libc::O_NOFOLLOW).open(&tmp).map_err(|_|"omabridge_routes_write_failed")?;f.write_all(&body).and_then(|_|f.sync_all()).map_err(|_|"omabridge_routes_write_failed")?;fs::rename(&tmp,path).map_err(|_|"omabridge_routes_write_failed".to_string())})();
 if result.is_err(){let _=fs::remove_file(tmp);}result
}
/// `read`, `set`, `clear` and `default`, compare-and-swap on the file revision. A
/// pinned route may only name a peer that is paired and currently grants the service.
pub fn execute(input:&Value)->Result<Value>{
 let action=text(input,"action");
 if action=="read"{return snapshot()}
 if let Some(result)=crate::omabridge_gateway::execute(action,input)?{return Ok(result)}
 if !matches!(action,"set"|"clear"|"default"){return Err("invalid_action".into())}
 if input["confirmed"]!=true{return Err("confirmation_required".into())}
 let path=config_path();let parent=path.parent().ok_or("local_error")?;fs::create_dir_all(parent).map_err(|_|"local_error")?;
 let lock=fs::OpenOptions::new().read(true).write(true).create(true).truncate(false).mode(0o600).custom_flags(libc::O_NOFOLLOW).open(parent.join(".omabridge-routes.lock")).map_err(|_|"local_error")?;
 if unsafe{libc::flock(std::os::fd::AsRawFd::as_raw_fd(&lock),libc::LOCK_EX|libc::LOCK_NB)}!=0{return Err("connections_busy".into())}
 let (raw,mut doc)=load()?;
 if revision_of(&raw)!=text(input,"revision"){return Err("catalog_changed_reload".into())}
 let config=parse_config(&doc)?;
 if action=="default"{
  let value=text(input,"value");if !matches!(value,"auto"|"off"){return Err("invalid_command".into())}
  doc["default"]=json!(value);
 } else {
  let env=text(input,"environmentId");if !valid_environment(env){return Err("invalid_command".into())}
  let mode=input["mode"].as_str().unwrap_or("prefer");
  if action=="clear"{
   if let Some(map)=doc["routes"].as_object_mut(){map.remove(env);}
  } else if mode=="off"{
   doc["routes"][env]=json!({"mode":"off"});
  } else {
   let (peer,service)=(text(input,"peer"),input["service"].as_str().unwrap_or("t3"));
   let relay=input["relayUrl"].as_str().filter(|s|!s.is_empty());
   if !valid_peer(peer)||!valid_service(service)||!matches!(mode,"prefer"|"only")||relay.is_some_and(|u|!valid_relay(u)){return Err("invalid_command".into())}
   let bin=binary().ok_or("omabridge_unavailable")?;
   let granted=peers(&bin,&config.state_dir())?.into_iter().any(|p|p["id"]==peer&&p["remoteServices"].as_array().is_some_and(|s|s.contains(&json!(service))));
   if !granted{return Err("omabridge_service_not_granted".into())}
   doc["routes"][env]=json!({"peer":peer,"service":service,"mode":mode,"relayUrl":relay});
  }
 }
 parse_config(&doc)?;
 write_private(&path,&doc)?;
 refresh_connections();snapshot()
}
