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
 let mut serve=Command::new(&bin).arg("--state-dir").arg(&host).args(["--bind","127.0.0.1:0","serve","--service","t3=127.0.0.1:9"]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
 for _ in 0..50{if host.join("endpoint.json").exists(){break}std::thread::sleep(std::time::Duration::from_millis(100));}
 std::thread::sleep(std::time::Duration::from_millis(300));
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
 };
 let outcome=std::panic::catch_unwind(result);
 let _=serve.kill();let _=serve.wait();fs::remove_dir_all(&root).unwrap();
 if let Err(e)=outcome{std::panic::resume_unwind(e)}
}
