//! Dedicated voice owner. Ordinary application watch restarts cannot interrupt
//! capture, and stdin closure drains native workers before this process exits.
use crate::{voice_host, Environment, Result};
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};

fn emit(value: Value) {
    let mut stdout = io::stdout().lock();
    let _ = writeln!(stdout, "{}", json!({"type":"native-voice","data":value}));
    let _ = stdout.flush();
}

pub fn run(resolve: impl Fn(&str) -> Result<Environment> + Send + Sync + 'static) -> Result<()> {
    serve(voice_host::Resources::configured().ok().flatten(), resolve)
}

/// The production parser and emitter, with explicitly admitted resources.
/// A test may supply typed synthetic resources; no environment variable enables
/// synthetic capture in the ordinary application entry point.
pub fn serve(
    resources: Option<voice_host::Resources>,
    resolve: impl Fn(&str) -> Result<Environment> + Send + Sync + 'static,
) -> Result<()> {
    let host = resources.map(|resources| voice_host::Host::new(resources, resolve, emit));
    if let Some(host) = &host {
        host.ready();
    } else {
        emit(json!({"kind":"ready","available":false}));
    }
    let mut input = io::stdin().lock();
    let mut line = Vec::new();
    let result = loop {
        line.clear();
        let mut limited = std::io::Read::take(&mut input, 32 * 1024 * 1024 + 1);
        match limited.read_until(b'\n', &mut line) {
            Ok(0) => break Ok(()),
            Ok(_) if line.len() > 32 * 1024 * 1024 || line.last() != Some(&b'\n') => {
                break Err("voice_control_invalid".into());
            }
            Ok(_) => {}
            Err(_) => break Err("voice_control_unavailable".into()),
        }
        let Ok(command) = serde_json::from_slice::<Value>(&line) else {
            break Err("voice_control_invalid".into());
        };
        if let Some(host) = &host {
            host.request(&command);
        } else if let Some(id) = command["requestId"].as_str() {
            emit(json!({"kind":"result","requestId":id,"error":"voice_host_unavailable"}));
        }
    };
    if let Some(host) = &host {
        host.shutdown()?;
    }
    result
}
