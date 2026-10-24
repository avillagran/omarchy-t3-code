// A failed RPC is a missing permission only when T3 says so with an
// EnvironmentAuthorizationError; git failures whose text mentions "Author" or
// "Authentication" stay rpc_rejected with the server's reason.
use omarchy_t3_client::{git_operations as git, protocol, write, Environment};
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    sync::Arc,
    thread,
};

const COMMIT_IDENTITY: &str = "Author identity unknown\n\n*** Please tell me who you are.\n\nRun\n\n  git config --global user.email \"you@example.com\"";
const PULL_AUTH: &str = "Authentication failed for 'https://github.com/o/r.git'\nfatal: could not read Username";

fn failure(error: Value) -> Value {
    json!({"_tag":"Failure","cause":[{"_tag":"Fail","error":error}]})
}
fn git_error(operation: &str, message: &str) -> Value {
    failure(json!({"_tag":"GitCommandError","operation":operation,"command":format!("git {operation}"),"cwd":"/remote/tree","detail":message,"message":message}))
}

#[test]
fn git_failures_with_auth_words_are_rejections_not_missing_permissions() {
    let root = std::env::temp_dir().join(format!("ot3-rejection-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&root).unwrap();
    std::env::set_var("XDG_STATE_HOME", root.join("state"));
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            let mut peek = [0; 512];
            let n = stream.peek(&mut peek).unwrap();
            let head = String::from_utf8_lossy(&peek[..n]).to_lowercase();
            if !head.contains("upgrade: websocket") {
                let mut headers = vec![];
                let mut byte = [0];
                while !headers.ends_with(b"\r\n\r\n") {
                    stream.read_exact(&mut byte).unwrap();
                    headers.push(byte[0]);
                }
                let body = if head.starts_with("get /api/auth/session ") {
                    json!({"authenticated":true,"permissions":["source-control:write","orchestration:read"]})
                } else {
                    assert!(head.starts_with("get /api/orchestration/shell "));
                    json!({"threads":[{"id":"thread","projectId":"p","worktreePath":"/remote/tree"}],"projects":[{"id":"p","workspaceRoot":"/remote/root"}]})
                }
                .to_string();
                write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
                continue;
            }
            let mut ws = tungstenite::accept(stream).unwrap();
            let request: Value = serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();
            let success = |value: Value| json!({"_tag":"Success","value":value});
            let exit = match request["tag"].as_str().unwrap() {
                "server.getSettings" => success(json!({"defaultProjectScripts":[]})),
                "vcs.refreshStatus" => success(json!({"isRepo":true,"hasPrimaryRemote":true,"isDefaultRef":false,"hasWorkingTreeChanges":false,"hasUpstream":true,"refName":"feature","aheadCount":0,"behindCount":2,"pr":null,"workingTree":{"files":[],"insertions":0,"deletions":0}})),
                "vcs.pull" => git_error("pull", PULL_AUTH),
                // The tag does not matter to the classification; only the error's _tag does.
                "vcs.init" => git_error("commit", COMMIT_IDENTITY),
                "vcs.switchRef" => failure(json!({"_tag":"EnvironmentAuthorizationError","message":"The authenticated token is missing required scope: source-control:write.","requiredScope":"source-control:write"})),
                other => panic!("unexpected RPC {other}"),
            };
            ws.send(tungstenite::Message::Text(json!({"_tag":"Exit","requestId":request["id"],"exit":exit}).to_string().into())).unwrap();
        }
    });
    let connection = protocol::Connection::new(&base, None, protocol::Credential::bearer("synthetic").unwrap()).unwrap();
    let e = Environment { id: "fixture".into(), name: "Fixture".into(), base_url: base, token_file: "/unused".into(), write_token_file: None, bearer: None, connection_error: None, connection: Some(Arc::new(connection)) };

    // The pull a user starts from the Git header: the journaled error is git's reason.
    let scope = json!({"environment":"fixture","thread":"thread","projectId":"p","cwd":"/remote/tree","requestId":uuid::Uuid::new_v4().to_string()});
    let view = git::read(&e, &scope).unwrap();
    assert_eq!(view["permissions"]["sourceControlWrite"], true);
    let id = uuid::Uuid::new_v4().to_string();
    let intent = json!({"scope":view["scope"],"revision":view["revision"],"operation":"pull","confirmed":true,"defaultBranchConfirmed":false,"actionId":id});
    let receipt = git::action(&e, &json!({"op":"git-action","environment":"fixture","thread":"thread","requestId":id,"intent":intent})).unwrap();
    assert_eq!(receipt["error"], "rpc_rejected: Authentication failed for 'https://github.com/o/r.git'");
    assert_ne!(receipt["state"], "succeeded");

    // The same classification on ot3's unary transport.
    let commit = write::rpc(&e, "vcs.init", json!({"cwd":"/remote/tree"})).unwrap_err();
    assert_eq!(commit, "rpc_rejected: Author identity unknown");
    assert_eq!(protocol::errors::required_scope(&commit), None);
    let pull = write::rpc(&e, "vcs.pull", json!({"cwd":"/remote/tree"})).unwrap_err();
    assert_eq!(protocol::error_code(&pull), "rpc_rejected");

    // A real missing scope is write_not_authorized and names the scope.
    let denied = write::rpc(&e, "vcs.switchRef", json!({"cwd":"/remote/tree","refName":"main"})).unwrap_err();
    assert_eq!(denied, "write_not_authorized: [source-control:write] The authenticated token is missing required scope: source-control:write.");
    assert_eq!(protocol::errors::required_scope(&denied), Some("source-control:write"));
    assert_eq!(protocol::errors::message(&denied), Some("The authenticated token is missing required scope: source-control:write."));
    fs::remove_dir_all(root).unwrap();
}
