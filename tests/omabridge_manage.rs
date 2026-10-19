use omarchy_t3_client::{omabridge_manage as manage,omabridge_route as bridge};
use serde_json::{json,Value};
use std::{fs,os::unix::fs::PermissionsExt,path::Path,process::{Command,Stdio}};
fn use_state(config:&Path,state:&Path){fs::create_dir_all(config.join("omarchy-t3-code")).unwrap();fs::write(config.join("omarchy-t3-code/omabridge-routes.json"),json!({"version":1,"stateDir":state,"routes":{}}).to_string()).unwrap();}
fn act(action:&str,values:Value)->Result<Value,String>{let mut input=json!({"action":action,"confirmed":true});for (k,v) in values.as_object().unwrap(){input[k]=v.clone();}bridge::execute(&input)}
#[test]
fn terminal_qr_and_cli_errors_are_mapped_without_echoing_output(){
 let block="█▀▀▀▀▀█ ▄ █▀▀▀▀▀█ ▄▄▄\n".repeat(12);
 let svg=manage::qr_svg(&block).unwrap();assert!(svg.starts_with("data:image/svg+xml;base64,"));
 assert!(manage::qr_svg("not a qr code at all, just text\n").is_none());
 assert_eq!(manage::error_code("omabridge: server not running: No such file"),"omabridge_not_published");
 assert_eq!(manage::error_code("omabridge: grant only currently published services"),"omabridge_service_not_published");
 assert_eq!(manage::error_code("token=secret something else"),"omabridge_command_failed");
 // The user unit quotes paths and refuses characters systemd would reinterpret.
 let unit=manage::unit_text(Path::new("/opt/my plugin/bin/omabridge"),Path::new("/home/user/.local/share/omabridge"),true).unwrap();
 assert!(unit.starts_with("# Managed by Omarchy T3 Code"));
 assert!(unit.contains("ExecStart=\"/opt/my plugin/bin/omabridge\" --state-dir \"/home/user/.local/share/omabridge\" serve --service t3=127.0.0.1:"));
 assert!(unit.contains(" --service ssh=127.0.0.1:22\n"));assert!(!manage::unit_text(Path::new("/a"),Path::new("/b"),false).unwrap().contains("ssh="));
 for bad in ["/a%h/omabridge","/a$HOME/omabridge","/a\"b","relative/omabridge"]{assert!(manage::unit_text(Path::new(bad),Path::new("/b"),false).is_err(),"{bad}");}
}
/// End to end with the bundled CLI: two identities on loopback, one publishing.
#[test]
fn identities_invitations_pairing_approval_and_revocation(){
 let bin=Path::new(env!("CARGO_MANIFEST_DIR")).join("bin/omabridge");
 if !bin.is_file(){eprintln!("skipped: bin/omabridge is not installed (run bin/fetch-omabridge)");return}
 let root=std::env::temp_dir().join(format!("ot3-omabridge-manage-{}",uuid::Uuid::new_v4()));fs::create_dir_all(&root).unwrap();fs::set_permissions(&root,fs::Permissions::from_mode(0o700)).unwrap();
 let (host,guest,config,downloads)=(root.join("host"),root.join("guest"),root.join("config"),root.join("downloads"));fs::create_dir_all(&downloads).unwrap();
 std::env::set_var("HOME",&root);std::env::set_var("XDG_CONFIG_HOME",&config);std::env::set_var("XDG_STATE_HOME",root.join("state"));std::env::set_var("T3CODE_HOME",root.join("t3"));std::env::set_var("OT3_OMABRIDGE_BIN",&bin);
 use_state(&config,&host);
 assert!(bridge::execute(&json!({"action":"read"})).unwrap()["device"].is_null());
 assert_eq!(bridge::execute(&json!({"action":"identity-create"})).unwrap_err(),"confirmation_required");
 let host_id=act("identity-create",json!({})).unwrap()["device"]["id"].as_str().unwrap().to_string();
 assert_eq!(fs::metadata(&host).unwrap().permissions().mode()&0o777,0o700);
 // Inviting needs this device to be published.
 assert_eq!(act("invite",json!({"services":["t3"],"directory":downloads})).unwrap_err(),"omabridge_not_published");
 // A stand-in systemctl that really runs the unit's ExecStart, so the unit text is exercised.
 let fakebin=root.join("fakebin");fs::create_dir_all(&fakebin).unwrap();let pid=root.join("serve.pid");
 fs::write(fakebin.join("systemctl"),format!("#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{calls}'\nunit=\"$XDG_CONFIG_HOME/systemd/user/{unit}\"\ncase \"$*\" in\n *is-active*) [ -f '{pid}' ] && kill -0 \"$(cat '{pid}')\" ;;\n *restart*) line=$(sed -n 's/^ExecStart=//p' \"$unit\"); eval \"set -- $line\"; \"$@\" </dev/null >/dev/null 2>&1 & echo $! > '{pid}' ;;\n *disable*) [ -f '{pid}' ] && kill \"$(cat '{pid}')\"; rm -f '{pid}' ;;\nesac\nexit 0\n",calls=root.join("systemctl.calls").display(),unit=manage::UNIT,pid=pid.display())).unwrap();
 fs::set_permissions(fakebin.join("systemctl"),fs::Permissions::from_mode(0o700)).unwrap();
 std::env::set_var("PATH",format!("{}:{}",fakebin.display(),std::env::var("PATH").unwrap()));
 fs::create_dir_all(root.join("t3/userdata")).unwrap();fs::write(root.join("t3/userdata/server-runtime.json"),r#"{"port":9}"#).unwrap();
 assert_eq!(bridge::execute(&json!({"action":"read"})).unwrap()["published"]["enabled"],false);
 assert_eq!(bridge::execute(&json!({"action":"publish"})).unwrap_err(),"confirmation_required");
 let published=act("publish",json!({"ssh":false})).unwrap()["published"].clone();
 assert_eq!(published["enabled"],true);assert_eq!(published["services"],json!(["t3"]));assert_eq!(published["stale"],false);
 let unit=fs::read_to_string(config.join("systemd/user").join(manage::UNIT)).unwrap();assert!(unit.contains(&format!("--state-dir \"{}\" serve --service t3=127.0.0.1:9",host.display())));
 for _ in 0..50{if host.join("endpoint.json").exists(){break}std::thread::sleep(std::time::Duration::from_millis(100));}
 std::thread::sleep(std::time::Duration::from_millis(300));
 assert_eq!(bridge::execute(&json!({"action":"read"})).unwrap()["published"]["active"],true);
 // T3 moving to another port marks the published unit as stale.
 fs::write(root.join("t3/userdata/server-runtime.json"),r#"{"port":10}"#).unwrap();assert_eq!(bridge::execute(&json!({"action":"read"})).unwrap()["published"]["stale"],true);
 fs::write(root.join("t3/userdata/server-runtime.json"),r#"{"port":9}"#).unwrap();
 let result=||{
  let invited=act("invite",json!({"services":["t3"],"qr":true,"directory":downloads})).unwrap();
  let created=&invited["created"];let path=created["path"].as_str().unwrap().to_string();
  assert!(path.starts_with(downloads.to_str().unwrap()));assert_eq!(fs::metadata(&path).unwrap().permissions().mode()&0o777,0o600);
  assert!(created["qrImage"].as_str().unwrap().starts_with("data:image/svg+xml;base64,"));assert_eq!(created["services"],json!(["t3"]));
  let token=serde_json::from_slice::<Value>(&fs::read(&path).unwrap()).unwrap()["token"].as_str().unwrap().to_string();
  assert!(!invited.to_string().contains(&token),"the invitation token never leaves the file");
  // The guest pairs from the file; ot3 keeps a private copy until approval.
  use_state(&config,&guest);
  let paired=act("pair",json!({"path":path})).unwrap();assert_eq!(paired["paired"]["status"],"pending");
  let waiting=paired["awaiting"].as_array().unwrap();assert_eq!(waiting.len(),1);assert!(!paired.to_string().contains(&token));
  let copy=root.join("state/omarchy-t3-code/omabridge-pairing").join(format!("{}.json",waiting[0]["id"].as_str().unwrap()));
  assert_eq!(fs::metadata(&copy).unwrap().permissions().mode()&0o777,0o600);assert_eq!(fs::metadata(copy.parent().unwrap()).unwrap().permissions().mode()&0o777,0o700);
  let guest_id=paired["device"]["id"].as_str().unwrap().to_string();assert_ne!(guest_id,host_id);
  // The host sees the claim without its invitation id or address, and approves it.
  use_state(&config,&host);
  let read=bridge::execute(&json!({"action":"read"})).unwrap();
  assert_eq!(read["pending"],json!([{"id":guest_id,"name":read["pending"][0]["name"],"services":["t3"],"expires":read["pending"][0]["expires"]}]));
  assert_eq!(bridge::execute(&json!({"action":"approve","peer":guest_id})).unwrap_err(),"confirmation_required");
  let approved=act("approve",json!({"peer":guest_id})).unwrap();assert!(approved["pending"].as_array().unwrap().is_empty());
  assert_eq!(approved["peers"][0]["id"],json!(guest_id));assert_eq!(approved["peers"][0]["services"],json!(["t3"]));
  // The guest checks again, which completes pairing and deletes the private copy.
  use_state(&config,&guest);
  let done=act("pair-retry",json!({"id":waiting[0]["id"]})).unwrap();assert_eq!(done["paired"]["status"],"approved");
  assert!(done["awaiting"].as_array().unwrap().is_empty());assert!(!copy.exists());
  assert_eq!(done["peers"][0]["id"],json!(host_id));assert_eq!(done["peers"][0]["remoteServices"],json!(["t3"]));
  // Only invitations created here may be discarded through ot3.
  use_state(&config,&host);
  assert_eq!(act("invite-discard",json!({"path":root.join("host/state.json")})).unwrap_err(),"invalid_command");
  act("invite-discard",json!({"path":path})).unwrap();assert!(!Path::new(&path).exists());
  let revoked=act("revoke",json!({"peer":guest_id})).unwrap();assert!(revoked["peers"].as_array().unwrap().is_empty());
  let off=act("unpublish",json!({})).unwrap();assert_eq!(off["published"]["enabled"],false);assert_eq!(off["published"]["active"],false);
  assert!(!config.join("systemd/user").join(manage::UNIT).exists());
  let calls=fs::read_to_string(root.join("systemctl.calls")).unwrap();for call in ["--user daemon-reload","--user enable omarchy-t3-omabridge.service","--user restart omarchy-t3-omabridge.service","--user disable --now omarchy-t3-omabridge.service"]{assert!(calls.contains(call),"{call}");}
 };
 let outcome=std::panic::catch_unwind(result);
 if let Ok(p)=fs::read_to_string(&pid){let _=Command::new("kill").arg(p.trim()).stdout(Stdio::null()).stderr(Stdio::null()).status();}
 fs::remove_dir_all(&root).unwrap();
 if let Err(e)=outcome{std::panic::resume_unwind(e)}
}
