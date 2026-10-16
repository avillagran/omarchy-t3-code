fn opaque_request_id()->String{format!("runtime-request:provider:acpRegistry:provider-turn:{}:native-request:elicitation:0", "provider-instance%253Aencoded%3A".repeat(16))}
use omarchy_t3_client::*;
use serde_json::{json,Value};
use std::{net::TcpListener,thread,os::unix::fs::PermissionsExt};
#[test]
fn native_question_response_retry_and_dismiss_are_exact(){
 let root=std::env::temp_dir().join(format!("t3-questions-{}",uuid::Uuid::new_v4()));std::fs::create_dir_all(&root).unwrap();std::env::set_var("XDG_STATE_HOME",&root);let token=root.join("token");std::fs::write(&token,"synthetic-private-token").unwrap();std::fs::set_permissions(&token,std::fs::Permissions::from_mode(0o600)).unwrap();
 let listener=TcpListener::bind("127.0.0.1:0").unwrap();let e=Environment{id:"fixture".into(),name:"Fixture".into(),base_url:format!("http://{}",listener.local_addr().unwrap()),token_file:token,write_token_file:None,bearer:None,connection_error:None,connection:None};
 let server=thread::spawn(move||{let mut first=Value::Null;for n in 0..3{
  let(stream,_)=listener.accept().unwrap();let mut ws=tungstenite::accept(stream).unwrap();let req:Value=serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();assert_eq!(req["tag"],"orchestration.dispatchCommand");let c=&req["payload"];assert_eq!(c["threadId"],"chat");assert_eq!(c["requestId"],opaque_request_id());assert!(c.get("text").is_none());
  if n<2{assert_eq!(c["type"],"runtime-request.respond");assert_eq!(c["answers"],json!({"when":" before ","providers":["codex","other"],"details":""}));if n==0{first=c.clone()}else{assert_eq!(*c,first)}}else{assert_eq!(c["type"],"thread.user-input.dismiss");assert!(c.get("answers").is_none())}
  ws.send(tungstenite::Message::Text(json!({"_tag":"Exit","requestId":"1","exit":{"_tag":"Success","value":{"sequence":n}}}).to_string().into())).unwrap();
 }});
 let mut input=json!({"op":"question","action":"respond","thread":"chat","runtimeRequestId":opaque_request_id(),"requestId":uuid::Uuid::new_v4().to_string(),"answers":{"when":" before ","providers":["codex","other"],"details":""}});
 write::dispatch(&e,&input).unwrap();write::dispatch(&e,&input).unwrap();input["answers"]["when"]=json!("changed");assert_eq!(write::dispatch(&e,&input).unwrap_err(),"retry_payload_changed");input["requestId"]=json!(uuid::Uuid::new_v4().to_string());input["action"]=json!("dismiss");write::dispatch(&e,&input).unwrap();server.join().unwrap();std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn pending_question_reaches_the_normalized_chat(){
 let result=normalize_messages(&json!({"projection":{"messages":[],"runtimeRequests":[{"id":"q","kind":"user_input","status":"pending","responseCapability":{"type":"message"}}],"turnItems":[{"type":"user_input_request","requestId":"q","questions":[{"id":"choice","header":"When","question":"When?","options":[]}]}]}})).unwrap();assert_eq!(result["questions"][0]["requestId"],"q");assert_eq!(result["questions"][0]["dismissible"],true);
}
