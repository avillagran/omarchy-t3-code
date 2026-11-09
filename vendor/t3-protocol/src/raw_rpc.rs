//! Raw JSON RPC preserves JavaScript UTF-16 strings in both directions.
use crate::{Connection, Result};
use serde_json::{json, value::RawValue};
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};
use tungstenite::Message;

/// The wire exit stays raw even on rejection: a failure cause may itself contain
/// an isolated surrogate. A missing success value stays None, not JSON null.
/// Transport errors remain the existing stable strings.
#[derive(Debug)]
pub enum RawRpcExit {
    Success(Option<Box<RawValue>>),
    Failure(Box<RawValue>),
}
impl RawRpcExit {
    pub fn value(&self) -> Option<&RawValue> {
        match self {
            Self::Success(value) => value.as_deref(),
            Self::Failure(_) => None,
        }
    }
    pub fn cause(&self) -> Option<&RawValue> {
        match self {
            Self::Failure(cause) => Some(cause),
            Self::Success(_) => None,
        }
    }
}
fn fields(value: &str) -> Result<BTreeMap<String, Box<RawValue>>> {
    serde_json::from_str(value).map_err(|_| "rpc_outcome_unknown".into())
}
fn text<'a>(fields: &'a BTreeMap<String, Box<RawValue>>, key: &str) -> Option<&'a str> {
    serde_json::from_str(fields.get(key)?.get()).ok()
}
impl Connection {
    /// Exactly one physical request, with no retry or lossy Value conversion.
    pub fn rpc_raw(&self, tag: &str, payload: &RawValue) -> Result<RawRpcExit> {
        self.rpc_raw_with_timeout(tag, payload, Duration::from_secs(20))
    }
    pub fn rpc_raw_with_timeout(
        &self,
        tag: &str,
        payload: &RawValue,
        timeout: Duration,
    ) -> Result<RawRpcExit> {
        if timeout.is_zero() || timeout > Duration::from_secs(300) {
            return Err("invalid_rpc_timeout".into());
        }
        let mut ws = self.websocket()?;
        match ws.get_mut() {
            tungstenite::stream::MaybeTlsStream::Plain(s) => {
                s.set_read_timeout(Some(Duration::from_millis(100)))
            }
            tungstenite::stream::MaybeTlsStream::Rustls(s) => {
                s.sock.set_read_timeout(Some(Duration::from_millis(100)))
            }
            _ => return Err("unsupported_tls".into()),
        }
        .map_err(|_| "rpc_outcome_unknown")?;
        let tag = serde_json::to_string(tag).map_err(|_| "invalid_input")?;
        let request = format!(
            "{{\"_tag\":\"Request\",\"id\":\"1\",\"tag\":{tag},\"payload\":{},\"headers\":[]}}",
            payload.get()
        );
        ws.send(Message::Text(request.into()))
            .map_err(|_| "rpc_outcome_unknown")?;
        let start = Instant::now();
        let mut ping = Instant::now();
        while start.elapsed() < timeout {
            match ws.read() {
                Ok(Message::Text(value)) => {
                    let frame = fields(&value)?;
                    match text(&frame, "_tag") {
                        Some("Exit") if text(&frame, "requestId") == Some("1") => {
                            let mut exit =
                                fields(frame.get("exit").ok_or("rpc_outcome_unknown")?.get())?;
                            let status = text(&exit, "_tag").map(str::to_owned);
                            let _ = ws.close(None);
                            return match status.as_deref() {
                                Some("Success") => Ok(RawRpcExit::Success(exit.remove("value"))),
                                Some("Failure") => exit
                                    .remove("cause")
                                    .map(RawRpcExit::Failure)
                                    .ok_or_else(|| "rpc_outcome_unknown".into()),
                                _ => Err("rpc_outcome_unknown".into()),
                            };
                        }
                        Some("Ping") => ws
                            .send(Message::Text(json!({"_tag":"Pong"}).to_string().into()))
                            .map_err(|_| "rpc_outcome_unknown")?,
                        _ => {}
                    }
                }
                Ok(Message::Ping(payload)) => ws
                    .send(Message::Pong(payload))
                    .map_err(|_| "rpc_outcome_unknown")?,
                Err(tungstenite::Error::Io(error))
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) => {}
                Ok(Message::Close(_)) | Err(_) => return Err("rpc_outcome_unknown".into()),
                _ => {}
            }
            if ping.elapsed() >= Duration::from_secs(15) {
                ws.send(Message::Text(json!({"_tag":"Ping"}).to_string().into()))
                    .map_err(|_| "rpc_outcome_unknown")?;
                ping = Instant::now();
            }
        }
        let _ = ws.send(Message::Text(
            json!({"_tag":"Interrupt","requestId":"1"})
                .to_string()
                .into(),
        ));
        let _ = ws.close(None);
        Err("rpc_outcome_unknown".into())
    }
}
