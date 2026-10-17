use omarchy_t3_client::{Environment,protocol,write};
use serde_json::{json,Value};
use std::{net::TcpListener,sync::{Arc,Mutex},thread,io::{Read,Write},fs};
#[test]
fn native_preparation_and_durable_mode_before_send_preserve_exact_retry_payload(){
 let root=std::env::temp_dir().join(format!("ot3-plan-transport-{}",uuid::Uuid::new_v4()));fs::create_dir_all(root.join("native/userdata")).unwrap();fs::write(root.join("native/userdata/client-settings.json"),"{\"planModeEnabled\":true}").unwrap();std::env::set_var("T3CODE_HOME",root.join("native"));std::env::set_var("XDG_STATE_HOME",root.join("state"));
 let listener=TcpListener::bind("127.0.0.1:0").unwrap();let base=format!("http://{}",listener.local_addr().unwrap());let seen=Arc::new(Mutex::new(vec![]));let captured=seen.clone();
 let server=thread::spawn(move||{
 for _ in 0..7{
 let(mut stream,_)=listener.accept().unwrap();stream.set_read_timeout(Some(std::time::Duration::from_secs(4))).unwrap();let mut peek=[0;512];let n=stream.peek(&mut peek).unwrap();
 if String::from_utf8_lossy(&peek[..n]).starts_with("GET /api/auth/session "){
 let mut buf=[0;4096];stream.read(&mut buf).unwrap();let body=json!({"authenticated":true,"permissions":["orchestration:operate"]}).to_string();write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();continue}
 if String::from_utf8_lossy(&peek[..n]).starts_with("GET /api/orchestration/threads/source/bounded?compactTurnItems=1 "){
 let mut buf=[0;4096];stream.read(&mut buf).unwrap();let body=json!({"projection":{"thread":{"id":"source","projectId":"p","interactionMode":"plan","branch":"feature/one","worktreePath":"/repo-wt"},"runs":[{"id":"run","status":"completed"}],"runtimeRequests":[],"turnItems":[{"type":"proposed_plan","planId":"plan","updatedAt":"2026-10-17T10:00:00Z"}],"plans":[{"id":"plan","threadId":"source","runId":"run","kind":"proposed_plan","status":"active","markdown":"# Native plan\nDo exactly this"}]}}).to_string();write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();continue}
 let mut ws=tungstenite::accept(stream).unwrap();let req:Value=serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();captured.lock().unwrap().push(req.clone());
 let value=if req["tag"]=="server.getConfig"{json!({"providers":[{"instanceId":"work","enabled":true,"status":"ready","showInteractionModeToggle":true,"models":[{"slug":"m","name":"M"}]}]})}else{assert_eq!(req["tag"],"orchestration.dispatchCommand");json!({})};
 ws.send(tungstenite::Message::Text(json!({"_tag":"Exit","requestId":"1","exit":{"_tag":"Success","value":value}}).to_string().into())).unwrap();
 }
 });
 let c=protocol::Connection::new(&base,None,protocol::Credential::bearer("synthetic").unwrap()).unwrap();let env=Environment{id:"fixture".into(),name:"Fixture".into(),base_url:base,token_file:"/unused".into(),write_token_file:None,bearer:None,connection_error:None,connection:Some(Arc::new(c))};
 let prepared=write::prepare_plan_follow_up(&env,&json!({"thread":"source","planId":"plan","interactionMode":"plan","draftText":"","hasAttachments":false,"modelSelection":{"instanceId":"work","model":"m"}})).unwrap();assert_eq!(prepared["text"],"PLEASE IMPLEMENT THIS PLAN:\n# Native plan\nDo exactly this");assert_eq!(prepared["sourcePlanRef"],json!({"threadId":"source","planId":"plan"}));assert_eq!(prepared["worktreePath"],"/repo-wt");
 let id=uuid::Uuid::new_v4().to_string();let input=json!({"op":"send","environment":"fixture","thread":"destination","requestId":id,"text":prepared["text"],"attachments":[],"interactionMode":"default","sourcePlanRef":prepared["sourcePlanRef"],"modelSelection":{"instanceId":"work","model":"m"}});
 write::dispatch(&env,&input).unwrap();write::dispatch(&env,&input).unwrap();server.join().unwrap();
 let calls=seen.lock().unwrap();assert_eq!(calls.len(),5);assert_eq!(calls[1]["payload"]["type"],"thread.interaction-mode.set");assert_eq!(calls[1]["payload"]["interactionMode"],"default");assert_eq!(calls[2]["payload"]["type"],"message.dispatch");assert_eq!(calls[2]["payload"]["sourcePlanRef"],prepared["sourcePlanRef"]);assert_eq!(calls[1],calls[3]);assert_eq!(calls[2],calls[4]);
 let before=calls.len();drop(calls);let mut changed=input.clone();changed["interactionMode"]=json!("plan");assert_eq!(write::dispatch(&env,&changed).unwrap_err(),"retry_payload_changed");assert_eq!(seen.lock().unwrap().len(),before);
 let mut invalid=input.clone();invalid["requestId"]=json!(uuid::Uuid::new_v4().to_string());invalid["interactionMode"]=json!("future");assert_eq!(write::dispatch(&env,&invalid).unwrap_err(),"invalid_interaction_mode");fs::remove_dir_all(root).unwrap();
}
