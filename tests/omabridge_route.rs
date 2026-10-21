use omarchy_t3_client::{omabridge_route as bridge,*};
use serde_json::json;
use base64::Engine;
use aes::cipher::{BlockEncryptMut,KeyIvInit,block_padding::Pkcs7};
use std::{fs,io::{Read,Write},net::TcpListener,thread,os::unix::fs::PermissionsExt,sync::{Arc,Mutex}};
const PEER:&str="f3fc84495ad12fcc7bcc5490eae2a4e98da1499b2668b7a05648efe899c17910";
fn write_catalog(path:&std::path::Path,origin:&str){
 let raw=json!({"schemaVersion":1,"targets":[{"_tag":"BearerConnectionTarget","environmentId":"remote","connectionId":"route","label":"Node"}],"profiles":[{"_tag":"BearerConnectionProfile","connectionId":"route","httpBaseUrl":origin}],"credentials":[{"connectionId":"route","credential":{"_tag":"BearerConnectionCredential","token":"synthetic-catalog-credential"}}]});
 let mut key=[0u8;16];pbkdf2::pbkdf2_hmac::<sha1::Sha1>(b"peanuts",b"saltysalt",1,&mut key);
 let mut bytes=b"v10".to_vec();bytes.extend(cbc::Encryptor::<aes::Aes128>::new_from_slices(&key,&[b' ';16]).unwrap().encrypt_padded_vec_mut::<Pkcs7>(raw.to_string().as_bytes()));
 fs::write(path,json!({"version":1,"encryptedCatalog":base64::engine::general_purpose::STANDARD.encode(bytes)}).to_string()).unwrap();
}
#[test]
fn route_documents_and_forward_output_are_strictly_validated(){
 let ok=json!({"version":1,"routes":{"remote":{"peer":PEER,"service":"t3","mode":"only","relayUrl":"https://relay.example"}}});
 let (_,routes)=bridge::parse(&ok).unwrap();let r=&routes["remote"];assert!(r.only);assert_eq!(r.relay_url.as_deref(),Some("https://relay.example"));
 for bad in [json!({"version":2,"routes":{}}),json!({"version":1,"routes":{"local":{"peer":PEER}}}),json!({"version":1,"routes":{"remote":{"peer":"ABC"}}}),
  json!({"version":1,"routes":{"remote":{"peer":PEER,"service":"t3;rm"}}}),json!({"version":1,"routes":{"remote":{"peer":PEER,"relayUrl":"http://plain"}}}),
  json!({"version":1,"routes":{"remote":{"peer":PEER,"mode":"always"}}}),json!({"version":1,"stateDir":"relative","routes":{}}),
  json!({"version":1,"default":"always","routes":{}}),json!({"version":1,"routes":{"remote":{"peer":PEER,"mode":"off"}}})]{assert!(bridge::parse(&bad).is_err(),"{bad}");}
 // Automatic routing is the default; "off" applies globally or per environment.
 use bridge::Plan;
 let auto=bridge::parse_config(&json!({"version":1,"routes":{"quiet":{"mode":"off"},"remote":{"peer":PEER,"service":"ssh"}}})).unwrap();
 assert_eq!(auto.plan("other"),Plan::Auto);assert_eq!(auto.plan("quiet"),Plan::Off);assert!(matches!(auto.plan("remote"),Plan::Pinned(r) if r.service=="ssh"&&!r.only));
 let off=bridge::parse_config(&json!({"version":1,"default":"off","routes":{"remote":{"peer":PEER}}})).unwrap();
 assert_eq!(off.plan("other"),Plan::Off);assert!(matches!(off.plan("remote"),Plan::Pinned(_)));
 assert!(bridge::carries("SshConnectionTarget","ssh")&&!bridge::carries("SshConnectionTarget","t3"));
 assert!(bridge::carries("BearerConnectionTarget","t3")&&!bridge::carries("BearerConnectionTarget","ssh")&&!bridge::carries("RelayConnectionTarget","t3"));
 let line=format!(r#"{{"event":"forwarding","listen":"127.0.0.1:4100","peer":"{PEER}","service":"t3"}}"#);
 assert_eq!(bridge::forwarding_url(&line,PEER,"t3").unwrap(),"http://127.0.0.1:4100");
 assert!(bridge::forwarding_url(&line.replace("127.0.0.1","0.0.0.0"),PEER,"t3").is_err());
 assert!(bridge::forwarding_url(&line,PEER,"egress").is_err());
 let peers=bridge::project_peers(&json!({"peers":[{"id":PEER,"name":"devbox","services":[],"remote_services":["t3"],"address":{"addrs":["192.0.2.1:1"]}},{"id":"bad","name":"x"}]}));
 assert_eq!(peers,vec![json!({"id":PEER,"name":"devbox","services":[],"remoteServices":["t3"]})]);
}
#[test]
fn bearer_connections_prefer_verified_peers_and_fall_back_only_when_allowed(){
 let root=std::env::temp_dir().join(format!("ot3-omabridge-{}",uuid::Uuid::new_v4()));let data=root.join("native/userdata");fs::create_dir_all(&data).unwrap();
 std::env::set_var("HOME",&root);std::env::set_var("T3CODE_HOME",root.join("native"));std::env::set_var("XDG_DATA_HOME",root.join("data"));std::env::set_var("XDG_CONFIG_HOME",root.join("config"));std::env::remove_var("OMARCHY_T3_FIXTURE_ENVIRONMENTS");
 fs::write(data.join("desktop-settings.json"),r#"{"localEnvironmentEnabled":false}"#).unwrap();
 let state=root.join("data/omabridge");fs::create_dir_all(&state).unwrap();fs::set_permissions(&state,fs::Permissions::from_mode(0o700)).unwrap();fs::write(state.join("state.json"),"{}").unwrap();
 // The T3 server reached through the bridge; the catalog's own origin is unreachable.
 let t3=TcpListener::bind("127.0.0.1:0").unwrap();let bridged=t3.local_addr().unwrap();
 let served=Arc::new(Mutex::new("remote".to_string()));let rpc=Arc::new(Mutex::new(0));
 {let (served,rpc)=(served.clone(),rpc.clone());thread::spawn(move||for socket in t3.incoming(){
  let mut socket=socket.unwrap();let mut bytes=[0;4096];let n=socket.read(&mut bytes).unwrap();let request=String::from_utf8_lossy(&bytes[..n]).to_string();
  // The descriptor is public; every other request must carry the catalog bearer.
  let body=if request.starts_with("GET /.well-known/t3/environment "){assert!(!request.contains("synthetic-catalog-credential"));json!({"environmentId":*served.lock().unwrap()}).to_string()}else{assert!(request.contains("Bearer synthetic-catalog-credential"));*rpc.lock().unwrap()+=1;"{}".into()};
  let _=socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes());
 });}
 write_catalog(&data.join("connection-catalog.json"),"http://127.0.0.1:9");
 let fake=root.join("omabridge");
 fs::write(&fake,format!("#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{calls}'\ncase \"$*\" in\n *' peers'*) printf '%s\\n' '{{\"peers\":[{{\"id\":\"{PEER}\",\"name\":\"devbox\",\"services\":[],\"remote_services\":[\"t3\"]}}]}}' ;;\n *forward*) [ \"$OT3_FAKE_FORWARD\" = ok ] || exit 3; printf '%s\\n' '{{\"event\":\"forwarding\",\"listen\":\"{bridged}\",\"peer\":\"{PEER}\",\"service\":\"t3\"}}'; exec sleep 30 ;;\nesac\n",calls=root.join("calls").display())).unwrap();
 fs::set_permissions(&fake,fs::Permissions::from_mode(0o700)).unwrap();std::env::set_var("OT3_OMABRIDGE_BIN",&fake);std::env::set_var("OT3_FAKE_FORWARD","ok");
 // Without a route file every granting peer is tried automatically and verified.
 let auto=environments().unwrap();assert_eq!(auto.len(),1);assert_eq!(auto[0].base_url,format!("http://{bridged}"));
 let read=bridge::execute(&json!({"action":"read"})).unwrap();assert_eq!(read["default"],"auto");assert_eq!(read["active"]["remote"]["peer"],PEER);assert_eq!(read["active"]["remote"]["auto"],true);
 assert_eq!(Client::new().unwrap().get(&auto[0],"/fixture",true).unwrap(),json!({}));assert_eq!(*rpc.lock().unwrap(),1);
 // A peer serving another environment never receives the credential.
 *served.lock().unwrap()="someone-else".into();refresh_connections();
 let wrong=environments().unwrap();assert_eq!(wrong[0].base_url,"http://127.0.0.1:9");assert!(wrong[0].connection_error.is_none());
 assert!(bridge::execute(&json!({"action":"read"})).unwrap()["active"].get("remote").is_none());
 *served.lock().unwrap()="remote".into();
 // "default: off" and a per-environment "off" keep the saved T3 route.
 assert_eq!(bridge::execute(&json!({"action":"default","revision":read["revision"],"value":"off"})).unwrap_err(),"confirmation_required");
 let off=bridge::execute(&json!({"action":"default","revision":read["revision"],"confirmed":true,"value":"off"})).unwrap();assert_eq!(off["default"],"off");
 assert_eq!(environments().unwrap()[0].base_url,"http://127.0.0.1:9");
 let on=bridge::execute(&json!({"action":"default","revision":off["revision"],"confirmed":true,"value":"auto"})).unwrap();
 let quiet=bridge::execute(&json!({"action":"set","revision":on["revision"],"confirmed":true,"environmentId":"remote","mode":"off"})).unwrap();assert_eq!(quiet["routes"]["remote"]["mode"],"off");
 assert_eq!(environments().unwrap()[0].base_url,"http://127.0.0.1:9");
 // CAS and grant checks before a pinned route is saved.
 let read=bridge::execute(&json!({"action":"read"})).unwrap();assert_eq!(read["available"],true);assert_eq!(read["identity"],true);assert_eq!(read["peers"][0]["remoteServices"][0],"t3");
 assert_eq!(bridge::execute(&json!({"action":"set","revision":read["revision"],"environmentId":"remote","peer":PEER})).unwrap_err(),"confirmation_required");
 assert_eq!(bridge::execute(&json!({"action":"set","revision":"stale","confirmed":true,"environmentId":"remote","peer":PEER})).unwrap_err(),"catalog_changed_reload");
 assert_eq!(bridge::execute(&json!({"action":"set","revision":read["revision"],"confirmed":true,"environmentId":"remote","peer":PEER,"service":"egress"})).unwrap_err(),"omabridge_service_not_granted");
 let saved=bridge::execute(&json!({"action":"set","revision":read["revision"],"confirmed":true,"environmentId":"remote","peer":PEER,"service":"t3","mode":"prefer"})).unwrap();
 assert_eq!(saved["routes"]["remote"]["peer"],PEER);assert_eq!(fs::metadata(bridge::config_path()).unwrap().permissions().mode()&0o777,0o600);
 assert!(!fs::read_to_string(bridge::config_path()).unwrap().contains("synthetic-catalog-credential"));
 let routed=environments().unwrap();assert_eq!(routed[0].base_url,format!("http://{bridged}"));
 assert_eq!(bridge::execute(&json!({"action":"read"})).unwrap()["active"]["remote"]["auto"],false);
 assert_eq!(Client::new().unwrap().get(&routed[0],"/fixture",true).unwrap(),json!({}));assert_eq!(*rpc.lock().unwrap(),2);
 let calls=fs::read_to_string(root.join("calls")).unwrap();assert!(calls.contains(&format!("forward --peer {PEER} --service t3 --listen 127.0.0.1:0")));assert!(calls.contains(&format!("--state-dir {}",state.display())));
 // A failing bridge falls back in "prefer" mode and is an explicit error in "only" mode.
 std::env::set_var("OT3_FAKE_FORWARD","fail");
 let current=bridge::execute(&json!({"action":"read"})).unwrap();
 bridge::execute(&json!({"action":"set","revision":current["revision"],"confirmed":true,"environmentId":"remote","peer":PEER,"mode":"prefer","relayUrl":"https://relay.example"})).unwrap();
 let fallback=environments().unwrap();assert_eq!(fallback[0].base_url,"http://127.0.0.1:9");assert!(fallback[0].connection_error.is_none());
 let current=bridge::execute(&json!({"action":"read"})).unwrap();
 bridge::execute(&json!({"action":"set","revision":current["revision"],"confirmed":true,"environmentId":"remote","peer":PEER,"mode":"only"})).unwrap();
 assert_eq!(environments().unwrap()[0].connection_error.as_deref(),Some("omabridge_route_failed"));
 let current=bridge::execute(&json!({"action":"read"})).unwrap();
 let cleared=bridge::execute(&json!({"action":"clear","revision":current["revision"],"confirmed":true,"environmentId":"remote"})).unwrap();assert!(cleared["routes"].as_object().unwrap().is_empty());
 // Back to automatic routing, which falls back while the bridge is down.
 assert_eq!(environments().unwrap()[0].base_url,"http://127.0.0.1:9");
 // A malformed document disables bridge routing instead of guessing.
 std::env::set_var("OT3_FAKE_FORWARD","ok");
 fs::write(bridge::config_path(),r#"{"version":1,"routes":{"remote":{"peer":"nope"}}}"#).unwrap();
 assert_eq!(environments().unwrap()[0].base_url,"http://127.0.0.1:9");assert_eq!(bridge::execute(&json!({"action":"read"})).unwrap()["error"],"invalid_omabridge_routes");
 fs::remove_dir_all(root).unwrap();
}
