//! Same-process guest ownership and private inspector admission. No UI callback
//! receives these frames; CDP message strings stay opaque throughout routing.
use crate::{
    browser_automation::{Inspector, Lease, Registry, Source, TargetProof},
    desktop_browser_ipc as native, json, text, Result,
};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    io::{BufRead, BufReader, Read, Write},
    net::Shutdown,
    os::unix::net::UnixStream,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
    time::Duration,
};
const LIMIT: usize = 256 * 1024 * 1024;
pub(super) type Native = Arc<dyn Fn(Value) + Send + Sync>;
struct Packet {
    bytes: Vec<u8>,
    budget: Arc<AtomicUsize>,
}
impl Drop for Packet {
    fn drop(&mut self) {
        self.budget.fetch_sub(self.bytes.len(), Ordering::AcqRel);
    }
}
struct Shared {
    source: Source,
    endpoint: UnixStream,
    registry: Arc<Mutex<Registry>>,
    approved: Mutex<HashMap<(String, String), String>>,
    closed: AtomicBool,
    budget: Arc<AtomicUsize>,
    native: Native,
    turns: Arc<Mutex<HashSet<String>>>,
    outgoing: Mutex<()>,
}
impl Shared {
    fn detached(&self, lease: Lease) {
        (self.native)(
            json!({"type":"detached","threadId":lease.inspector().thread,"tabId":lease.inspector().tab}),
        );
    }
    fn close(&self) {
        if self.closed.swap(true, Ordering::AcqRel) {
            return;
        }
        let _ = self.endpoint.shutdown(Shutdown::Both);
        let leases = self
            .registry
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .retire_guest(&self.source);
        for lease in leases {
            self.detached(lease);
        }
    }
    fn inspector(&self, frame: &Value) -> Option<Inspector> {
        let thread = text(frame, "threadId");
        let tab = text(frame, "tabId");
        let instance = text(frame, "instance");
        if uuid::Uuid::parse_str(instance).is_err()
            || !self.approved.lock().ok()?.iter().any(
                |((allowed_thread, allowed_tab), allowed_instance)| {
                    allowed_thread == thread && allowed_tab == tab && allowed_instance == instance
                },
            )
        {
            return None;
        }
        Some(Inspector {
            source: self.source.clone(),
            thread: thread.into(),
            tab: tab.into(),
            instance: instance.into(),
        })
    }
    fn handle(&self, frame: Value, sender: &mpsc::SyncSender<Packet>) -> Result<()> {
        // A closed tab or delayed callback from its destroyed inspector cannot
        // close a newer view or another tab in this physical guest.
        let Some(inspector) = self.inspector(&frame) else {
            return Ok(());
        };
        match text(&frame, "kind") {
            "expected" => {
                let retired = self
                    .registry
                    .lock()
                    .map_err(|_| "local_error")?
                    .expect_inspector(&inspector);
                let Ok(retired) = retired else {
                    return Ok(());
                };
                if let Some(lease) = retired {
                    self.detached(lease);
                }
            }
            "proof" => {
                let _outgoing = self.outgoing.lock().map_err(|_| "local_error")?;
                let proof = TargetProof::verify(
                    inspector.clone(),
                    text(&frame, "viewTarget"),
                    text(&frame, "cdpTarget"),
                );
                let result = match proof {
                    Ok(proof) => self
                        .registry
                        .lock()
                        .map_err(|_| "local_error")?
                        .admit(proof),
                    Err(error) => Err(error),
                };
                let lease = match result {
                    Ok(lease) => lease,
                    Err("browser_inspector_replaced" | "browser_guest_replaced") => return Ok(()),
                    Err(_) => {
                        self.approved
                            .lock()
                            .map_err(|_| "local_error")?
                            .remove(&(inspector.thread.clone(), inspector.tab.clone()));
                        let retired = self
                            .registry
                            .lock()
                            .map_err(|_| "local_error")?
                            .retire_inspector(&inspector);
                        let lease = retired.as_ref().map(|lease| lease.id().to_owned());
                        if let Some(retired) = retired {
                            self.detached(retired);
                        }
                        let mut response = json!({"kind":"retire","threadId":inspector.thread,"tabId":inspector.tab,"instance":inspector.instance});
                        if let Some(lease) = lease {
                            response["lease"] = json!(lease);
                        }
                        self.send(sender, response)?;
                        return Ok(());
                    }
                };
                let active = self
                    .turns
                    .lock()
                    .map_err(|_| "local_error")?
                    .contains(&lease.inspector().thread);
                self.send(sender,json!({"kind":"admitted","threadId":lease.inspector().thread,"tabId":lease.inspector().tab,"instance":lease.inspector().instance,"lease":lease.id(),"agentTurn":active}))?;
            }
            "event" => {
                let accepted = {
                    let registry = self.registry.lock().map_err(|_| "local_error")?;
                    registry
                        .current(&self.source, &inspector.thread, &inspector.tab)
                        .is_some_and(|lease| {
                            lease.inspector() == &inspector && lease.id() == text(&frame, "lease")
                        })
                };
                if !accepted {
                    return Ok(());
                }
                let event =
                    native::decode_event(&frame["event"]).ok_or("browser_private_event_invalid")?;
                if event["threadId"] != inspector.thread || event["tabId"] != inspector.tab {
                    return Err("browser_private_scope_mismatch".into());
                }
                (self.native)(event);
            }
            "retired" => {
                self.approved
                    .lock()
                    .map_err(|_| "local_error")?
                    .remove(&(inspector.thread.clone(), inspector.tab.clone()));
                let retired = self
                    .registry
                    .lock()
                    .map_err(|_| "local_error")?
                    .retire_inspector(&inspector);
                if let Some(lease) = retired {
                    self.detached(lease);
                }
            }
            _ => return Err("browser_private_frame_invalid".into()),
        }
        Ok(())
    }
    fn send(&self, sender: &mpsc::SyncSender<Packet>, value: Value) -> Result<()> {
        if self.closed.load(Ordering::Acquire) {
            return Err("browser_private_closed".into());
        }
        // Count before allocating another large opaque CDP frame.
        struct Count(usize);
        impl Write for Count {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.0 = self
                    .0
                    .checked_add(bytes.len())
                    .filter(|size| *size < LIMIT)
                    .ok_or_else(|| std::io::Error::other("private record limit"))?;
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut count = Count(0);
        serde_json::to_writer(&mut count, &value).map_err(|_| "browser_private_record_limit")?;
        let length = count.0 + 1;
        let mut used = self.budget.load(Ordering::Acquire);
        loop {
            let next = used
                .checked_add(length)
                .filter(|size| *size <= LIMIT)
                .ok_or("browser_private_queue_limit")?;
            match self
                .budget
                .compare_exchange_weak(used, next, Ordering::AcqRel, Ordering::Acquire)
            {
                Ok(_) => break,
                Err(actual) => used = actual,
            }
        }
        let mut packet = Packet {
            bytes: Vec::with_capacity(length),
            budget: self.budget.clone(),
        };
        // The counted immutable Value is serialized once; message strings are not decoded.
        if serde_json::to_writer(&mut packet.bytes, &value).is_err() {
            self.budget
                .fetch_sub(length - packet.bytes.len(), Ordering::AcqRel);
            return Err("browser_private_frame_invalid".into());
        }
        packet.bytes.push(b'\n');
        sender
            .try_send(packet)
            .map_err(|_| "browser_private_queue_limit".into())
    }
}
pub(super) struct Guest {
    shared: Arc<Shared>,
    sender: mpsc::SyncSender<Packet>,
    reader: Option<thread::JoinHandle<()>>,
    writer: Option<thread::JoinHandle<()>>,
}
impl Guest {
    pub(super) fn start(
        source: Source,
        endpoint: UnixStream,
        registry: Arc<Mutex<Registry>>,
        receive: Native,
        turns: Arc<Mutex<HashSet<String>>>,
    ) -> Result<Self> {
        endpoint
            .set_write_timeout(Some(Duration::from_millis(100)))
            .map_err(|_| "browser_private_unavailable")?;
        let input = endpoint
            .try_clone()
            .map_err(|_| "browser_private_unavailable")?;
        let mut output = endpoint
            .try_clone()
            .map_err(|_| "browser_private_unavailable")?;
        let shared = Arc::new(Shared {
            source,
            endpoint,
            registry,
            approved: Mutex::new(HashMap::new()),
            closed: AtomicBool::new(false),
            budget: Arc::new(AtomicUsize::new(0)),
            native: receive,
            turns,
            outgoing: Mutex::new(()),
        });
        let retired = shared
            .registry
            .lock()
            .map_err(|_| "local_error")?
            .register_guest(&shared.source);
        for lease in retired {
            shared.detached(lease);
        }
        let (sender, receiver) = mpsc::sync_channel::<Packet>(4096);
        let writing = shared.clone();
        let writer = thread::Builder::new()
            .name("o3-browser-private-write".into())
            .spawn(move || {
                while !writing.closed.load(Ordering::Acquire) {
                    match receiver.recv_timeout(Duration::from_millis(100)) {
                        Ok(packet) => {
                            if output.write_all(&packet.bytes).is_err() {
                                break;
                            }
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                }
                writing.close();
            })
            .map_err(|_| {
                shared.close();
                "browser_private_unavailable"
            })?;
        let reading = shared.clone();
        let responses = sender.clone();
        let reader = match thread::Builder::new()
            .name("o3-browser-private-read".into())
            .spawn(move || {
                let mut input = BufReader::new(input);
                while !reading.closed.load(Ordering::Acquire) {
                    let mut line = Vec::new();
                    let result = input
                        .by_ref()
                        .take(LIMIT as u64 + 1)
                        .read_until(b'\n', &mut line);
                    let Ok(length) = result else { break };
                    if length == 0 || length > LIMIT || line.last() != Some(&b'\n') {
                        break;
                    }
                    let Ok(frame) = serde_json::from_slice::<Value>(&line) else {
                        break;
                    };
                    if reading.handle(frame, &responses).is_err() {
                        break;
                    }
                }
                reading.close();
            }) {
            Ok(reader) => reader,
            Err(_) => {
                shared.close();
                let _ = writer.join();
                return Err("browser_private_unavailable".into());
            }
        };
        Ok(Self {
            shared,
            sender,
            reader: Some(reader),
            writer: Some(writer),
        })
    }
    pub(super) fn is_open(&self) -> bool {
        !self.shared.closed.load(Ordering::Acquire)
    }
    pub(super) fn approve(&self, thread: &str, tab: &str) -> Result<String> {
        if !self.is_open() {
            return Err("browser_private_closed".into());
        }
        let (instance, retired) = {
            let mut approved = self.shared.approved.lock().map_err(|_| "local_error")?;
            let key = (thread.to_owned(), tab.to_owned());
            if let Some(instance) = approved.get(&key) {
                return Ok(instance.clone());
            }
            // This nonce is allocated for the next actual inspector object and
            // passed to that object's constructor before any asynchronous probe.
            let instance = uuid::Uuid::new_v4().to_string();
            let inspector = Inspector {
                source: self.shared.source.clone(),
                thread: thread.into(),
                tab: tab.into(),
                instance: instance.clone(),
            };
            let retired = self
                .shared
                .registry
                .lock()
                .map_err(|_| "local_error")?
                .expect_inspector(&inspector)
                .map_err(str::to_owned)?;
            approved.insert(key, instance.clone());
            (instance, retired)
        };
        if let Some(lease) = retired {
            self.shared.detached(lease);
        }
        Ok(instance)
    }
    pub(super) fn command(&self, thread: &str, tab: &str, command: Value) -> Result<()> {
        let command = native::decode_command(&command).ok_or("browser_private_command_invalid")?;
        if command["threadId"] != thread || (command["type"] != "turn" && command["tabId"] != tab) {
            return Err("browser_private_scope_mismatch".into());
        }
        let _outgoing = self.shared.outgoing.lock().map_err(|_| "local_error")?;
        let lease = self
            .shared
            .registry
            .lock()
            .map_err(|_| "local_error")?
            .current(&self.shared.source, thread, tab);
        let Some(lease) = lease else {
            return Ok(());
        };
        let result=self.shared.send(&self.sender,json!({"kind":"command","threadId":thread,"tabId":tab,"instance":lease.inspector().instance,"lease":lease.id(),"command":command}));
        if result.is_err() {
            self.shared.close();
        }
        result
    }
    pub(super) fn remove(&self, thread: &str, tab: &str) {
        self.shared
            .approved
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&(thread.into(), tab.into()));
        let lease = {
            let mut registry = self
                .shared
                .registry
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            registry
                .current(&self.shared.source, thread, tab)
                .and_then(|lease| registry.retire_lease(&lease))
        };
        if let Some(lease) = lease {
            self.shared.detached(lease);
        }
    }
    pub(super) fn close(&self) {
        self.shared.close();
    }
}
impl Drop for Guest {
    fn drop(&mut self) {
        self.close();
        for handle in [self.reader.take(), self.writer.take()]
            .into_iter()
            .flatten()
        {
            if handle.thread().id() != thread::current().id() {
                let _ = handle.join();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn source(environment: &str) -> Source {
        Source {
            environment: environment.into(),
            profile: "default".into(),
            guest: uuid::Uuid::new_v4().to_string(),
        }
    }
    fn peer(
        source: Source,
        registry: Arc<Mutex<Registry>>,
        turns: Arc<Mutex<HashSet<String>>>,
    ) -> (Guest, BufReader<UnixStream>, mpsc::Receiver<Value>) {
        let (endpoint, peer) = UnixStream::pair().unwrap();
        peer.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
        let (sender, receiver) = mpsc::channel();
        let guest = Guest::start(
            source,
            endpoint,
            registry,
            Arc::new(move |event| {
                let _ = sender.send(event);
            }),
            turns,
        )
        .unwrap();
        (guest, BufReader::new(peer), receiver)
    }
    fn send(peer: &mut BufReader<UnixStream>, frame: Value) {
        writeln!(peer.get_mut(), "{frame}").unwrap();
    }
    fn frame(peer: &mut BufReader<UnixStream>) -> Value {
        let mut line = String::new();
        peer.read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap()
    }
    fn proof(peer: &mut BufReader<UnixStream>, instance: &str, target: &str) -> Value {
        send(
            peer,
            json!({"kind":"expected","threadId":"thread","tabId":"tab","instance":instance}),
        );
        send(
            peer,
            json!({"kind":"proof","threadId":"thread","tabId":"tab","instance":instance,"viewTarget":target,"cdpTarget":target}),
        );
        let value = frame(peer);
        assert_eq!(value["kind"], "admitted");
        assert_eq!(value["instance"], instance);
        value
    }
    #[test]
    fn late_inspector_frames_cannot_retire_successor_or_unrelated_guest() {
        let registry = Arc::new(Mutex::new(Registry::default()));
        let (a, mut a_peer, a_events) = peer(source("a"), registry.clone(), Default::default());
        let (b, mut b_peer, _b_events) = peer(source("b"), registry, Default::default());
        let old = a.approve("thread", "tab").unwrap();
        let old_lease = proof(&mut a_peer, &old, "target-old");
        let other = b.approve("thread", "tab").unwrap();
        let other_lease = proof(&mut b_peer, &other, "target-other");
        a.remove("thread", "tab");
        assert_eq!(
            a_events.recv_timeout(Duration::from_secs(1)).unwrap()["type"],
            "detached"
        );
        let current = a.approve("thread", "tab").unwrap();
        assert_ne!(old, current);
        for kind in ["expected", "proof", "retired", "event"] {
            send(
                &mut a_peer,
                json!({"kind":kind,"threadId":"thread","tabId":"tab","instance":old,"lease":old_lease["lease"],"viewTarget":"wrong","cdpTarget":"wrong","event":{"type":"attached","threadId":"thread","tabId":"tab"}}),
            );
        }
        let admitted = proof(&mut a_peer, &current, "target-current");
        let opaque="{\"id\":7,\"method\":\"Runtime.evaluate\",\"params\":{\"z\":1,\"a\":{\"threadId\":\"foreign\",\"b\":2}}}";
        send(
            &mut a_peer,
            json!({"kind":"event","threadId":"thread","tabId":"tab","instance":current,"lease":admitted["lease"],"event":{"type":"cdp","threadId":"thread","tabId":"tab","message":opaque}}),
        );
        assert_eq!(
            a_events.recv_timeout(Duration::from_secs(1)).unwrap()["message"],
            opaque
        );
        assert!(a.is_open());
        assert!(b.is_open());
        a.close();
        b.command(
            "thread",
            "tab",
            json!({"type":"cdp","threadId":"thread","tabId":"tab","message":opaque}),
        )
        .unwrap();
        let command = frame(&mut b_peer);
        assert_eq!(command["lease"], other_lease["lease"]);
        assert_eq!(command["command"]["message"], opaque);
        b.close();
    }
    #[test]
    fn admission_precedes_concurrent_command_and_turn_state_is_latched() {
        let turns = Arc::new(Mutex::new(HashSet::from(["thread".to_owned()])));
        let (guest, mut peer, _events) = peer(source("race"), Default::default(), turns);
        let guest = Arc::new(guest);
        let instance = guest.approve("thread", "tab").unwrap();
        let barrier = guest.shared.outgoing.lock().unwrap();
        send(
            &mut peer,
            json!({"kind":"proof","threadId":"thread","tabId":"tab","instance":instance,"viewTarget":"target","cdpTarget":"target"}),
        );
        let concurrent = guest.clone();
        let command = thread::spawn(move || {
            concurrent
                .command(
                    "thread",
                    "tab",
                    json!({"type":"turn","threadId":"thread","active":false}),
                )
                .unwrap()
        });
        drop(barrier);
        let admitted = frame(&mut peer);
        assert_eq!(admitted["kind"], "admitted");
        assert_eq!(admitted["agentTurn"], true);
        command.join().unwrap();
        // Whether the pre-admission command was absent or queued, a subsequent
        // current command always follows the admission acknowledgment.
        guest
            .command(
                "thread",
                "tab",
                json!({"type":"cdp","threadId":"thread","tabId":"tab","message":"opaque"}),
            )
            .unwrap();
        loop {
            let packet = frame(&mut peer);
            assert_eq!(packet["kind"], "command");
            if packet["command"]["type"] == "cdp" {
                assert_eq!(packet["command"]["message"], "opaque");
                break;
            }
        }
        guest.close();
    }
}
