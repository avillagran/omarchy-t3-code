use omarchy_t3_client::{
    environments,
    rpc_sessions::{self, Owners},
};
use serde_json::{json, Value};
use std::os::unix::fs::PermissionsExt;
use std::{
    fs,
    net::TcpListener,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Condvar, Mutex,
    },
    thread,
    time::Duration,
};
struct Peer {
    base: String,
    stop: Arc<AtomicBool>,
    gate: Arc<(Mutex<bool>, Condvar)>,
    hold: Arc<AtomicBool>,
    blocked: mpsc::Receiver<()>,
    worker: Option<thread::JoinHandle<()>>,
    calls: Arc<Mutex<Vec<Value>>>,
}
impl Peer {
    fn new() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let stop = Arc::new(AtomicBool::new(false));
        let gate = Arc::new((Mutex::new(true), Condvar::new()));
        let hold = Arc::new(AtomicBool::new(false));
        let calls = Arc::new(Mutex::new(vec![]));
        let (blocked_tx, blocked) = mpsc::channel();
        let (s, g, h, c) = (stop.clone(), gate.clone(), hold.clone(), calls.clone());
        let worker = thread::spawn(move || {
            let mut workers = vec![];
            while !s.load(Ordering::Acquire) {
                let (socket, _) = match listener.accept() {
                    Ok(v) => v,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2));
                        continue;
                    }
                    Err(e) => panic!("{e}"),
                };
                let (s, g, c, blocked_tx) = (s.clone(), g.clone(), c.clone(), blocked_tx.clone());
                let delay = h.swap(false, Ordering::AcqRel);
                workers.push(thread::spawn(move||{
     if delay {blocked_tx.send(()).unwrap();let mut ready=g.0.lock().unwrap();while !*ready{ready=g.1.wait(ready).unwrap();}}
     socket.set_read_timeout(Some(Duration::from_millis(100))).unwrap();socket.set_write_timeout(Some(Duration::from_millis(100))).unwrap();
     let mut ws=match tungstenite::accept(socket){Ok(ws)=>ws,Err(_)=>return};
     while !s.load(Ordering::Acquire){
      match ws.read(){
       Ok(tungstenite::Message::Text(text))=>{
        let v:Value=serde_json::from_str(&text).unwrap();
        if v["_tag"]=="Ping"{ws.send(tungstenite::Message::Text(json!({"_tag":"Pong"}).to_string().into())).unwrap();continue}
        if v["_tag"]!="Request"{continue}
        c.lock().unwrap().push(v.clone());
        if v["tag"]=="drop-peer"{let _=ws.close(None);return}
        let answer=json!({"_tag":"Exit","requestId":v["id"],"exit":{"_tag":"Success","value":v["payload"]}});
        if ws.send(tungstenite::Message::Text(answer.to_string().into())).is_err(){return}
       }
       Ok(tungstenite::Message::Ping(p))=>{if ws.send(tungstenite::Message::Pong(p)).is_err(){return}},
       Err(tungstenite::Error::Io(e)) if matches!(e.kind(),std::io::ErrorKind::WouldBlock|std::io::ErrorKind::TimedOut)=>{},
       _=>return,
      }
     }
    }));
            }
            for worker in workers {
                worker.join().unwrap()
            }
        });
        Self {
            base,
            stop,
            gate,
            hold,
            blocked,
            worker: Some(worker),
            calls,
        }
    }
    fn hold_next(&self) {
        *self.gate.0.lock().unwrap() = false;
        self.hold.store(true, Ordering::Release)
    }
    fn release(&self) {
        *self.gate.0.lock().unwrap() = true;
        self.gate.1.notify_all()
    }
}
impl Drop for Peer {
    fn drop(&mut self) {
        self.release();
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            worker.join().unwrap()
        }
    }
}
#[test]
fn physical_owners_preserve_unrelated_environments_and_reject_retired_handshakes() {
    assert!(std::path::Path::new("/run/.containerenv").is_file());
    let root = std::env::temp_dir().join(format!("rpc-owner-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let first_token = root.join("first-token");
    let second_token = root.join("second-token");
    fs::write(&first_token, "synthetic-first-token").unwrap();
    fs::write(&second_token, "synthetic-second-token").unwrap();
    fs::set_permissions(&first_token, fs::Permissions::from_mode(0o600)).unwrap();
    fs::set_permissions(&second_token, fs::Permissions::from_mode(0o600)).unwrap();
    let first = Peer::new();
    let second = Peer::new();
    let config = root.join("environments.json");
    fs::write(&config,json!({"environments":[{"id":"first","base_url":first.base,"token_file":first_token,"write_token_file":first_token},{"id":"second","base_url":second.base,"token_file":second_token,"write_token_file":second_token}]}).to_string()).unwrap();
    std::env::set_var("OMARCHY_T3_FIXTURE_ENVIRONMENTS", &config);
    let (events_tx, events_rx) = mpsc::channel();
    rpc_sessions::set_events(events_tx);
    let owners = Arc::new(Owners::default());
    let one = owners.session("first", true).unwrap();
    let two = owners.session("second", true).unwrap();
    assert_eq!(one.id(), owners.session("first", true).unwrap().id());
    assert_eq!(
        one.rpc("metadata", json!({"file":"a"}), Duration::from_secs(2))
            .unwrap(),
        json!({"file":"a"})
    );
    fs::write(&first_token, "synthetic-updated-token").unwrap();
    owners.reconcile(&environments().unwrap());
    assert!(!one.is_open());
    assert!(two.is_open());
    assert_eq!(two.id(), owners.session("second", true).unwrap().id());
    let changed = owners.session("first", true).unwrap();
    assert_ne!(one.id(), changed.id());
    let mut old_closed = false;
    for _ in 0..8 {
        let event = events_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        if event["state"] == "closed" && event["sessionId"] == one.id() {
            old_closed = true;
            break;
        }
    }
    assert!(
        old_closed,
        "physical close observer reports the retired socket identity"
    );
    owners.close_environment("first");
    assert!(!changed.is_open());
    assert!(two.is_open());
    first.hold_next();
    let opening = owners.clone();
    let pending = thread::spawn(move || opening.session("first", true));
    first.blocked.recv_timeout(Duration::from_secs(2)).unwrap();
    owners.close_environment("first");
    assert!(two.is_open());
    first.release();
    assert_eq!(pending.join().unwrap().err().unwrap(), "connection_changed");
    let current = owners.session("first", true).unwrap();
    assert_ne!(changed.id(), current.id());
    assert_eq!(
        current
            .rpc("drop-peer", json!({"once":true}), Duration::from_secs(2))
            .unwrap_err(),
        "rpc_outcome_unknown"
    );
    let recovered = owners.session("first", true).unwrap();
    assert_ne!(current.id(), recovered.id());
    assert_eq!(
        first
            .calls
            .lock()
            .unwrap()
            .iter()
            .filter(|v| v["tag"] == "drop-peer")
            .count(),
        1
    );
    assert_eq!(two.id(), owners.session("second", true).unwrap().id());
    let mut remaining = environments().unwrap();
    remaining.retain(|e| e.id == "second");
    owners.reconcile(&remaining);
    assert!(!recovered.is_open());
    assert!(two.is_open());
    owners.close_all();
    assert!(!two.is_open());
}
