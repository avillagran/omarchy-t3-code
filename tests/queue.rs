use omarchy_t3_client::*;
use serde_json::{json,Value};
use std::{net::TcpListener,thread,os::unix::fs::PermissionsExt};
#[test]
fn native_queue_actions_are_exact_and_edit_retry_is_stable(){
 let root=std::env::temp_dir().join(format!("t3-queue-{}",uuid::Uuid::new_v4()));std::fs::create_dir_all(&root).unwrap();std::env::set_var("XDG_STATE_HOME",&root);let token=root.join("token");std::fs::write(&token,"synthetic-private-token").unwrap();std::fs::set_permissions(&token,std::fs::Permissions::from_mode(0o600)).unwrap();
 let listener=TcpListener::bind("127.0.0.1:0").unwrap();let e=Environment{id:"fixture".into(),name:"Fixture".into(),base_url:format!("http://{}",listener.local_addr().unwrap()),token_file:token,write_token_file:None,bearer:None,connection_error:None,connection:None};
 let server=thread::spawn(move||{let mut edit=Value::Null;for (n,typ) in ["queued-run.cancel","queued-run.reorder","queued-message.promote-to-steer","queue.resume","queued-run.edit","queued-run.edit"].iter().enumerate(){
  let(stream,_)=listener.accept().unwrap();let mut ws=tungstenite::accept(stream).unwrap();let req:Value=serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();assert_eq!(req["tag"],"orchestration.dispatchCommand");let c=&req["payload"];assert_eq!(c["type"],*typ);assert_eq!(c["threadId"],"chat");assert!(c.get("modelSelection").is_none());
  if n==1{assert_eq!(c["beforeRunId"],"earlier")}if n==2{assert_eq!(c["queuedRunId"],"queued");assert_eq!(c["targetRunId"],"active")}
  if n>=4{assert_eq!(c["text"],"Edited queued text");assert_eq!(c["attachments"][0]["id"],"kept-file");assert_eq!(c["context"]["refs"],json!([]));if n==4{edit=c.clone()}else{assert_eq!(*c,edit)}}
  ws.send(tungstenite::Message::Text(json!({"_tag":"Exit","requestId":"1","exit":{"_tag":"Success","value":{"sequence":n}}}).to_string().into())).unwrap();
 }});
 for action in ["cancel","reorder","steer","resume"]{write::dispatch(&e,&json!({"op":"queue","action":action,"thread":"chat","requestId":uuid::Uuid::new_v4().to_string(),"runId":"queued","beforeRunId":"earlier","targetRunId":"active"})).unwrap();}
 let edit=json!({"op":"queue","action":"edit","thread":"chat","requestId":uuid::Uuid::new_v4().to_string(),"runId":"queued","text":"Edited queued text","attachments":[{"type":"file","id":"kept-file","name":"file.txt","mimeType":"text/plain","sizeBytes":5}],"context":{"refs":[]}});write::dispatch(&e,&edit).unwrap();write::dispatch(&e,&edit).unwrap();let mut changed=edit;changed["text"]=json!("Different");assert_eq!(write::dispatch(&e,&changed).unwrap_err(),"retry_payload_changed");server.join().unwrap();std::fs::remove_dir_all(root).unwrap();
}
