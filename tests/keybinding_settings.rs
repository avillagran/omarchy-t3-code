use omarchy_t3_client::keybinding_settings::{payload, present};
use serde_json::{json, Value};
fn rule(command: &str) -> Value {
    json!({"command":command,"shortcut":{"key":" ","metaKey":false,"ctrlKey":false,"shiftKey":false,"altKey":false,"modKey":true},"whenAst":{"type":"identifier","name":"composerFocus"},"private":"omit"})
}
#[test]
fn safe_projection_preserves_native_space_and_forward_compatible_rules() {
    let raw = json!({"keybindingsConfigPath":"/remote/.t3/keybindings.json","keybindings":[rule("composer.mode"),rule("new.command"),rule("script.check.run")],"credentials":"PRIVATE","issues":[{"kind":"provider.invalid","message":"PRIVATE"},{"kind":"keybindings.invalid-entry","message":"Entry 3 is invalid","private":"omit"}]});
    let out = present(&raw).unwrap();
    assert_eq!(out["keybindings"].as_array().unwrap().len(), 2);
    assert_eq!(out["keybindings"][0]["shortcut"]["key"], " ");
    assert_eq!(out["keybindings"][1]["command"], "script.check.run");
    assert_eq!(out["issues"].as_array().unwrap().len(), 1);
    assert!(!out.to_string().contains("PRIVATE"));
    assert!(out["keybindings"][0].get("private").is_none());
    assert_eq!(out["path"], "/remote/.t3/keybindings.json");
    let mut invalid = raw.clone();
    invalid["keybindings"][0]["whenAst"] = json!({"type":"future","credential":"PRIVATE"});
    assert_eq!(
        present(&invalid).unwrap()["keybindings"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let mut missing = raw.clone();
    missing.as_object_mut().unwrap().remove("keybindings");
    assert!(present(&missing).is_err());
    assert!(present(&json!({"keybindings":vec![rule("composer.mode");257]})).is_err());
}
#[test]
fn exact_native_replace_remove_and_schema_bounds() {
    let input = json!({"action":"upsert","input":{"command":"composer.sendAndNewThread","key":"mod+shift+enter","when":"composerFocus && !draftThreadRoute","replace":{"command":"composer.sendAndNewThread","key":"mod+alt+enter","when":"composerFocus && !draftThreadRoute"},"private":"drop"}});
    let p = payload(&input).unwrap();
    assert_eq!(p.as_object().unwrap().len(), 4);
    assert_eq!(p["replace"]["key"], "mod+alt+enter");
    assert!(p.get("private").is_none());
    for key in ["".to_string(), " x".into(), "x\n".into(), "x".repeat(65)] {
        let mut wrong = input.clone();
        wrong["input"]["key"] = json!(key);
        assert!(payload(&wrong).is_err());
    }
    let mut wrong = input.clone();
    wrong["input"]["command"] = json!("execute.shell");
    assert!(payload(&wrong).is_err());
    wrong["input"]["command"] = json!("script.invalid/path.run");
    assert!(payload(&wrong).is_err());
    wrong = input.clone();
    wrong["input"]["when"] = json!(" ");
    assert!(payload(&wrong).is_err());
    wrong = input.clone();
    wrong["action"] = json!("remove");
    assert!(payload(&wrong).is_err());
    wrong["input"].as_object_mut().unwrap().remove("replace");
    assert!(payload(&wrong).unwrap().get("replace").is_none());
}
#[test]
fn native_transport_checks_roles_fresh_config_and_never_replays_a_consumed_view() {
    use omarchy_t3_client::{keybinding_settings as keys, protocol, Environment};
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc, Mutex,
        },
        thread,
        time::Duration,
    };
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let stop = Arc::new(AtomicBool::new(false));
    let writable = Arc::new(AtomicBool::new(false));
    let lost = Arc::new(AtomicBool::new(false));
    let config = Arc::new(Mutex::new(
        json!({"keybindings":[rule("composer.mode")],"keybindingsConfigPath":"/remote/.t3/keybindings.json","issues":[],"availableEditors":["zed","cursor"],"providerSecret":"PRIVATE"}),
    ));
    let calls = Arc::new(Mutex::new(Vec::<Value>::new()));
    let (ss, ww, ll, cc, log) = (
        stop.clone(),
        writable.clone(),
        lost.clone(),
        config.clone(),
        calls.clone(),
    );
    let server = thread::spawn(move || {
        while !ss.load(Ordering::Relaxed) {
            let (mut stream, _) = match listener.accept() {
                Ok(s) => s,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(2));
                    continue;
                }
                Err(e) => panic!("{e}"),
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut peek = [0; 512];
            let n = stream.peek(&mut peek).unwrap();
            if String::from_utf8_lossy(&peek[..n]).starts_with("GET /api/auth/session ") {
                let mut header = vec![];
                let mut b = [0];
                while !header.ends_with(b"\r\n\r\n") {
                    stream.read_exact(&mut b).unwrap();
                    header.push(b[0]);
                }
                let body=json!({"authenticated":true,"permissions":if ww.load(Ordering::Relaxed){vec!["orchestration:read","settings:write","orchestration:operate"]}else{vec!["orchestration:read"]}}).to_string();
                write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
                continue;
            }
            let mut ws = tungstenite::accept(stream).unwrap();
            let request: Value =
                serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();
            let value = match request["tag"].as_str().unwrap() {
                "server.getConfig" => cc.lock().unwrap().clone(),
                "server.upsertKeybinding" | "server.removeKeybinding" => {
                    assert!(ww.load(Ordering::Relaxed));
                    log.lock().unwrap().push(request.clone());
                    let mut c = cc.lock().unwrap();
                    if request["tag"] == "server.upsertKeybinding" {
                        assert_eq!(
                            request["payload"],
                            json!({"command":"composer.sendAndNewThread","key":"mod+shift+enter","when":"composerFocus","replace":{"command":"composer.mode","key":"mod+space","when":"composerFocus"}})
                        );
                        let mut r = rule("composer.sendAndNewThread");
                        r["shortcut"]["key"] = json!("enter");
                        r["shortcut"]["shiftKey"] = json!(true);
                        c["keybindings"] = json!([r]);
                    } else {
                        assert_eq!(
                            request["payload"],
                            json!({"command":"composer.sendAndNewThread","key":"mod+shift+enter","when":"composerFocus"})
                        );
                        c["keybindings"] = json!([]);
                    }
                    let v = json!({"keybindings":c["keybindings"],"issues":[]});
                    if ll.load(Ordering::Relaxed) {
                        drop(c);
                        drop(ws);
                        continue;
                    }
                    v
                }
                "shell.openInEditor" => {
                    assert_eq!(
                        request["payload"],
                        json!({"cwd":"/remote/.t3/keybindings.json","editor":"cursor"})
                    );
                    log.lock().unwrap().push(request.clone());
                    Value::Null
                }
                method => panic!("unexpected {method}"),
            };
            ws.send(tungstenite::Message::Text(
                json!({"_tag":"Exit","requestId":"1","exit":{"_tag":"Success","value":value}})
                    .to_string()
                    .into(),
            ))
            .unwrap();
        }
    });
    let e = Environment {
        id: "fixture".into(),
        name: "Fixture".into(),
        base_url: base.clone(),
        token_file: "/unused".into(),
        write_token_file: None,
        bearer: None,
        connection_error: None,
        connection: Some(Arc::new(
            protocol::Connection::new(
                &base,
                None,
                protocol::Credential::bearer("synthetic").unwrap(),
            )
            .unwrap(),
        )),
    };
    let mut input = json!({"action":"upsert","input":{"command":"composer.sendAndNewThread","key":"mod+shift+enter","when":"composerFocus","replace":{"command":"composer.mode","key":"mod+space","when":"composerFocus"}}});
    let view = keys::read(&e).unwrap();
    assert_eq!(view["writable"], false);
    assert!(!view.to_string().contains("PRIVATE"));
    input["viewId"] = view["viewId"].clone();
    assert_eq!(keys::apply(&e, &input).unwrap_err(), "permission_denied");
    assert!(calls.lock().unwrap().is_empty());
    writable.store(true, Ordering::Relaxed);
    config.lock().unwrap()["keybindings"][0]["shortcut"]["key"] = json!("x");
    assert_eq!(
        keys::apply(&e, &input).unwrap_err(),
        "keybindings_changed_reload"
    );
    assert!(calls.lock().unwrap().is_empty());
    input["viewId"] = keys::read(&e).unwrap()["viewId"].clone();
    lost.store(true, Ordering::Relaxed);
    assert_eq!(keys::apply(&e, &input).unwrap()["state"], "uncertain");
    assert_eq!(calls.lock().unwrap().len(), 1);
    assert_eq!(
        keys::apply(&e, &input).unwrap_err(),
        "keybindings_snapshot_expired"
    );
    assert_eq!(calls.lock().unwrap().len(), 1);
    lost.store(false, Ordering::Relaxed);
    let view = keys::read(&e).unwrap();
    assert_eq!(
        view["keybindings"][0]["command"],
        "composer.sendAndNewThread"
    );
    input["viewId"] = view["viewId"].clone();
    input["action"] = json!("remove");
    input["input"].as_object_mut().unwrap().remove("replace");
    let done = keys::apply(&e, &input).unwrap();
    assert_eq!(done["state"], "complete");
    assert_eq!(done["data"]["keybindings"], json!([]));
    assert_eq!(calls.lock().unwrap().len(), 2);
    let open = json!({"viewId":done["data"]["viewId"]});
    assert_eq!(keys::open(&e, &open).unwrap()["opened"], true);
    assert_eq!(calls.lock().unwrap().len(), 3);
    writable.store(false, Ordering::Relaxed);
    assert_eq!(keys::open(&e, &open).unwrap_err(), "permission_denied");
    writable.store(true, Ordering::Relaxed);
    config.lock().unwrap()["keybindingsConfigPath"] = json!("/moved/keybindings.json");
    assert_eq!(
        keys::open(&e, &open).unwrap_err(),
        "keybindings_changed_reload"
    );
    assert_eq!(calls.lock().unwrap().len(), 3);
    let mut other = e.clone();
    other.base_url = "http://127.0.0.1:1".into();
    input["viewId"] = done["data"]["viewId"].clone();
    assert_eq!(
        keys::apply(&other, &input).unwrap_err(),
        "keybindings_scope_changed"
    );
    stop.store(true, Ordering::Relaxed);
    server.join().unwrap();
}

