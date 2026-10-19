use omarchy_t3_client::*;
use serde_json::json;
use base64::Engine;
use aes::cipher::{BlockEncryptMut,KeyIvInit,block_padding::Pkcs7};
use std::{fs,io::{Read,Write},net::TcpListener,thread,os::unix::fs::PermissionsExt};
fn write_catalog(path:&std::path::Path, disabled:Vec<&str>) {
    let raw=json!({"schemaVersion":1,"targets":[{"_tag":"BearerConnectionTarget","environmentId":"remote","connectionId":"route","label":"From T3"},{"_tag":"BearerConnectionTarget","environmentId":"disabled","connectionId":"disabled","label":"Disabled"}],"profiles":[{"_tag":"BearerConnectionProfile","connectionId":"route","httpBaseUrl":fs::read_to_string(path.parent().unwrap().join("origin")).unwrap()}],"credentials":[{"connectionId":"route","credential":{"_tag":"BearerConnectionCredential","token":"synthetic-catalog-credential"}}],"disabledEnvironmentIds":disabled});
    let mut key=[0u8;16];pbkdf2::pbkdf2_hmac::<sha1::Sha1>(b"peanuts",b"saltysalt",1,&mut key);
    let mut bytes=b"v10".to_vec();bytes.extend(cbc::Encryptor::<aes::Aes128>::new_from_slices(&key,&[b' ';16]).unwrap().encrypt_padded_vec_mut::<Pkcs7>(raw.to_string().as_bytes()));
    fs::write(path,json!({"version":1,"encryptedCatalog":base64::engine::general_purpose::STANDARD.encode(bytes)}).to_string()).unwrap();
}
#[test]
fn t3_is_authoritative_for_origin_credentials_disabled_routes_and_reload() {
    let root=std::env::temp_dir().join(format!("t3-inherit-test-{}",uuid::Uuid::new_v4()));let data=root.join("native/userdata");fs::create_dir_all(&data).unwrap();
    std::env::set_var("HOME",&root);std::env::set_var("T3CODE_HOME",root.join("native"));std::env::set_var("XDG_DATA_HOME",root.join("data"));std::env::remove_var("OMARCHY_T3_FIXTURE_ENVIRONMENTS");
    let listener=TcpListener::bind("127.0.0.1:0").unwrap();let origin=format!("http://{}",listener.local_addr().unwrap());
    fs::write(data.join("origin"),&origin).unwrap();fs::write(data.join("server-runtime.json"),json!({"origin":origin,"pid":std::process::id(),"port":listener.local_addr().unwrap().port()}).to_string()).unwrap();
    fs::write(data.join("environment-id"),"primary").unwrap();
    let token=xdg("XDG_DATA_HOME",".local/share").join("local.write.token");fs::create_dir_all(token.parent().unwrap()).unwrap();
    fs::write(&token,format!("{}.synthetic-signature",base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json!({"exp":chrono::Utc::now().timestamp_millis()+86_400_000,"scopes":["orchestration:read","orchestration:operate","filesystem:read","settings:write","terminal:read","terminal:operate"]}).to_string()))).unwrap();fs::set_permissions(&token,fs::Permissions::from_mode(0o600)).unwrap();
    // Upgrade an existing chat-only local credential through T3's native issuer.
    let full=fs::read_to_string(&token).unwrap();
    let legacy=format!("{}.synthetic-signature",base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json!({"exp":chrono::Utc::now().timestamp_millis()+86_400_000,"scopes":["orchestration:read","orchestration:operate","filesystem:read","settings:write"]}).to_string()));fs::write(&token,legacy).unwrap();
    let cli=root.join(".t3/bin/t3");fs::create_dir_all(cli.parent().unwrap()).unwrap();fs::write(&cli,"#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$OT3_TEST_AUTH_ARGUMENTS\"\nprintf '%s\\n' \"$OT3_TEST_AUTH_TOKEN\"\n").unwrap();fs::set_permissions(&cli,fs::Permissions::from_mode(0o700)).unwrap();
    std::env::set_var("OT3_TEST_AUTH_ARGUMENTS",root.join("auth-args"));std::env::set_var("OT3_TEST_AUTH_TOKEN",&full);
    // A stale plugin registry must have no influence over the native connection catalog.
    let legacy=root.join(".config/omarchy-t3-code");fs::create_dir_all(&legacy).unwrap();fs::write(legacy.join("environments.json"),r#"{"environments":[{"id":"wrong","base_url":"http://example.invalid"}]}"#).unwrap();
    write_catalog(&data.join("connection-catalog.json"),vec!["disabled"]);
    let environments=environments().unwrap();let args=fs::read_to_string(root.join("auth-args")).unwrap();assert!(args.contains("terminal:read")&&args.contains("terminal:operate"));assert_eq!(fs::read_to_string(&token).unwrap(),full);assert_eq!(environments.len(),2);assert_eq!(environments[0].base_url,origin);assert_eq!(environments[1].name,"From T3");assert_eq!(environment_token(&environments[1],false).unwrap(),"synthetic-catalog-credential");assert!(environments[1].token_file.as_os_str().is_empty());
    let server=thread::spawn(move||{let (mut socket,_)=listener.accept().unwrap();let mut bytes=[0;4096];let n=socket.read(&mut bytes).unwrap();assert!(String::from_utf8_lossy(&bytes[..n]).contains("Bearer synthetic-catalog-credential"));socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}").unwrap();});
    assert_eq!(Client::new().unwrap().get(&environments[1],"/fixture",true).unwrap(),json!({}));server.join().unwrap();
    write_catalog(&data.join("connection-catalog.json"),vec!["disabled","remote"]);
    let updated=omarchy_t3_client::environments().unwrap();assert_eq!(updated.len(),1);assert_eq!(updated[0].id,"local");
    write_catalog(&data.join("connection-catalog.json"),vec!["disabled","remote","primary"]);
    assert!(omarchy_t3_client::environments().unwrap().is_empty());
    write_catalog(&data.join("connection-catalog.json"),vec!["disabled","remote"]);
    fs::write(data.join("desktop-settings.json"),r#"{"localEnvironmentEnabled":false}"#).unwrap();
    assert!(omarchy_t3_client::environments().unwrap().is_empty());
    fs::write(data.join("desktop-settings.json"),r#"{"localEnvironmentEnabled":true}"#).unwrap();
    assert_eq!(omarchy_t3_client::environments().unwrap().len(),1);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn only_idle_ot3_sessions_from_this_device_are_retired(){
 use chrono::{TimeZone,Utc};
 let now=Utc.with_ymd_and_hms(2026,10,19,12,0,0).unwrap();
 let me="omarchy-t3-code:0123456789abcdef";
 let s=|id:&str,subject:&str,label:&str,last:Option<&str>,issued:&str,connected:bool,current:bool|serde_json::json!({"sessionId":id,"subject":subject,"client":{"label":label},"lastConnectedAt":last,"issuedAt":issued,"connected":connected,"current":current});
 let clients=serde_json::json!([
  s("old",me,"omarchy-t3-code",Some("2026-10-19T11:50:00Z"),"2026-10-19T09:00:00Z",false,false),
  s("never-used",me,"omarchy-t3-code",None,"2026-10-19T11:00:00Z",false,false),
  s("just-issued",me,"omarchy-t3-code",None,"2026-10-19T11:59:30Z",false,false),
  s("live",me,"omarchy-t3-code",Some("2026-10-19T10:00:00Z"),"2026-10-19T09:00:00Z",true,false),
  s("mine-now",me,"omarchy-t3-code",None,"2026-10-19T09:00:00Z",false,true),
  s("other-device","omarchy-t3-code:fedcba9876543210","omarchy-t3-code",None,"2026-10-01T00:00:00Z",false,false),
  s("legacy","cli-issued-session","omarchy-t3-code",Some("2026-10-19T10:00:00Z"),"2026-10-18T00:00:00Z",false,false),
  s("legacy-recent","cli-issued-session","omarchy-t3-code",Some("2026-10-19T11:30:00Z"),"2026-10-18T00:00:00Z",false,false),
  s("someone-else","cli-issued-session","backup-script",None,"2026-10-01T00:00:00Z",false,false),
  s("desktop","desktop-bootstrap","T3 Code",None,"2026-10-01T00:00:00Z",false,false),
 ]);
 assert_eq!(stale_ot3_sessions(&clients,me,now),vec!["old","never-used","legacy"]);
 assert_eq!(stale_ot3_sessions(&serde_json::json!({"clientSessions":[clients[0].clone()]}),me,now),vec!["old"]);
}
