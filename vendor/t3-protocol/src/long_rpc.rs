//! Long-running native lifecycle RPCs. Reuses this crate's existing authenticated
//! websocket factory, including DPoP/relay admission. Never retries a request.
use super::*;

impl Connection {
    /// Same Effect non-stream request/exit protocol as `rpc`, with an explicit
    /// bounded deadline and idle heartbeats. `rpc` keeps its original behavior.
    pub fn rpc_with_timeout(&self, tag: &str, payload: Value, timeout: Duration) -> Result<Value> {
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
        ws.send(Message::Text(
            json!({"_tag":"Request","id":"1","tag":tag,"payload":payload,"headers":[]})
                .to_string()
                .into(),
        ))
        .map_err(|_| "rpc_outcome_unknown")?;
        let start = Instant::now();
        let mut ping = Instant::now();
        while start.elapsed() < timeout {
            match ws.read() {
                Ok(Message::Text(text)) => {
                    let value: Value =
                        serde_json::from_str(&text).map_err(|_| "rpc_outcome_unknown")?;
                    if value["_tag"] == "Exit" && value["requestId"] == "1" {
                        let _ = ws.close(None);
                        return match value["exit"]["_tag"].as_str() {
                            Some("Success") => Ok(value["exit"]["value"].clone()),
                            Some("Failure") => Err(rejection(&value["exit"]["cause"])),
                            _ => Err("rpc_outcome_unknown".into()),
                        };
                    }
                    if value["_tag"] == "Ping" {
                        ws.send(Message::Text(json!({"_tag":"Pong"}).to_string().into()))
                            .map_err(|_| "rpc_outcome_unknown")?;
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
