//! One cancellable metadata subscription for the visible native browser view.
use omarchy_t3_client::*;
use serde_json::{json, Value};
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc, Arc,
    },
    time::Duration,
};
pub struct Control {
    generation: Arc<AtomicU64>,
    sender: mpsc::Sender<(u64, Option<(String, String, String)>)>,
}
impl Default for Control {
    fn default() -> Self {
        Self::with_observer(|_| {})
    }
}
impl Control {
    pub fn with_observer(observer: impl Fn(Value) + Send + 'static) -> Self {
        let generation = Arc::new(AtomicU64::new(0));
        let (sender, receiver) = mpsc::channel::<(u64, Option<(String, String, String)>)>();
        let current = generation.clone();
        std::thread::spawn(move || {
            let mut pending = None;
            loop {
                let Some((id, target)) = pending.take().or_else(|| receiver.recv().ok()) else {
                    return;
                };
                let Some((environment, thread, request)) = target else {
                    continue;
                };
                let mut delay = 1;
                while current.load(Ordering::Relaxed) == id {
                    let result = (|| -> Result<()> {
                        let e = environments()?
                            .into_iter()
                            .find(|e| e.id == environment)
                            .ok_or("unknown_environment")?;
                        preview_transport::observe(
                            &e,
                            &thread,
                            || current.load(Ordering::Relaxed) != id,
                            |mut data| {
                                if current.load(Ordering::Relaxed) == id {
                                    data["requestId"] = json!(request);
                                    observer(data.clone());
                                    crate::emit("preview-state", data);
                                }
                                Ok(())
                            },
                        )
                    })();
                    if current.load(Ordering::Relaxed) != id {
                        break;
                    }
                    // Never send transport diagnostics/credentials to QML.
                    let data = json!({"environment":environment,"thread":thread,"requestId":request,"kind":"disconnected","error":if result.as_ref().err().is_some_and(|e|e=="permission_denied"){"permission_denied"}else{"preview_unavailable"}});
                    observer(data.clone());
                    crate::emit("preview-state", data);
                    match receiver.recv_timeout(Duration::from_secs(delay)) {
                        Ok(next) => {
                            pending = Some(next);
                            break;
                        }
                        Err(mpsc::RecvTimeoutError::Disconnected) => return,
                        Err(_) => delay = (delay * 2).min(30),
                    }
                }
            }
        });
        Self { generation, sender }
    }
}
impl Control {
    pub fn clear(&self) {
        self.select(&json!({}), false);
    }
    pub fn select(&self, input: &Value, visible: bool) {
        let id = self.generation.fetch_add(1, Ordering::Relaxed) + 1;
        let (e, t, r) = (
            text(input, "environment"),
            text(input, "thread"),
            text(input, "requestId"),
        );
        let target = if visible && valid_id(e) && valid_id(t) && !r.is_empty() && r.len() <= 128 {
            Some((e.to_owned(), t.to_owned(), r.to_owned()))
        } else {
            None
        };
        let _ = self.sender.send((id, target));
    }
}
impl Drop for Control {
    fn drop(&mut self) {
        self.generation.fetch_add(1, Ordering::Relaxed);
    }
}
