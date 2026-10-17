//! Native config notifications for visible primary/active/editor environments.
//! No config payload or provider credentials are emitted to QML.
use omarchy_t3_client::*;
use serde_json::{json, Value};
use std::{collections::HashMap, sync::{Arc, atomic::{AtomicBool, Ordering}}, time::Duration};

#[derive(Default)]
pub struct Control { watches: HashMap<String, Arc<AtomicBool>> }
impl Control {
    pub fn clear(&mut self) {
        for (_, cancelled) in self.watches.drain() { cancelled.store(true, Ordering::Relaxed); }
    }
    pub fn select(&mut self, input: &Value, visible: bool) {
        self.clear();
        if !visible { return; }
        let Some(ids) = input["environments"].as_array().filter(|ids| ids.len() <= 64) else { return; };
        let nonce = text(input,"requestId").to_owned();
        if nonce.is_empty() || nonce.len() > 128 { return; }
        for id in ids.iter().filter_map(Value::as_str) {
            if id.is_empty() || id.len()>256 || self.watches.contains_key(id) { continue; }
            let id=id.to_owned();
            let cancelled=Arc::new(AtomicBool::new(false));
            self.watches.insert(id.clone(),cancelled.clone());
            let nonce=nonce.clone();
            std::thread::spawn(move || {
                let mut retry=1;
                while !cancelled.load(Ordering::Relaxed) {
                    let result=(|| -> Result<()> {
                        let e=environments()?.into_iter().find(|e| e.id==id).ok_or("unknown_environment")?;
                        keybinding_settings::subscribe_changes(&e, || cancelled.load(Ordering::Relaxed), || {
                            if !cancelled.load(Ordering::Relaxed) {
                                crate::emit("keybindings-changed",json!({"environment":id,"requestId":nonce}));
                            }
                            Ok(())
                        })
                    })();
                    if cancelled.load(Ordering::Relaxed) { break; }
                    // Errors are invalidations too: fresh read updates permissions
                    // and stale routes cannot persist through an auth revocation.
                    crate::emit("keybindings-changed",json!({"environment":id,"requestId":nonce,"disconnected":true}));
                    let _=result; // Native transport errors may contain private details.
                    for _ in 0..retry*10 {
                        if cancelled.load(Ordering::Relaxed) { return; }
                        std::thread::sleep(Duration::from_millis(100));
                    }
                    retry=(retry*2).min(30);
                }
            });
        }
    }
}
impl Drop for Control { fn drop(&mut self) { self.clear(); } }
