use omarchy_t3_client::provider_instances::{mutation, projection, confirm_target};
use serde_json::json;
use serde_json::Value;
use std::{io::{Read,Write},net::TcpListener,sync::{Arc,Mutex},thread};

#[test]
fn claude_instance_create_matches_t3_contract_and_preserves_existing_instances() {
    let settings=json!({"providerInstances":{"codex":{"driver":"codex","config":{"apiKey":"private"}},"claudeAgent":{"driver":"claudeAgent","config":{"homePath":""}}}});
    let read=projection(&settings,&json!({"providers":[{"instanceId":"claudeAgent","driver":"claudeAgent","auth":{"status":"authenticated","email":"private@example.com"}}]}),true);
    assert!(read["instances"].as_array().unwrap().iter().any(|v|v["id"]=="claudeAgent"));
    assert!(read.to_string().find("private@example.com").is_none());
    assert!(read.to_string().find("\"private\"").is_none());
    let fallback=projection(&json!({"providerInstances":{}}),&json!({"providers":[{"instanceId":"claudeAgent","installed":true,"auth":{"status":"authenticated","email":"private@example.com"}}]}),true);
    assert!(fallback["instances"].as_array().unwrap().iter().any(|v|v["id"]=="claudeAgent"&&v["authStatus"]=="authenticated"));
    assert!(!fallback.to_string().contains("private@example.com"));
    let input=json!({"action":"create","driver":"claudeAgent","instanceId":"claudeAgent_personal","label":"Claude Personal","config":{"binaryPath":"claude","homePath":"~/.claude_personal","autoCompactWindow":"300000","launchArgs":""}});
    let wire=mutation(&input,&settings).unwrap();
    assert_eq!(wire,json!({"patch":{},"providerInstanceMutation":{"operation":"create","instanceId":"claudeAgent_personal","instance":{"driver":"claudeAgent","enabled":true,"displayName":"Claude Personal","config":{"binaryPath":"claude","homePath":"~/.claude_personal","autoCompactWindow":"300000"}}}}));
    assert!(mutation(&input,&json!({"providerInstances":{"claudeAgent_personal":{"driver":"claudeAgent"}}})).is_err());
}

#[test]
fn validation_rejects_invalid_ids_and_compaction_before_rpc() {
    let settings=json!({"providerInstances":{}});
    for id in ["claude agent", "1claude", "claudeAgent", "a-very-very-very-very-very-very-very-very-very-very-very-long-id-extra"] {
        assert!(mutation(&json!({"action":"create","driver":"claudeAgent","instanceId":id,"label":"Claude","config":{"homePath":"~/.claude_personal"}}),&settings).is_err(),"{id}");
    }
    for value in ["99999","1000001","300000.5","03","abc"] {
        assert!(mutation(&json!({"action":"create","driver":"claudeAgent","instanceId":"claudeAgent_two","label":"Claude","config":{"autoCompactWindow":value}}),&settings).is_err(),"{value}");
    }
}

#[test]
fn upsert_preserves_unrelated_claude_config_and_remove_rejects_default() {
    let settings=json!({"providerInstances":{"claudeAgent_extra":{"driver":"claudeAgent","enabled":false,"config":{"customModels":[{"slug":"foo"}],"homePath":"~/.claude_extra","launchArgs":"--chrome"}}}});
    let update=mutation(&json!({"action":"update","instanceId":"claudeAgent_extra","label":"Extra","config":{"homePath":"~/.claude_extra2"},"enabled":true}),&settings).unwrap();
    assert_eq!(update["providerInstanceMutation"]["operation"],"upsert");
    assert_eq!(update["providerInstanceMutation"]["instance"]["config"]["customModels"][0]["slug"],"foo");
    assert_eq!(update["providerInstanceMutation"]["instance"]["config"]["launchArgs"],"--chrome");
    assert!(mutation(&json!({"action":"remove","instanceId":"claudeAgent"}),&settings).is_err());
    assert_eq!(mutation(&json!({"action":"remove","instanceId":"claudeAgent_extra"}),&settings).unwrap()["providerInstanceMutation"]["operation"],"remove");
    let default=mutation(&json!({"action":"update","instanceId":"claudeAgent","enabled":false}),&settings).unwrap();
    assert_eq!(default["providerInstanceMutation"]["instance"]["enabled"],false);
}

