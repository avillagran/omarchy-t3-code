//! Automatic T3 sharing between paired OmaBridge devices.
//!
//! When a paired peer grants this device its `t3` service, ot3 adds that peer's
//! T3 Code to T3's own connection catalog, which T3's desktop app on this device
//! reads too. The credential is T3's native one: a one-use pairing that the peer's
//! OmaBridge server issues with the peer's own T3 CLI (`omabridge t3-pair`, only
//! for a device holding its `t3` grant, over the bridge itself: no SSH), exchanged
//! here for a session after the host's descriptor named the same environment. The
//! connection's address is a
//! loopback OmaBridge forward on a fixed port, kept by a systemd user unit, so
//! clients that cannot run OmaBridge reach the host as well.
//!
//! It also links T3 itself to that peer (`t3 environment link`, with another
//! one-use pairing over the same forward) and turns on T3's Linked environments
//! on both ends, so agents here can start and read threads there.
//!
//! Revocation runs both ways: when the peer stops sharing, its route and forward
//! are removed here; on the host, sessions paired for a device it no longer
//! shares T3 with are revoked, as are sessions of devices a bridge invited with
//! full access once that bridge loses `t3` or its consent. A route the user
//! removes is not added again.
use crate::*;
use zeroize::Zeroizing;
use std::{io::Write,net::TcpListener,os::unix::fs::OpenOptionsExt,process::{Command,Stdio},sync::Mutex,time::Instant};

/// Session label prefix on the host, set by OmaBridge; the suffix is the receiving
/// device's OmaBridge id.
pub const LABEL_PREFIX:&str="omabridge:";
/// Same, for a device a bridge invited with full access: `<bridge>:<client>`. That
/// device is no peer here, so it answers for the bridge's standing, not its own.
pub const DELEGATED_PREFIX:&str="omabridge-delegated:";
const RETRY_SECS:u64=600;
const PORT_BASE:u16=47300;
const PORT_SPAN:u16=500;

pub fn config_path()->PathBuf{xdg("XDG_CONFIG_HOME",".config").join("omabridge-shares.json")}
fn unit_dir()->PathBuf{
 std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).filter(|p|p.is_absolute()).unwrap_or_else(||home().join(".config")).join("systemd/user")
}
pub fn unit_name(peer:&str)->String{format!("ot3-omabridge-share-{}.service",&peer[..12])}
fn now()->u64{std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d|d.as_secs()).unwrap_or(0)}

/// `{"version":1,"enabled":true,"skip":[peer],"shares":{peer:{…}},"status":{peer:{…}}}`.
/// A missing file means enabled with nothing shared yet.
pub fn load()->Value{
 let doc=read_json(&config_path()).unwrap_or_else(|_|json!({}));
 normalize(doc)
}
pub fn normalize(mut doc:Value)->Value{
 if !doc.is_object()||doc["version"]!=1{doc=json!({"version":1})}
 if !doc["enabled"].is_boolean(){doc["enabled"]=json!(true)}
 if !doc["skip"].is_array(){doc["skip"]=json!([])}
 for key in ["shares","status"]{if !doc[key].is_object(){doc[key]=json!({})}}
 let valid=|k:&String|omabridge_route::valid_peer(k);
 for key in ["shares","status"]{doc[key].as_object_mut().unwrap().retain(|k,_|valid(k));}
 doc["skip"].as_array_mut().unwrap().retain(|v|v.as_str().is_some_and(omabridge_route::valid_peer));
 doc
}
fn save(doc:&Value)->Result<()>{save_json(&config_path(),doc)}

/// The fixed loopback port for a peer: derived from its id, moved on if taken.
pub fn preferred_port(peer:&str)->u16{
 let digest=Sha256::digest(peer.as_bytes());
 PORT_BASE+(u16::from_be_bytes([digest[0],digest[1]])%PORT_SPAN)
}
fn free(port:u16)->bool{TcpListener::bind(("127.0.0.1",port)).is_ok()}
fn pick_port(peer:&str,taken:&[u16])->Result<u16>{
 let start=preferred_port(peer);
 (0..PORT_SPAN).map(|i|PORT_BASE+(start-PORT_BASE+i)%PORT_SPAN).find(|p|!taken.contains(p)&&free(*p)).ok_or_else(||"share_port_unavailable".into())
}

