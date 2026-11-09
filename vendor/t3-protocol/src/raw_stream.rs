//! Additive raw Effect streams: native JSON strings never enter scalar Value.
//! Hosts own retry, cursor reduction and any opted-in compact-wire decoding.
use crate::stream::{STREAM_IDLE_TIMEOUT, STREAM_PING_INTERVAL, STREAM_PROBE_TIMEOUT};
use crate::{Connection, RawJson, Result};
use serde_json::json;
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};
use tungstenite::Message;

#[derive(Debug)]
pub enum RawStreamTick {
    Item(Box<RawJson>),
    ChunkEnd,
    Idle,
}
/// A native exit and a local cancellation are distinct. An omitted native void
/// value remains None, while JSON null remains Some(raw null).
#[derive(Debug)]
pub enum RawStreamExit {
    Cancelled,
    Success(Option<Box<RawJson>>),
    Failure(Box<RawJson>),
}
fn fields(text: &str) -> Result<BTreeMap<String, Box<RawJson>>> {
    serde_json::from_str(text).map_err(|_| "malformed_stream".into())
}
fn control(frame: &BTreeMap<String, Box<RawJson>>, name: &str) -> Option<String> {
    serde_json::from_str(frame.get(name)?.get()).ok()
}
fn item_limit(tag: &str, payload: &RawJson) -> u64 {
    let bounded = fields(payload.get())
        .ok()
        .and_then(|fields| {
            fields
                .get("acceptBoundedSnapshot")
                .and_then(|v| serde_json::from_str::<bool>(v.get()).ok())
        })
        .unwrap_or(false);
    if tag == "orchestration.subscribeThread" && bounded {
        crate::THREAD_SNAPSHOT_MAX_BYTES
    } else {
        crate::MAX_BYTES
    }
}
impl Connection {
    /// One physical subscription; no reconnection, replay, cursor mutation or
    /// scalar decoding of items. Chunk ACK follows all callbacks, as in the
    /// existing typed loop. A callback error refuses ACK and ends this socket.
    pub fn subscribe_raw(
        &self,
        tag: &str,
        payload: &RawJson,
        cancel: impl Fn() -> bool,
        receive: impl FnMut(RawStreamTick) -> Result<()>,
    ) -> Result<RawStreamExit> {
        self.subscribe_raw_probed(tag, payload, cancel, || None, receive)
    }
    /// Probe deadlines start at the supplied request instant, or socket open if
    /// later. Constants, read ticks, ping cadence and authenticated factory are
    /// shared with the original Value subscription. Raw items are actual wire
    /// items: callers must decode compact IDs only when they opted into them.
    pub fn subscribe_raw_probed(
        &self,
        tag: &str,
        payload: &RawJson,
        cancel: impl Fn() -> bool,
        mut probe: impl FnMut() -> Option<Instant>,
        mut receive: impl FnMut(RawStreamTick) -> Result<()>,
    ) -> Result<RawStreamExit> {
        if cancel() {
            return Ok(RawStreamExit::Cancelled);
        }
        let mut socket = self.websocket_with_limit(item_limit(tag, payload))?;
        if cancel() {
            let _ = socket.close(None);
            return Ok(RawStreamExit::Cancelled);
        }
        match socket.get_mut() {
            tungstenite::stream::MaybeTlsStream::Plain(stream) => {
                stream.set_read_timeout(Some(Duration::from_millis(100)))
            }
            tungstenite::stream::MaybeTlsStream::Rustls(stream) => stream
                .sock
                .set_read_timeout(Some(Duration::from_millis(100))),
            _ => return Err("unsupported_tls".into()),
        }
        .map_err(|_| "disconnected")?;
        let tag_json = serde_json::to_string(tag).map_err(|_| "invalid_input")?;
        let request=format!("{{\"_tag\":\"Request\",\"id\":\"1\",\"tag\":{tag_json},\"payload\":{},\"headers\":[]}}",payload.get());
        socket
            .send(Message::Text(request.into()))
            .map_err(|_| "disconnected")?;
        let opened = Instant::now();
        let mut ping = opened;
        let mut seen = opened;
        let mut probing: Option<Instant> = None;
        while !cancel() {
            match socket.read() {
                Ok(Message::Text(text)) => {
                    seen = Instant::now();
                    probing = None;
                    let mut frame = fields(&text)?;
                    drop(text);
                    match control(&frame, "_tag").as_deref() {
                        Some("Chunk") if control(&frame, "requestId").as_deref() == Some("1") => {
                            let values = frame.remove("values").ok_or("malformed_stream")?;
                            let values: Vec<Box<RawJson>> = serde_json::from_str(values.get())
                                .map_err(|_| "malformed_stream")?;
                            for item in values {
                                receive(RawStreamTick::Item(item))?;
                            }
                            receive(RawStreamTick::ChunkEnd)?;
                            socket
                                .send(Message::Text(
                                    json!({"_tag":"Ack","requestId":"1"}).to_string().into(),
                                ))
                                .map_err(|_| "disconnected")?;
                        }
                        Some("Exit") if control(&frame, "requestId").as_deref() == Some("1") => {
                            let mut exit =
                                fields(frame.remove("exit").ok_or("malformed_stream")?.get())?;
                            let kind = control(&exit, "_tag");
                            let _ = socket.close(None);
                            return match kind.as_deref() {
                                Some("Success") => Ok(RawStreamExit::Success(exit.remove("value"))),
                                Some("Failure") => exit
                                    .remove("cause")
                                    .map(RawStreamExit::Failure)
                                    .ok_or_else(|| "malformed_stream".into()),
                                _ => Err("malformed_stream".into()),
                            };
                        }
                        Some("Ping") => socket
                            .send(Message::Text(json!({"_tag":"Pong"}).to_string().into()))
                            .map_err(|_| "disconnected")?,
                        _ => {}
                    }
                }
                Ok(Message::Ping(payload)) => socket
                    .send(Message::Pong(payload))
                    .map_err(|_| "disconnected")?,
                Ok(Message::Close(_)) => return Err("disconnected".into()),
                Ok(_) => {}
                Err(tungstenite::Error::Io(error))
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
                {
                    receive(RawStreamTick::Idle)?
                }
                Err(_) => return Err("disconnected".into()),
            }
            if let Some(asked) = probe() {
                socket
                    .send(Message::Text(json!({"_tag":"Ping"}).to_string().into()))
                    .map_err(|_| "disconnected")?;
                ping = Instant::now();
                probing.get_or_insert(asked.max(opened));
            }
            if probing.is_some_and(|at| at.elapsed() > STREAM_PROBE_TIMEOUT) {
                return Err("stream_timeout".into());
            }
            if ping.elapsed() >= STREAM_PING_INTERVAL {
                socket
                    .send(Message::Text(json!({"_tag":"Ping"}).to_string().into()))
                    .map_err(|_| "disconnected")?;
                ping = Instant::now();
            }
            if seen.elapsed() > STREAM_IDLE_TIMEOUT {
                return Err("stream_timeout".into());
            }
        }
        let _ = socket.send(Message::Text(
            json!({"_tag":"Interrupt","requestId":"1"})
                .to_string()
                .into(),
        ));
        let _ = socket.close(None);
        Ok(RawStreamExit::Cancelled)
    }
}
