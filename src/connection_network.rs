//! Native desktop exposure preferences and T3's service launch configuration.
//! T3 owns listener startup, Tailscale Serve and orderly shutdown; we configure
//! its documented environment and use the existing ownership-checked lifecycle.
use crate::*;
use std::{io::Write,os::unix::fs::OpenOptionsExt,process::{Command,Stdio}};
fn settings_path()->PathBuf{t3_home().join("userdata/desktop-settings.json")}
fn override_path()->PathBuf{home().join(".config/systemd/user/t3code.service.d/ot3-network.conf")}
fn bytes(path:&Path)->Result<Option<Vec<u8>>>{match fs::read(path){Ok(v)=>Ok(Some(v)),Err(e) if e.kind()==std::io::ErrorKind::NotFound=>Ok(None),Err(_)=>Err("network_settings_read_failed".into())}}
fn revision()->Result<String>{Ok(format!("{:x}",Sha256::digest(serde_json::to_vec(&json!([bytes(&settings_path())?,bytes(&override_path())?])).map_err(|_|"local_error")?)))}
fn command(program:&str,args:&[&str],seconds:&str)->Result<String>{
 let out=Command::new("timeout").arg(seconds).arg(program).args(args).stdin(Stdio::null()).output().map_err(|_|"network_command_failed")?;
 if !out.status.success(){return Err("network_command_failed".into())}
 String::from_utf8(out.stdout).map_err(|_|"malformed".into())
}
fn tailscale()->Value{
 match command("tailscale",&["status","--json"],"3s").and_then(|s|serde_json::from_str::<Value>(&s).map_err(|_|"malformed".into())){
  Ok(v)=>{let serve=command("tailscale",&["serve","status","--json"],"3s").ok().and_then(|s|serde_json::from_str::<Value>(&s).ok());json!({"available":true,"running":v["BackendState"]=="Running","dnsName":text(&v["Self"],"DNSName").trim_end_matches('.'),"serve":serve,"error":""})},
  Err(_)=>json!({"available":false,"running":false,"dnsName":"","serve":null,"error":"tailscale_unavailable"})
 }
}
fn runtime_environment(pid:u64)->Value{
 let mut result=json!({});if pid==0{return result}
 if let Ok(raw)=fs::read(format!("/proc/{pid}/environ")){
  for entry in raw.split(|b|*b==0){let Some(pos)=entry.iter().position(|b|*b==b'=')else{continue};let Ok(key)=std::str::from_utf8(&entry[..pos])else{continue};if matches!(key,"T3CODE_HOST"|"T3CODE_TAILSCALE_SERVE"|"T3CODE_TAILSCALE_SERVE_PORT"){if let Ok(value)=std::str::from_utf8(&entry[pos+1..]){result[key]=json!(value)}}}
 }result
}
/// Match only this T3 server's HTTPS proxy, never another application on the tailnet.
pub fn serve_mapping(serve:&Value,dns:&str,port:u16,local_port:u16)->(&'static str,Option<String>){
 let authority=format!("{}:{port}",dns.trim_end_matches('.'));
 let site=&serve["Web"][&authority];
 if site.is_null(){return ("absent",None)}
 let target=site["Handlers"]["/"]["Proxy"].as_str().unwrap_or("");
 let expected=[format!("http://127.0.0.1:{local_port}"),format!("http://localhost:{local_port}"),format!("http://[::1]:{local_port}")];
 if !expected.iter().any(|v|v==target.trim_end_matches('/')){return ("conflict",None)}
 let url=if port==443{format!("https://{}/",dns.trim_end_matches('.'))}else{format!("https://{}:{port}/",dns.trim_end_matches('.'))};("active",Some(url))
}
pub fn snapshot()->Result<Value>{
 let local=backend_service::status();let runtime=read_json(&t3_home().join("userdata/server-runtime.json")).unwrap_or(json!({}));
 let settings=match bytes(&settings_path())?{Some(raw)=>serde_json::from_slice::<Value>(&raw).map_err(|_|"malformed")?,None=>json!({})};
 let env=runtime_environment(runtime["pid"].as_u64().unwrap_or(0));
 let host=env["T3CODE_HOST"].as_str().or(runtime["host"].as_str());
 let network=if local["owner"]=="gui"{settings["serverExposureMode"]=="network-accessible"}else{host.is_none_or(|h|!matches!(h,"localhost"|"127.0.0.1"|"::1"))};
 let mode=if network{"network-accessible"}else{"local-only"};
 let port=runtime["port"].as_u64().filter(|p|*p>0&&*p<=65535).unwrap_or(3773) as u16;
 let enabled=env["T3CODE_TAILSCALE_SERVE"].as_str().map(|v|v=="true").unwrap_or(settings["tailscaleServeEnabled"]==true);
 let serve_port=env["T3CODE_TAILSCALE_SERVE_PORT"].as_str().and_then(|v|v.parse::<u16>().ok()).filter(|p|*p>0).or(settings["tailscaleServePort"].as_u64().filter(|p|*p>0&&*p<=65535).map(|p|p as u16)).unwrap_or(443);
 let tail=tailscale();let(mapping,url)=serve_mapping(&tail["serve"],text(&tail,"dnsName"),serve_port,port);
 let mut endpoints=vec![json!({"id":"loopback","name":"This machine","url":format!("http://127.0.0.1:{port}"),"qrShareable":false})];
 if network {
  if let Ok(raw)=command("ip",&["-j","-4","address","show","scope","global"],"2s"){
   if let Ok(interfaces)=serde_json::from_str::<Value>(&raw){for interface in interfaces.as_array().into_iter().flatten(){for addr in interface["addr_info"].as_array().into_iter().flatten(){let ip=text(addr,"local");if ip.parse::<std::net::Ipv4Addr>().is_ok_and(|a|!a.is_loopback()&&!a.is_link_local()&&!a.is_unspecified()) {endpoints.push(json!({"id":format!("lan:{ip}"),"name":if ip.starts_with("100."){"Tailscale"}else{"Local network"},"url":format!("http://{ip}:{port}"),"qrShareable":true}));}}}}
  }
 }
 if let Some(url)=url{endpoints.push(json!({"id":"tailscale-https","name":"Tailscale HTTPS","url":url,"qrShareable":true}));}
 let manageable=local["canControl"]==true&&local["installed"]==true&&matches!(text(&local,"owner"),"headless"|"stopped");
 Ok(json!({"revision":revision()?,"local":local,"canManage":manageable,"mode":mode,"desiredMode":settings["serverExposureMode"].as_str().unwrap_or(mode),"tailscaleServeEnabled":enabled,"tailscaleServePort":serve_port,"tailscale":tail,"tailscaleMapping":mapping,"endpoints":endpoints,"warning":if enabled&&mapping!="active"{"tailscale_serve_not_active"}else{""}}))
}
pub fn launch_override(mode:&str,enabled:bool,port:u16)->Result<String>{
 let host=match mode{"local-only"=>"127.0.0.1","network-accessible"=>"0.0.0.0",_=>return Err("invalid_command".into())};if port==0{return Err("invalid_command".into())}
 Ok(format!("# T3 Code native launch configuration, managed from ot3 Connections.\n[Service]\nEnvironment=T3CODE_HOST={host}\nEnvironment=T3CODE_TAILSCALE_SERVE={enabled}\nEnvironment=T3CODE_TAILSCALE_SERVE_PORT={port}\n"))
}
fn atomic_bytes(path:&Path,value:&[u8])->Result<()>{
 let parent=path.parent().ok_or("local_error")?;fs::create_dir_all(parent).map_err(|_|"network_settings_write_failed")?;
 let tmp=parent.join(format!(".ot3-network-{}",uuid::Uuid::new_v4()));
 let result:Result<()>=(||{let mut f=fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&tmp).map_err(|_|"network_settings_write_failed")?;f.write_all(value).and_then(|_|f.sync_all()).map_err(|_|"network_settings_write_failed")?;fs::rename(&tmp,path).map_err(|_|"network_settings_write_failed".to_string())})();if result.is_err(){let _=fs::remove_file(tmp);}result
}
fn restore(path:&Path,old:&Option<Vec<u8>>)->Result<()>{if let Some(raw)=old{atomic_bytes(path,raw)}else{match fs::remove_file(path){Ok(())=>Ok(()),Err(e)if e.kind()==std::io::ErrorKind::NotFound=>Ok(()),Err(_)=>Err("network_settings_write_failed".into())}}}
pub fn execute(input:&Value)->Result<Value>{
 if input["action"]=="read"{return snapshot()}
 if input["action"]!="apply"{return Err("invalid_action".into())}
 if input["confirmed"]!=true{return Err("confirmation_required".into())}
 let dir=t3_home().join("userdata");fs::create_dir_all(&dir).map_err(|_|"local_error")?;
 let lock=fs::OpenOptions::new().read(true).write(true).create(true).truncate(false).mode(0o600).custom_flags(libc::O_NOFOLLOW).open(dir.join(".ot3-connection-catalog.lock")).map_err(|_|"local_error")?;
 if unsafe{libc::flock(std::os::fd::AsRawFd::as_raw_fd(&lock),libc::LOCK_EX|libc::LOCK_NB)}!=0{return Err("connections_busy".into())}
 if revision()?!=text(input,"revision"){return Err("catalog_changed_reload".into())}
 let status=backend_service::status();if status["canControl"]!=true||status["installed"]!=true||!matches!(text(&status,"owner"),"headless"|"stopped"){return Err("backend_owned_by_gui".into())}
 let enabled=input["tailscaleServeEnabled"].as_bool().ok_or("invalid_command")?;let port=input["tailscaleServePort"].as_u64().filter(|p|*p>0&&*p<=65535).ok_or("invalid_command")? as u16;
 let config=launch_override(text(input,"mode"),enabled,port)?;
 if enabled{
  let tail=tailscale();if tail["running"]!=true||text(&tail,"dnsName").is_empty()||tail["serve"].is_null(){return Err("tailscale_unavailable".into())}
  let runtime=read_json(&dir.join("server-runtime.json")).unwrap_or(json!({}));let local_port=runtime["port"].as_u64().unwrap_or(3773) as u16;
  if serve_mapping(&tail["serve"],text(&tail,"dnsName"),port,local_port).0=="conflict"{return Err("tailscale_port_in_use".into())}
 }
 let old_settings=bytes(&settings_path())?;let old_override=bytes(&override_path())?;
 let mut settings=if let Some(raw)=&old_settings{serde_json::from_slice::<Value>(raw).map_err(|_|"malformed")?}else{json!({})};
 if !settings.is_object(){return Err("malformed".into())}
 settings["serverExposureMode"]=input["mode"].clone();settings["tailscaleServeEnabled"]=json!(enabled);settings["tailscaleServePort"]=json!(port);
 if revision()?!=text(input,"revision"){return Err("catalog_changed_reload".into())}
 atomic_bytes(&override_path(),config.as_bytes())?;
 let apply=(||{save_json(&settings_path(),&settings)?;command("systemctl",&["--user","daemon-reload"],"8s")?;backend_service::control("restart")?;Ok(())})();
 if let Err(error)=apply{
  let restored=restore(&settings_path(),&old_settings).and_then(|_|restore(&override_path(),&old_override));
  let _=command("systemctl",&["--user","daemon-reload"],"8s");
  if restored.is_err(){return Err("network_rollback_failed".into())}
  // Restore the prior native launch configuration if activation failed.
  let _=backend_service::control("restart");return Err(error)
 }
 refresh_connections();snapshot()
}
