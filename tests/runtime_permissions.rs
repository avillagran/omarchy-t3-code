use omarchy_t3_client::{write,Environment};
use serde_json::{json,Value};
use std::{net::TcpListener,os::unix::fs::PermissionsExt};
#[test]
fn permissions_precede_messages_and_keep_the_same_identity_on_retry(){
 let root=std::env::temp_dir().join(format!("ot3-permissions-{}",uuid::Uuid::new_v4()));std::fs::create_dir_all(&root).unwrap();std::env::set_var("XDG_STATE_HOME",&root);
 let token=root.join("token");std::fs::write(&token,"synthetic-private-token").unwrap();std::fs::set_permissions(&token,std::fs::Permissions::from_mode(0o600)).unwrap();
 let server=TcpListener::bind("127.0.0.1:0").unwrap();let env=Environment{id:"fixture".into(),name:"Fixture".into(),base_url:format!("http://{}",server.local_addr().unwrap()),token_file:token.clone(),write_token_file:Some(token),bearer:None,connection_error:None,connection:None};
 let modes=["approval-required","auto-accept-edits","auto","full-access"];
 let worker=std::thread::spawn(move||{
  for mode in modes{let mut first=Value::Null;for retry in 0..2{for kind in ["thread.runtime-mode.set","message.dispatch"]{
   let(stream,_)=server.accept().unwrap();let mut ws=tungstenite::accept(stream).unwrap();let req:Value=serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();assert_eq!(req["tag"],"orchestration.dispatchCommand");let c=&req["payload"];assert_eq!(c["type"],kind);assert_eq!(c["threadId"],"thread");
   if kind=="thread.runtime-mode.set"{assert_eq!(c["runtimeMode"],mode);assert!(c["commandId"].as_str().unwrap().ends_with("-runtime-mode"));if retry==0{first=c.clone()}else{assert_eq!(&first,c)}}else{assert!(c.get("runtimeMode").is_none());assert_eq!(c["text"],"fixture prompt")}
   ws.send(tungstenite::Message::Text(json!({"_tag":"Exit","requestId":"1","exit":{"_tag":"Success","value":{"sequence":1}}}).to_string().into())).unwrap();
  }}}
  for mode in modes{
   let(stream,_)=server.accept().unwrap();let mut ws=tungstenite::accept(stream).unwrap();let req:Value=serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();assert_eq!(req["payload"]["type"],"thread.create");assert_eq!(req["payload"]["runtimeMode"],mode);
   ws.send(tungstenite::Message::Text(json!({"_tag":"Exit","requestId":"1","exit":{"_tag":"Success","value":{"sequence":2}}}).to_string().into())).unwrap();
  }
 });
 for mode in modes{let input=json!({"op":"send","requestId":uuid::Uuid::new_v4().to_string(),"thread":"thread","runtimeMode":mode,"text":"fixture prompt","attachments":[],"modelSelection":{"instanceId":"fixture","model":"fixture"}});write::dispatch(&env,&input).unwrap();write::dispatch(&env,&input).unwrap();let mut changed=input.clone();changed["runtimeMode"]=json!(if mode=="full-access"{"approval-required"}else{"full-access"});assert_eq!(write::dispatch(&env,&changed).unwrap_err(),"retry_payload_changed")}
 for mode in modes{write::manage(&env,&json!({"requestId":uuid::Uuid::new_v4().to_string(),"action":"new-thread","projectId":"p","title":"Fixture","modelSelection":{"instanceId":"fixture","model":"fixture"},"runtimeMode":mode})).unwrap();}
 assert_eq!(write::dispatch(&env,&json!({"op":"send","requestId":uuid::Uuid::new_v4().to_string(),"thread":"thread","runtimeMode":"invalid","text":"fixture prompt","attachments":[]})).unwrap_err(),"invalid_runtime_mode");
 worker.join().unwrap();std::fs::remove_dir_all(root).unwrap();
}
