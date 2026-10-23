use omarchy_t3_client::protocol;
use serde_json::{json, Value};
use std::{
    net::TcpListener,
    thread,
    time::{Duration, Instant},
};

/// Fake server for one subscription: answers the client's Ping with a Pong only when `answer`.
fn server(answer: bool) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let handle = thread::spawn(move || {
        let (socket, _) = listener.accept().unwrap();
        socket.set_read_timeout(Some(Duration::from_secs(8))).unwrap();
        let mut ws = tungstenite::accept(socket).unwrap();
        let request: Value = serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();
        assert_eq!(request["tag"], "orchestration.subscribeShell");
        while let Ok(message) = ws.read() {
            let Ok(text) = message.to_text() else { continue };
            let Ok(v) = serde_json::from_str::<Value>(text) else { continue };
            if v["_tag"] == "Interrupt" {
                break;
            }
            if v["_tag"] == "Ping" && answer {
                ws.send(tungstenite::Message::Text(json!({"_tag":"Pong"}).to_string().into())).unwrap();
            }
        }
    });
    (base, handle)
}

#[test]
fn a_wakeup_probe_drops_a_silent_socket_within_seconds() {
    let (base, handle) = server(false);
    let c = protocol::Connection::new(&base, None, protocol::Credential::bearer("synthetic").unwrap()).unwrap();
    let start = Instant::now();
    let mut asked = false;
    let result = c.subscribe_probed("orchestration.subscribeShell", json!({}), || false, || !std::mem::replace(&mut asked, true), |_| Ok(()));
    assert_eq!(result.unwrap_err(), "stream_timeout");
    assert!(start.elapsed() < Duration::from_secs(5), "{:?}", start.elapsed());
    drop(c);
    let _ = handle.join();
}

#[test]
fn a_wakeup_probe_keeps_a_socket_that_answers() {
    let (base, handle) = server(true);
    let c = protocol::Connection::new(&base, None, protocol::Credential::bearer("synthetic").unwrap()).unwrap();
    let start = Instant::now();
    let mut asked = false;
    let result = c.subscribe_probed("orchestration.subscribeShell", json!({}), || start.elapsed() > Duration::from_secs(5), || !std::mem::replace(&mut asked, true), |_| Ok(()));
    assert!(result.is_ok(), "{result:?}");
    handle.join().unwrap();
}
