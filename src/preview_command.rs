//! Native lifecycle scheduling mirrors T3's per-environment/thread serial key.
//! Unrelated threads run independently. Accepted commands outlive navigation.
use omarchy_t3_client::*;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{mpsc, Arc, Mutex},
    time::Duration,
};
struct Queue {
    sender: mpsc::SyncSender<Value>,
    alive: Arc<Mutex<bool>>,
}
pub struct Control {
    observer: Arc<dyn Fn(Value, Result<Value>) + Send + Sync>,
    queues: HashMap<(String, String), Queue>,
}
fn receipt(v: &Value, result: Result<Value>) {
    crate::emit(
        "action",
        json!({"op":"preview-action","environment":v["environment"],"thread":v["thread"],"requestId":v["requestId"],"result":result.as_ref().ok(),"error":result.as_ref().err()}),
    );
}
impl Default for Control {
    fn default() -> Self {
        Self::with_observer(|_, _| {})
    }
}
impl Control {
    pub fn with_observer(observer: impl Fn(Value, Result<Value>) + Send + Sync + 'static) -> Self {
        Self {
            queues: HashMap::new(),
            observer: Arc::new(observer),
        }
    }
    pub fn send(&mut self, v: Value, visible: bool) {
        if !visible {
            receipt(&v, Err("panel_hidden".into()));
            return;
        }
        let (e, t) = (text(&v, "environment"), text(&v, "thread"));
        if !valid_id(e) || !valid_id(t) {
            receipt(&v, Err("invalid_preview_command".into()));
            return;
        }
        self.queues.retain(|_, q| *q.alive.lock().unwrap());
        let key = (e.to_owned(), t.to_owned());
        if let Some(q) = self.queues.get(&key) {
            let alive = q.alive.lock().unwrap();
            if *alive {
                match q.sender.try_send(v.clone()) {
                    Ok(()) => return,
                    Err(mpsc::TrySendError::Full(_)) => {
                        receipt(&v, Err("preview_busy".into()));
                        return;
                    }
                    Err(mpsc::TrySendError::Disconnected(_)) => {}
                }
            }
            drop(alive);
            self.queues.remove(&key);
        }
        if self.queues.len() >= 32 {
            receipt(&v, Err("preview_busy".into()));
            return;
        }
        let (sender, receiver) = mpsc::sync_channel::<Value>(32);
        let alive = Arc::new(Mutex::new(true));
        let active = alive.clone();
        sender.try_send(v).expect("new preview queue has capacity");
        self.queues.insert(key, Queue { sender, alive });
        let observer = self.observer.clone();
        std::thread::spawn(move || {
            struct Lifetime(Arc<Mutex<bool>>);
            impl Drop for Lifetime {
                fn drop(&mut self) {
                    *self.0.lock().unwrap() = false;
                }
            }
            let _lifetime = Lifetime(active.clone());
            loop {
                let v = match receiver.recv_timeout(Duration::from_secs(60)) {
                    Ok(v) => v,
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        // Retirement and admission share a gate. A command
                        // accepted at the idle deadline must still be drained.
                        let mut gate = active.lock().unwrap();
                        match receiver.try_recv() {
                            Ok(v) => v,
                            Err(_) => {
                                *gate = false;
                                break;
                            }
                        }
                    }
                };
                let result = (|| -> Result<Value> {
                    let e = environments()?
                        .into_iter()
                        .find(|e| e.id == text(&v, "environment"))
                        .ok_or("unknown_environment")?;
                    preview_actions::execute(&e, &v)
                })();
                observer(v.clone(), result.clone());
                receipt(&v, result);
            }
        });
    }
}
