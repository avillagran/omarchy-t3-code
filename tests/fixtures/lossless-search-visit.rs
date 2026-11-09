//! Private genuine client/SDK/WebSocket/V8 acceptance; never execute on the host.
use omarchy_t3_client::{
    json,
    native_search::Engine,
    normalize,
    protocol::{wtf16::Wtf16String, RawJson, RawRpcExit},
    search_json::{self, Object},
    shell_state, thread_search,
    voice_capture::resources::Runtime,
};
use serde_json::Value;
use std::{
    fs, net::TcpListener, os::unix::fs::PermissionsExt, path::Path, sync::Arc, thread,
    time::Duration,
};
use tungstenite::{
    handshake::server::{Callback, ErrorResponse, Request, Response},
    Message,
};
struct PrivateUpgrade;
impl Callback for PrivateUpgrade {
    fn on_request(self, request: &Request, response: Response) -> Result<Response, ErrorResponse> {
        assert_eq!(
            request.headers()["authorization"],
            "Bearer private-lossless-token"
        );
        Ok(response)
    }
}
fn raw(value: &impl serde::Serialize) -> Box<RawJson> {
    search_json::encode(value).unwrap()
}
fn text(units: Vec<u16>) -> Box<RawJson> {
    Wtf16String::from_units(units).to_json()
}
fn object(pairs: Vec<(&str, Box<RawJson>)>) -> Box<RawJson> {
    raw(&pairs
        .into_iter()
        .map(|(key, value)| (key.to_owned(), value))
        .collect::<Object>())
}
fn units(value: &RawJson) -> Vec<u16> {
    Wtf16String::from_json(value).unwrap().into_units()
}
fn field(raw: &RawJson, key: &str) -> Box<RawJson> {
    search_json::object(raw).unwrap().remove(key).unwrap()
}
fn peer(failed: bool) -> (String, thread::JoinHandle<Vec<(String, Box<RawJson>)>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let task = thread::spawn(move || {
        let mut seen = Vec::new();
        for _ in 0..if failed { 5 } else { 9 } {
            let (stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(10)))
                .unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(10)))
                .unwrap();
            let mut socket = tungstenite::accept_hdr(stream, PrivateUpgrade).unwrap();
            let packet = loop {
                match socket.read().unwrap() {
                    Message::Text(value) => break RawJson::from_string(value.to_string()).unwrap(),
                    Message::Ping(value) => socket.send(Message::Pong(value)).unwrap(),
                    other => panic!("Unexpected private peer packet {other:?}"),
                }
            };
            let tag: String = search_json::scalar(&field(&packet, "tag")).unwrap();
            let payload = field(&packet, "payload");
            let exit = match tag.as_str() {
                "server.getConfig" => {
                    raw(&json!({"_tag":"Success","value":{"shellOlderSettled":true}}))
                }
                "orchestration.searchThreads" => {
                    let query = field(&payload, "query");
                    assert!((2..=200).contains(&units(&query).len()));
                    if failed {
                        object(vec![
                            ("_tag", raw(&"Failure")),
                            (
                                "cause",
                                object(vec![("_tag", raw(&"Fail")), ("query", query)]),
                            ),
                        ])
                    } else {
                        let mut title_units = vec![97, 98, 0xd800, 116, 105, 116, 108, 101];
                        title_units.push(*units(&query).last().unwrap());
                        let title = text(title_units);
                        let mut snippet = vec![115; 239];
                        snippet.push(0xd83d);
                        let matched = object(vec![
                            ("threadId", raw(&"older")),
                            ("environmentId", raw(&"forged")),
                            ("snippet", text(snippet)),
                            ("query", query),
                        ]);
                        let row = object(vec![
                            ("id", raw(&"older")),
                            ("environmentId", raw(&"forged")),
                            ("title", title),
                            ("branch", text(vec![98, 0xdc00])),
                            ("createdAt", raw(&"2026-10-01T00:00:00.000Z")),
                            ("updatedAt", raw(&"2026-10-02T00:00:00.000Z")),
                            ("lastVisitedAt", raw(&"2026-10-01T00:00:00.000Z")),
                        ]);
                        object(vec![
                            ("_tag", raw(&"Success")),
                            (
                                "value",
                                object(vec![
                                    ("matches", raw(&vec![matched])),
                                    ("snapshotSequence", raw(&10)),
                                    ("threads", raw(&vec![row])),
                                ]),
                            ),
                        ])
                    }
                }
                "orchestration.dispatchCommand" => {
                    assert_eq!(
                        search_json::scalar::<String>(&field(&payload, "type")).unwrap(),
                        "thread.visit"
                    );
                    assert_eq!(
                        search_json::scalar::<String>(&field(&payload, "threadId")).unwrap(),
                        "older"
                    );
                    assert!(search_json::scalar::<bool>(&field(&payload, "opened")).unwrap());
                    object(vec![
                        ("_tag", raw(&"Success")),
                        (
                            "value",
                            object(vec![
                                ("sequence", raw(&11)),
                                ("picked", field(&payload, "searchQuery")),
                            ]),
                        ),
                    ])
                }
                other => panic!("Unexpected real RPC {other}"),
            };
            let id = field(&packet, "id");
            let reply = object(vec![
                ("_tag", raw(&"Exit")),
                ("requestId", id),
                ("exit", exit),
            ]);
            socket
                .send(Message::Text(reply.get().to_owned().into()))
                .unwrap();
            let _ = socket.close(None);
            seen.push((tag, payload));
        }
        seen
    });
    (endpoint, task)
}
fn mount(consumer: &str, key: &str, pages: &RawJson) -> Value {
    let payload = object(vec![
        ("consumer", raw(&consumer)),
        ("key", raw(&key)),
        (
            "rows",
            RawJson::from_string(pages.get().to_owned()).unwrap(),
        ),
    ]);
    json!({"op":"thread-search-mount","payloadJson":payload.get()})
}
fn emit(value: Value) {
    use std::io::Write;
    let mut output = std::io::stdout().lock();
    writeln!(output, "{value}").unwrap();
    output.flush().unwrap();
}
fn qt_pipe(
    mut engine: Engine,
    shell: Value,
    good_peer: thread::JoinHandle<Vec<(String, Box<RawJson>)>>,
    bad_peer: thread::JoinHandle<Vec<(String, Box<RawJson>)>>,
) {
    use std::io::BufRead;
    emit(json!({"type":"ready"}));
    let mut visits = 0;
    let mut searches = 0;
    for line in std::io::stdin().lock().lines() {
        let line = line.unwrap();
        assert!(line.len() <= 16 * 1024 * 1024);
        let input = search_json::pipe_input(&line).unwrap();
        let request = input["requestId"].clone();
        match input["op"].as_str().unwrap() {
            "fixture-codec" => {
                let packet = input["payloadJson"].as_str().unwrap();
                assert!(packet.contains(r#""key\ud800":"value\udc00""#));
                let encoded = serde_json::to_string(&input).unwrap();
                let decoded: Value = serde_json::from_str(&encoded).unwrap();
                assert_eq!(decoded["payloadJson"].as_str().unwrap(), packet);
                emit(json!({"type":"codec-ready"}));
            }
            "native-search" => {
                let mut input = input.clone();
                input["op"] = input["action"].clone();
                let result = engine.call(input).unwrap();
                emit(
                    json!({"type":"action","data":{"op":"native-search-result","requestId":request,"result":result}}),
                );
            }
            "thread-search" => {
                searches += 1;
                let result=thread_search::search_packed(&input,|partial|emit(json!({"type":"action","data":{"op":"thread-search","requestId":request,"result":partial}}))).unwrap();
                emit(
                    json!({"type":"action","data":{"op":"thread-search","requestId":request,"result":result}}),
                );
            }
            "thread-search-mount" => {
                shell_state::mount_search_json(&input).unwrap();
            }
            "fixture-state" => {
                let rows =
                    normalize(&shell_state::enriched("good", shell.clone()), "good", 0).unwrap();
                emit(json!({"type":"snapshot","data":{"threads":rows}}));
            }
            "thread-visit" => {
                visits += 1;
                let RawRpcExit::Success(Some(value)) = thread_search::visit_raw(&input).unwrap()
                else {
                    panic!("Actual Qt visit failed")
                };
                emit(
                    json!({"type":"action","data":{"op":"thread-visit","requestId":request,"result":{"resultJson":value.get()}}}),
                );
            }
            "fixture-close" => {
                assert_eq!(visits, 4);
                assert_eq!(searches, 4);
                engine.close().unwrap();
                assert_eq!(good_peer.join().unwrap().len(), 9);
                assert_eq!(bad_peer.join().unwrap().len(), 5);
                emit(
                    json!({"type":"complete","searches":searches,"visits":visits,"workerRetired":true}),
                );
                return;
            }
            other => panic!("Unexpected private Qt operation {other}"),
        }
    }
    panic!("Qt fixture closed before actual completion");
}
fn main() {
    assert!(Path::new("/run/.containerenv").is_file());
    let directory = Path::new("/tmp").join(format!("lossless-search-{}", std::process::id()));
    fs::create_dir(&directory).unwrap();
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
    let token = directory.join("token");
    fs::write(&token, b"private-lossless-token").unwrap();
    fs::set_permissions(&token, fs::Permissions::from_mode(0o600)).unwrap();
    let (good, good_peer) = peer(false);
    let (bad, bad_peer) = peer(true);
    let config = directory.join("environments.json");
    fs::write(&config,json!({"environments":[{"id":"good","name":"good","base_url":good,"token_file":token},{"id":"bad","name":"bad","base_url":bad,"token_file":token}]}).to_string()).unwrap();
    std::env::set_var("OMARCHY_T3_FIXTURE_ENVIRONMENTS", config);
    let shell = json!({"snapshotSequence":10,"threads":[],"projects":[],"olderSettled":{"before":"2026-10-01T00:00:00.000Z","asOfSequence":10}});
    for env in ["good", "bad"] {
        shell_state::reset(env);
        shell_state::apply(env, &json!({"kind":"snapshot","snapshot":shell}));
        shell_state::set_live(env, true);
    }
    let runtime = Arc::new(
        Runtime::admit(Path::new(
            &std::env::var_os("O3_NATIVE_ELECTRON_RUNTIME")
                .expect("Admitted private runtime required"),
        ))
        .unwrap(),
    );
    let mut engine = Engine::launch(runtime).unwrap();
    if std::env::args().any(|arg| arg == "--qt-pipe") {
        qt_pipe(engine, shell, good_peer, bad_peer);
        return;
    }
    let mut clip = vec![97; 199];
    clip.push(0xd83d);
    let cases = vec![
        clip,
        vec![97, 98, 0xdc00],
        vec![97, 98, 0xd83d, 0xde00],
        vec![97, 98, 0xd800],
    ];
    let mut expected_picks = Vec::new();
    for (index, query) in cases.iter().enumerate() {
        let request = object(vec![
            ("op", raw(&"thread-search")),
            ("requestId", raw(&format!("case-{index}"))),
            ("query", text(query.clone())),
            ("environmentIds", raw(&vec!["good", "bad"])),
            ("searchMessages", raw(&true)),
        ]);
        let input = search_json::pipe_input(request.get()).unwrap();
        assert!(input.get("query").is_none());
        let answer = thread_search::search_raw(&input, |_| {}).unwrap();
        assert_eq!(units(&field(&answer, "query")), *query);
        let matches = search_json::array(&field(&answer, "matches")).unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(
            search_json::scalar::<String>(&field(&matches[0], "environmentId")).unwrap(),
            "good"
        );
        let snippet = units(&field(&matches[0], "snippet"));
        assert_eq!(snippet.len(), 240);
        assert_eq!(snippet[239], 0xd83d);
        let errors = search_json::object(&field(&answer, "errors")).unwrap();
        assert_eq!(
            units(&field(&field(&errors["bad"], "cause"), "query")),
            *query
        );
        let pages = field(&answer, "rows");
        let key = format!("native-key-{index}");
        shell_state::mount_search_json(&mount("sidebar", &key, &pages)).unwrap();
        shell_state::mount_search_json(&mount("palette", &key, &pages)).unwrap();
        shell_state::mount_search_json(&mount("sidebar", "", &raw(&Vec::<Value>::new()))).unwrap();
        let enriched = shell_state::enriched("good", shell.clone());
        let items = normalize(&enriched, "good", 0).unwrap();
        assert_eq!(items.len(), 1);
        let packed =
            RawJson::from_string(items[0]["searchShellJson"].as_str().unwrap().to_owned()).unwrap();
        assert_eq!(
            units(&field(&packed, "title")),
            [
                vec![97, 98, 0xd800, 116, 105, 116, 108, 101],
                vec![*query.last().unwrap()]
            ]
            .concat()
        );
        assert_eq!(
            search_json::scalar::<String>(&field(&packed, "environmentId")).unwrap(),
            "good"
        );
        let native_row = object(vec![("key", raw(&"good/older")), ("thread", packed)]);
        let native = object(vec![
            ("key", raw(&"good/older")),
            ("query", text(query.clone())),
            ("now", raw(&(1_700_000_000_000u64 + index as u64))),
            ("rows", raw(&vec![native_row])),
        ]);
        let reply = engine
            .call(json!({"op":"open","payloadJson":native.get()}))
            .unwrap();
        let picked =
            RawJson::from_string(reply["resultJson"].as_str().unwrap().to_owned()).unwrap();
        let picked = field(&picked, "searchQuery");
        let pick_units = units(&picked);
        assert!(pick_units.len() <= 32);
        if index == 1 {
            assert_eq!(pick_units, vec![97, 98, 0xdc00]);
        }
        if index == 0 {
            assert_eq!(pick_units, vec![97; 32]);
        }
        if index == 3 {
            assert_eq!(pick_units, vec![97, 98]);
        }
        expected_picks.push(pick_units.clone());
        let visit = object(vec![
            ("op", raw(&"thread-visit")),
            ("environment", raw(&"good")),
            ("thread", raw(&"older")),
            (
                "visitedAt",
                raw(&if index == 0 {
                    "2026-10-08"
                } else {
                    "2026-10-08T10:20:30"
                }),
            ),
            ("opened", raw(&true)),
            ("searchQuery", picked),
        ]);
        let input = search_json::pipe_input(visit.get()).unwrap();
        let result = if index == 1 {
            let RawRpcExit::Success(Some(result)) = thread_search::visit_raw(&input).unwrap()
            else {
                panic!("Actual raw visit failed")
            };
            result
        } else {
            // Scalar callers retain the actual typed write path. Date-only and
            // zone-free strings must reach the wire without the old RFC filter.
            let scalar: Value = serde_json::from_str(visit.get()).unwrap();
            raw(&omarchy_t3_client::write::execute(&scalar).unwrap())
        };
        assert_eq!(units(&field(&result, "picked")), pick_units);
        shell_state::mount_search_json(&mount("palette", "", &raw(&Vec::<Value>::new()))).unwrap();
        assert!(shell_state::enriched("good", shell.clone())["threads"]
            .as_array()
            .unwrap()
            .is_empty());
    }
    engine.close().unwrap();
    let good = good_peer.join().unwrap();
    let bad = bad_peer.join().unwrap();
    assert_eq!(good.len(), 9);
    assert_eq!(bad.len(), 5);
    let actual: Vec<_> = good
        .iter()
        .filter(|(tag, _)| tag == "orchestration.dispatchCommand")
        .map(|(_, request)| units(&field(request, "searchQuery")))
        .collect();
    assert_eq!(actual, expected_picks);
    println!("LOSSLESS_SEARCH_VISIT_PASS clip200 snippet240 high low pair realRPC nativePickedQuery rawCause sharedOwners trustedScope normalWorkerRetirement");
}
