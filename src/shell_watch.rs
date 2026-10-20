//! One shell subscription per environment. Updates only nudge the main loop to rebuild the
//! sidebar from the HTTP snapshot, so the list follows the server live instead of every 10-60 s.
use omarchy_t3_client::*;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    time::Duration,
};

pub fn start(tx: mpsc::Sender<Value>) {
    std::thread::spawn(move || {
        let mut running: HashMap<String, Arc<AtomicBool>> = HashMap::new();
        loop {
            let current = environments().unwrap_or_default();
            running.retain(|id, stop| {
                let keep = current.iter().any(|e| &e.id == id);
                if !keep {
                    stop.store(true, Ordering::Relaxed);
                }
                keep
            });
            for e in current {
                if running.contains_key(&e.id) {
                    continue;
                }
                let stop = Arc::new(AtomicBool::new(false));
                running.insert(e.id.clone(), stop.clone());
                let tx = tx.clone();
                std::thread::spawn(move || watch(e.id, stop, tx));
            }
            std::thread::sleep(Duration::from_secs(30));
        }
    });
}

fn watch(env: String, stop: Arc<AtomicBool>, tx: mpsc::Sender<Value>) {
    let mut retry = 2;
    while !stop.load(Ordering::Relaxed) {
        let result = (|| -> Result<()> {
            let e = environments()?
                .into_iter()
                .find(|e| e.id == env)
                .ok_or("unknown_environment")?;
            let c = match e.connection {
                Some(c) => c.as_ref().clone(),
                None => protocol::Connection::new(
                    &e.base_url,
                    None,
                    protocol::Credential::bearer(&environment_token(&e, true)?)?,
                )?,
            };
            let mut synced = false;
            c.subscribe(
                "orchestration.subscribeShell",
                json!({}),
                || stop.load(Ordering::Relaxed),
                |update| {
                    let Some(item) = update else { return Ok(()) };
                    retry = 2;
                    // The first snapshot repeats what the poll already shows.
                    if text(&item, "kind") == "snapshot" && !synced {
                        synced = true;
                        return Ok(());
                    }
                    if text(&item, "kind") != "synchronized" {
                        let _ = tx.send(json!({"op":"shell-changed","environment":env}));
                    }
                    Ok(())
                },
            )
        })();
        if stop.load(Ordering::Relaxed) {
            return;
        }
        let _ = result;
        std::thread::sleep(Duration::from_secs(retry));
        retry = (retry * 2).min(60);
    }
}
