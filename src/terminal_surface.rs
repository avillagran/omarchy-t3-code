//! Embedded VT views attach to server-owned PTYs. Detaching never kills a shell.
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

#[derive(Clone)]
struct Target {
    request: String,
    environment: String,
    thread: String,
    terminal: String,
}
struct Session {
    cancel: Arc<AtomicBool>,
    input: mpsc::SyncSender<Value>,
}
impl Drop for Session {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
#[derive(Default)]
pub struct Manager {
    sessions: HashMap<String, Session>,
}
fn emit(target: &Target, mut value: Value) {
    value["requestId"] = json!(target.request);
    crate::emit("terminal-surface", value);
}
fn target(v: &Value) -> Result<Target> {
    let environment = text(v, "environment");
    let request = text(v, "requestId");
    if environment.is_empty()
        || environment.len() > 128
        || environment.chars().any(char::is_control)
        || request.is_empty()
        || request.len() > 128
        || request.chars().any(char::is_control)
    {
        return Err("invalid_terminal".into());
    }
    protocol::terminal::session(text(v, "thread"), text(v, "terminalId"))?;
    Ok(Target {
        request: request.into(),
        environment: environment.into(),
        thread: text(v, "thread").into(),
        terminal: text(v, "terminalId").into(),
    })
}
fn dimensions(v: &Value) -> Result<(u16, u16)> {
    let cols = v["columns"]
        .as_u64()
        .filter(|n| (1..=1000).contains(n))
        .ok_or("invalid_terminal_size")?;
    let rows = v["rows"]
        .as_u64()
        .filter(|n| (1..=500).contains(n))
        .ok_or("invalid_terminal_size")?;
    Ok((cols as u16, rows as u16))
}
impl Manager {
    pub fn stop(&mut self) {
        self.sessions.clear();
    }
    pub fn request(&mut self, v: &Value, visible: bool) {
        let request = text(v, "requestId");
        if text(v, "op") == "terminal-surface-detach" {
            self.sessions.remove(request);
            return;
        }
        if text(v, "op") == "terminal-surface-attach" {
            let result = (|| -> Result<()> {
                if !visible {
                    return Err("panel_hidden".into());
                }
                let target = target(v)?;
                let size = dimensions(v)?;
                let command = v["command"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned);
                if command
                    .as_deref()
                    .is_some_and(|s| !protocol::code_blocks::valid_command(s))
                {
                    return Err("invalid_terminal_input".into());
                }
                if self.sessions.len() >= 16 && !self.sessions.contains_key(request) {
                    return Err("terminal_limit".into());
                }
                self.sessions.remove(request);
                let cancel = Arc::new(AtomicBool::new(false));
                let (tx, rx) = mpsc::sync_channel(128);
                let session = Session {
                    cancel: cancel.clone(),
                    input: tx,
                };
                let restart = v["restart"] == true;
                std::thread::spawn(move || {
                    if let Err(error) = run(&target, size, restart, command, cancel.clone(), rx) {
                        if !cancel.load(Ordering::Relaxed) {
                            emit(&target, json!({"kind":"error","error":error}));
                        }
                    }
                });
                self.sessions.insert(request.into(), session);
                Ok(())
            })();
            if let Err(error) = result {
                crate::emit(
                    "terminal-surface",
                    json!({"requestId":request,"kind":"error","error":error}),
                );
            }
            return;
        }
        let Some(session) = self.sessions.get(request) else {
            return;
        };
        if let Err(error) = session.input.try_send(v.clone()) {
            crate::emit(
                "terminal-surface",
                json!({"requestId":request,"kind":"operation-error","error":if matches!(error,mpsc::TrySendError::Full(_)){"terminal_input_pending"}else{"terminal_disconnected"}}),
            );
        }
    }
}
fn run(
    target: &Target,
    size: (u16, u16),
    restart: bool,
    command: Option<String>,
    cancel: Arc<AtomicBool>,
    rx: mpsc::Receiver<Value>,
) -> Result<()> {
    let env = crate::env_by_id(&target.environment)?;
    let conn = env.connection(true)?;
    let shell = Client::shared()?.get(&env, "/api/orchestration/shell", true)?;
    let context = protocol::terminal::context(&shell, &target.thread, "").or_else(|error| {
        if error != "terminal_directory_unavailable" {
            return Err(error);
        }
        let config = protocol::api::server::config(&conn)?;
        protocol::terminal::context(
            &shell,
            &target.thread,
            text(&config, "scratchWorkspaceRoot"),
        )
    })?;
    let mut payload =
        protocol::terminal::attach(&target.thread, &target.terminal, &context, size.0, size.1)?;
    payload["restartIfNotRunning"] = json!(restart);
    payload["resume"] = json!(!restart);
    let ready = Arc::new(AtomicBool::new(false));
    let input_done = Arc::new(AtomicBool::new(false));
    let writer_target = target.clone();
    let writer_conn = conn.clone();
    let writer_cancel = cancel.clone();
    let writer_ready = ready.clone();
    let writer_done = input_done.clone();
    let writer = std::thread::spawn(move || {
        let mut initial = command;
        while !writer_cancel.load(Ordering::Relaxed) && !writer_done.load(Ordering::Relaxed) {
            if !writer_ready.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(15));
                continue;
            }
            // Execute a code-block request once after the initial live snapshot.
            if let Some(command) = initial.take() {
                if let Err(error) = writer_conn.terminal_write(
                    &writer_target.thread,
                    &writer_target.terminal,
                    &format!("{command}\r"),
                ) {
                    emit(
                        &writer_target,
                        json!({"kind":"operation-error","error":error}),
                    );
                }
            }
            let v = match rx.recv_timeout(Duration::from_millis(40)) {
                Ok(v) => v,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(_) => break,
            };
            if writer_cancel.load(Ordering::Relaxed) {
                break;
            }
            let result = match text(&v, "op") {
                "terminal-surface-write" => writer_conn.terminal_write(
                    &writer_target.thread,
                    &writer_target.terminal,
                    text(&v, "data"),
                ),
                "terminal-surface-resize" => dimensions(&v).and_then(|(cols, rows)| {
                    writer_conn.terminal_resize(
                        &writer_target.thread,
                        &writer_target.terminal,
                        cols,
                        rows,
                    )
                }),
                "terminal-surface-clear" => protocol::api::terminal::clear(
                    &writer_conn,
                    &writer_target.thread,
                    &writer_target.terminal,
                ),
                "terminal-surface-close" => protocol::api::terminal::close(
                    &writer_conn,
                    &writer_target.thread,
                    Some(&writer_target.terminal),
                    Some(true),
                ),
                _ => continue,
            };
            if let Err(error) = result {
                emit(
                    &writer_target,
                    json!({"kind":"operation-error","error":error}),
                );
            }
        }
    });
    let result = (|| -> Result<()> {
        let mut output = protocol::terminal::Output::default();
        let mut first = true;
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Ok(());
            }
            let spec = protocol::api::streams::spec(
                protocol::api::streams::TERMINAL_ATTACH.name,
                &payload,
            )?;
            let subscribed = spec.subscribe(
                &conn,
                || cancel.load(Ordering::Relaxed),
                || false,
                |event| {
                    if cancel.load(Ordering::Relaxed) {
                        return Ok(());
                    }
                    if let Some(event) = event {
                        let reset = matches!(
                            text(&event, "type"),
                            "snapshot" | "started" | "restarted" | "cleared"
                        );
                        let data = output.apply(&event)?;
                        if reset {
                            ready.store(true, Ordering::Relaxed);
                        }
                        emit(
                            target,
                            json!({"kind":"event","event":event,"reset":reset,"data":data}),
                        );
                    }
                    Ok(())
                },
            );
            ready.store(false, Ordering::Relaxed);
            if cancel.load(Ordering::Relaxed) {
                return Ok(());
            }
            // Reconnection only resumes the same session; it never retypes a command.
            if output.finished {
                return subscribed;
            }
            emit(
                target,
                json!({"kind":"reconnecting","error":subscribed.err().unwrap_or_else(||"terminal_disconnected".into())}),
            );
            payload["restartIfNotRunning"] = json!(false);
            payload["resume"] = json!(true);
            let delay = if first { 10 } else { 40 };
            first = false;
            for _ in 0..delay {
                if cancel.load(Ordering::Relaxed) {
                    return Ok(());
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    })();
    input_done.store(true, Ordering::Relaxed);
    let _ = writer.join();
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn view_targets_require_valid_scope_and_native_grid_bounds() {
        let v = json!({"requestId":"view","environment":"remote","thread":"thread","terminalId":"term-1","columns":1000,"rows":500});
        let t = target(&v).unwrap();
        assert_eq!(t.environment, "remote");
        assert_eq!(t.terminal, "term-1");
        assert_eq!(dimensions(&v).unwrap(), (1000, 500));
        for (key, value) in [
            ("columns", json!(1001)),
            ("rows", json!(0)),
            ("rows", json!(1.2)),
        ] {
            let mut bad = v.clone();
            bad[key] = value;
            assert!(dimensions(&bad).is_err());
        }
        let mut bad = v;
        bad["terminalId"] = json!("bad\n");
        assert!(target(&bad).is_err());
    }
    #[test]
    fn detach_cancels_view_without_queuing_terminal_close() {
        let mut manager = Manager::default();
        let cancel = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::sync_channel(1);
        manager.sessions.insert(
            "view".into(),
            Session {
                cancel: cancel.clone(),
                input: tx,
            },
        );
        manager.request(
            &json!({"op":"terminal-surface-detach","requestId":"view"}),
            true,
        );
        assert!(cancel.load(Ordering::Relaxed));
        assert!(rx.try_recv().is_err());
        assert!(manager.sessions.is_empty());
    }
}
