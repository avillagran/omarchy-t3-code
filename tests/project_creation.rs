use omarchy_t3_client::{write,Environment};
use serde_json::{json,Value};
use std::{net::TcpListener,os::unix::fs::PermissionsExt};
#[test]
fn native_sources_and_creation_are_environment_owned_and_never_replayed_blindly(){
 let root=std::env::temp_dir().join(format!("ot3-project-{}",uuid::Uuid::new_v4()));std::fs::create_dir_all(&root).unwrap();std::env::set_var("XDG_STATE_HOME",&root);
 let token=root.join("token");std::fs::write(&token,"synthetic-private-token").unwrap();std::fs::set_permissions(&token,std::fs::Permissions::from_mode(0o600)).unwrap();
 let server=TcpListener::bind("127.0.0.1:0").unwrap();let e=Environment{id:"fixture".into(),name:"Fixture".into(),base_url:format!("http://{}",server.local_addr().unwrap()),token_file:token.clone(),write_token_file:Some(token),bearer:None,connection_error:None,connection:None};
 let remote=std::thread::spawn(move||{
  for method in ["server.getConfig","server.discoverSourceControl","projects.createNew","projectClone.start","projectClone.start"]{
   let(stream,_)=server.accept().unwrap();let mut ws=tungstenite::accept(stream).unwrap();let req:Value=serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();assert_eq!(req["tag"],method);
   let value=match method{
    "server.getConfig"=>json!({"newProjectsRoot":"/remote/Projects","settings":{"addProjectBaseDirectory":"/remote/Work"},"environment":{"capabilities":{"projectCloneTracking":true}},"secret":"must-not-leak"}),
    "server.discoverSourceControl"=>json!({"sourceControlProviders":[{"kind":"github","status":"available","auth":{"status":"authenticated","detail":"must-not-leak"}}]}),
    "projects.createNew"=>{assert_eq!(req["payload"]["name"],"Fresh");json!({"projectId":"fresh","workspaceRoot":"/remote/Projects/Fresh"})},
    _=>{let p=&req["payload"];assert_eq!(p["destinationPath"],"/remote/Work/repo");assert!(p["projectId"].as_str().unwrap().starts_with("omarchy-project-"));if p["provider"].is_null(){assert_eq!(p["remoteUrl"],"https://github.com/owner/repo.git");}else{assert_eq!(p["provider"],"github");assert_eq!(p["repository"],"owner/repo");}json!({"projectId":p["projectId"],"cwd":p["destinationPath"]})}
   };
   ws.send(tungstenite::Message::Text(json!({"_tag":"Exit","requestId":"1","exit":{"_tag":"Success","value":value}}).to_string().into())).unwrap();
  }
 });
 let sources=write::project_sources(&e).unwrap();assert_eq!(sources["newProjectsRoot"],"/remote/Projects");assert_eq!(sources["baseDirectory"],"/remote/Work");assert_eq!(sources["providers"][0]["auth"]["status"],"authenticated");assert!(!sources.to_string().contains("must-not-leak"));
 let create=json!({"requestId":uuid::Uuid::new_v4().to_string(),"action":"create-project","name":"Fresh"});let result=write::manage(&e,&create).unwrap();assert_eq!(result["projectId"],"fresh");assert_eq!(write::manage(&e,&create).unwrap(),result);
 for source in ["github","url"]{let clone=json!({"requestId":uuid::Uuid::new_v4().to_string(),"action":"clone-project","source":source,"repository":"owner/repo","workspaceRoot":"/remote/Work/repo","createdAt":"2026-10-16T12:00:00.000Z"});let result=write::manage(&e,&clone).unwrap();assert_eq!(write::manage(&e,&clone).unwrap(),result);}
 remote.join().unwrap();std::fs::remove_dir_all(root).unwrap();
}