fn safe_path(p:&Path)->bool{let s=p.to_string_lossy();p.is_absolute()&&!s.is_empty()&&s.bytes().all(|b|b.is_ascii_alphanumeric()||b"/._-+@".contains(&b))}
/// The forward unit: OmaBridge's own `forward`, through the running server when it
/// has `forward-through-server`, restarted if the peer is briefly away. It waits for
/// that server: a forward started without it opens a second endpoint with the same
/// identity, which takes the server's relay connection and cuts its public gateways.
pub fn unit_text(bin:&Path,state_dir:&Path,peer:&str,name:&str,port:u16)->Result<String>{
 if !omabridge_route::valid_peer(peer)||!safe_path(bin)||!safe_path(state_dir){return Err("share_invalid_unit".into())}
 let name:String=name.chars().filter(|c|c.is_alphanumeric()||" ._-".contains(*c)).take(64).collect();
 Ok(format!("[Unit]\nDescription=O3: T3 Code of OmaBridge peer {name} on 127.0.0.1:{port}\nAfter=network-online.target omabridge-serve.service\n\n[Service]\nExecStartPre=-/usr/bin/timeout 30 /usr/bin/sh -c 'until ss -xl | grep -qF {}/control.sock; do sleep 1; done'\nExecStart={} --state-dir {} --name ot3 forward --peer {peer} --service t3 --listen 127.0.0.1:{port}\nRestart=always\nRestartSec=5\n\n[Install]\nWantedBy=default.target\n",state_dir.display(),bin.display(),state_dir.display()))
}
fn systemctl(args:&[&str])->bool{
 Command::new("timeout").arg("20s").arg("systemctl").arg("--user").args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s|s.success())
}
fn install_unit(peer:&str,body:&str)->Result<()>{
 let dir=unit_dir();fs::create_dir_all(&dir).map_err(|_|"share_unit_failed")?;
 let path=dir.join(unit_name(peer));
 if fs::read_to_string(&path).ok().as_deref()!=Some(body){
  let tmp=dir.join(format!(".{}.tmp",unit_name(peer)));
  let mut f=fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o644).custom_flags(libc::O_NOFOLLOW).open(&tmp).map_err(|_|"share_unit_failed")?;
  f.write_all(body.as_bytes()).map_err(|_|"share_unit_failed")?;drop(f);
  fs::rename(&tmp,&path).map_err(|_|"share_unit_failed")?;
  systemctl(&["daemon-reload"]);
 }
 if !systemctl(&["enable","--now",&unit_name(peer)]){return Err("share_unit_failed".into())}
 Ok(())
}
fn remove_unit(peer:&str){
 let name=unit_name(peer);
 systemctl(&["disable","--now",&name]);
 if fs::remove_file(unit_dir().join(&name)).is_ok(){systemctl(&["daemon-reload"]);}
}

