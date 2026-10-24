use omarchy_t3_client::{native_settings as settings, Environment};
use serde_json::{json,Value};
use std::{net::TcpListener,sync::{Arc,Mutex},thread};
fn config()->Value{json!({"settings":{"favorites":[]},"providers":[{"enabled":true,"instanceId":"work","driver":"codex","status":"ready","models":[{"slug":"m","name":"Model","capabilities":{"optionDescriptors":[{"id":"effort","type":"select","options":[{"id":"high","label":"High"}]},{"id":"fast","type":"boolean"}]}}]}]})}
#[test]
fn native_default_model_and_permissions_are_typed_and_secret_free(){
 let raw=json!({"defaultModelSelection":{"instanceId":"work","model":"m","options":[{"id":"effort","value":"high"}],"credential":"NEVER-EXPORT"},"defaultRuntimeMode":"full-access","providerInstances":{"private":"NEVER-EXPORT"},"sourceControlWriterModelSelection":null});
 let view=settings::snapshot(&raw);assert!(!view.to_string().contains("NEVER-EXPORT"));
 let rows=view["fields"].as_array().unwrap();let model=rows.iter().find(|f|f["key"]=="defaultModelSelection").unwrap();assert_eq!(model["value"],json!({"instanceId":"work","model":"m","options":[{"id":"effort","value":"high"}]}));assert_eq!(model["resetValue"],Value::Null);
 assert!(rows.iter().any(|f|f["key"]=="defaultThreadEnvMode"&&f["value"].is_null()));
 for (key,value) in [("defaultModelSelection",Value::Null),("defaultRuntimeMode",json!("approval-required")),("defaultThreadEnvMode",json!("worktree"))]{assert_eq!(settings::patch(&json!({"key":key,"value":value}),None).unwrap()["patch"][key],value)}
 assert!(settings::patch(&json!({"key":"defaultRuntimeMode","value":"unrestricted"}),None).is_err());assert!(settings::patch(&json!({"key":"providerInstances","value":{}}),None).is_err());assert!(settings::patch(&json!({"key":"textGenerationModelSelection","value":null}),None).is_err());
}
#[test]
fn models_and_options_are_checked_against_the_current_native_registry(){
 let selection=json!({"instanceId":"work","model":"m","options":[{"id":"effort","value":"high"},{"id":"fast","value":true}]});let input=json!({"key":"defaultModelSelection","value":selection});
 assert_eq!(settings::patch(&input,Some(&config())).unwrap()["patch"]["defaultModelSelection"],selection);
 for bad in [json!({"instanceId":"wrong","model":"m"}),json!({"instanceId":"work","model":"m","options":[{"id":"effort","value":"unknown"}]}),json!({"instanceId":"work","model":"m","options":[{"id":"effort","value":"high"},{"id":"effort","value":"high"}]}),json!({"instanceId":"work","model":"m","options":[{"id":"fast","value":"true"}]}),json!({"instanceId":"work","model":"m","options":[{"id":"secret","value":"x"}]})]{assert!(settings::patch(&json!({"key":"defaultModelSelection","value":bad}),Some(&config())).is_err())}
 let mut unavailable=config();unavailable["providers"][0]["status"]=json!("error");assert!(settings::patch(&input,Some(&unavailable)).is_err());
}
#[test]
fn native_rpc_receives_exact_patch_and_readback_without_provider_credentials(){
 let listener=TcpListener::bind("127.0.0.1:0").unwrap();let base=format!("http://{}",listener.local_addr().unwrap());
 let seen=Arc::new(Mutex::new(vec![]));let captured=seen.clone();
 let server=thread::spawn(move||{for index in 0..3{let (stream,_)=listener.accept().unwrap();stream.set_read_timeout(Some(std::time::Duration::from_secs(4))).unwrap();let mut ws=tungstenite::accept(stream).unwrap();let request:Value=serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();captured.lock().unwrap().push(request.clone());let result=match index{0=>config(),1|2=>json!({"defaultModelSelection":{"instanceId":"work","model":"m","options":[]},"defaultRuntimeMode":"full-access","providerInstances":{"secret":"NEVER-EXPORT"}}),_=>unreachable!()};ws.send(tungstenite::Message::Text(json!({"_tag":"Exit","requestId":"1","exit":{"_tag":"Success","value":result}}).to_string().into())).unwrap();}});
 let connection=omarchy_t3_client::protocol::Connection::new(&base,None,omarchy_t3_client::protocol::Credential::bearer("synthetic-fixture").unwrap()).unwrap();
 let env=Environment{id:"fixture".into(),name:"Fixture".into(),base_url:base,token_file:"/unused".into(),write_token_file:None,bearer:None,connection_error:None,connection:Some(Arc::new(connection))};
 let view=settings::execute(&env,&json!({"action":"update","key":"defaultModelSelection","value":{"instanceId":"work","model":"m","options":[]}})).unwrap();assert!(!view.to_string().contains("NEVER-EXPORT"));server.join().unwrap();let requests=seen.lock().unwrap();assert_eq!(requests[0]["tag"],"server.getConfig");assert_eq!(requests[1]["tag"],"server.updateSettings");assert_eq!(requests[1]["payload"],json!({"patch":{"defaultModelSelection":{"instanceId":"work","model":"m","options":[]}}}));assert_eq!(requests[2]["tag"],"server.getSettings");
}

