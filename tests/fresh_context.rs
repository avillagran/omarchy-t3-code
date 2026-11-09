use omarchy_t3_client::{protocol, write, drafts, Environment};
use serde_json::{json, Value};
use std::{net::TcpListener, sync::Arc, thread};

#[test]
fn fresh_context_is_durable_queued_instead_of_steered_and_capability_checked() {
    let root=std::env::temp_dir().join(format!("o3-fresh-{}",uuid::Uuid::new_v4()));
    std::env::set_var("XDG_STATE_HOME", &root);
    for capable in [true,false] {
        let listener=TcpListener::bind("127.0.0.1:0").unwrap();
        let base=format!("http://{}",listener.local_addr().unwrap());
        let connection=protocol::Connection::new(&base,None,protocol::Credential::bearer("synthetic-token").unwrap()).unwrap();
        let environment=Environment{id:"fixture".into(),name:"Fixture".into(),base_url:base,token_file:"/unused".into(),write_token_file:None,bearer:None,connection_error:None,connection:Some(Arc::new(connection))};
        let count=if capable {4} else {1};
        let server=thread::spawn(move||{
            let mut sent=None;
            for index in 0..count {
                let (socket,_)=listener.accept().unwrap(); let mut ws=tungstenite::accept(socket).unwrap();
                let request:Value=serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();
                let value=if index%2==0 {
                    assert_eq!(request["tag"],"server.getConfig");json!({"environment":{"capabilities":{"freshContext":capable}}})
                } else {
                    assert_eq!(request["tag"],"orchestration.dispatchCommand");let command=&request["payload"];
                    assert_eq!(command["freshContext"],true);assert_eq!(command["dispatchMode"]["type"],"queue_after_active");assert!(command.get("deliveryIntent").is_none());
                    if let Some(previous)=sent {assert_eq!(previous,*command);}sent=Some(command.clone());json!({"sequence":1})
                };
                ws.send(tungstenite::Message::Text(json!({"_tag":"Exit","requestId":"1","exit":{"_tag":"Success","value":value}}).to_string().into())).unwrap();
            }
        });
        let id=uuid::Uuid::new_v4().to_string();
        let input=json!({"op":"send","thread":"chat","requestId":id,"text":"New context","attachments":[],"freshContext":true,"freshSessionArm":id,"delivery":"steer"});
        if capable {
            write::dispatch(&environment,&input).unwrap();write::dispatch(&environment,&input).unwrap();
            let persisted:Value=serde_json::from_slice(&std::fs::read(root.join("omarchy-t3-code/outbox").join(format!("{id}.json"))).unwrap()).unwrap();
            assert_eq!(persisted["command"]["freshContext"],true);
            let draft=drafts::clean(&json!({"text":input["text"],"attempt":input})).unwrap();
            assert_eq!(draft["attempt"]["freshContext"],true);
            assert_eq!(draft["attempt"]["freshSessionArm"],id);
            let mut invalid=json!({"text":input["text"],"attempt":input});
            invalid["attempt"]["freshContext"]=json!("true");assert!(drafts::clean(&invalid).is_err());
        } else {assert_eq!(write::dispatch(&environment,&input).unwrap_err(),"fresh_context_unsupported");}
        server.join().unwrap();
    }
}

#[test]
fn fresh_session_marker_comes_from_the_user_turn_item_and_keeps_history() {
    let mut projection=json!({"thread":{"id":"chat"},"messages":[{"id":"old","role":"user","text":"Earlier","createdAt":1},{"id":"new","role":"user","text":"Next","createdAt":2}],"turnItems":[{"id":"new-item","type":"user_message","messageId":"new","freshContext":true}],"runs":[]});
    let view=omarchy_t3_client::normalize_messages(&json!({"projection":projection})).unwrap();
    assert_eq!(view["messages"].as_array().unwrap().len(),2);
    assert_eq!(view["messages"][0]["freshContext"],false);
    assert_eq!(view["messages"][1]["freshContext"],true);
    let next=view["timeline"].as_array().unwrap().iter().find(|m|m["id"]=="new").unwrap();
    assert_eq!(next["freshContext"],true);
    let before=next["rev"].clone();
    projection["turnItems"][0]["freshContext"]=json!(false);
    let after=omarchy_t3_client::normalize_messages(&json!({"projection":projection})).unwrap();
    let next=after["timeline"].as_array().unwrap().iter().find(|m|m["id"]=="new").unwrap();
    assert_ne!(next["rev"],before,"a marker update refreshes the visible row");
}

#[test]
fn explicit_context_dispatch_preserves_all_optional_flags_and_opaque_order() {
    let listener=TcpListener::bind("127.0.0.1:0").unwrap();
    let base=format!("http://{}",listener.local_addr().unwrap());
    let server=thread::spawn(move||{
        for fresh in [None,Some(false),Some(true)] {
            let mut accepted:Option<String>=None;
            for _ in 0..2 {
                let(socket,_)=listener.accept().unwrap();let mut ws=tungstenite::accept(socket).unwrap();
                let frame=ws.read().unwrap();let text=frame.to_text().unwrap();
                let request:Value=serde_json::from_str(text).unwrap();
                let payload=&request["payload"];
                assert_eq!(payload.get("freshContext").and_then(Value::as_bool),fresh);
                assert!(text.contains("\"opaque\":{\"z\":1,\"a\":2}"));
                if let Some(previous)=&accepted {assert_eq!(text,previous.as_str())}else{accepted=Some(text.to_owned())}
                ws.send(tungstenite::Message::Text(json!({"_tag":"Exit","requestId":"1","exit":{"_tag":"Success","value":{}}}).to_string().into())).unwrap();
            }
        }
    });
    let connection=protocol::Connection::new(&base,None,protocol::Credential::bearer("synthetic-context").unwrap()).unwrap();
    for fresh in [None,Some(false),Some(true)] {
        let mut command:Value=serde_json::from_str(r#"{"type":"message.dispatch","commandId":"command","threadId":"thread","messageId":"message","text":"hello","attachments":[],"dispatchMode":{"type":"queue_after_active"},"createdBy":"user","creationSource":"web","context":{"opaque":{"z":1,"a":2}}}"#).unwrap();
        if let Some(fresh)=fresh{command["freshContext"]=json!(fresh);}
        write::send_command(&connection,&command).unwrap();
        write::send_command(&connection,&command).unwrap();
    }
    server.join().unwrap();
}
