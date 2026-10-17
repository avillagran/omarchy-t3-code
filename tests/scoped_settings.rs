use omarchy_t3_client::{scoped_settings as s,Environment,protocol};
use serde_json::{json,Value};
#[test]
fn native_project_override_precedence_sources_and_folded_reset(){
 let raw=json!({"projectSettingsFolded":true,"defaultThreadEnvMode":null,"worktreeSubmodules":null,"defaultRuntimeMode":"full-access","defaultModelSelection":{"instanceId":"global","model":"one"},"providerInstances":{"disabled":{"enabled":false}},"projectSettingsOverrides":{"p":{"defaultModelSelection":{"instanceId":"disabled","model":"other"},"defaultRuntimeMode":"approval-required"}}});
 let project=json!({"id":"p","defaultThreadEnvMode":"worktree"});let file=json!({"defaultThreadEnvMode":"local","worktreeSubmodules":"none"});
 let(e,sources,_)=s::effective(&raw,Some(&project),Some(&file));assert_eq!(e["defaultModelSelection"]["instanceId"],"global");assert_eq!(e["defaultRuntimeMode"],"approval-required");assert_eq!(sources["defaultRuntimeMode"],"project");assert_eq!(e["defaultThreadEnvMode"],"local");assert_eq!(sources["defaultThreadEnvMode"],"t3.json");
 let mut legacy=raw.clone();legacy["projectSettingsFolded"]=json!(false);assert_eq!(s::effective(&legacy,Some(&project),None).0["defaultThreadEnvMode"],"worktree");
 let mut automatic=raw.clone();automatic["projectSettingsOverrides"]["p"]["defaultModelSelection"]=Value::Null;assert!(s::effective(&automatic,Some(&project),None).0["defaultModelSelection"].is_null());
}
#[test]
fn project_patch_keeps_other_overrides_and_completes_native_nested_values(){
 let raw=json!({"projectSettingsFolded":true,"sourceControlWritingStyle":{"mode":"repo_conventions","customInstructions":"keep","followChangeRequestTemplates":true},"projectSettingsOverrides":{"p":{"defaultModelSelection":null,"providerInstances":{"custom":{"enabled":true}}},"unrelated":{"defaultRuntimeMode":"full-access"}}});let project=json!({"id":"p"});
 let p=s::project_patch(&raw,&project,"sourceControlWritingStyle.mode",&json!({"patch":{"sourceControlWritingStyle":{"mode":"custom"}}}),false).unwrap();
 let e=&p["patch"]["projectSettingsOverrides"]["p"];assert_eq!(e["sourceControlWritingStyle"]["customInstructions"],"keep");assert_eq!(e["sourceControlWritingStyle"]["mode"],"custom");assert_eq!(e["providerInstances"]["custom"]["enabled"],true);assert!(e["defaultModelSelection"].is_null());assert!(p["patch"]["projectSettingsOverrides"].get("unrelated").is_none());
 let p=s::project_patch(&raw,&project,"defaultModelSelection",&json!({}),true).unwrap();assert!(p["patch"]["projectSettingsOverrides"]["p"].get("defaultModelSelection").is_none());
 assert!(s::project_patch(&raw,&project,"enableProviderUpdateChecks",&json!({"patch":{"enableProviderUpdateChecks":false}}),false).is_err());
}
#[test]
fn scopes_mixed_values_and_jsonc_keep_data_and_reject_invalid_files(){
 let fields=|v,source|json!([{"key":"defaultRuntimeMode","value":v,"source":source,"scopeWritable":true,"overridden":source=="project"}]);
 let a=s::aggregate(&[json!({"fields":fields("full-access","project")}),json!({"fields":fields("approval-required","environment")})]);assert_eq!(a["fields"][0]["mixed"],true);assert_eq!(a["fields"][0]["source"],"mixed");assert_eq!(a["fields"][0]["overridden"],true);
 let f=s::project_file("{/* note */\"defaultThreadEnvMode\":\"worktree\",// note\n\"worktreeSubmodules\":\"none\",\"scripts\":[{\"name\":\"dev\",\"command\":\"echo https://host/* literal */\",},],}").unwrap();assert_eq!(f["scripts"][0]["command"],"echo https://host/* literal */");assert_eq!(f["defaultThreadEnvMode"],"worktree");
 assert!(s::project_file("{\"defaultThreadEnvMode\":\"future\"}").is_none());assert!(s::project_file("{\"scripts\":[{\"command\":\"echo\"}],\"worktreeSubmodules\":\"none\"}").is_none());
 for targets in [json!([]),json!([{"environment":"gone","projectId":null}]),json!([{"environment":"env","projectId":null},{"environment":"env","projectId":"p"}])]{assert!(s::execute_in(&[],&json!({"action":"update","key":"defaultRuntimeMode","value":"full-access","scope":{"targets":targets}})).is_err())}
}
#[test]
fn project_update_uses_native_entry_and_readback_and_keeps_credentials_private(){
 use std::{net::TcpListener,sync::{Arc,Mutex},thread,io::{Read,Write}};
 let listener=TcpListener::bind("127.0.0.1:0").unwrap();let base=format!("http://{}",listener.local_addr().unwrap());let seen=Arc::new(Mutex::new(vec![]));let capture=seen.clone();
 let server=thread::spawn(move||{
 let mut raw=json!({"projectSettingsFolded":true,"defaultRuntimeMode":"full-access","defaultThreadEnvMode":null,"worktreeSubmodules":null,"providerInstances":{"secret":"DO-NOT-EXPORT"},"projectSettingsOverrides":{"p":{"defaultAutoPull":true}}});
 for _ in 0..7{
 let(mut stream,_)=listener.accept().unwrap();stream.set_read_timeout(Some(std::time::Duration::from_secs(4))).unwrap();let mut peek=[0;1024];let size=stream.peek(&mut peek).unwrap();let header=String::from_utf8_lossy(&peek[..size]);
 if header.starts_with("GET /api/auth/session "){let mut buf=[0;4096];stream.read(&mut buf).unwrap();let body=json!({"authenticated":true,"permissions":["settings:write"]}).to_string();write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();continue}
 if header.starts_with("GET /api/orchestration/shell "){let mut buf=[0;4096];stream.read(&mut buf).unwrap();let body=json!({"projects":[{"id":"p","workspaceRoot":"/repo"}]}).to_string();write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();continue}
 let mut ws=tungstenite::accept(stream).unwrap();let req:Value=serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();capture.lock().unwrap().push(req.clone());
 let result=match req["tag"].as_str().unwrap(){"server.getConfig"=>json!({"environment":{"capabilities":{"projectSettingsOverrides":true}}}),"server.getSettings"=>raw.clone(),"server.updateSettings"=>{assert_eq!(req["payload"]["patch"]["projectSettingsOverrides"]["p"],json!({"defaultAutoPull":true,"defaultRuntimeMode":"approval-required"}));raw["projectSettingsOverrides"]["p"]=req["payload"]["patch"]["projectSettingsOverrides"]["p"].clone();raw.clone()},"projects.readFile"=>json!({"truncated":false,"contents":"{\"worktreeSubmodules\":\"none\"}","byteLength":28}),tag=>panic!("Unexpected native tag {tag}")};
 ws.send(tungstenite::Message::Text(json!({"_tag":"Exit","requestId":"1","exit":{"_tag":"Success","value":result}}).to_string().into())).unwrap();
 }
 });
 let connection=protocol::Connection::new(&base,None,protocol::Credential::bearer("synthetic").unwrap()).unwrap();let env=Environment{id:"env".into(),name:"Env".into(),base_url:base,token_file:"/unused".into(),write_token_file:None,bearer:None,connection_error:None,connection:Some(Arc::new(connection))};
 let result=s::execute_in(&[env],&json!({"action":"update","key":"defaultRuntimeMode","value":"approval-required","scope":{"targets":[{"environment":"env","projectId":"p"}]}})).unwrap();server.join().unwrap();assert_eq!(result["complete"],true);assert_eq!(result["savedTargets"].as_array().unwrap().len(),1);assert!(!result.to_string().contains("DO-NOT-EXPORT"));let f=result["fields"].as_array().unwrap().iter().find(|f|f["key"]=="defaultRuntimeMode").unwrap();assert_eq!(f["value"],"approval-required");assert_eq!(f["source"],"project");assert_eq!(seen.lock().unwrap().len(),5);
}