#[test]
fn config_stream_only_invalidates_keybindings_and_cancels_without_leaking_config() {
    use omarchy_t3_client::{keybinding_settings as keys, protocol, Environment};
    use std::{io::{Read,Write},net::TcpListener,sync::{Arc,atomic::{AtomicBool,AtomicUsize,Ordering}},thread,time::Duration};
    let listener=TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let base=format!("http://{}",listener.local_addr().unwrap());
    let stop=Arc::new(AtomicBool::new(false));
    let allowed=Arc::new(AtomicBool::new(true));
    let streams=Arc::new(AtomicUsize::new(0));
    let interrupted=Arc::new(AtomicBool::new(false));
    let (ss,aa,nn,ii)=(stop.clone(),allowed.clone(),streams.clone(),interrupted.clone());
    let server=thread::spawn(move || {
        while !ss.load(Ordering::Relaxed) {
            let (mut socket,_)=match listener.accept(){Ok(v)=>v,Err(e) if e.kind()==std::io::ErrorKind::WouldBlock=>{thread::sleep(Duration::from_millis(2));continue},Err(e)=>panic!("{e}")};
            socket.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
            let mut peek=[0;512];let n=socket.peek(&mut peek).unwrap();
            if String::from_utf8_lossy(&peek[..n]).starts_with("GET /api/auth/session ") {
                let mut header=vec![];let mut byte=[0];while !header.ends_with(b"\r\n\r\n"){socket.read_exact(&mut byte).unwrap();header.push(byte[0]);}
                let body=json!({"authenticated":true,"permissions":if aa.load(Ordering::Relaxed){vec!["orchestration:read"]}else{vec![]}}).to_string();
                write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();continue;
            }
            nn.fetch_add(1,Ordering::Relaxed);
            let mut ws=tungstenite::accept(socket).unwrap();
            let request:Value=serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();
            assert_eq!(request["tag"],"subscribeServerConfig");assert_eq!(request["payload"],json!({}));
            ws.send(tungstenite::Message::Text(json!({"_tag":"Chunk","requestId":"1","values":[
                {"version":1,"type":"providerStatuses","payload":{"secret":"PRIVATE"}},
                {"version":2,"type":"keybindingsUpdated","payload":{"secret":"PRIVATE"}},
                {"version":1,"type":"snapshot","config":{"keybindings":[],"secret":"PRIVATE"}},
                {"version":1,"type":"settingsUpdated","payload":{"secret":"PRIVATE"}},
                {"version":1,"type":"keybindingsUpdated","payload":{"keybindings":[],"secret":"PRIVATE"}}
            ]}).to_string().into())).unwrap();
            while let Ok(message)=ws.read(){if let tungstenite::Message::Text(text)=message{let v:Value=serde_json::from_str(&text).unwrap();if v["_tag"]=="Interrupt"{ii.store(true,Ordering::Relaxed);break}}}
        }
    });
    let e=Environment{id:"fixture".into(),name:"Fixture".into(),base_url:base.clone(),token_file:"/unused".into(),write_token_file:None,bearer:None,connection_error:None,connection:Some(Arc::new(protocol::Connection::new(&base,None,protocol::Credential::bearer("synthetic").unwrap()).unwrap()))};
    let count=AtomicUsize::new(0);
    keys::subscribe_changes(&e,||count.load(Ordering::Relaxed)==2,||{count.fetch_add(1,Ordering::Relaxed);Ok(())}).unwrap();
    assert_eq!(count.load(Ordering::Relaxed),2);
    for _ in 0..100{if interrupted.load(Ordering::Relaxed){break}thread::sleep(Duration::from_millis(2));}
    assert!(interrupted.load(Ordering::Relaxed));
    allowed.store(false,Ordering::Relaxed);
    assert_eq!(keys::subscribe_changes(&e,||false,||panic!("permission denied")).unwrap_err(),"permission_denied");
    assert_eq!(streams.load(Ordering::Relaxed),1);
    stop.store(true,Ordering::Relaxed);server.join().unwrap();
}