/// Paired peers as OmaBridge lists them. `omabridge peers` prints its stored peer,
/// so fields are snake_case, unlike the camelCase `manage` snapshot.
#[derive(Clone,Debug,PartialEq)]
pub struct Peer{pub id:String,pub name:String,pub grants:Vec<String>,pub remote:Vec<String>,pub bridge_consent:bool}
pub fn parse_peers(doc:&Value)->Vec<Peer>{
 let list=|v:&Value|v.as_array().into_iter().flatten().filter_map(|s|s.as_str()).filter(|s|omabridge_route::valid_service(s)).map(str::to_owned).collect::<Vec<_>>();
 doc["peers"].as_array().into_iter().flatten().filter(|p|omabridge_route::valid_peer(text(p,"id"))).map(|p|Peer{
  id:text(p,"id").into(),name:text(p,"name").chars().filter(|c|!c.is_control()).take(64).collect(),
  grants:list(&p["services"]),remote:list(&p["remote_services"]),bridge_consent:p["bridge_consent"]==true,
 }).collect()
}
fn read_peers(bin:&Path,state_dir:&Path)->Result<Vec<Peer>>{
 if !omabridge_route::identity_ready(state_dir){return Err("omabridge_identity_missing".into())}
 let out=Command::new("timeout").arg("8s").arg(bin).arg("--state-dir").arg(state_dir).args(["--name","ot3","peers"]).stdin(Stdio::null()).stderr(Stdio::null()).output().map_err(|_|"omabridge_unavailable")?;
 if !out.status.success(){return Err("omabridge_unavailable".into())}
 Ok(parse_peers(&serde_json::from_slice(&out.stdout).map_err(|_|"omabridge_unavailable")?))
}
/// A one-use pairing from the peer over OmaBridge itself: its server checks this
/// device's authenticated id against its `t3` grant and has its own T3 issue the
/// pairing, labelled with that id so the host can revoke it. No SSH.
fn mint(peer:&Peer,environment:&str,bin:&Path,state_dir:&Path)->Result<(Zeroizing<String>,String)>{
 let out=Command::new("timeout").arg("70s").arg(bin).arg("--state-dir").arg(state_dir).args(["--name","ot3","t3-pair","--peer",&peer.id])
  .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).output().map_err(|_|"omabridge_unavailable")?;
 let stdout=Zeroizing::new(out.stdout);
 if !out.status.success(){
  let error=String::from_utf8_lossy(&out.stderr);
  return Err(["omabridge_t3_pair_refused","omabridge_service_not_granted","omabridge_peer_unreachable","omabridge_peer_not_found"].into_iter().find(|c|error.contains(c)).unwrap_or("omabridge_t3_pair_unavailable").into())
 }
 let doc:Value=serde_json::from_slice(&stdout).map_err(|_|"omabridge_t3_pair_unavailable")?;
 if text(&doc,"environmentId")!=environment{return Err("t3_environment_mismatch".into())}
 let label=text(&doc,"label");
 let credential=Zeroizing::new(text(&doc,"credential").to_string());
 if credential.is_empty()||!label.strip_prefix(LABEL_PREFIX).is_some_and(omabridge_route::valid_peer){return Err("omabridge_t3_pair_unavailable".into())}
 Ok((credential,label.to_string()))
}
fn wait_descriptor(base:&str)->Result<Value>{
 let options=protocol::ConnectionOptions::default().with_request_timeout(Duration::from_secs(4));
 let deadline=Instant::now()+Duration::from_secs(25);
 loop{
  match protocol::fetch_descriptor_with(base,&options){
   Ok(d)=>return Ok(d),
   Err(_) if Instant::now()<deadline=>std::thread::sleep(Duration::from_millis(500)),
   Err(_)=>return Err("share_forward_unreachable".into()),
  }
 }
}
/// The catalog revision, an environment's routes and its saved name.
fn catalog_routes(environment:&str)->Result<(String,Vec<Value>,String)>{
 let view=connections::manage_connections(&json!({"action":"read"}))?;
 let env=view["environments"].as_array().into_iter().flatten().find(|e|e["id"]==environment);
 Ok((text(&view,"revision").into(),env.and_then(|e|e["routes"].as_array().cloned()).unwrap_or_default(),env.map(|e|text(e,"name").to_string()).unwrap_or_default()))
}
pub fn connection_id(environment:&str,peer:&str)->String{format!("bearer:{environment}:omabridge-{}",&peer[..16])}

/// Add one peer's T3 Code: forward, descriptor, native pairing, catalog.
fn enroll(peer:&Peer,taken:&[u16],bin:&Path,state_dir:&Path)->Result<Value>{
 let port=pick_port(&peer.id,taken)?;
 install_unit(&peer.id,&unit_text(bin,state_dir,&peer.id,&peer.name,port)?)?;
 let base=format!("http://127.0.0.1:{port}");
 let result=(||{
  let descriptor=wait_descriptor(&base)?;
  let environment=text(&descriptor,"environmentId").to_string();
  uuid::Uuid::parse_str(&environment).map_err(|_|"invalid_t3_environment_id")?;
  if connections::native_primary().is_some_and(|p|p["environmentId"]==environment.as_str()){return Err("share_is_this_device".to_string())}
  let id=connection_id(&environment,&peer.id);
  let (revision,routes,saved)=catalog_routes(&environment)?;
  if !routes.iter().any(|r|r["connectionId"]==id.as_str()){
   let (code,label)=mint(peer,&environment,bin,state_dir)?;
   // An environment already in the catalog keeps its name; this adds a route.
   let name=if !saved.is_empty(){saved}else if !peer.name.is_empty(){peer.name.clone()}else{text(&descriptor,"label").to_string()};
   connections::manage_connections(&json!({"action":"save-route","revision":revision,"kind":"bearer","url":base,"pairingCode":code.as_str(),
    "environmentId":environment,"label":name,"connectionId":id,"clientLabel":label}))?;
  }
  Ok(json!({"environmentId":environment,"connectionId":id,"port":port,"name":peer.name}))
 })();
 if result.is_err(){remove_unit(&peer.id)}
 result
}
fn remove_route(share:&Value){
 let (env,id)=(text(share,"environmentId"),text(share,"connectionId"));
 for _ in 0..3{
  let Ok((revision,routes,_))=catalog_routes(env) else {return};
  if !routes.iter().any(|r|r["connectionId"]==id){return}
  let action=if routes.len()==1{"remove"}else{"remove-route"};
  match connections::manage_connections(&json!({"action":action,"revision":revision,"environmentId":env,"connectionId":id,"confirmed":true})){
   Err(e) if e=="catalog_changed_reload"=>continue,_=>return,
  }
 }
}

