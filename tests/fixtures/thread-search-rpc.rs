//! Real client search requests against three private, controlled protocol peers.
//! This proves transport/concurrency and consumer ownership, not server search SQL.
use omarchy_t3_client::{shell_state, thread_search};
use serde_json::{json, Value};
use std::{
    fs,
    net::TcpListener,
    os::unix::fs::PermissionsExt,
    sync::{Arc, Condvar, Mutex},
    thread,
    time::Duration,
};
use tungstenite::Message;

const BEFORE: &str = "2026-10-08T00:00:00.000Z";
type Gate = Arc<(Mutex<bool>, Condvar)>;

fn peer(name: &'static str, gate: Gate) -> (String, thread::JoinHandle<Vec<Value>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let task = thread::spawn(move || {
        let mut requests = Vec::new();
        for _ in 0..2 {
            let (stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(10)))
                .unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(10)))
                .unwrap();
            let mut socket = tungstenite::accept_hdr(
                stream,
                |request: &tungstenite::handshake::server::Request, response| {
                    assert_eq!(request.uri().path(), "/ws");
                    assert_eq!(
                        request.headers()["authorization"],
                        "Bearer private-search-token"
                    );
                    Ok(response)
                },
            )
            .unwrap();
            let packet: Value = loop {
                match socket.read().unwrap() {
                    Message::Text(value) => break serde_json::from_str(&value).unwrap(),
                    Message::Ping(value) => socket.send(Message::Pong(value)).unwrap(),
                    other => panic!("Unexpected private peer packet: {other:?}"),
                }
            };
            assert_eq!(packet["_tag"], "Request");
            let tag = packet["tag"].as_str().unwrap();
            let exit = match tag {
                "server.getConfig" => json!({"_tag":"Success","value":{"shellOlderSettled":true}}),
                "orchestration.searchThreads" => {
                    assert_eq!(
                        packet["payload"],
                        json!({"query":"ab","olderSettledBefore":BEFORE,
                        "projectIds":["project"],"environmentLabel":name,"titlesOnly":true})
                    );
                    if name == "slow" {
                        let (lock, changed) = &*gate;
                        let (released, timeout) = changed
                            .wait_timeout_while(
                                lock.lock().unwrap(),
                                Duration::from_secs(8),
                                |released| !*released,
                            )
                            .unwrap();
                        assert!(
                            *released && !timeout.timed_out(),
                            "Fast result never released the slow peer"
                        );
                    }
                    if name == "failed" {
                        json!({"_tag":"Failure","cause":{"_tag":"Fail","error":{
                            "_tag":"Unsupported","message":"Private unsupported search"}}})
                    } else {
                        json!({"_tag":"Success","value":{"matches":[],"snapshotSequence":10,
                            "threads":[{"id":format!("older-{name}"),"title":format!("ab {name}")}]}})
                    }
                }
                _ => panic!("Unexpected actual RPC tag: {tag}"),
            };
            requests.push(json!({"tag":tag,"payload":packet["payload"]}));
            socket
                .send(Message::Text(
                    json!({"_tag":"Exit","requestId":packet["id"],"exit":exit})
                        .to_string()
                        .into(),
                ))
                .unwrap();
            let _ = socket.close(None);
        }
        requests
    });
    (endpoint, task)
}

fn main() {
    assert!(std::path::Path::new("/run/.containerenv").is_file());
    let directory = std::path::Path::new("/tmp/search-rpc-fixture");
    fs::create_dir(directory).unwrap();
    fs::set_permissions(directory, fs::Permissions::from_mode(0o700)).unwrap();
    let token = directory.join("token");
    fs::write(&token, b"private-search-token").unwrap();
    fs::set_permissions(&token, fs::Permissions::from_mode(0o600)).unwrap();
    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    let mut environments = Vec::new();
    let mut peers = Vec::new();
    let shell = json!({"snapshotSequence":10,"threads":[],"projects":[],
        "olderSettled":{"before":BEFORE,"asOfSequence":10}});
    for name in ["slow", "failed", "fast"] {
        let (endpoint, handle) = peer(name, gate.clone());
        environments.push(json!({"id":name,"name":name,"base_url":endpoint,"token_file":token}));
        peers.push(handle);
        shell_state::apply(name, &json!({"kind":"snapshot","snapshot":shell}));
        shell_state::set_live(name, true);
    }
    let config = directory.join("environments.json");
    fs::write(&config, json!({"environments":environments}).to_string()).unwrap();
    std::env::set_var("OMARCHY_T3_FIXTURE_ENVIRONMENTS", config);
    let mut partials = Vec::new();
    let mut fast_rows_visible = false;
    let result = thread_search::search_with_progress(
        &json!({"query":"ab", "searchMessages":false,
        "environmentIds":["slow","failed","fast"],"projectIds":["project"],
        "environmentLabels":{"slow":"slow","failed":"failed","fast":"fast"}}),
        |partial| {
            // Always release the controlled peer before asserting final expectations.
            // A regression cannot leave an abandoned server thread behind a failed assertion.
            if partial["rows"]
                .as_array()
                .is_some_and(|rows| !rows.is_empty())
            {
                let mounted = shell_state::mount_search("sidebar", "shown-rpc", &partial["rows"]);
                fast_rows_visible = mounted.is_ok()
                    && shell_state::enriched("fast", shell.clone())["threads"][0]["id"]
                        == "older-fast";
                let (lock, changed) = &*gate;
                *lock.lock().unwrap() = true;
                changed.notify_all();
            }
            partials.push(partial);
        },
    )
    .unwrap();
    let joined: Vec<_> = peers.into_iter().map(|peer| peer.join()).collect();
    let requests: Vec<_> = joined.into_iter().map(|peer| peer.unwrap()).collect();
    assert!(fast_rows_visible);
    assert!(partials
        .iter()
        .any(|partial| partial["pending"].as_u64().unwrap() > 0
            && partial["rows"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["environmentId"] == "fast")));
    assert_eq!(result["pending"], 0);
    assert_eq!(
        result["rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["environmentId"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["slow", "fast"]
    );
    assert_eq!(result["errors"]["failed"]["code"], "rpc_rejected");
    assert_eq!(result["errors"]["failed"]["cause"]["error"]["message"], "Private unsupported search");
    assert!(
        shell_state::enriched("slow", shell.clone())["threads"]
            .as_array()
            .unwrap()
            .is_empty(),
        "Search replies must not mount themselves"
    );
    shell_state::mount_search("sidebar", "shown-rpc", &result["rows"]).unwrap();
    assert_eq!(
        shell_state::enriched("slow", shell.clone())["threads"][0]["id"],
        "older-slow"
    );
    shell_state::mount_search("sidebar", "", &json!([])).unwrap();
    for name in ["slow", "fast"] {
        assert!(shell_state::enriched(name, shell.clone())["threads"]
            .as_array()
            .unwrap()
            .is_empty());
    }
    assert!(requests.iter().all(|peer| peer.len() == 2
        && peer[0]["tag"] == "server.getConfig"
        && peer[1]["tag"] == "orchestration.searchThreads"));
    println!("THREAD_SEARCH_RPC_PASS: three actual SDK peers, scoped older-title payloads, fast partial before slow release, independent failure, ordered final rows, consumer-only mount and release, all peer threads joined");
}
