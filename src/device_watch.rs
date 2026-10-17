//! Visible Integrations subscriptions. Native current state and progress only;
//! never invoke list/inspect/setup as a side effect of observing devices.
use omarchy_t3_client::*;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
#[derive(Default)]
pub struct Control {
    watches: HashMap<String, Arc<AtomicBool>>,
}
impl Control {
    pub fn clear(&mut self) {
        for (_, cancelled) in self.watches.drain() {
            cancelled.store(true, Ordering::Relaxed);
        }
    }
    pub fn select(&mut self, input: &Value, visible: bool) {
        self.clear();
        if !visible {
            return;
        }
        let Some(ids) = input["environments"].as_array().filter(|v| v.len() <= 64) else {
            return;
        };
        let nonce = text(input, "requestId");
        if nonce.is_empty() || nonce.len() > 128 {
            return;
        }
        for id in ids.iter().filter_map(Value::as_str) {
            if id.is_empty() || id.len() > 256 || self.watches.contains_key(id) {
                continue;
            }
            let id = id.to_owned();
            let nonce = nonce.to_owned();
            let cancelled = Arc::new(AtomicBool::new(false));
            self.watches.insert(id.clone(), cancelled.clone());
            std::thread::spawn(move || {
                let mut delay = 1;
                while !cancelled.load(Ordering::Relaxed) {
                    let result = (|| -> Result<()> {
                        let environment = environments()?
                            .into_iter()
                            .find(|e| e.id == id)
                            .ok_or("unknown_environment")?;
                        integration_devices::watch(
                            &environment,
                            || cancelled.load(Ordering::Relaxed),
                            |origin, state| {
                                if !cancelled.load(Ordering::Relaxed) {
                                    crate::emit(
                                        "devices-state",
                                        json!({"environment":id,"requestId":nonce,"originKey":origin,"state":state}),
                                    );
                                }
                                Ok(())
                            },
                        )
                    })();
                    if cancelled.load(Ordering::Relaxed) {
                        break;
                    }
                    // Never expose raw transport errors. Force a fresh authorized
                    // view before a UI enables controls after a disconnect.
                    let _ = result;
                    crate::emit(
                        "devices-state",
                        json!({"environment":id,"requestId":nonce,"disconnected":true}),
                    );
                    for _ in 0..delay * 10 {
                        if cancelled.load(Ordering::Relaxed) {
                            return;
                        }
                        std::thread::sleep(Duration::from_millis(100));
                    }
                    delay = (delay * 2).min(30);
                }
            });
        }
    }
}
impl Drop for Control {
    fn drop(&mut self) {
        self.clear();
    }
}
