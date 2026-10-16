use omarchy_t3_client::*;
use serde_json::{json,Value};
use std::{io::{Read,Write},net::TcpListener,os::unix::fs::PermissionsExt,thread};
#[test]
fn suggestions_use_server_workspace_and_cache_without_dispatching(){
 let root=std::env::temp_dir().join(format!("t3-suggest-{}",uuid::Uuid::new_v4()));std::fs::create_dir_all(&root).unwrap();let token=root.join("token");std::fs::write(&token,"synthetic-token").unwrap();std::fs::set_permissions(&token,std::fs::Permissions::from_mode(0o600)).unwrap();
 let listener=TcpListener::bind("127.0.0.1:0").unwrap();let e=Environment{id:format!("fixture-{}",uuid::Uuid::new_v4()),name:"Fixture".into(),base_url:format!("http://{}",listener.local_addr().unwrap()),token_file:token,write_token_file:None,bearer:None,connection_error:None,connection:None};
 let server=thread::spawn(move||{
  let(mut stream,_)=listener.accept().unwrap();let mut req=vec![];while !req.ends_with(b"\r\n\r\n"){let mut b=[0];stream.read_exact(&mut b).unwrap();req.push(b[0])}assert!(String::from_utf8_lossy(&req).starts_with("GET /api/orchestration/shell "));
  let shell=json!({"threads":[{"id":"chat","projectId":"p","worktreePath":"/remote/worktree"}],"projects":[{"id":"p","workspaceRoot":"/wrong/root"}]}).to_string();write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",shell.len(),shell).unwrap();drop(stream);
  for phase in 0..2 {
   let(stream,_)=listener.accept().unwrap();let mut ws=tungstenite::accept(stream).unwrap();let req:Value=serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();assert_eq!(req["payload"]["cwd"],"/remote/worktree");
   let value=if phase==0{assert_eq!(req["tag"],"projects.searchEntries");assert_eq!(req["payload"]["limit"],40);json!({"entries":[{"path":"docs/My File.md","kind":"file"},{"path":"src","kind":"directory"}],"truncated":false})}else{assert_eq!(req["tag"],"server.refreshProviders");assert_eq!(req["payload"]["instanceId"],"codex");json!({"auth":"do-not-export","providers":[{"instanceId":"codex","enabled":true,"skills":[{"name":"global","enabled":true}],"workspaceSnapshots":[{"cwd":"/remote/worktree","skills":[{"name":"review","enabled":true,"path":"private-path"}],"slashCommands":[{"name":"compact"}]}]}]})};
   ws.send(tungstenite::Message::Text(json!({"_tag":"Exit","requestId":"1","exit":{"_tag":"Success","value":value}}).to_string().into())).unwrap();
  }
 });
 let input=json!({"thread":"chat","instanceId":"codex","kind":"path","query":""});let paths=suggestions::query(&e,&input).unwrap();assert_eq!(paths["items"][0]["insert"],"[My File.md](docs/My%20File.md) ");assert_eq!(paths["items"][1]["description"],"directory");
 let mut input=input;input["kind"]=json!("skill");let skills=suggestions::query(&e,&input).unwrap();assert_eq!(skills["items"][0]["insert"],"$review ");assert!(!skills.to_string().contains("private-path"));input["kind"]=json!("command");input["query"]=json!("compact");assert_eq!(suggestions::query(&e,&input).unwrap()["items"][0]["insert"],"/compact ");server.join().unwrap();std::fs::remove_dir_all(root).unwrap();
}
