//! One shell subscription per environment. Stream items are applied to the
//! environment's shell in `shell_state`, so a change only rebuilds the sidebar from
//! memory; nothing is fetched again over HTTP. A reconnect resumes after the last
//! applied sequence (T3 replays up to 1000 events, otherwise sends a fresh snapshot).
use omarchy_t3_client::*;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
};

/// Follows the environment list the main loop already reads for every snapshot,
/// instead of polling the connection catalog on its own.
pub fn start(tx: mpsc::Sender<Value>) -> mpsc::Sender<Vec<String>> {
    let (environments_tx, environments_rx) = mpsc::channel::<Vec<String>>();
    std::thread::spawn(move || {
        let mut running: HashMap<String, Arc<AtomicBool>> = HashMap::new();
        for current in environments_rx {
            running.retain(|id, stop| {
                let keep = current.contains(id);
                if !keep {
                    stop.store(true, Ordering::Relaxed);
                    shell_state::forget(id);
                }
                keep
            });
            for id in current {
                if running.contains_key(&id) {
                    continue;
                }
                let stop = Arc::new(AtomicBool::new(false));
                running.insert(id.clone(), stop.clone());
                let tx = tx.clone();
                std::thread::spawn(move || watch(id, stop, tx));
            }
        }
        for stop in running.values() {
            stop.store(true, Ordering::Relaxed);
        }
    });
    environments_tx
}

fn watch(env: String, stop: Arc<AtomicBool>, tx: mpsc::Sender<Value>) {
    let mut backoff = reconnect::Backoff::default();
    let mut waiter = reconnect::Waiter::default();
    let mut last_error = String::new();
    let stopped = || stop.load(Ordering::Relaxed);
    let mut woke = false;
    while !stopped() {
        // After a resume or network change, check the server answers before reconnecting.
        if woke && !reachable(&env) {
            woke = waiter.sleep(backoff.next_delay(), stopped);
            if woke {
                backoff.reset();
            }
            continue;
        }
        let mut probed = false;
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
            let request_marker = server_capabilities::get(&env, &c)?.shell_completion_marker;
            let mut payload = json!({});
            if let Some(sequence) = shell_state::cursor(&env) {
                payload["afterSequence"] = json!(sequence);
            }
            if request_marker {
                payload["requestCompletionMarker"] = json!(true);
            }
            let mut changed = false;
            let mut caught_up = false;
            c.subscribe_probed(
                "orchestration.subscribeShell",
                payload,
                stopped,
                // A wakeup checks the open socket instead of trusting it.
                || {
                    let woke = waiter.take();
                    probed |= woke;
                    woke
                },
                |update| {
                    match update {
                        Some(item) if item["kind"] == "synchronized" => caught_up = true,
                        Some(item) => changed |= shell_state::apply(&env, &item),
                        // Without a completion marker the server gives no catch-up signal;
                        // like T3, treat the stream as live once a shell is held.
                        None if !request_marker && shell_state::has_shell(&env) => caught_up = true,
                        None => {}
                    }
                    if caught_up && !shell_state::is_live(&env) {
                        shell_state::set_live(&env, true);
                        backoff.connected();
                        changed = true;
                    }
                    // Flush once per chunk/idle tick so a replay burst is one rebuild.
                    if changed && caught_up {
                        changed = false;
                        let _ = tx.send(json!({"op":"shell-changed","environment":env}));
                    }
                    Ok(())
                },
            )
        })();
        let was_live = shell_state::is_live(&env);
        shell_state::set_live(&env, false);
        if stopped() {
            return;
        }
        if was_live {
            last_error.clear();
            // Let the sidebar fall back to HTTP (and show staleness) while disconnected.
            let _ = tx.send(json!({"op":"shell-changed","environment":env}));
        }
        let error = result.err().unwrap_or_else(|| "stream_closed".into());
        // The server ending the stream may mean it rejected the cursor: start over.
        if !reconnect::transient(&error) {
            shell_state::reset(&env);
            server_capabilities::invalidate(&env);
        }
        if error != last_error {
            eprintln!("ot3: shell stream for environment {env} ended: {error}");
            last_error = error;
        }
        // A socket that failed its wakeup probe reconnects at once.
        woke = probed || waiter.sleep(backoff.next_delay(), stopped);
        if woke {
            backoff.reset();
        }
    }
}

fn reachable(env: &str) -> bool {
    environments().ok().and_then(|list| list.into_iter().find(|e| e.id == env)).is_some_and(|e| reconnect::reachable(&e))
}
