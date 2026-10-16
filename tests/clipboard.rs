use omarchy_t3_client::{clipboard,protocol};
use serde_json::{json,Value};
use std::{fs,io::{Read,Write},net::TcpListener,os::unix::fs::PermissionsExt,thread};
#[test]
fn explicit_clipboard_paste_handles_text_and_signed_image_upload(){
 let root=std::env::temp_dir().join(format!("t3-clipboard-{}",uuid::Uuid::new_v4()));fs::create_dir_all(&root).unwrap();
 // Dedicated executable fixture; never touches or replaces the user's clipboard.
 let fake=root.join("wl-paste");fs::write(&fake,"#!/bin/sh\nif [ \"$1\" = --list-types ]; then cat \"$TEST_CLIP/types\"; else cat \"$TEST_CLIP/data\"; fi\n").unwrap();fs::set_permissions(&fake,fs::Permissions::from_mode(0o700)).unwrap();
 std::env::set_var("TEST_CLIP",&root);std::env::set_var("PATH",format!("{}:{}",root.display(),std::env::var("PATH").unwrap()));
 fs::write(root.join("types"),"text/plain\n").unwrap();fs::write(root.join("data"),"text without a newline").unwrap();
 assert_eq!(clipboard::paste(&json!({"capacity":100})).unwrap()["text"],"text without a newline");
 fs::write(root.join("types"),"image/png\ntext/plain\n").unwrap();fs::write(root.join("data"),b"\x89PNG\r\n\x1a\nsynthetic").unwrap();
 assert_eq!(clipboard::paste(&json!({"capacity":0})).unwrap_err(),"invalid_attachments");
 let listener=TcpListener::bind("127.0.0.1:0").unwrap();let base=format!("http://{}",listener.local_addr().unwrap());
 let token=root.join("token");fs::write(&token,"synthetic-token").unwrap();fs::set_permissions(&token,fs::Permissions::from_mode(0o600)).unwrap();
 let config=root.join("environments.json");fs::write(&config,json!({"environments":[{"id":"fixture","name":"Fixture","base_url":base,"token_file":token,"write_token_file":token}]}).to_string()).unwrap();std::env::set_var("OMARCHY_T3_FIXTURE_ENVIRONMENTS",config);
 let server=thread::spawn(move||{
  for (mime,body) in [("image/png",&b"\x89PNG\r\n\x1a\nsynthetic"[..]),("text/plain",&b"file!"[..])]{
  let (stream,_)=listener.accept().unwrap();let mut ws=tungstenite::accept(stream).unwrap();let req:Value=serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();assert_eq!(req["tag"],"attachments.createUploadUrl");assert_eq!(req["payload"]["mimeType"],mime);let size=req["payload"]["sizeBytes"].as_u64().unwrap() as usize;
  ws.send(tungstenite::Message::Text(json!({"_tag":"Exit","requestId":"1","exit":{"_tag":"Success","value":{"attachmentId":"clip_image","relativeUrl":"/api/attachments/upload/fixture","expiresAt":9999999999999u64}}}).to_string().into())).unwrap();
  let (mut stream,_)=listener.accept().unwrap();let mut bytes=vec![];let mut buf=[0;1024];let split=loop{let n=stream.read(&mut buf).unwrap();assert!(n>0);bytes.extend_from_slice(&buf[..n]);if let Some(i)=bytes.windows(4).position(|b|b==b"\r\n\r\n"){break i+4}};while bytes.len()<split+size{let n=stream.read(&mut buf).unwrap();assert!(n>0);bytes.extend_from_slice(&buf[..n])}assert_eq!(&bytes[split..],body);assert!(!String::from_utf8_lossy(&bytes[..split]).to_lowercase().contains("authorization:"));stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}").unwrap();
  }
 });
 let result=clipboard::paste(&json!({"capacity":100,"environment":"fixture"})).unwrap();assert_eq!(result["attachments"][0]["type"],"image");assert!(protocol::composer::attachment_reference(&result["attachments"][0]).contains("t3-context://v1/image/clip_image"));
 let file=root.join("fixture.txt");fs::write(&file,b"file!").unwrap();fs::write(root.join("types"),"text/uri-list\n").unwrap();fs::write(root.join("data"),reqwest::Url::from_file_path(file).unwrap().as_str()).unwrap();assert_eq!(clipboard::paste(&json!({"capacity":100,"environment":"fixture"})).unwrap()["attachments"][0]["type"],"file");server.join().unwrap();
 fs::write(root.join("types"),"text/uri-list\n").unwrap();fs::write(root.join("data"),"https://example.com/a").unwrap();assert_eq!(clipboard::paste(&json!({"capacity":100})).unwrap_err(),"invalid_clipboard_file");fs::remove_dir_all(root).unwrap();
}
