use omarchy_t3_client::*;
use serde_json::json;
use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
};
fn serve(status: &str, typ: &str, body: &str) -> (String, thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let status = status.to_owned();
    let typ = typ.to_owned();
    let body = body.to_owned();
    let handle = thread::spawn(move || {
        let (mut conn, _) = listener.accept().unwrap();
        let mut buf = [0; 8192];
        let len = conn.read(&mut buf).unwrap();
        write!(conn,"HTTP/1.1 {status}\r\nContent-Type: {typ}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
        String::from_utf8_lossy(&buf[..len]).into_owned()
    });
    (base, handle)
}
#[test]
fn headers_and_failure_boundaries() {
    let c = Client::new().unwrap();
    let (base, h) = serve("200 OK", "application/json", "{\"threads\":[]}");
    assert_eq!(
        c.fetch(&base, "synthetic-test-token", true).unwrap(),
        json!({"threads":[]})
    );
    let req = h.join().unwrap().to_lowercase();
    assert!(req.contains("authorization: bearer synthetic-test-token"));
    assert!(req.contains("x-t3-orchestration-protocol: 2"));
    assert!(req.contains("accept-encoding: identity"));
    for (status, typ, body, error) in [
        ("401 Unauthorized", "application/json", "{}", "unauthorized"),
        ("403 Forbidden", "application/json", "{}", "unauthorized"),
        ("200 OK", "text/html", "<html>", "not_json"),
        ("200 OK", "application/json", "{", "malformed"),
        ("302 Found", "text/html", "", "http_302"),
    ] {
        let (base, h) = serve(status, typ, body);
        assert_eq!(
            c.fetch(&base, "synthetic-test-token", true).unwrap_err(),
            error
        );
        h.join().unwrap();
    }
}
#[test]
fn discovery_omits_protocol_header() {
    let c = Client::new().unwrap();
    let (base, h) = serve("200 OK", "application/json", "{}");
    c.fetch(&base, "synthetic-test-token", false).unwrap();
    assert!(!h
        .join()
        .unwrap()
        .to_lowercase()
        .contains("x-t3-orchestration-protocol"));
}
#[test]
fn private_token_permissions_and_symlink() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let dir = std::env::temp_dir().join(format!("t3-rust-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("token");
    std::fs::write(&p, "synthetic-test-token").unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(private_token(&p).is_err());
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(private_token(&p).unwrap(), "synthetic-test-token");
    let link = dir.join("link");
    symlink(&p, &link).unwrap();
    assert!(private_token(&link).is_err());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn markdown_has_no_external_resources_or_raw_html() {
    let out=rendered_markdown("**Hello** ![alt](https://remote/image.png) [link](https://remote/) <img src='file:///secret'>");
    assert!(out.contains("<strong>Hello</strong>"));
    assert!(!out.contains("<img"));
    assert!(out.contains("href=\"https://remote/\""));
    assert!(out.contains("&lt;img"));
}

#[test]
fn project_monograms_match_t3() {
    assert_eq!(project_identity("omarchy-t3-code"), "OC");
    assert_eq!(project_identity("flea"), "FA");
    assert_eq!(project_identity("T3 Code"), "T3");
    assert_eq!(project_identity("ohm-launcher"), "OL");
}

#[test]
fn queued_messages_are_separated_from_conversation_until_started(){
 let c=Client::new().unwrap();
 for status in ["queued","running"]{
 let projection=json!({"thread":{"id":"chat"},"messages":[{"id":"already-sent","role":"user","text":"Sent","createdAt":1792036800000i64},{"id":"pending","role":"user","text":"Not sent to agent yet","createdAt":1792036800100i64}],"runs":[{"id":"q","userMessageId":"pending","ordinal":2,"status":status}],"plans":[{"id":"todo","kind":"todo_list","runId":"q","steps":[{"id":"step","text":"Work","status":"running"}]}],"turnItems":[]});
 let(base,server)=serve("200 OK","application/json",&json!({"projection":projection}).to_string());
 let e=Environment{id:"fixture".into(),name:"Fixture".into(),base_url:base,token_file:std::path::PathBuf::new(),write_token_file:None,bearer:Some("synthetic-token".into()),connection_error:None,connection:None};
 let result=c.messages(&e,"chat").unwrap();let queued=status=="queued";if queued{assert!(result["tasks"].is_null())}else{assert_eq!(result["tasks"]["total"],1);assert_eq!(result["tasks"]["current"],"Work")}assert_eq!(result["messages"].as_array().unwrap().len(),if queued{1}else{2});assert_eq!(result["queue"]["items"].as_array().unwrap().len(),if queued{1}else{0});assert_eq!(result["timeline"].as_array().unwrap().iter().any(|r|r["id"]=="pending"),!queued);server.join().unwrap();
 }
}
