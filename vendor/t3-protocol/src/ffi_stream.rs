//! Bounded foreign-host event inbox. No host callbacks or unbounded worker queues.
use crate::{Connection, Result, MAX_BYTES};
use serde_json::{json, Value};
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Condvar, Mutex,
    },
    time::Duration,
};
const MAX_ITEMS: usize = 64;
#[derive(Default)]
struct Inbox {
    items: VecDeque<(Value, usize)>,
    bytes: usize,
    closed: bool,
    error: Option<String>,
}
#[derive(Default)]
struct Shared {
    inbox: Mutex<Inbox>,
    ready: Condvar,
}
impl Shared {
    fn push(&self, item: Value) -> Result<()> {
        let size = serde_json::to_vec(&item)
            .map_err(|_| "malformed_stream")?
            .len();
        let mut inbox = self.inbox.lock().map_err(|_| "internal_error")?;
        if inbox.closed {
            return Err("stream_cancelled".into());
        }
        if inbox.items.len() >= MAX_ITEMS || size > MAX_BYTES as usize - inbox.bytes {
            return Err("stream_backpressure".into());
        }
        inbox.bytes += size;
        inbox.items.push_back((item, size));
        self.ready.notify_all();
        Ok(())
    }
    fn finish(&self, error: Option<String>) {
        if let Ok(mut inbox) = self.inbox.lock() {
            inbox.closed = true;
            inbox.error = error;
            self.ready.notify_all();
        }
    }
}
pub(crate) struct Subscription {
    shared: Arc<Shared>,
    cancel: Arc<AtomicBool>,
}
impl Subscription {
    pub(crate) fn start(connection: Connection, tag: String, payload: Value) -> Result<Self> {
        let shared = Arc::new(Shared::default());
        let cancel = Arc::new(AtomicBool::new(false));
        let (queue, stop) = (shared.clone(), cancel.clone());
        std::thread::Builder::new()
            .name("t3-events".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    connection.subscribe(
                        &tag,
                        payload,
                        || stop.load(Ordering::Acquire),
                        |item| {
                            if let Some(value) = item {
                                queue.push(value)?;
                            }
                            Ok(())
                        },
                    )
                }))
                .unwrap_or_else(|_| Err("internal_error".into()));
                queue.finish(if stop.load(Ordering::Acquire) {
                    None
                } else {
                    result.err()
                });
            })
            .map_err(|_| "local_error")?;
        Ok(Self { shared, cancel })
    }
    pub(crate) fn poll(&self, timeout_ms: u64) -> Result<Value> {
        let inbox = self.shared.inbox.lock().map_err(|_| "internal_error")?;
        let (mut inbox, _) = self
            .shared
            .ready
            .wait_timeout_while(inbox, Duration::from_millis(timeout_ms), |v| {
                v.items.is_empty() && !v.closed
            })
            .map_err(|_| "internal_error")?;
        let items: Vec<_> = inbox.items.drain(..).map(|(v, _)| v).collect();
        inbox.bytes = 0;
        Ok(json!({"items":items,"closed":inbox.closed,"error":inbox.error}))
    }
    pub(crate) fn cancel(&self) {
        self.cancel.store(true, Ordering::Release);
        if let Ok(mut inbox) = self.shared.inbox.lock() {
            inbox.items.clear();
            inbox.bytes = 0;
            inbox.closed = true;
            inbox.error = None;
            self.shared.ready.notify_all();
        }
    }
}
impl Drop for Subscription {
    fn drop(&mut self) {
        self.cancel();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounds_never_silently_drop_events() {
        let shared = Shared::default();
        for n in 0..MAX_ITEMS {
            shared.push(json!({"sequence":n})).unwrap();
        }
        assert_eq!(
            shared.push(json!({"sequence":64})).unwrap_err(),
            "stream_backpressure"
        );
        assert_eq!(shared.inbox.lock().unwrap().items.len(), MAX_ITEMS);
        let shared = Shared::default();
        assert_eq!(
            shared
                .push(json!("a".repeat(MAX_BYTES as usize)))
                .unwrap_err(),
            "stream_backpressure"
        );
        assert!(shared.inbox.lock().unwrap().items.is_empty());
    }
    #[test]
    fn cancel_wakes_a_waiter_without_a_network_tick() {
        let s = Arc::new(Subscription {
            shared: Arc::default(),
            cancel: Arc::default(),
        });
        let waiting = s.clone();
        let worker = std::thread::spawn(move || waiting.poll(1000).unwrap());
        s.cancel();
        let value = worker.join().unwrap();
        assert_eq!(value["closed"], true);
        assert!(value["items"].as_array().unwrap().is_empty());
    }
}
