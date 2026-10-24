// T3 requests that ot3 used to make with its own HTTP client and now makes through
// t3-protocol. They fix the error codes QML and the callers expect.
use omarchy_t3_client::*;
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
};

fn fixture(base: String) -> Environment {
    Environment{id:"fixture".into(),name:"Fixture".into(),base_url:base,token_file:std::path::PathBuf::new(),write_token_file:None,bearer:Some("synthetic-test-token".into()),connection_error:None,connection:None}
}
fn respond(listener: &TcpListener, status: &str, typ: &str, body: &[u8]) -> String {
    let (mut s, _) = listener.accept().unwrap();
    let mut bytes = [0; 8192];
    let n = s.read(&mut bytes).unwrap();
    write!(s,"HTTP/1.1 {status}\r\nContent-Type: {typ}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len()).unwrap();
    s.write_all(body).unwrap();
    String::from_utf8_lossy(&bytes[..n]).to_lowercase()
}
/// One `assets.createUrl` grant over the websocket, then the signed GET when the
/// grant stays under `/api/assets/`.
fn asset_server(relative: &'static str, status: &'static str, body: &'static [u8]) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let handle = thread::spawn(move || {
        let (s, _) = listener.accept().unwrap();
        let mut ws = tungstenite::accept(s).unwrap();
        let req: Value = serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();
        assert_eq!(req["tag"], "assets.createUrl");
        assert_eq!(req["payload"]["resource"], json!({"_tag":"attachment","attachmentId":"video1","fileName":"clip.mp4","mimeType":"video/mp4","disposition":"inline"}));
        ws.send(tungstenite::Message::Text(json!({"_tag":"Exit","requestId":req["id"],"exit":{"_tag":"Success","value":{"relativeUrl":relative}}}).to_string().into())).unwrap();
        if relative.starts_with("/api/assets/") {
            let header = respond(&listener, status, "video/mp4", body);
            assert!(header.starts_with("get /api/assets/signed-video?signature=synthetic "));
            assert!(!header.contains("authorization:"));
        }
    });
    (base, handle)
}

#[test]
fn attachment_bytes_reads_signed_assets_with_attachment_codes() {
    const SIGNED: &str = "/api/assets/signed-video?signature=synthetic";
    let (base, server) = asset_server(SIGNED, "200 OK", b"twenty bytes of clip");
    assert_eq!(attachment_bytes(&fixture(base), "video1", "clip.mp4", "video/mp4", 20).unwrap(), b"twenty bytes of clip");
    server.join().unwrap();
    for (relative, status, limit, error) in [
        (SIGNED, "200 OK", 19, "attachment_too_large"),
        (SIGNED, "404 Not Found", 20, "attachment_unavailable"),
        ("/elsewhere/clip.mp4", "200 OK", 20, "invalid_asset_url"),
    ] {
        let (base, server) = asset_server(relative, status, b"twenty bytes of clip");
        assert_eq!(attachment_bytes(&fixture(base), "video1", "clip.mp4", "video/mp4", limit).unwrap_err(), error);
        server.join().unwrap();
    }
    assert_eq!(attachment_bytes(&fixture("http://127.0.0.1:1".into()), "", "clip.mp4", "video/mp4", 20).unwrap_err(), "invalid_attachment");
}

#[test]
fn bridged_peer_must_name_the_environment_without_credentials() {
    let serve = |status: &'static str, body: &'static str| {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let handle = thread::spawn(move || {
            let header = respond(&listener, status, "application/json", body.as_bytes());
            assert!(header.starts_with("get /.well-known/t3/environment "));
            assert!(!header.contains("authorization:"));
        });
        (base, handle)
    };
    let (base, server) = serve("200 OK", r#"{"environmentId":"remote"}"#);
    omabridge_route::verify_environment(&base, "remote").unwrap();
    server.join().unwrap();
    for (status, body, error) in [
        ("200 OK", r#"{"environmentId":"someone-else"}"#, "t3_environment_mismatch"),
        ("500 Internal Server Error", "{}", "omabridge_route_failed"),
        ("200 OK", "{", "omabridge_route_failed"),
    ] {
        let (base, server) = serve(status, body);
        assert_eq!(omabridge_route::verify_environment(&base, "remote").unwrap_err(), error);
        server.join().unwrap();
    }
    assert_eq!(omabridge_route::verify_environment("http://127.0.0.1:1", "remote").unwrap_err(), "omabridge_route_failed");
}
