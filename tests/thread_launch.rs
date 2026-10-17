use omarchy_t3_client::{write,Environment};
use serde_json::{json,Value};
use std::{net::TcpListener,os::unix::fs::PermissionsExt};
#[test]
fn native_launch_owns_workspace_and_initial_message_with_exact_retries(){
 let root=std::env::temp_dir().join(format!("ot3-launch-{}",uuid::Uuid::new_v4()));std::fs::create_dir_all(&root).unwrap();std::env::set_var("XDG_STATE_HOME",&root);
 let token=root.join("token");std::fs::write(&token,"synthetic-private-token").unwrap();std::fs::set_permissions(&token,std::fs::Permissions::from_mode(0o600)).unwrap();
 let server=TcpListener::bind("127.0.0.1:0").unwrap();let e=Environment{id:"fixture".into(),name:"Fixture".into(),base_url:format!("http://{}",server.local_addr().unwrap()),token_file:token.clone(),write_token_file:Some(token),bearer:None,connection_error:None,connection:None};
 let remote=std::thread::spawn(move||{
  let mut original=None;
  for i in 0..4{
   let(stream,_)=server.accept().unwrap();let mut ws=tungstenite::accept(stream).unwrap();let req:Value=serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();assert_eq!(req["tag"],"orchestration.launchThread");let p=&req["payload"];
   assert!(p["threadId"].as_str().unwrap().starts_with("omarchy-thread-"));assert!(p["type"].is_null());
   if i<2{
    assert_eq!(p["workspaceStrategy"],json!({"type":"worktree","baseRef":"main"}));
    assert_eq!(p["initialMessage"]["text"],"Independent prompt");assert_eq!(p["initialMessage"]["attachments"],json!([]));assert!(p["workspaceStrategy"]["branch"].is_null());
    if i==0{original=Some(p.clone());continue}assert_eq!(original.as_ref().unwrap(),p);
   }else{assert!(p["initialMessage"].is_null());assert_eq!(p["workspaceStrategy"]["type"],if i==2{"root"}else{"existing_worktree"});}
   ws.send(tungstenite::Message::Text(json!({"_tag":"Exit","requestId":"1","exit":{"_tag":"Success","value":{"threadId":p["threadId"],"resumed":i==1,"projection":{"thread":{"id":p["threadId"]}}}}}).to_string().into())).unwrap();
  }
 });
 let launch=json!({"action":"new-thread","requestId":uuid::Uuid::new_v4().to_string(),"projectId":"p","title":"New thread","modelSelection":{"instanceId":"codex","model":"fixture"},"workspaceStrategy":{"type":"worktree","baseRef":"main"},"launchWithMessage":true,"text":"Independent prompt","attachments":[],"sendRequestId":uuid::Uuid::new_v4().to_string()});
 assert!(write::manage(&e,&launch).is_err());let result=write::manage(&e,&launch).unwrap();assert_eq!(result["action"],"new-thread");
 let mut changed=launch.clone();changed["text"]=json!("Changed retry");assert_eq!(write::manage(&e,&changed).unwrap_err().to_string(),"retry_payload_changed");
 for strategy in [json!({"type":"root"}),json!({"type":"existing_worktree","worktreePath":"/remote/worktree"})]{let mut request=launch.clone();request["requestId"]=json!(uuid::Uuid::new_v4().to_string());request["launchWithMessage"]=json!(false);request["workspaceStrategy"]=strategy;write::manage(&e,&request).unwrap();}
 let mut invalid=launch;invalid["workspaceStrategy"]["baseRef"]=json!("");assert_eq!(write::manage(&e,&invalid).unwrap_err().to_string(),"invalid_workspace");
 remote.join().unwrap();std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn approval_is_saved_in_native_settings_and_project_overrides_are_respected(){
 let root=std::env::temp_dir().join(format!("ot3-workspace-setting-{}",uuid::Uuid::new_v4()));std::fs::create_dir_all(&root).unwrap();let token=root.join("token");std::fs::write(&token,"synthetic-private-token").unwrap();std::fs::set_permissions(&token,std::fs::Permissions::from_mode(0o600)).unwrap();
 let server=TcpListener::bind("127.0.0.1:0").unwrap();let e=Environment{id:"fixture".into(),name:"Fixture".into(),base_url:format!("http://{}",server.local_addr().unwrap()),token_file:token.clone(),write_token_file:Some(token),bearer:None,connection_error:None,connection:None};
 let remote=std::thread::spawn(move||{
  for i in 0..7{
   let(stream,_)=server.accept().unwrap();let mut ws=tungstenite::accept(stream).unwrap();let req:Value=serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();assert_eq!(req["tag"],if i==1{"server.updateSettings"}else{"server.getSettings"});
   if i==1{assert_eq!(req["payload"],json!({"patch":{"defaultThreadEnvMode":"worktree"}}));}
   if i==6{continue}
   let raw=match i{0=>json!({"defaultThreadEnvMode":null}),3=>json!({"defaultThreadEnvMode":"worktree","projectSettingsOverrides":{"p":{"defaultThreadEnvMode":"local"}}}),4=>json!({"defaultThreadEnvMode":"local"}),5=>json!({"defaultThreadEnvMode":"worktree","newWorktreesStartFromOrigin":true,"projectSettingsOverrides":{"p":{"newWorktreesStartFromOrigin":false}}}),_=>json!({"defaultThreadEnvMode":"worktree","providerInstances":{"secret":"never-export"}})};
   ws.send(tungstenite::Message::Text(json!({"_tag":"Exit","requestId":"1","exit":{"_tag":"Success","value":raw}}).to_string().into())).unwrap();
  }
 });
 let read=json!({"action":"workspace-read","projectId":"p"});
 assert!(write::workspace_preference(&e,&read).unwrap()["mode"].is_null());
 let enabled=write::workspace_preference(&e,&json!({"action":"workspace-update","projectId":"p","mode":"worktree"})).unwrap();assert_eq!(enabled["mode"],"worktree");assert!(!enabled.to_string().contains("never-export"));
 assert_eq!(write::workspace_preference(&e,&read).unwrap()["mode"],"worktree");
 assert_eq!(write::workspace_preference(&e,&read).unwrap()["mode"],"local");
 assert_eq!(write::workspace_preference(&e,&read).unwrap()["mode"],"local");
 assert_eq!(write::workspace_preference(&e,&read).unwrap()["startFromOrigin"],false);
 assert!(write::workspace_preference(&e,&read).is_err());
 assert!(write::workspace_preference(&e,&json!({"action":"workspace-update","projectId":"p","mode":"arbitrary"})).is_err());
 remote.join().unwrap();std::fs::remove_dir_all(root).unwrap();
}
