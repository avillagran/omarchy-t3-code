use omarchy_t3_client::*;
use serde_json::{json, Value};
use std::{net::TcpListener, thread};
#[test]
fn remote_folder_is_validated_on_remote_before_project_create() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let e = Environment {
        id: "remote".into(),
        name: "NODE".into(),
        base_url: format!("http://{}", listener.local_addr().unwrap()),
        token_file: Default::default(),
        write_token_file: None,
        bearer: Some("synthetic-token".into()),
        connection_error: None,
        connection: None,
    };
    let root = std::env::temp_dir().join(format!("ot3-project-{}", uuid::Uuid::new_v4()));
    std::env::set_var("XDG_STATE_HOME", &root);
    let server = thread::spawn(move || {
        for tag in ["filesystem.getMetadata", "projects.mutate"] {
            let (s, _) = listener.accept().unwrap();
            let mut ws = tungstenite::accept(s).unwrap();
            let req: Value = serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();
            assert_eq!(req["tag"], tag);
            let result = if tag == "filesystem.getMetadata" {
                assert_eq!(req["payload"], json!({"paths":["/home/node/Only on NODE"]}));
                json!({"entries":[{"kind":"directory"}]})
            } else {
                assert_eq!(req["payload"]["workspaceRoot"], "/home/node/Only on NODE");
                assert_eq!(req["payload"]["title"], "Only on NODE");
                assert_eq!(req["payload"]["createWorkspaceRootIfMissing"], false);
                json!({})
            };
            ws.send(tungstenite::Message::Text(
                json!({"_tag":"Exit","requestId":"1","exit":{"_tag":"Success","value":result}})
                    .to_string()
                    .into(),
            ))
            .unwrap();
        }
    });
    write::manage(&e,&json!({"action":"add-project","environment":"remote","workspaceRoot":"/home/node/Only on NODE","requestId":uuid::Uuid::new_v4().to_string()})).unwrap();
    server.join().unwrap();
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn browse_paths_and_results_keep_native_scope() {
    use protocol::projects::{folder_path, folders};
    for p in ["/", "~/Work", "/home/node/A B"] {
        assert_eq!(folder_path(p).unwrap(), p)
    }
    for p in ["relative", "file:///tmp", "\n", "/tmp\nsecret"] {
        assert!(folder_path(p).is_err())
    }
    let d=folders(&json!({"parentPath":"/home/node","entries":[{"name":"Repo","fullPath":"/home/node/Repo"},{"name":"bad","fullPath":"file:///bad"}]})).unwrap();
    assert_eq!(d["entries"].as_array().unwrap().len(), 1);
    assert_eq!(d["parentPath"], "/home/node");
}
