use omarchy_t3_client::{protocol, scheduled_tasks, Environment};
use serde_json::{json, Value};
use std::{net::TcpListener, sync::Arc, thread};

fn exercise(capable: bool, explicit: bool) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let connection = protocol::Connection::new(
        &format!("http://{}", listener.local_addr().unwrap()),
        None, protocol::Credential::bearer("synthetic-token").unwrap(),
    ).unwrap();
    let environment = Environment {
        id: "fixture".into(), name: "Fixture".into(), base_url: connection.base_url().into(),
        token_file: "/unused".into(), write_token_file: None, bearer: None,
        connection_error: None, connection: Some(Arc::new(connection)),
    };
    let server = thread::spawn(move || {
        let count = if explicit && !capable { 1 } else if capable { 3 } else { 4 };
        for index in 0..count {
            let (socket, _) = listener.accept().unwrap();
            let mut ws = tungstenite::accept(socket).unwrap();
            let request: Value = serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();
            let value = match index {
                0 => { assert_eq!(request["tag"], "server.getConfig"); json!({"environment":{"capabilities":{"scheduledTaskThreadModes":capable}}}) }
                1 => {
                    assert_eq!(request["tag"], "scheduledTasks.upsert");
                    if explicit { assert_eq!(request["payload"]["threadMode"], "fresh_session"); }
                    else { assert!(request["payload"].get("threadMode").is_none()); }
                    assert_eq!(request["payload"]["threadId"], "bound");
                    json!({})
                }
                2 => { assert_eq!(request["tag"], "scheduledTasks.subscribe"); json!({"tasks":[]}) }
                _ => { assert_eq!(request["tag"], "scheduledTasks.list"); json!({"tasks":[]}) }
            };
            let frame = if index == 2 && !capable { json!({"_tag":"Exit","requestId":"1","exit":{"_tag":"Failure","cause":{"_tag":"Fail","error":{"message":"Unknown request tag scheduledTasks.subscribe"}}}}) }
                else if index == 2 && capable { json!({"_tag":"Chunk","requestId":"1","values":[value]}) }
                else { json!({"_tag":"Exit","requestId":"1","exit":{"_tag":"Success","value":value}}) };
            ws.send(tungstenite::Message::Text(frame.to_string().into())).unwrap();
            if index == 2 && capable { let _ = ws.read(); }
        }
    });
    let mut input = json!({"action":"upsert","title":"Daily","prompt":"Check CI","projectId":"p","threadId":"bound","modelSelection":{"instanceId":"codex","model":"m"},"schedule":{"type":"interval","minutes":60}});
    if explicit { input["threadMode"] = json!("fresh_session"); }
    let result = scheduled_tasks::run(&environment, &input);
    if explicit && !capable { assert_eq!(result.unwrap_err(), "scheduled_task_thread_modes_unsupported"); }
    else { assert_eq!(result.unwrap()["supportsThreadModes"], capable); }
    server.join().unwrap();
}
#[test]
fn capable_server_receives_fresh_session_mode() { exercise(true, true); }
#[test]
fn legacy_server_keeps_bound_task_contract() { exercise(false, false); }
#[test]
fn unsupported_mode_is_rejected_before_mutation() { exercise(false, true); }