/// What the receiving side should do for each peer, from OmaBridge's grants, the
/// saved shares and which shared routes are still in the catalog.
#[derive(Debug,PartialEq)]
pub enum Step{Enroll(String),Keep(String),Withdraw(String),UserRemoved(String)}
pub fn plan(doc:&Value,peers:&[Peer],in_catalog:&dyn Fn(&Value)->bool,now:u64)->Vec<Step>{
 let mut out=vec![];
 let skip=|id:&str|doc["skip"].as_array().is_some_and(|s|s.contains(&json!(id)));
 for (id,share) in doc["shares"].as_object().into_iter().flatten(){
  let shared=peers.iter().any(|p|&p.id==id&&p.remote.iter().any(|s|s=="t3"));
  out.push(if !shared{Step::Withdraw(id.clone())}else if !in_catalog(share){Step::UserRemoved(id.clone())}else{Step::Keep(id.clone())});
 }
 if doc["enabled"]!=true{return out}
 for p in peers.iter().filter(|p|p.remote.iter().any(|s|s=="t3")){
  if doc["shares"].get(&p.id).is_some()||skip(&p.id){continue}
  let last=&doc["status"][&p.id];
  if last["state"]=="error"&&now.saturating_sub(last["at"].as_u64().unwrap_or(0))<RETRY_SECS&&last["grants"]==json!(p.remote){continue}
  out.push(Step::Enroll(p.id.clone()));
 }
 out
}

/// Host side: sessions paired for a device this host no longer shares T3 with, and
/// sessions a bridge delegated once it lost `t3` or `bridge_consent`. Only the bridge
/// is read from a delegated label: its client part is the bridge's word.
pub fn revocable(sessions:&Value,peers:&[Peer])->Vec<String>{
 let holds_t3=|p:&Peer|p.grants.iter().any(|g|g=="t3");
 sessions.as_array().into_iter().flatten().filter_map(|s|{
  let label=text(&s["client"],"label");
  let shared=if let Some(device)=label.strip_prefix(LABEL_PREFIX){
   if !omabridge_route::valid_peer(device){return None}
   peers.iter().any(|p|p.id==device&&holds_t3(p))
  }else{
   let (bridge,_)=label.strip_prefix(DELEGATED_PREFIX)?.split_once(':')?;
   if !omabridge_route::valid_peer(bridge){return None}
   peers.iter().any(|p|p.id==bridge&&p.bridge_consent&&holds_t3(p))
  };
  let id=text(s,"sessionId");
  (!shared&&!id.is_empty()&&id.len()<=128&&id.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'-')).then(||id.to_string())
 }).collect()
}
fn local_cli()->Option<PathBuf>{
 [connections::t3_home().join("bin/t3"),home().join(".local/bin/t3")].into_iter().find(|p|p.is_file())
}
fn revoke_withdrawn(peers:&[Peer]){
 let Some(cli)=local_cli() else {return};
 let base=connections::t3_home();
 let Ok(out)=Command::new("timeout").arg("15s").arg(&cli).args(["auth","session","list","--json","--base-dir"]).arg(&base).stdin(Stdio::null()).stderr(Stdio::null()).output() else {return};
 if !out.status.success(){return}
 let Ok(sessions)=serde_json::from_slice::<Value>(&out.stdout) else {return};
 for id in revocable(&sessions,peers){
  let _=Command::new("timeout").arg("15s").arg(&cli).args(["auth","session","revoke","--base-dir"]).arg(&base).arg(&id).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status();
 }
}

