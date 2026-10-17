use omarchy_t3_client::{connection_access,Environment,protocol};
use serde_json::{json,Value};
use std::{io::{Read,Write},net::TcpListener,sync::Arc,thread};
fn fixture(responses:Vec<(&'static str,Value)>)->(Environment,thread::JoinHandle<Vec<String>>){fixture_token(responses,"fixture-private-token")}
fn fixture_token(responses:Vec<(&'static str,Value)>,token:&str)->(Environment,thread::JoinHandle<Vec<String>>){
 let expected_header=format!("authorization: bearer {token}").to_lowercase();
 let listener=TcpListener::bind("127.0.0.1:0").unwrap();let base=format!("http://{}",listener.local_addr().unwrap());
 let server=thread::spawn(move||{let mut requests=vec![];for (path,value) in responses{
  let (mut s,_)=listener.accept().unwrap();s.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();let mut raw=vec![];let mut bytes=[0;4096];loop{let n=s.read(&mut bytes).unwrap();assert!(n>0);raw.extend_from_slice(&bytes[..n]);let text=String::from_utf8_lossy(&raw);if let Some(end)=text.find("\r\n\r\n"){let len=text[..end].lines().find_map(|l|l.to_lowercase().strip_prefix("content-length: ").and_then(|n|n.parse::<usize>().ok())).unwrap_or(0);if raw.len()>=end+4+len{break}}}
  let req=String::from_utf8(raw).unwrap();assert!(req.starts_with(path),"{req}");assert!(req.to_lowercase().contains(&expected_header));requests.push(req);
  let body=value.to_string();write!(s,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
 }requests});
 let conn=protocol::Connection::new(&base,None,protocol::Credential::bearer("fixture-private-token").unwrap()).unwrap();
 (Environment{id:"remote".into(),name:"Remote".into(),base_url:base,token_file:Default::default(),write_token_file:None,bearer:None,connection_error:None,connection:Some(Arc::new(conn))},server)
}
fn session(scopes:&[&str])->Value{json!({"authenticated":true,"permissions":scopes,"scopes":["orchestration:read"],"auth":{"policy":"remote-reachable"}})}
#[test]
fn lists_native_clients_links_and_connect_without_credentials(){
 let (e,server)=fixture(vec![("GET /api/auth/session",session(&["access:read","access:write","relay:read"])),("GET /.well-known/t3/environment",json!({"environmentId":"remote","serverVersion":"1.2"})),("GET /api/auth/clients",json!([{"sessionId":"current","current":true},{"sessionId":"other","current":false}])),("GET /api/auth/pairing-links",json!([{"id":"link","permissions":["orchestration:read"]}])),("GET /api/connect/link-state",json!({"linked":true,"publishAgentActivity":true}))]);
 let v=connection_access::execute(&e,&json!({"action":"read"})).unwrap();assert_eq!(v["clients"].as_array().unwrap().len(),2);assert_eq!(v["pairingLinks"][0]["id"],"link");assert_eq!(v["connect"]["linked"],true);assert!(!v.to_string().contains("fixture-private-token"));assert!(v.get("credential").is_none());server.join().unwrap();
}
#[test]
fn delegated_scopes_are_bounded_and_terminal_permissions_stay_consistent(){
 let grants=vec!["access:write".to_string(),"terminal:operate".to_string(),"terminal:read".to_string()];
 for selected in [json!([]),json!(["terminal:operate"]),json!(["relay:write"]),json!(["terminal:read","terminal:read"]),json!(["unknown"])]{assert!(connection_access::pairing_payload(&json!({"scopes":selected}),&grants).is_err());}
 assert_eq!(connection_access::pairing_payload(&json!({"label":" My device ","scopes":["terminal:read","terminal:operate"]}),&grants).unwrap()["label"],"My device");
 assert!(connection_access::pairing_payload(&json!({"scopes":["terminal:read"]}),&[]).is_err());
}
#[test]
fn create_returns_credential_once_and_revoke_requires_explicit_confirmation(){
 let grants=["access:write","orchestration:read"];
 let (e,server)=fixture(vec![("GET /api/auth/session",session(&grants)),("POST /api/auth/pairing-token",json!({"id":"new","credential":"one-time-code","expiresAt":"2030-01-01T00:00:00Z"}))]);
 let v=connection_access::execute(&e,&json!({"action":"create-link","label":"Phone","scopes":["orchestration:read"]})).unwrap();assert_eq!(v["mutation"]["credential"],"one-time-code");assert_eq!(v["mutation"]["qrShareable"],false);assert!(v["mutation"]["pairingUrl"].as_str().unwrap().ends_with("/pair#token=one-time-code"));
 let req=server.join().unwrap();assert!(req[1].contains("\"label\":\"Phone\""));
 let (e,server)=fixture(vec![("GET /api/auth/session",session(&grants))]);assert_eq!(connection_access::execute(&e,&json!({"action":"revoke-others"})).unwrap_err(),"confirmation_required");server.join().unwrap();
 let (e,server)=fixture(vec![("GET /api/auth/session",session(&grants)),("POST /api/auth/clients/revoke",json!({"revoked":true}))]);
 assert_eq!(connection_access::execute(&e,&json!({"action":"revoke-client","sessionId":"other","confirmed":true})).unwrap()["mutation"]["revoked"],true);assert!(server.join().unwrap()[1].contains("\"sessionId\":\"other\""));
}
#[test]
fn restricted_connection_never_requests_access_lists_or_mutations(){
 let (e,server)=fixture(vec![("GET /api/auth/session",session(&["orchestration:read"])),("GET /.well-known/t3/environment",json!({"environmentId":"remote"}))]);let v=connection_access::execute(&e,&json!({"action":"read"})).unwrap();assert!(v["clients"].is_null());assert!(v["pairingLinks"].is_null());server.join().unwrap();
 let (e,server)=fixture(vec![("GET /api/auth/session",session(&["orchestration:read"]))]);assert_eq!(connection_access::execute(&e,&json!({"action":"revoke-link","id":"link","confirmed":true})).unwrap_err(),"scope_required:access:write");server.join().unwrap();
}

#[test]
fn local_management_upgrades_and_reuses_the_current_client_credential(){
 use base64::Engine;
 use std::os::unix::fs::{PermissionsExt,MetadataExt};
 let root=std::env::temp_dir().join(format!("ot3-local-access-{}",uuid::Uuid::new_v4()));let cli=root.join(".t3/bin/t3");std::fs::create_dir_all(cli.parent().unwrap()).unwrap();
 let scopes=["orchestration:read","orchestration:operate","settings:write","providers:manage","environment:maintain","preview:operate","diagnostics:read","terminal:read","terminal:operate","source-control:write","filesystem:read","filesystem:write","access:read","access:write","relay:read","relay:write"];
 let token=format!("{}.fixture-signature",base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json!({"scopes":scopes,"exp":chrono::Utc::now().timestamp_millis()+86_400_000}).to_string()));
 std::fs::write(&cli,format!("#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\nprintf '%s' '{}'\n",root.join("arguments").display(),token)).unwrap();std::fs::set_permissions(&cli,std::fs::Permissions::from_mode(0o700)).unwrap();std::env::set_var("HOME",&root);
 let replies=vec![("GET /api/auth/session",session(&["orchestration:read"])),("GET /.well-known/t3/environment",json!({"environmentId":"primary"})),("GET /api/auth/session",session(&["orchestration:read"])),("GET /.well-known/t3/environment",json!({"environmentId":"primary"}))];
 let (mut e,server)=fixture_token(replies,&token);e.id="local".into();e.token_file=root.join("local.write.token");
 assert!(connection_access::execute(&e,&json!({"action":"read"})).is_ok());assert_eq!(std::fs::read_to_string(&e.token_file).unwrap(),token);assert_eq!(std::fs::metadata(&e.token_file).unwrap().mode()&0o777,0o600);
 let args=std::fs::read_to_string(root.join("arguments")).unwrap();assert!(args.contains("access:write")&&args.contains("relay:write"));std::fs::remove_file(&cli).unwrap();
 assert!(connection_access::execute(&e,&json!({"action":"read"})).is_ok(),"fresh management grant must be reused, not reissued on every refresh");server.join().unwrap();std::fs::remove_dir_all(root).unwrap();
}