#[test]
fn multi_environment_partial_write_reports_exact_success_and_denies_readonly_target(){
 use std::{net::TcpListener,sync::Arc,thread,io::{Read,Write}};
 fn env(base:String,id:&str)->Environment{let c=protocol::Connection::new(&base,None,protocol::Credential::bearer("synthetic").unwrap()).unwrap();Environment{id:id.into(),name:id.into(),base_url:base,token_file:"/unused".into(),write_token_file:None,bearer:None,connection_error:None,connection:Some(Arc::new(c))}}
 let writable=TcpListener::bind("127.0.0.1:0").unwrap();let readonly=TcpListener::bind("127.0.0.1:0").unwrap();let environments=vec![env(format!("http://{}",writable.local_addr().unwrap()),"one"),env(format!("http://{}",readonly.local_addr().unwrap()),"two")];
 let server=thread::spawn(move||{let mut raw=json!({"defaultRuntimeMode":"full-access"});let mut writes=0;
 for _ in 0..5{let(mut stream,_)=writable.accept().unwrap();stream.set_read_timeout(Some(std::time::Duration::from_secs(4))).unwrap();let mut peek=[0;512];let size=stream.peek(&mut peek).unwrap();if String::from_utf8_lossy(&peek[..size]).starts_with("GET /api/auth/session "){let mut buf=[0;2048];stream.read(&mut buf).unwrap();let body=json!({"authenticated":true,"scopes":["settings:write"]}).to_string();write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();continue}
 let mut ws=tungstenite::accept(stream).unwrap();let req:Value=serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();let result=match req["tag"].as_str().unwrap(){"server.getConfig"=>json!({}),"server.getSettings"=>raw.clone(),"server.updateSettings"=>{assert_eq!(req["payload"],json!({"patch":{"defaultRuntimeMode":"approval-required"}}));writes+=1;raw["defaultRuntimeMode"]=json!("approval-required");raw.clone()},_=>panic!("unexpected RPC")};ws.send(tungstenite::Message::Text(json!({"_tag":"Exit","requestId":"1","exit":{"_tag":"Success","value":result}}).to_string().into())).unwrap();}assert_eq!(writes,1);
 });
 let denied=thread::spawn(move||{let(mut stream,_)=readonly.accept().unwrap();let mut buf=[0;2048];stream.read(&mut buf).unwrap();let body=json!({"authenticated":true,"permissions":["orchestration:read"]}).to_string();write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();});
 let result=s::execute_in(&environments,&json!({"action":"update","key":"defaultRuntimeMode","value":"approval-required","scope":{"targets":[{"environment":"one","projectId":null},{"environment":"two","projectId":null}]}})).unwrap();server.join().unwrap();denied.join().unwrap();assert_eq!(result["complete"],false);assert_eq!(result["writable"],false);assert_eq!(result["savedTargets"],json!([{"environment":"one","projectId":null}]));assert_eq!(result["errors"][0]["target"]["environment"],"two");assert_eq!(result["errors"][0]["error"],"settings_write_forbidden");
}