/// T3's own switch for agents working on other machines (Settings → Labs →
/// Linked environments). Both ends need it: this side to link, the host to
/// answer a linked agent. Sharing T3 with a paired device is that consent.
fn ensure_linked_environments(){
 if read_json(&connections::t3_home().join("userdata/settings.json")).is_ok_and(|s|s["linkedEnvironmentsEnabled"]==true){return}
 if let Ok(c)=connections::local_environment().and_then(|e|e.connection(true)){
  let _=write::update_settings(&c,&json!({"patch":{"linkedEnvironmentsEnabled":true}}));
 }
}
fn t3_environment(cli:&Path,args:&[&str])->Option<std::process::Output>{
 Command::new("timeout").arg("40s").arg(cli).arg("environment").args(args).arg("--base-dir").arg(connections::t3_home()).stdin(Stdio::null()).stderr(Stdio::piped()).stdout(Stdio::piped()).output().ok()
}
/// Whether this T3 already has a live link to `environment`.
pub fn has_link(links:&Value,environment:&str)->bool{
 links.as_array().into_iter().flatten().any(|l|l["environmentId"]==environment&&l["status"]!="expired")
}
/// T3's own link to a shared peer, so agents here can start and read threads
/// there, through the same loopback forward and a fresh one-use pairing.
fn link_agents(peer:&Peer,share:&Value,bin:&Path,state_dir:&Path)->Result<()>{
 let cli=local_cli().ok_or("t3_cli_missing")?;
 let environment=text(share,"environmentId");
 let port=share["port"].as_u64().ok_or("share_invalid")?;
 let listed=t3_environment(&cli,&["list","--json"]).filter(|o|o.status.success()).ok_or("t3_link_unavailable")?;
 let links:Value=serde_json::from_slice(&listed.stdout).map_err(|_|"t3_link_unavailable")?;
 if has_link(&links,environment){return Ok(())}
 let url=format!("http://127.0.0.1:{port}");
 // A host whose T3 predates linking would refuse it: no pairing is spent on it.
 let descriptor=wait_descriptor(&url)?;
 if !links_supported(&descriptor){return Err("t3_link_peer_outdated".into())}
 let (code,_)=mint(peer,environment,bin,state_dir)?;
 // The CLI writes T3's database next to the running server: a busy moment is retried.
 for attempt in 0..3{
  let out=t3_environment(&cli,&["link",&url,"--pairing-code",code.as_str(),"--access","full-access"]).ok_or("t3_link_unavailable")?;
  if out.status.success(){return Ok(())}
  if attempt==2||!String::from_utf8_lossy(&out.stderr).contains("database is locked"){break}
  std::thread::sleep(Duration::from_secs(2));
 }
 Err("t3_link_failed".into())
}
/// T3 links only to a host that keeps a linked agent's limits.
pub fn links_supported(descriptor:&Value)->bool{
 let c=&descriptor["capabilities"];
 ["peerLinks","mcpModeLimitHeader","linkFence"].iter().all(|k|c[*k]==true)
}
fn unlink_agents(share:&Value){
 let environment=text(share,"environmentId");
 if environment.is_empty()||uuid::Uuid::parse_str(environment).is_err(){return}
 if let Some(cli)=local_cli(){let _=t3_environment(&cli,&["unlink",environment]);}
}

