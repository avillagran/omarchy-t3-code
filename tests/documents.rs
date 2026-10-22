use omarchy_t3_client::{protocol,rendered_markdown};
#[test]
fn preserves_report_links_without_html_injection(){
 let rendered=rendered_markdown("[Informe](/home/node/report.html) [Site](https://example.test/) [bad](javascript:alert%281%29) <script>no</script>");
 assert!(rendered.contains("href=\"/home/node/report.html\""));
 assert!(rendered.contains("href=\"https://example.test/\""));
 assert!(!rendered.contains("href=\"javascript:"));
 assert!(!rendered.contains("<script>"));
}
#[test]
fn scoped_preview_paths_cover_remote_absolute_and_relative_files(){
 let html=protocol::documents::resource("thread","/home/node/reports/informe.html").unwrap();
 assert_eq!(html["_tag"],"media-file");assert_eq!(html["threadId"],"thread");
 assert_eq!(protocol::documents::resource("thread","report.HTM").unwrap()["_tag"],"media-file");
 assert_eq!(protocol::documents::resource("thread","src/main.rs").unwrap()["_tag"],"workspace-file");
 assert!(protocol::documents::resource("thread","bad\nfile.html").is_err());
 assert!(protocol::documents::resource("thread",&"x".repeat(1025)).is_err());
}
#[test]
fn html_artifacts_use_native_structured_output_and_skip_errors(){
 use serde_json::json;
 let mut item=json!({"toolName":"mcp__t3_code__html_render","output":{"structuredContent":{"htmlRender":{"attachmentId":"report-id","title":"Comparación","height":500}}}});
 let a=protocol::documents::html_render(&item).unwrap();assert_eq!(a["mimeType"],"text/html");assert_eq!(a["name"],"Comparación.html");
 item["output"]["isError"]=json!(true);assert!(protocol::documents::html_render(&item).is_none());
 item["toolName"]=json!("other");assert!(protocol::documents::html_render(&item).is_none());
}
#[test]
fn file_chips_preserve_host_paths_and_leave_web_links_as_links(){
 let refs=omarchy_t3_client::file_references("[Informe](/home/node/report.html) [site](https://example.test/) `/home/node/report.html` `./src/main.rs`");
 assert_eq!(refs.len(),1);assert_eq!(refs[0]["name"],"report.html");assert_eq!(refs[0]["path"],"/home/node/report.html");
}
#[test]
fn document_grant_uses_thread_scope_and_signed_get_without_bearer(){
 use std::{io::{Read,Write},net::TcpListener,thread};use serde_json::{json,Value};
 let listener=TcpListener::bind("127.0.0.1:0").unwrap();let addr=listener.local_addr().unwrap();
 let server=thread::spawn(move||{
  let(s,_)=listener.accept().unwrap();let mut ws=tungstenite::accept(s).unwrap();let req:Value=serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();assert_eq!(req["tag"],"assets.createUrl");assert_eq!(req["payload"]["resource"]["threadId"],"thread");assert_eq!(req["payload"]["resource"]["path"],"/home/node/report.html");
  ws.send(tungstenite::Message::Text(json!({"_tag":"Exit","requestId":"1","exit":{"_tag":"Success","value":{"relativeUrl":"/api/assets/report?signature=fixture","expiresAt":9999999999999u64}}}).to_string().into())).unwrap();
  let(mut s,_)=listener.accept().unwrap();let mut b=[0u8;8192];let n=s.read(&mut b).unwrap();let header=String::from_utf8_lossy(&b[..n]).to_lowercase();assert!(header.starts_with("get /api/assets/report?signature=fixture "));assert!(!header.contains("authorization:"));let html="<h1>Informe comparativo</h1>";write!(s,"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{html}",html.len()).unwrap();
 });
 let c=protocol::Connection::new(&format!("http://{addr}"),None,protocol::Credential::bearer("fixture-token").unwrap()).unwrap();let d=c.document(protocol::documents::resource("thread","/home/node/report.html").unwrap()).unwrap();assert_eq!(d["contents"],"<h1>Informe comparativo</h1>");server.join().unwrap();
}
#[test]
fn markdown_uses_native_text_rpc_for_absolute_paths_and_renders_safely(){
 use std::{net::TcpListener,thread,sync::Arc};use serde_json::{json,Value};
 let listener=TcpListener::bind("127.0.0.1:0").unwrap();let base=format!("http://{}",listener.local_addr().unwrap());
 let server=thread::spawn(move||{
  for truncated in [false,true]{
   let(s,_)=listener.accept().unwrap();let mut ws=tungstenite::accept(s).unwrap();let req:Value=serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();
   assert_eq!(req["tag"],"projects.readFile");assert_eq!(req["payload"],json!({"cwd":"/work/tree","relativePath":"/outside/report.md"}));
   ws.send(tungstenite::Message::Text(json!({"_tag":"Exit","requestId":"1","exit":{"_tag":"Success","value":{"contents":"# Report\n\n**Bold** <script>bad</script>","truncated":truncated,"byteLength":42}}}).to_string().into())).unwrap();
  }
 });
 let mut e:omarchy_t3_client::Environment=serde_json::from_value(json!({"id":"fixture","base_url":base,"token_file":"/unused"})).unwrap();e.connection=Some(Arc::new(protocol::Connection::new(&base,None,protocol::Credential::bearer("fixture-token").unwrap()).unwrap()));
 let input=json!({"thread":"thread","cwd":"/work/tree","path":"/outside/report.md"});let d=omarchy_t3_client::documents::read(&e,&input).unwrap();
 assert_eq!(d["markdown"],true);assert!(d["rendered"].as_str().unwrap().contains("<h1>Report</h1>"));assert!(!d["rendered"].as_str().unwrap().contains("<script>"));assert!(d["contents"].as_str().unwrap().contains("**Bold**"));
 assert_eq!(omarchy_t3_client::documents::read(&e,&input).unwrap_err(),"document_limit_4MiB");server.join().unwrap();
}

#[test]
fn user_messages_carry_their_turn_item_input_intent() {
    // T3 shows a "Steer" marker on messages that steered the active run; the
    // intent lives on the user_message turn item, keyed by messageId.
    let p = serde_json::json!({"messages":[],"turnItems":[
        {"item":{"type":"user_message","messageId":"m1","inputIntent":"steer","runId":"r1"}},
        {"item":{"type":"user_message","messageId":"m2","inputIntent":"turn_start","runId":"r2"}}
    ]});
    let find = |id: &str| omarchy_t3_client::checkpoint_revert::message_item(&p, &serde_json::json!({"id":id})).and_then(|i| i["inputIntent"].as_str().map(String::from));
    assert_eq!(find("m1").as_deref(), Some("steer"));
    assert_eq!(find("m2").as_deref(), Some("turn_start"));
    assert_eq!(find("m3"), None);
}
