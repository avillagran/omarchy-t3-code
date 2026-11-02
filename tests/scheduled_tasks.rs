use omarchy_t3_client::{protocol, scheduled_tasks, Environment};
use serde_json::{json, Value};
use std::{net::TcpListener, sync::Arc, thread};

#[test]
fn rejected_webhook_diagnostics_never_expose_the_submitted_secret() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut ws = tungstenite::accept(stream).unwrap();
        let request: Value = serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();
        assert_eq!(request["tag"], "scheduledTasks.upsert");
        assert_eq!(request["payload"]["schedule"]["signature"]["secret"], "fixture-signing-value");
        ws.send(tungstenite::Message::Text(json!({"_tag":"Exit","requestId":request["id"],"exit":{"_tag":"Failure","cause":{"_tag":"Fail","error":{"message":"Rejected fixture-signing-value"}}}}).to_string().into())).unwrap();
    });
    let connection = protocol::Connection::new(&base, None, protocol::Credential::bearer("fixture-only").unwrap()).unwrap();
    let e = Environment { id:"fixture".into(), name:"Fixture".into(), base_url:base, token_file:"/unused".into(), write_token_file:None, bearer:None, connection_error:None, connection:Some(Arc::new(connection)) };
    let input = json!({"action":"upsert","title":"Hook","prompt":"{{body.action}}","projectId":"p","modelSelection":{"instanceId":"codex","model":"m"},"schedule":{"type":"webhook","signature":{"header":"x-signature","encoding":"hex","prefix":"sha256=","secret":"fixture-signing-value"}}});
    assert_eq!(scheduled_tasks::run(&e, &input).unwrap_err(), "scheduled_task_definition_rejected");
    server.join().unwrap();
}