static RUNNING:Mutex<()>=Mutex::new(());
/// One reconciliation pass. Any OmaBridge read failure changes nothing.
pub fn reconcile()->Result<Value>{
 let _guard=RUNNING.lock().map_err(|_|"share_busy")?;
 let bin=omabridge_route::binary().ok_or("omabridge_unavailable")?;
 let state_dir=omabridge_route::config().map(|c|c.state_dir()).unwrap_or_else(|_|omabridge_route::default_state_dir());
 let peers=read_peers(&bin,&state_dir)?;
 revoke_withdrawn(&peers);
 let mut doc=load();
 let catalog=connections::manage_connections(&json!({"action":"read"})).ok();
 let in_catalog=|share:&Value|catalog.as_ref().is_none_or(|v|v["environments"].as_array().into_iter().flatten().flat_map(|e|e["routes"].as_array().cloned().unwrap_or_default()).any(|r|r["connectionId"]==share["connectionId"]));
 let steps=plan(&doc,&peers,&in_catalog,now());
 if doc["enabled"]==true&&peers.iter().any(|p|p.grants.iter().chain(&p.remote).any(|s|s=="t3")){ensure_linked_environments()}
 for step in steps{
  match step{
   Step::Keep(id)=>{
    let unit=unit_name(&id);if !systemctl(&["is-active","--quiet",&unit]){systemctl(&["start",&unit]);}
    if let Some(peer)=peers.iter().find(|p|p.id==id){
     let agents=link_agents(peer,&doc["shares"][&id],&bin,&state_dir);
     doc["status"][&id]["agents"]=match agents{Ok(())=>json!("linked"),Err(e)=>json!(e)};
    }
   }
   Step::Withdraw(id)=>{unlink_agents(&doc["shares"][&id]);remove_route(&doc["shares"][&id]);remove_unit(&id);doc["shares"].as_object_mut().unwrap().remove(&id);doc["status"][&id]=json!({"state":"withdrawn","at":now()});}
   Step::UserRemoved(id)=>{unlink_agents(&doc["shares"][&id]);remove_unit(&id);doc["shares"].as_object_mut().unwrap().remove(&id);doc["skip"].as_array_mut().unwrap().push(json!(id));doc["status"][&id]=json!({"state":"removed","at":now()});}
   Step::Enroll(id)=>{
    let Some(peer)=peers.iter().find(|p|p.id==id) else {continue};
    let taken:Vec<u16>=doc["shares"].as_object().into_iter().flatten().filter_map(|(_,s)|s["port"].as_u64()).map(|p|p as u16).collect();
    let result=enroll(peer,&taken,&bin,&state_dir);
    match result{
     Ok(share)=>{
      let agents=match link_agents(peer,&share,&bin,&state_dir){Ok(())=>json!("linked"),Err(e)=>json!(e)};
      doc["status"][&id]=json!({"state":"shared","at":now(),"environmentId":share["environmentId"],"agents":agents});doc["shares"][&id]=share;
     }
     Err(e)=>{doc["status"][&id]=json!({"state":"error","error":e,"at":now(),"grants":peer.remote});}
    }
   }
  }
 }
 save(&doc)?;
 Ok(snapshot(&doc,&peers))
}
/// Public view for the UI: names, states and error codes, never credentials.
pub fn snapshot(doc:&Value,peers:&[Peer])->Value{
 let rows=peers.iter().filter(|p|p.remote.iter().any(|s|s=="t3")||doc["shares"].get(&p.id).is_some()).map(|p|{
  let share=&doc["shares"][&p.id];let status=&doc["status"][&p.id];
  json!({"peer":p.id,"name":p.name,"shared":share.is_object(),"port":share["port"],"environmentId":share["environmentId"],
   "state":if share.is_object(){"shared"}else if doc["skip"].as_array().is_some_and(|s|s.contains(&json!(p.id))){"skipped"}else{status["state"].as_str().unwrap_or("pending")},
   "error":status["error"]})
 }).collect::<Vec<_>>();
 json!({"enabled":doc["enabled"],"peers":rows})
}
/// Per-peer sharing state for Preferences, merged into the OmaBridge routing
/// snapshot (which already lists the peers): no peer list of its own.
pub fn public_state()->Value{
 let doc=load();
 let mut peers=serde_json::Map::new();
 for (id,status) in doc["status"].as_object().into_iter().flatten(){peers.insert(id.clone(),json!({"state":status["state"],"error":status["error"]}));}
 for id in doc["skip"].as_array().into_iter().flatten().filter_map(|v|v.as_str()){peers.insert(id.into(),json!({"state":"skipped"}));}
 for (id,share) in doc["shares"].as_object().into_iter().flatten(){peers.insert(id.clone(),json!({"state":"shared","port":share["port"],"environmentId":share["environmentId"]}));}
 json!({"enabled":doc["enabled"],"peers":peers})
}
pub fn read()->Result<Value>{
 let doc=load();
 let peers=omabridge_route::binary().and_then(|bin|{let state=omabridge_route::config().map(|c|c.state_dir()).unwrap_or_else(|_|omabridge_route::default_state_dir());read_peers(&bin,&state).ok()}).unwrap_or_default();
 Ok(snapshot(&doc,&peers))
}
/// `read`, `run`, `enabled` (`{"enabled":bool}`) and `retry` (`{"peer"}`: share a
/// skipped or failed peer again). Changes need `"confirmed":true`.
pub fn execute(input:&Value)->Result<Value>{
 match text(input,"action"){
  "read"=>read(),
  "run"=>reconcile(),
  action@("enabled"|"retry")=>{
   if input["confirmed"]!=true{return Err("confirmation_required".into())}
   let mut doc=load();
   if action=="enabled"{doc["enabled"]=json!(input["enabled"].as_bool().ok_or("invalid_command")?)}
   else{
    let peer=text(input,"peer");if !omabridge_route::valid_peer(peer){return Err("invalid_command".into())}
    doc["skip"].as_array_mut().unwrap().retain(|v|v!=peer);
    if let Some(s)=doc["status"].as_object_mut(){s.remove(peer);}
   }
   save(&doc)?;
   std::thread::spawn(||{let _=reconcile();});
   read()
  }
  _=>Err("invalid_action".into()),
 }
}
/// Background loop for the running app: on OmaBridge or catalog changes, and
/// every five minutes to restart forwards and retry failures.
pub fn start(){
 std::thread::spawn(||{
  let mut last:Option<(String,Instant)>=None;
  loop{
   let signature=format!("{:x}",Sha256::digest([omabridge_route::config_bytes(),fs::read(config_path()).unwrap_or_default(),fs::read(connections::t3_home().join("userdata/connection-catalog.json")).unwrap_or_default()].concat()));
   let due=last.as_ref().is_none_or(|(s,at)|*s!=signature||at.elapsed()>=Duration::from_secs(300));
   if due{
    let _=reconcile();
    let after=format!("{:x}",Sha256::digest([omabridge_route::config_bytes(),fs::read(config_path()).unwrap_or_default(),fs::read(connections::t3_home().join("userdata/connection-catalog.json")).unwrap_or_default()].concat()));
    last=Some((after,Instant::now()));
   }
   std::thread::sleep(Duration::from_secs(30));
  }
 });
}

