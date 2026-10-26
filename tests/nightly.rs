use omarchy_t3_client::{nightly, protocol, Environment};
use serde_json::{json,Value};
use std::{io::{Read,Write},net::TcpListener,sync::Arc,thread};
fn environment(base:&str)->Environment {
 let c=protocol::Connection::new(base,None,protocol::Credential::bearer("synthetic-token").unwrap()).unwrap();
 Environment{id:format!("nightly-{}",uuid::Uuid::new_v4()),name:"Fixture".into(),base_url:base.into(),token_file:"/unused".into(),write_token_file:None,bearer:None,connection_error:None,connection:Some(Arc::new(c))}
}
#[test]
fn reachability_is_post_and_expired_snooze_never_dispatches() {
 let listener=TcpListener::bind("127.0.0.1:0").unwrap();let e=environment(&format!("http://{}",listener.local_addr().unwrap()));
 let server=thread::spawn(move||{let(mut s,_)=listener.accept().unwrap();let mut buf=[0;8192];let n=s.read(&mut buf).unwrap();let req=String::from_utf8_lossy(&buf[..n]);assert!(req.starts_with("POST /api/connect/reachability HTTP/1.1\r\n"));let body=r#"{"reachable":true}"#;write!(s,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();});
 assert_eq!(nightly::execute(&e,&json!({"action":"connect-reachability"})).unwrap()["reachable"],true);server.join().unwrap();
 let err=nightly::execute(&e,&json!({"action":"limit-recovery","thread":"chat","requestId":uuid::Uuid::new_v4().to_string(),"args":{"runId":"run","resetAt":"2020-01-01T00:00:00Z","snooze":true}})).unwrap_err();assert_eq!(err,"usage_reset_passed");
}
#[test]
fn guest_owner_actions_preserve_the_exact_server_message_identity() {
 let root=std::env::temp_dir().join(format!("ot3-nightly-{}",uuid::Uuid::new_v4()));std::env::set_var("XDG_STATE_HOME",&root);
 let listener=TcpListener::bind("127.0.0.1:0").unwrap();let e=environment(&format!("http://{}",listener.local_addr().unwrap()));
 let server=thread::spawn(move||{for typ in ["message.dispatch","thread.guest-message.dismiss","thread.guest-share.close"] {let(s,_)=listener.accept().unwrap();let mut ws=tungstenite::accept(s).unwrap();let req:Value=serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();assert_eq!(req["tag"],"orchestration.dispatchCommand");let p=&req["payload"];assert_eq!(p["type"],typ);assert_eq!(p["threadId"],"chat");if typ!="thread.guest-share.close"{assert_eq!(p["guestMessageId"],"server-message")}if typ=="message.dispatch"{assert_eq!(p["text"],"");assert_eq!(p["attachments"],json!([]));}ws.send(tungstenite::Message::Text(json!({"_tag":"Exit","requestId":"1","exit":{"_tag":"Success","value":{"sequence":1}}}).to_string().into())).unwrap();}});
 for action in ["guest-send","guest-dismiss","guest-close"] {nightly::execute(&e,&json!({"action":action,"thread":"chat","requestId":uuid::Uuid::new_v4().to_string(),"args":{"guestMessageId":"server-message","text":"ignored tampered text"}})).unwrap();}
 server.join().unwrap();let _=std::fs::remove_dir_all(root);
}