#[test]
fn new_threads_resolve_project_defaults_and_disabled_provider_fallback(){
 let raw=json!({"defaultModelSelection":{"instanceId":"global","model":"m"},"defaultRuntimeMode":"full-access","providerInstances":{"global":{"enabled":true},"project":{"enabled":false}},"projectSettingsOverrides":{"p":{"defaultModelSelection":{"instanceId":"project","model":"other"},"defaultRuntimeMode":"approval-required","providerInstances":{"project":{"enabled":true}}},"automatic":{"defaultModelSelection":null},"disabled":{"defaultModelSelection":{"instanceId":"project","model":"other"}}}});
 let p=settings::thread_defaults(&raw,"p");assert_eq!(p["defaultModelSelection"]["instanceId"],"project");assert_eq!(p["defaultRuntimeMode"],"approval-required");
 assert!(settings::thread_defaults(&raw,"automatic")["defaultModelSelection"].is_null());assert_eq!(settings::thread_defaults(&raw,"automatic")["modelDefaultConfigured"],true);
 assert_eq!(settings::thread_defaults(&raw,"disabled")["defaultModelSelection"]["instanceId"],"global");
}

#[test]
fn native_agent_access_fields_and_project_only_device_patch_use_existing_scope_planner(){
 let raw=json!({"enableAgentBrowserAccess":true,"enableAgentDeviceAccess":false,"providerSecret":"PRIVATE"});
 let view=omarchy_t3_client::native_settings::snapshot(&raw);let fields=view["fields"].as_array().unwrap();assert!(!view.to_string().contains("PRIVATE"));for key in ["enableAgentBrowserAccess","enableAgentDeviceAccess"] {assert!(fields.iter().any(|f|f["key"]==key && f["section"]=="integrations"));}
 let input=json!({"key":"enableAgentBrowserAccess","value":false});assert_eq!(omarchy_t3_client::native_settings::patch(&input,None).unwrap(),json!({"patch":{"enableAgentBrowserAccess":false}}));
 assert!(omarchy_t3_client::native_settings::patch(&json!({"key":"enableAgentDeviceAccess","value":"yes"}),None).is_err());
 let project=json!({"id":"p"});let raw=json!({"enableAgentDeviceAccess":false,"projectSettingsOverrides":{"p":{"branchNamePrefix":"keep/"}}});let patch=omarchy_t3_client::native_settings::patch(&json!({"key":"enableAgentDeviceAccess","value":true}),None).unwrap();let update=omarchy_t3_client::scoped_settings::project_patch(&raw,&project,"enableAgentDeviceAccess",&patch,false).unwrap();assert_eq!(update["patch"]["projectSettingsOverrides"]["p"],json!({"branchNamePrefix":"keep/","enableAgentDeviceAccess":true}));
}
#[test]
fn idle_compaction_reads_and_writes_the_form_each_server_understands(){
 let field=|raw:Value|settings::snapshot(&raw)["fields"].as_array().unwrap().iter().find(|f|f["key"]=="idleCompaction").cloned();
 let ids=|f:&Value|f["choices"].as_array().unwrap().iter().map(|c|c["id"].as_str().unwrap().to_string()).collect::<Vec<_>>();
 // Before modes: the boolean switch only, so "Never" cannot be stored.
 let legacy=field(json!({"compactIdleThreads":true})).unwrap();assert_eq!(legacy["value"],"beforeCacheExpires");assert_eq!(ids(&legacy),["onReturn","beforeCacheExpires"]);
 assert_eq!(field(json!({"compactIdleThreads":false})).unwrap()["value"],"onReturn");
 // 0.1.2 kept the mode in the legacy field; 0.1.3 moved it to idleCompaction.
 assert_eq!(field(json!({"compactIdleThreads":"off"})).unwrap()["value"],"off");
 let split=field(json!({"compactIdleThreads":false,"idleCompaction":"off"})).unwrap();assert_eq!(split["value"],"off");assert_eq!(ids(&split).len(),3);
 assert!(field(json!({})).is_none());
 let validated=|mode:&str|settings::patch(&json!({"key":"idleCompaction","value":mode}),None).unwrap();
 assert!(settings::patch(&json!({"key":"idleCompaction","value":"sometimes"}),None).is_err());
 assert_eq!(settings::idle_compaction_patch(&json!({"compactIdleThreads":false,"idleCompaction":"onReturn"}),&validated("off")).unwrap(),json!({"patch":{"idleCompaction":"off"}}));
 assert_eq!(settings::idle_compaction_patch(&json!({"compactIdleThreads":"onReturn"}),&validated("off")).unwrap(),json!({"patch":{"compactIdleThreads":"off"}}));
 assert_eq!(settings::idle_compaction_patch(&json!({"compactIdleThreads":false}),&validated("beforeCacheExpires")).unwrap(),json!({"patch":{"compactIdleThreads":true}}));
 assert!(settings::idle_compaction_patch(&json!({"compactIdleThreads":false}),&validated("off")).is_err());
}
#[test]
fn claude_instances_may_override_the_idle_compaction_mode(){
 let raw=json!({"idleCompaction":"onReturn","providerInstances":{"claudeAgent":{"config":{"idleCompaction":"beforeCacheExpires"}},"work":{"config":{"idleCompaction":"default"}}}});
 assert_eq!(settings::resolve_idle_compaction(&raw,"claudeAgent"),Some("beforeCacheExpires"));
 assert_eq!(settings::resolve_idle_compaction(&raw,"work"),Some("onReturn"));
 assert_eq!(settings::resolve_idle_compaction(&json!({"compactIdleThreads":true}),"claudeAgent"),Some("beforeCacheExpires"));
 assert_eq!(settings::resolve_idle_compaction(&json!({}),"claudeAgent"),None);
}
#[test]
fn thread_title_instructions_are_trimmed_bounded_text(){
 let view=settings::snapshot(&json!({"threadTitleInstructions":"Short titles"}));
 let field=view["fields"].as_array().unwrap().iter().find(|f|f["key"]=="threadTitleInstructions").unwrap();
 assert_eq!((field["section"].as_str(),field["multiline"].as_bool(),field["value"].as_str()),(Some("general"),Some(true),Some("Short titles")));
 assert!(settings::snapshot(&json!({}))["fields"].as_array().unwrap().iter().all(|f|f["key"]!="threadTitleInstructions"));
 assert_eq!(settings::patch(&json!({"key":"threadTitleInstructions","value":"  Lead with the subject  "}),None).unwrap(),json!({"patch":{"threadTitleInstructions":"Lead with the subject"}}));
 for bad in [json!("x".repeat(16385)),json!("bad\0value"),json!(3)]{assert!(settings::patch(&json!({"key":"threadTitleInstructions","value":bad}),None).is_err())}
}