#[cfg(test)]
mod tests{
 use super::*;
 const A:&str="75ebf82c1c0f3c6df7c6ccab7ce79e0c5c957e2027d9ea8e144a6966b43fe764";
 const B:&str="0f6da8065c780e468ec60ecb8c9640717766692666135a81e934c246c7f42329";
 fn peer(id:&str,remote:&[&str],grants:&[&str])->Peer{Peer{id:id.into(),name:"NODE".into(),grants:grants.iter().map(|s|s.to_string()).collect(),remote:remote.iter().map(|s|s.to_string()).collect(),bridge_consent:false}}
 fn bridge(id:&str,grants:&[&str],consent:bool)->Peer{Peer{bridge_consent:consent,..peer(id,&[],grants)}}
 #[test]
 fn peers_keep_valid_ids_names_and_services(){
  let doc=json!({"peers":[{"id":A,"name":"mbp\u{7}","services":["ssh","t3"],"remote_services":["t3","bad;x"],"address":{"addrs":[{"Relay":"https://relay.example/"},{"Ip":"192.0.2.7:48702"}]}},{"id":"short"}]});
  let peers=parse_peers(&doc);
  assert_eq!(peers.len(),1);assert_eq!(peers[0].name,"mbp");assert_eq!(peers[0].remote,vec!["t3"]);
  assert!(!peers[0].bridge_consent,"an OmaBridge that does not list consent gives none");
  // The stored peer `omabridge peers` prints, not the camelCase `manage` snapshot.
  assert!(parse_peers(&json!({"peers":[{"id":A,"name":"h","services":["t3"],"bridge_consent":true}]}))[0].bridge_consent);
  assert!(!parse_peers(&json!({"peers":[{"id":A,"name":"h","services":["t3"],"bridgeConsent":true}]}))[0].bridge_consent);
 }
 #[test]
 fn plan_enrolls_withdraws_and_respects_removal(){
  let shared=json!({"connectionId":"c"});
  let doc=normalize(json!({"version":1,"shares":{A:shared}}));
  let yes=|_:&Value|true;let no=|_:&Value|false;
  assert_eq!(plan(&doc,&[peer(A,&["t3"],&[])],&yes,0),vec![Step::Keep(A.into())]);
  assert_eq!(plan(&doc,&[peer(A,&["ssh"],&[])],&yes,0),vec![Step::Withdraw(A.into())]);
  assert_eq!(plan(&doc,&[],&yes,0),vec![Step::Withdraw(A.into())]);
  assert_eq!(plan(&doc,&[peer(A,&["t3"],&[])],&no,0),vec![Step::UserRemoved(A.into())]);
  let fresh=normalize(json!({}));
  assert_eq!(plan(&fresh,&[peer(B,&["t3","ssh"],&[]),peer(A,&["ssh"],&[])],&yes,0),vec![Step::Enroll(B.into())]);
  let skipped=normalize(json!({"version":1,"skip":[B]}));
  assert!(plan(&skipped,&[peer(B,&["t3"],&[])],&yes,0).is_empty());
  let off=normalize(json!({"version":1,"enabled":false,"shares":{A:shared}}));
  assert_eq!(plan(&off,&[peer(A,&["t3"],&[]),peer(B,&["t3"],&[])],&yes,0),vec![Step::Keep(A.into())]);
  // A failure waits before retrying, unless the peer's grants changed.
  let failed=normalize(json!({"version":1,"status":{B:{"state":"error","at":1000,"grants":["t3"]}}}));
  assert!(plan(&failed,&[peer(B,&["t3"],&[])],&yes,1000+RETRY_SECS-1).is_empty());
  assert_eq!(plan(&failed,&[peer(B,&["t3","ssh"],&[])],&yes,1001),vec![Step::Enroll(B.into())]);
  assert_eq!(plan(&failed,&[peer(B,&["t3"],&[])],&yes,1000+RETRY_SECS),vec![Step::Enroll(B.into())]);
 }
 #[test]
 fn host_revokes_only_withdrawn_share_sessions(){
  let sessions=json!([
   {"sessionId":"s-1","client":{"label":format!("{LABEL_PREFIX}{A}")}},
   {"sessionId":"s-2","client":{"label":format!("{LABEL_PREFIX}{B}")}},
   {"sessionId":"s-3","client":{"label":"omarchy-t3-code"}},
   {"sessionId":"s;4","client":{"label":format!("{LABEL_PREFIX}{B}")}},
   {"sessionId":"s-5","client":{"label":format!("{LABEL_PREFIX}nothex")}}]);
  assert_eq!(revocable(&sessions,&[peer(A,&[],&["t3","ssh"]),peer(B,&[],&["ssh"])]),vec!["s-2"]);
 }
 #[test]
 fn host_revokes_delegated_sessions_of_a_withdrawn_bridge(){
  let session=|id:&str,bridge:&str,client:&str|json!({"sessionId":id,"client":{"label":format!("{DELEGATED_PREFIX}{bridge}:{client}")}});
  let sessions=json!([session("d-1",A,B),session("d-2",A,"odd client"),session("d-3",B,A)]);
  // Consent stands: the phone is no peer here, and that does not matter.
  assert!(revocable(&sessions,&[bridge(A,&["t3"],true),bridge(B,&["t3"],true)]).is_empty());
  // Only the bridge that lost its standing, whatever follows its id.
  assert_eq!(revocable(&sessions,&[bridge(A,&["t3"],false),bridge(B,&["t3"],true)]),vec!["d-1","d-2"]);
  assert_eq!(revocable(&sessions,&[bridge(A,&["ssh"],true),bridge(B,&["t3"],true)]),vec!["d-1","d-2"]);
  assert_eq!(revocable(&sessions,&[bridge(B,&["t3"],true)]),vec!["d-1","d-2"]);
  // Labels it cannot attribute to a bridge are left alone; ids stay checked.
  let odd=json!([session("d-4","nothex",A),{"sessionId":"d-5","client":{"label":format!("{DELEGATED_PREFIX}{A}")}},session("d;6",A,B)]);
  assert!(revocable(&odd,&[]).is_empty());
  // The two kinds are judged apart: a plain session needs no consent, and a
  // consented bridge's own plain session goes with its `t3` grant, not with this.
  let plain=json!([{"sessionId":"p-1","client":{"label":format!("{LABEL_PREFIX}{A}")}},session("d-1",A,B)]);
  assert_eq!(revocable(&plain,&[bridge(A,&["t3"],false)]),vec!["d-1"]);
  assert_eq!(revocable(&plain,&[]),vec!["p-1","d-1"]);
 }
 #[test]
 fn links_need_a_current_host(){
  assert!(links_supported(&json!({"capabilities":{"peerLinks":true,"mcpModeLimitHeader":true,"linkFence":true}})));
  assert!(!links_supported(&json!({"capabilities":{"peerLinks":true,"mcpModeLimitHeader":true}})));
  assert!(!links_supported(&json!({})));
 }
 #[test]
 fn live_links_only(){
  let links=json!([{"environmentId":"e1","status":"reachable"},{"environmentId":"e2","status":"expired"},{"environmentId":"e3","status":"unreachable"}]);
  assert!(has_link(&links,"e1"));assert!(!has_link(&links,"e2"));assert!(has_link(&links,"e3"));assert!(!has_link(&links,"e4"));assert!(!has_link(&json!(null),"e1"));
 }
 #[test]
 fn ports_and_units(){
  let p=preferred_port(A);assert!((PORT_BASE..PORT_BASE+PORT_SPAN).contains(&p));assert_eq!(p,preferred_port(A));
  let unit=unit_text(Path::new("/opt/ot3/bin/omabridge"),Path::new("/home/u/.local/share/omabridge"),A,"mbp m2\n[Service]",p).unwrap();
  assert!(unit.contains(&format!("forward --peer {A} --service t3 --listen 127.0.0.1:{p}")));
  assert_eq!(unit.matches("[Service]").count(),1);
  assert!(unit.contains("grep -qF /home/u/.local/share/omabridge/control.sock"));
  assert!(unit_text(Path::new("/opt/a b/omabridge"),Path::new("/s"),A,"x",p).is_err());
  assert!(unit_text(Path::new("/opt/omabridge"),Path::new("/s"),"abc","x",p).is_err());
  assert_eq!(unit_name(A),"ot3-omabridge-share-75ebf82c1c0f.service");
  assert_eq!(connection_id("e",A),"bearer:e:omabridge-75ebf82c1c0f3c6d");
 }
}
