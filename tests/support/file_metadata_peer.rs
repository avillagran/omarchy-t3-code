use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};
// Synthetic loopback HTTP and persistent RPC peer. No profile or filesystem reads.
pub struct Peer {
    pub base: String,
    pub allow: Arc<AtomicBool>,
    pub operate: Arc<AtomicBool>,
    pub hold: Arc<AtomicBool>,
    pub present: Arc<AtomicBool>,
    pub calls: Arc<Mutex<Vec<Value>>>,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}
fn respond(
    ws: &mut tungstenite::WebSocket<std::net::TcpStream>,
    request: &Value,
    present: bool,
) -> bool {
    let value = match request["tag"].as_str().unwrap_or("") {
        "filesystem.getMetadata" => {
            json!({"entries":request["payload"]["paths"].as_array().unwrap().iter().map(|path| {
            if !present || path.as_str().unwrap().contains("missing") { Value::Null }
            else { json!({"kind":if path.as_str().unwrap().contains("directory"){"directory"}else{"file"},"byteLength":7,"mimeType":"text/plain"}) }
        }).collect::<Vec<_>>()})
        }
        "projects.readFile" => {
            json!({"exists":true,"relativePath":request["payload"]["relativePath"],"contents":"fixture","byteLength":7,"truncated":false})
        }
        "projects.searchEntries" => {
            json!({"entries":[{"path":"seeded.ts","kind":"file"}],"truncated":false})
        }
        "server.getConfig" => json!({"availableEditors":["zed"]}),
        "shell.openInEditor" => json!({}),
        _ => request["payload"].clone(),
    };
    ws.send(tungstenite::Message::Text(
        json!({"_tag":"Exit","requestId":request["id"],"exit":{"_tag":"Success","value":value}})
            .to_string()
            .into(),
    ))
    .is_ok()
}
impl Peer {
    pub fn new() -> Self {
        assert!(std::path::Path::new("/run/.containerenv").is_file());
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let allow = Arc::new(AtomicBool::new(true));
        let operate = Arc::new(AtomicBool::new(true));
        let hold = Arc::new(AtomicBool::new(false));
        let present = Arc::new(AtomicBool::new(true));
        let stop = Arc::new(AtomicBool::new(false));
        let calls = Arc::new(Mutex::new(Vec::new()));
        let (s, a, o, h, p, c) = (
            stop.clone(),
            allow.clone(),
            operate.clone(),
            hold.clone(),
            present.clone(),
            calls.clone(),
        );
        let worker = thread::spawn(move || {
            let mut workers = Vec::new();
            while !s.load(Ordering::Acquire) {
                let (mut socket, _) = match listener.accept() {
                    Ok(value) => value,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2));
                        continue;
                    }
                    Err(error) => panic!("{error}"),
                };
                let (s, a, o, h, p, c) = (
                    s.clone(),
                    a.clone(),
                    o.clone(),
                    h.clone(),
                    p.clone(),
                    c.clone(),
                );
                workers.push(thread::spawn(move || {
                    socket.set_read_timeout(Some(Duration::from_millis(100))).unwrap();
                    socket.set_write_timeout(Some(Duration::from_millis(500))).unwrap();
                    let mut header=[0;4096];
                    let deadline=Instant::now()+Duration::from_secs(2);
                    let size=loop {
                        if s.load(Ordering::Acquire)||Instant::now()>=deadline{return}
                        match socket.peek(&mut header) {
                            Ok(size) if header[..size].windows(4).any(|bytes|bytes==b"\r\n\r\n")=>break size,
                            Ok(size) if size==header.len()||size==0=>return,
                            Ok(_)=>thread::sleep(Duration::from_millis(1)),
                            Err(error) if matches!(error.kind(),std::io::ErrorKind::WouldBlock|std::io::ErrorKind::TimedOut)=>{},
                            Err(_)=>return,
                        }
                    };
                    let start=String::from_utf8_lossy(&header[..size]);
                    if start.starts_with("GET /api/auth/session ") || start.starts_with("GET /api/orchestration/shell ") {
                        let auth=start.starts_with("GET /api/auth/session ");
                        let mut header=Vec::new();let mut byte=[0];
                        while !header.ends_with(b"\r\n\r\n") { if socket.read_exact(&mut byte).is_err(){return};header.push(byte[0]);assert!(header.len()<16384); }
                        if !auth {assert!(String::from_utf8_lossy(&header).to_ascii_lowercase().contains("x-t3-orchestration-protocol: 2"),"shell read must keep native protocol negotiation");}
                        let value=if auth {let mut scopes=vec!["orchestration:read"];if a.load(Ordering::Acquire){scopes.push("filesystem:read")};if o.load(Ordering::Acquire){scopes.push("orchestration:operate")};json!({"authenticated":true,"permissions":scopes})} else {json!({"projects":[{"id":"project","workspaceRoot":"/work"}],"threads":[{"id":"thread","projectId":"project","worktreePath":"/work"}]})};
                        let value=value.to_string();let _=write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",value.len(),value);return
                    }
                    let mut ws=match tungstenite::accept(socket){Ok(value)=>value,Err(_)=>return};
                    let mut pending:Vec<Value>=Vec::new();
                    while !s.load(Ordering::Acquire) {
                        if !h.load(Ordering::Acquire) { for request in std::mem::take(&mut pending) {if !respond(&mut ws,&request,p.load(Ordering::Acquire)){return}} }
                        match ws.read() {
                            Ok(tungstenite::Message::Text(text))=>{
                                let value:Value=serde_json::from_str(&text).unwrap();
                                if value["_tag"]=="Ping" {let _=ws.send(tungstenite::Message::Text(json!({"_tag":"Pong"}).to_string().into()));continue}
                                c.lock().unwrap().push(value.clone());
                                if value["_tag"]=="Interrupt" {pending.retain(|request|request["id"]!=value["requestId"]);continue}
                                if value["_tag"]!="Request" {continue}
                                if h.load(Ordering::Acquire) {pending.push(value)} else if !respond(&mut ws,&value,p.load(Ordering::Acquire)){return}
                            },
                            Ok(tungstenite::Message::Ping(value))=>{let _=ws.send(tungstenite::Message::Pong(value));},
                            Err(tungstenite::Error::Io(error)) if matches!(error.kind(),std::io::ErrorKind::WouldBlock|std::io::ErrorKind::TimedOut)=>{},
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
            allow,
            operate,
            hold,
            present,
            calls,
            stop,
            worker: Some(worker),
        }
    }
    pub fn metadata_calls(&self) -> Vec<Value> {
        self.calls
            .lock()
            .unwrap()
            .iter()
            .filter(|value| value["tag"] == "filesystem.getMetadata")
            .cloned()
            .collect()
    }
    pub fn wait(&self, predicate: impl Fn(&[Value]) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !predicate(&self.calls.lock().unwrap()) {
            assert!(Instant::now() < deadline, "synthetic RPC event deadline");
            thread::sleep(Duration::from_millis(2))
        }
    }
}
impl Drop for Peer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            worker.join().unwrap()
        }
    }
}
