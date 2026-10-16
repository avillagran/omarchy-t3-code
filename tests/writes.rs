use omarchy_t3_client::write as writing;
use omarchy_t3_client::*;
use serde_json::{json, Value};
use std::{io::{Read,Write},net::TcpListener,os::unix::fs::PermissionsExt,thread};

#[test]
fn native_write_contract_and_upload_boundaries() {
    let root=std::env::temp_dir().join(format!("omarchy-write-test-{}",uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    std::env::set_var("XDG_STATE_HOME",&root);
    let token=root.join("token");std::fs::write(&token,"synthetic-private-token").unwrap();std::fs::set_permissions(&token,std::fs::Permissions::from_mode(0o600)).unwrap();
    let listener=TcpListener::bind("127.0.0.1:0").unwrap();
    let mut e=Environment{id:"fixture".into(),name:"Fixture".into(),base_url:format!("http://{}",listener.local_addr().unwrap()),token_file:token,write_token_file:None,bearer:None,connection_error:None,connection:None};
    let server=thread::spawn(move || {
        // Model catalog: client must whitelist and exclude disabled providers.
        let (stream,_)=listener.accept().unwrap();
        let mut ws=tungstenite::accept(stream).unwrap();
        let req:Value=serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();
        assert_eq!(req["headers"],json!([]));assert_eq!(req["tag"],"server.getConfig");
        ws.send(tungstenite::Message::Text(json!({"_tag":"Exit","requestId":"1","exit":{"_tag":"Success","value":{"auth":{"secret":"must-not-leak"},"providers":[{"enabled":true,"instanceId":"codex","displayName":"Codex","auth":"must-not-leak","models":[{"slug":"custom-model","name":"Custom","capabilities":{"optionDescriptors":[]}}]},{"enabled":false,"instanceId":"disabled","models":[{"slug":"hidden"}]}]}}}).to_string().into())).unwrap();
        // Same command twice: frame and command identity must remain identical.
        let mut first=Value::Null;
        for n in 0..2 {
            let (stream,_)=listener.accept().unwrap();let mut ws=tungstenite::accept(stream).unwrap();
            let req:Value=serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();
            assert_eq!(req["tag"],"orchestration.dispatchCommand");
            let c=&req["payload"];assert_eq!(c["type"],"message.dispatch");assert_eq!(c["createdBy"],"user");assert_eq!(c["deliveryIntent"],"auto");assert_eq!(c["dispatchMode"]["type"],"start_immediately");
            assert!(c.get("runtimeMode").is_none());assert_eq!(c["text"],"safe synthetic fixture fixture.txt");assert_eq!(c["attachments"][0]["id"],"fixture_attachment");
            if n==0 {first=c.clone()} else {assert_eq!(&first,c)}
            ws.send(tungstenite::Message::Text(json!({"_tag":"Exit","requestId":"1","exit":{"_tag":"Success","value":{"sequence":123}}}).to_string().into())).unwrap();
        }
        // Real streaming HTTP upload follows the signed grant with POST, no bearer.
        let (stream,_)=listener.accept().unwrap();let mut ws=tungstenite::accept(stream).unwrap();
        let req:Value=serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();
        assert_eq!(req["tag"],"attachments.createUploadUrl");assert_eq!(req["payload"]["sizeBytes"],7);
        ws.send(tungstenite::Message::Text(json!({"_tag":"Exit","requestId":"1","exit":{"_tag":"Success","value":{"attachmentId":"fixture-upload","relativeUrl":"/api/attachments/upload/synthetic-grant","expiresAt":9999999999999u64}}}).to_string().into())).unwrap();
        let (mut stream,_)=listener.accept().unwrap();let mut bytes=Vec::new();let mut buf=[0;1024];
        let split=loop {let n=stream.read(&mut buf).unwrap();assert!(n>0);bytes.extend_from_slice(&buf[..n]);if let Some(i)=bytes.windows(4).position(|b|b==b"\r\n\r\n"){break i+4}};
        while bytes.len()<split+7 {let n=stream.read(&mut buf).unwrap();assert!(n>0);bytes.extend_from_slice(&buf[..n])}
        let headers=String::from_utf8_lossy(&bytes[..split]).to_lowercase();
        assert!(headers.starts_with("post /api/attachments/upload/"));assert!(!headers.contains("authorization:"));assert!(headers.contains("content-length: 7"));assert_eq!(&bytes[split..],b"hello\0!");
        stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}").unwrap();
        drop(stream);
        let (stream,_)=listener.accept().unwrap();let mut ws=tungstenite::accept(stream).unwrap();
        let _=ws.read().unwrap();
        ws.send(tungstenite::Message::Text(json!({"_tag":"Exit","requestId":"1","exit":{}}).to_string().into())).unwrap();
        for typ in ["thread.create","thread.create","project.create","thread.settle","thread.unsettle","thread.snooze","thread.unsnooze"] {
            let (stream,_)=listener.accept().unwrap();let mut ws=tungstenite::accept(stream).unwrap();
            let request:Value=serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();
            let c=&request["payload"];assert_eq!(c["type"],typ);
            assert_eq!(request["tag"],if typ=="project.create" {"projects.mutate"} else {"orchestration.dispatchCommand"});
            if typ=="thread.create" {assert_eq!(c["runtimeMode"],"approval-required");assert!(c["branch"].is_null());assert!(c["worktreePath"].is_null());assert!(c.get("dispatchMode").is_none());assert!(c.get("message").is_none());}
            if typ=="project.create" {assert_eq!(c["createWorkspaceRootIfMissing"],false);}
            if typ=="thread.snooze" {assert!(c["snoozedUntil"].as_str().unwrap().starts_with("2099-"));}
            if typ=="thread.unsnooze" {assert_eq!(c["reason"],"user");}
            if typ=="thread.unsettle" {assert_eq!(c["reason"],"user");}
            ws.send(tungstenite::Message::Text(json!({"_tag":"Exit","requestId":"1","exit":{"_tag":"Success","value":{"sequence":456}}}).to_string().into())).unwrap();
        }


    });
    let catalog=writing::catalog(&e).unwrap();assert_eq!(catalog["models"].as_array().unwrap().len(),1);assert!(!catalog.to_string().contains("must-not-leak"));
    e.write_token_file=Some(e.token_file.clone());
    let request=json!({"op":"send","requestId":uuid::Uuid::new_v4().to_string(),"thread":"fixture-thread","text":"safe synthetic fixture [fixture.txt](t3-context://v1/file/fixture_attachment)","attachments":[{"type":"file","id":"fixture_attachment","name":"fixture.txt","mimeType":"text/plain","sizeBytes":7}],"modelSelection":{"instanceId":"codex","model":"custom-model","options":[{"id":"reasoningEffort","value":"medium"}]}});
    assert_eq!(writing::dispatch(&e,&request).unwrap()["sequence"],123);
    assert_eq!(writing::dispatch(&e,&request).unwrap()["sequence"],123);
    let mut changed=request.clone();changed["text"]=json!("different");assert_eq!(writing::dispatch(&e,&changed).unwrap_err(),"retry_payload_changed");
    let file=root.join("fixture.txt");std::fs::write(&file,b"hello\0!").unwrap();
    let url=reqwest::Url::from_file_path(&file).unwrap();let a=writing::upload(&e,url.as_str()).unwrap();assert_eq!(a["id"],"fixture-upload");assert_eq!(a["sizeBytes"],7);
    let link=root.join("link.txt");std::os::unix::fs::symlink(&file,&link).unwrap();assert_eq!(writing::upload(&e,reqwest::Url::from_file_path(link).unwrap().as_str()).unwrap_err(),"invalid_file");
    let huge=root.join("huge.png");let f=std::fs::File::create(&huge).unwrap();f.set_len(11*1024*1024).unwrap();assert_eq!(writing::upload(&e,reqwest::Url::from_file_path(huge).unwrap().as_str()).unwrap_err(),"image_limit_10MiB");
    let mut invalid=request.clone();invalid["requestId"]=json!(uuid::Uuid::new_v4().to_string());invalid["attachments"]=json!([{"type":"image","id":"fixture","name":"image.png","mimeType":"image/png","sizeBytes":u64::MAX}]);
    assert_eq!(writing::dispatch(&e,&invalid).unwrap_err(),"invalid_attachments");
    assert_eq!(writing::rpc(&e,"server.getConfig",json!({})).unwrap_err(),"rpc_outcome_unknown");
    let create=json!({"requestId":uuid::Uuid::new_v4().to_string(),"action":"new-thread","projectId":"fixture-project","title":"Empty fixture","modelSelection":{"instanceId":"codex","model":"custom-model"}});
    let created=writing::manage(&e,&create).unwrap();
    assert!(created["threadId"].as_str().unwrap().starts_with("omarchy-thread-"));
    assert_eq!(writing::manage(&e,&create).unwrap(),created);
    let mut local=e.clone();local.id="local".into();
    writing::manage(&local,&json!({"requestId":uuid::Uuid::new_v4().to_string(),"action":"add-project","url":reqwest::Url::from_directory_path(&root).unwrap().as_str()})).unwrap();
    for action in ["settle","unsettle","snooze","unsnooze"] { writing::manage(&e,&json!({"requestId":uuid::Uuid::new_v4().to_string(),"action":action,"thread":"fixture-thread","snoozedUntil":"2099-01-01T00:00:00Z"})).unwrap(); }
    server.join().unwrap();std::fs::remove_dir_all(root).unwrap();
}