#[test]
fn native_rpc_creates_only_requested_instance_and_reads_back_without_credentials() {
    let listener=TcpListener::bind("127.0.0.1:0").unwrap();
    let base=format!("http://{}",listener.local_addr().unwrap());
    let seen=Arc::new(Mutex::new(Vec::<Value>::new()));
    let captured=seen.clone();
    let server=thread::spawn(move||{
        let (mut stream,_)=listener.accept().unwrap();
        let mut buf=[0;8192];let n=stream.read(&mut buf).unwrap();
        assert!(String::from_utf8_lossy(&buf[..n]).starts_with("GET /api/auth/session "));
        let body=r#"{"authenticated":true,"permissions":["orchestration:read","providers:manage"]}"#;
        write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();drop(stream);
        let instance=json!({"driver":"claudeAgent","enabled":true,"displayName":"Claude Personal","config":{"homePath":"~/.claude_personal"}});
        for index in 0..5 {
            let (stream,_)=listener.accept().unwrap();let mut ws=tungstenite::accept(stream).unwrap();
            let request:Value=serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();captured.lock().unwrap().push(request.clone());
            let result=match index {
                0=>json!({"providerInstances":{"codex":{"driver":"codex","config":{"apiKey":"PRIVATE"}}}}),
                1=>json!({}),
                2|3=>json!({"providerInstances":{"codex":{"driver":"codex","config":{"apiKey":"PRIVATE"}},"claudeAgent_personal":instance}}),
                _=>json!({"providers":[{"instanceId":"claudeAgent_personal","driver":"claudeAgent","auth":{"status":"authenticated","email":"PRIVATE"}}]}),
            };
            ws.send(tungstenite::Message::Text(json!({"_tag":"Exit","requestId":"1","exit":{"_tag":"Success","value":result}}).to_string().into())).unwrap();
        }
    });
    let connection=omarchy_t3_client::protocol::Connection::new(&base,None,omarchy_t3_client::protocol::Credential::bearer("fixture-token").unwrap()).unwrap();
    let env=omarchy_t3_client::Environment{id:"fixture".into(),name:"Fixture".into(),base_url:base,token_file:"/unused".into(),write_token_file:None,bearer:None,connection_error:None,connection:Some(Arc::new(connection))};
    let result=omarchy_t3_client::provider_instances::execute(&env,&json!({"action":"create","driver":"claudeAgent","instanceId":"claudeAgent_personal","label":"Claude Personal","config":{"homePath":"~/.claude_personal"}})).unwrap();
    server.join().unwrap();
    assert!(result["instances"].as_array().unwrap().iter().any(|v|v["id"]=="claudeAgent_personal"));
    assert!(!result.to_string().contains("PRIVATE"));
    let requests=seen.lock().unwrap();
    assert_eq!(requests.iter().map(|r|r["tag"].as_str().unwrap()).collect::<Vec<_>>(),vec!["server.getSettings","server.updateSettings","server.getSettings","server.getSettings","server.getConfig"]);
    assert_eq!(requests[1]["payload"]["providerInstanceMutation"]["operation"],"create");
    assert_eq!(requests[1]["payload"]["patch"],json!({}));
}

#[test]
fn every_t3_driver_is_available_in_one_environment_scoped_registry() {
    let view=projection(&json!({"providerInstances":{}}),&json!({"providers":[]}),true);
    let kinds=view["drivers"].as_array().unwrap().iter().map(|v|v["id"].as_str().unwrap()).collect::<Vec<_>>();
    assert_eq!(kinds,["codex","claudeAgent","cursor","grok","opencode","antigravity","muse","pi","acpRegistry"]);
    let instances=view["instances"].as_array().unwrap();
    assert!(instances.iter().any(|v|v["id"]=="codex"&&v["enabled"]==true));
    assert!(instances.iter().any(|v|v["id"]=="opencode"&&v["enabled"]==false));
    assert_eq!(view["drivers"][1]["fields"][1]["key"],"homePath");
}

#[test]
fn different_drivers_create_via_same_atomic_mutation_without_touching_other_accounts() {
    let raw=json!({"providerInstances":{"claudeAgent":{"driver":"claudeAgent","config":{"homePath":"~/.claude"}}}});
    let codex=mutation(&json!({"action":"create","driver":"codex","instanceId":"codex_work","label":"Work","config":{"setupMode":"existing","homePath":"~/.codex_work","shadowHomePath":"~/.codex-shadow"}}),&raw).unwrap();
    assert_eq!(codex["providerInstanceMutation"]["instance"]["driver"],"codex");
    assert_eq!(codex["providerInstanceMutation"]["instance"]["config"]["shadowHomePath"],"~/.codex-shadow");
    assert_eq!(codex["patch"],json!({}));
    let acp=mutation(&json!({"action":"create","driver":"acpRegistry","instanceId":"acpRegistry_local","label":"Local ACP","config":{"source":"local","commandPath":"/usr/bin/agent","commandArgs":[]}}),&raw).unwrap();
    assert_eq!(acp["providerInstanceMutation"]["instance"]["config"]["source"],"local");
    assert!(mutation(&json!({"action":"create","driver":"acpRegistry","instanceId":"acpRegistry_missing","config":{"source":"local"}}),&raw).is_err());
    assert!(mutation(&json!({"action":"create","driver":"unknown","instanceId":"unknown_test"}),&raw).is_err());
    assert!(mutation(&json!({"action":"create","instanceId":"claudeAgent_unnamed"}),&raw).is_err());
}

