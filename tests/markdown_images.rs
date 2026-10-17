use omarchy_t3_client::{markdown_images,normalize_messages,protocol};
use serde_json::{json,Value};
use std::{io::{Read,Write},net::TcpListener,thread};
#[test]
fn chat_preserves_inline_positions_and_code_action_indices_without_exposing_raw_sources(){
 let raw="Before **image**\n\n```sh\npwd\n```\n\n![Panel actualizado](/home/other/screen%20one.png)\n\nAfter image\n\n```sh\nls\n```\n\n![Again][ref]\n\n[ref]: file:///tmp/second.png";
 let v=normalize_messages(&json!({"projection":{"thread":{"id":"thread"},"messages":[{"id":"m","role":"assistant","text":raw}]}})).unwrap();let m=&v["messages"][0];let h=m["rendered"].as_str().unwrap();
 assert!(h.find("Before").unwrap()<h.find("ot3-image-open://0").unwrap());assert!(h.find("ot3-image-open://0").unwrap()<h.find("After image").unwrap());
 assert!(h.contains("ot3-code-copy://0")&&h.contains("ot3-code-copy://1"));assert!(!h.contains("src=\"/home/"));assert!(!h.contains("src=\"file:"));
 assert_eq!(m["markdownImages"].as_array().unwrap().len(),2);assert_eq!(m["markdownImages"][0]["source"],"/home/other/screen%20one.png");assert_eq!(m["markdownImages"][0]["name"],"Panel actualizado");
 assert!(markdown_images::descriptors("`![fake](/tmp/fake.png)`\n\n```\n![fake](/tmp/fake.png)\n```").is_empty());
}
#[test]
fn filesystem_references_follow_native_environment_classification(){
 for (raw,path) in [("/home/other/screen%20one.png#preview","/home/other/screen one.png"),("screens/a.png?x=1","screens/a.png"),("file:///tmp/a%20b.png","/tmp/a b.png"),("/tmp/niño.png","/tmp/niño.png")]{assert_eq!(markdown_images::workspace_path(raw).unwrap(),path)}
 for raw in ["javascript:alert(1)","https://example.test/a.png","~/a.png","#fragment","?query","/tmp/a%00.png","file://foreign/tmp/a.png"]{assert!(markdown_images::workspace_path(raw).is_err(),"{raw}")}
 assert!(markdown_images::workspace_path("%ñ").is_ok());
}
#[test]
fn native_media_grant_uses_exact_thread_and_host_path_and_no_credentials_on_asset_get(){
 let listener=TcpListener::bind("127.0.0.1:0").unwrap();let base=format!("http://{}",listener.local_addr().unwrap());
 let png=include_bytes!("fixtures/image.png").to_vec();let expected=png.clone();
 let server=thread::spawn(move||{let (s,_)=listener.accept().unwrap();let mut ws=tungstenite::accept_hdr(s,|req:&tungstenite::handshake::server::Request,response|{assert_eq!(req.headers()["authorization"],"Bearer synthetic");Ok(response)}).unwrap();let r:Value=serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();assert_eq!(r["tag"],"assets.createUrl");assert_eq!(r["payload"]["resource"],json!({"_tag":"media-file","threadId":"remote-thread","path":"/home/other/a.png"}));ws.send(tungstenite::Message::Text(json!({"_tag":"Exit","requestId":"1","exit":{"_tag":"Success","value":{"relativeUrl":"/api/assets/signed-media?signature=fixture"}}}).to_string().into())).unwrap();let (mut s,_)=listener.accept().unwrap();let mut header=Vec::new();let mut b=[0];while !header.ends_with(b"\r\n\r\n"){s.read_exact(&mut b).unwrap();header.push(b[0])}let header=String::from_utf8(header).unwrap().to_lowercase();assert!(!header.contains("authorization:"));write!(s,"HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",png.len()).unwrap();s.write_all(&png).unwrap();});
 let c=protocol::Connection::new(&base,None,protocol::Credential::bearer("synthetic").unwrap()).unwrap();let (bytes,mime)=c.media_image("remote-thread","/home/other/a.png").unwrap();assert_eq!(bytes,expected);assert_eq!(mime,"image/png");server.join().unwrap();
}