#[test]
fn public_projection_never_exports_secret_values_or_other_driver_config() {
    let raw=json!({"providerInstances":{"antigravity_work":{"driver":"antigravity","displayName":"Work","enabled":true,"config":{"apiKey":"REDACTED_MARKER","authMethod":"gemini-api-key"},"environment":[{"name":"ANTHROPIC_AUTH_TOKEN","value":"REDACTED_MARKER","sensitive":true,"valueRedacted":true},{"name":"BASE_URL","value":"https://example.test","sensitive":false}]},"fork_private":{"driver":"fork","config":{"secret":"REDACTED_MARKER"}}}});
    let view=projection(&raw,&json!({"providers":[{"instanceId":"fork_private","driver":"fork","auth":{"email":"REDACTED_MARKER"}}]}),true);
    assert!(!view.to_string().contains("REDACTED_MARKER"));
    let a=view["instances"].as_array().unwrap().iter().find(|i|i["id"]=="antigravity_work").unwrap();
    assert_eq!(a["config"]["authMethod"],"gemini-api-key");
    assert_eq!(a["config"]["apiKey"],Value::Null);
    assert_eq!(a["environment"][0]["value"],"");
    assert_eq!(a["environment"][0]["configured"],true);
    assert_eq!(a["environment"][1]["value"],"https://example.test");
}

#[test]
fn secret_environment_preserves_same_instance_only_and_rejects_foreign_redaction() {
    let settings=json!({"providerInstances":{"opencode_work":{"driver":"opencode","environment":[{"name":"OPENCODE_API_KEY","value":"","sensitive":true,"valueRedacted":true},{"name":"BASE_URL","value":"https://example.test","sensitive":false}]}}});
    let update=mutation(&json!({"action":"update","driver":"opencode","instanceId":"opencode_work","providerEnvironment":[{"name":"OPENCODE_API_KEY","value":"","configured":true,"sensitive":true},{"name":"BASE_URL","value":"https://other.test","sensitive":false}]}),&settings).unwrap();
    assert_eq!(update["providerInstanceMutation"]["instance"]["environment"][0]["valueRedacted"],true);
    assert_eq!(update["providerInstanceMutation"]["instance"]["environment"][1]["value"],"https://other.test");
    assert!(mutation(&json!({"action":"create","driver":"opencode","instanceId":"opencode_other","providerEnvironment":[{"name":"OPENCODE_API_KEY","value":"","configured":true,"sensitive":true}]}),&settings).is_err());
    assert!(mutation(&json!({"action":"update","driver":"opencode","instanceId":"opencode_work","providerEnvironment":[{"name":"BAD-KEY","value":"x","sensitive":false}]}),&settings).is_err());
}

#[test]
fn unknown_existing_driver_roundtrips_on_toggle_without_exposing_config() {
    let settings=json!({"providerInstances":{"fork_custom":{"driver":"fork","enabled":true,"config":{"private":"NEVER_IN_QML"}}}});
    let view=projection(&settings,&json!({"providers":[]}),true);
    assert!(!view.to_string().contains("NEVER_IN_QML"));
    let update=mutation(&json!({"action":"update","driver":"fork","instanceId":"fork_custom","enabled":false}),&settings).unwrap();
    assert_eq!(update["providerInstanceMutation"]["instance"]["config"]["private"],"NEVER_IN_QML");
    assert!(mutation(&json!({"action":"update","driver":"fork","instanceId":"fork_custom","config":{"private":"changed"}}),&settings).is_err());
}

#[test]
fn readback_accepts_host_redaction_but_rejects_dropped_nonsecret_fields() {
    let payload=json!({"patch":{},"providerInstanceMutation":{"operation":"create","instanceId":"antigravity_work","instance":{"driver":"antigravity","enabled":true,"config":{"authMethod":"gemini-api-key","apiKey":"synthetic-secret"},"environment":[{"name":"GOOGLE_TOKEN","value":"synthetic-token","sensitive":true}]}}});
    let saved=json!({"providerInstances":{"antigravity_work":{"driver":"antigravity","enabled":true,"config":{"authMethod":"gemini-api-key","apiKey":"<redacted>"},"environment":[{"name":"GOOGLE_TOKEN","value":"","sensitive":true,"valueRedacted":true}]}}});
    assert!(confirm_target("create","antigravity_work",&payload,&saved));
    let mut dropped=saved.clone();dropped["providerInstances"]["antigravity_work"]["config"]["authMethod"]=json!("oauth-personal");
    assert!(!confirm_target("create","antigravity_work",&payload,&dropped));
    let remove=json!({"providerInstanceMutation":{"operation":"remove","instanceId":"antigravity_work"}});
    assert!(!confirm_target("remove","antigravity_work",&remove,&saved));
    assert!(confirm_target("remove","antigravity_work",&remove,&json!({"providerInstances":{}})));
}
