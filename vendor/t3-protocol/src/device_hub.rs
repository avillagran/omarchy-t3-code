//! Device screens and input through T3's device hub proxy.
//!
//! Native T3 proxies expo-device-hub at `<origin><hubBasePath>/*` (normally
//! `/api/device-hub`). Media requests cannot carry bearer or DPoP headers in
//! the web client, so they authenticate with a session-bound `wsTicket` (valid
//! five minutes, shared by every request a panel opens) plus `hostId` on each
//! request. Only the paths the native proxy allows are built here:
//!
//! - iOS (serve-sim): MJPEG at `/vendor/serve-sim/helper/<udid>/stream.mjpeg`,
//!   input at `/vendor/serve-sim/helper/ws?device=<udid>` as `[tag][json]`.
//! - Android (serve-emu): one socket at `/vendor/serve-emu/ws?device=<serial>`
//!   carrying H.264 access units behind a 16-byte `SEMU` header downstream and
//!   JSON input upstream.
//! - Both: `POST /vendor/{serve-sim|serve-emu}/api/screenshot?device=<id>`.
use super::*;
use std::io::Read;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    Ios,
    Android,
}
impl Platform {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "ios" => Ok(Self::Ios),
            "android" => Ok(Self::Android),
            _ => Err("invalid_device".into()),
        }
    }
    fn vendor(self) -> &'static str {
        match self {
            Self::Ios => "/vendor/serve-sim",
            Self::Android => "/vendor/serve-emu",
        }
    }
}

/// Ticket and host for one connection to the hub. Mint a fresh one on every
/// (re)connect; the server keeps a ticket for five minutes.
#[derive(Clone, Debug)]
pub struct HubAccess {
    base: String,
    host_id: String,
    ticket: String,
}

fn hub_id(value: &str, max: usize) -> Result<&str> {
    if value.is_empty()
        || value.len() > max
        || !value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | ':'))
    {
        return Err("invalid_device".into());
    }
    Ok(value)
}
fn hub_base(value: &str) -> Result<&str> {
    let ok = value.starts_with("/api/")
        && value.len() <= 128
        && !value.ends_with('/')
        && value[1..]
            .split('/')
            .all(|s| !s.is_empty() && s != "." && s != ".." && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    if ok {
        Ok(value)
    } else {
        Err("invalid_device_hub".into())
    }
}

impl Connection {
    /// Prepares hub access for `host_id`, minting a media ticket.
    pub fn device_hub(&self, hub_base_path: &str, host_id: &str) -> Result<HubAccess> {
        let base = hub_base(hub_base_path)?.to_string();
        let host_id = hub_id(host_id, 128)?.to_string();
        let ticket = self.request_json("POST", "/api/auth/websocket-ticket", None, false)?;
        Ok(HubAccess {
            base,
            host_id,
            ticket: field(&ticket, "ticket")?.to_string(),
        })
    }
    fn hub_url(&self, access: &HubAccess, path: &str, device: Option<&str>, socket: bool) -> Result<Url> {
        let mut url = endpoint(&self.base, &format!("{}{}", access.base, path))?;
        {
            let mut query = url.query_pairs_mut();
            if let Some(device) = device {
                query.append_pair("device", device);
            }
            if path.ends_with("/serve-emu/ws") {
                query.append_pair("frame-meta", "1");
            }
            query.append_pair("hostId", &access.host_id);
            query.append_pair("wsTicket", &access.ticket);
        }
        if socket {
            url.set_scheme(if url.scheme() == "https" { "wss" } else { "ws" })
                .map_err(|_| "invalid_endpoint")?;
        }
        Ok(url)
    }
    /// PNG of the current screen. Slow; used when no stream is available.
    pub fn device_screenshot(&self, access: &HubAccess, platform: Platform, device: &str) -> Result<Vec<u8>> {
        let device = hub_id(device, 256)?;
        let url = self.hub_url(access, &format!("{}/api/screenshot", platform.vendor()), Some(device), false)?;
        let response = self
            .http
            .post(url)
            .header("Accept", "image/png")
            .send()
            .map_err(|_| "unreachable")?;
        hub_bytes(response, 16 * 1024 * 1024)
    }
    /// The iOS MJPEG stream. The body never ends; read it with [`MjpegFrames`].
    pub fn device_mjpeg(&self, access: &HubAccess, device: &str) -> Result<reqwest::blocking::Response> {
        let device = hub_id(device, 256)?;
        let url = self.hub_url(access, &format!("/vendor/serve-sim/helper/{device}/stream.mjpeg"), None, false)?;
        let response = self
            .http
            .get(url)
            // A decoder would hold frames back; JPEG does not compress anyway.
            .header("Accept-Encoding", "identity")
            .timeout(Duration::from_secs(60 * 60 * 12))
            .send()
            .map_err(|_| "unreachable")?;
        match response.status().as_u16() {
            200 => Ok(response),
            401 | 403 => Err("unauthorized".into()),
            404 => Err("device_unavailable".into()),
            _ => Err("device_stream_failed".into()),
        }
    }
    /// Input socket (iOS) or video and input socket (Android). Reads time out
    /// after 40 ms (`WouldBlock`), so callers can poll it alongside input.
    pub fn device_socket(
        &self,
        access: &HubAccess,
        platform: Platform,
        device: &str,
    ) -> Result<tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<TcpStream>>> {
        let device = hub_id(device, 256)?;
        let path = match platform {
            Platform::Ios => "/vendor/serve-sim/helper/ws",
            Platform::Android => "/vendor/serve-emu/ws",
        };
        let url = self.hub_url(access, path, Some(device), true)?;
        let req = url.as_str().into_client_request().map_err(|_| "invalid_endpoint")?;
        let host = url.host_str().ok_or("invalid_endpoint")?;
        let port = url.port_or_known_default().ok_or("invalid_endpoint")?;
        let deadline = Instant::now() + Duration::from_secs(8);
        let mut stream = None;
        for addr in (host, port).to_socket_addrs().map_err(|_| "disconnected")? {
            let Some(left) = deadline.checked_duration_since(Instant::now()) else { break };
            if let Ok(s) = TcpStream::connect_timeout(&addr, left) {
                stream = Some(s);
                break;
            }
        }
        let stream = stream.ok_or("disconnected")?;
        stream.set_write_timeout(Some(Duration::from_secs(12))).map_err(|_| "disconnected")?;
        stream.set_read_timeout(Some(Duration::from_secs(12))).map_err(|_| "disconnected")?;
        let config = tungstenite::protocol::WebSocketConfig::default()
            .max_message_size(Some(8 * 1024 * 1024))
            .max_frame_size(Some(8 * 1024 * 1024));
        match tungstenite::client_tls_with_config(req, stream, Some(config), None) {
            Ok((ws, _)) => {
                // Short reads after the handshake let one thread interleave
                // video reads with input writes.
                let tcp = match ws.get_ref() {
                    tungstenite::stream::MaybeTlsStream::Plain(s) => s,
                    tungstenite::stream::MaybeTlsStream::Rustls(s) => s.get_ref(),
                    _ => return Err("device_stream_failed".into()),
                };
                tcp.set_read_timeout(Some(Duration::from_millis(40))).map_err(|_| "disconnected")?;
                Ok(ws)
            }
            Err(tungstenite::HandshakeError::Failure(tungstenite::Error::Http(r)))
                if matches!(r.status().as_u16(), 401 | 403) => Err("unauthorized".into()),
            Err(_) => Err("device_stream_failed".into()),
        }
    }
}
fn hub_bytes(response: reqwest::blocking::Response, limit: u64) -> Result<Vec<u8>> {
    match response.status().as_u16() {
        200 => {}
        401 | 403 => return Err("unauthorized".into()),
        404 => return Err("device_unavailable".into()),
        _ => return Err("device_screenshot_failed".into()),
    }
    let mut out = Vec::new();
    read_limited(response, limit, &mut out).map_err(|e| match e {
        BodyError::TooLarge => "device_screenshot_too_large",
        BodyError::Unreadable => "device_screenshot_failed",
    })?;
    Ok(out)
}

/// Splits a JPEG byte stream (MJPEG multipart, or ffmpeg's image2pipe) into
/// whole frames by their start and end markers, ignoring part headers.
pub struct MjpegFrames<R: Read> {
    source: R,
    buffer: Vec<u8>,
    limit: usize,
}
impl<R: Read> MjpegFrames<R> {
    pub fn new(source: R) -> Self {
        Self { source, buffer: Vec::with_capacity(512 * 1024), limit: 16 * 1024 * 1024 }
    }
    fn take_frame(&mut self) -> Option<Vec<u8>> {
        let start = self.buffer.windows(2).position(|w| w == [0xff, 0xd8])?;
        let end = self.buffer[start + 2..].windows(2).position(|w| w == [0xff, 0xd9])? + start + 4;
        let frame = self.buffer[start..end].to_vec();
        self.buffer.drain(..end);
        Some(frame)
    }
}
impl<R: Read> Iterator for MjpegFrames<R> {
    type Item = Result<Vec<u8>>;
    fn next(&mut self) -> Option<Self::Item> {
        let mut chunk = [0u8; 64 * 1024];
        loop {
            if let Some(frame) = self.take_frame() {
                return Some(Ok(frame));
            }
            if self.buffer.len() > self.limit {
                return Some(Err("device_stream_failed".into()));
            }
            match self.source.read(&mut chunk) {
                Ok(0) => return None,
                Ok(n) => self.buffer.extend_from_slice(&chunk[..n]),
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => return Some(Err("device_stream_failed".into())),
            }
        }
    }
}

/// One Android access unit: `SEMU` magic, version, flags (bit 0 = keyframe),
/// two reserved bytes and a big-endian u64 presentation time, then H.264
/// Annex-B data.
pub fn parse_semu(packet: &[u8]) -> Option<(bool, &[u8])> {
    if packet.len() < 16 || packet[..4] != [0x53, 0x45, 0x4d, 0x55] {
        return None;
    }
    Some((packet[5] & 1 == 1, &packet[16..]))
}

/// Input from the panel, in screen-normalized coordinates (0..1).
pub fn input_message(platform: Platform, event: &Value) -> Result<tungstenite::Message> {
    let unit = |key: &str| -> Result<f64> {
        event[key].as_f64().filter(|v| v.is_finite() && (0.0..=1.0).contains(v)).ok_or_else(|| "invalid_device_input".into())
    };
    let kind = event["type"].as_str().unwrap_or("");
    match platform {
        Platform::Ios => {
            let (tag, body) = match kind {
                "touch" => {
                    let phase = match event["phase"].as_str() {
                        Some("down") => "begin",
                        Some("move") => "move",
                        Some("up") => "end",
                        _ => return Err("invalid_device_input".into()),
                    };
                    (0x03u8, json!({"type":phase,"x":unit("x")?,"y":unit("y")?}))
                }
                "button" => {
                    let button = match event["button"].as_str() {
                        Some("home") => "home",
                        Some("recents") => "app_switcher",
                        Some("power") => "lock",
                        _ => return Err("invalid_device_input".into()),
                    };
                    (0x04, json!({"button":button}))
                }
                "key" => {
                    let usage = event["usage"].as_u64().filter(|u| *u > 0 && *u < 0x10000).ok_or("invalid_device_input")?;
                    let phase = if event["down"] == true { "down" } else { "up" };
                    (0x06, json!({"type":phase,"usage":usage}))
                }
                _ => return Err("invalid_device_input".into()),
            };
            let mut bytes = vec![tag];
            bytes.extend_from_slice(body.to_string().as_bytes());
            Ok(tungstenite::Message::Binary(bytes.into()))
        }
        Platform::Android => {
            let body = match kind {
                "touch" => {
                    let action = match event["phase"].as_str() {
                        Some("down") => "down",
                        Some("move") => "move",
                        Some("up") => "up",
                        _ => return Err("invalid_device_input".into()),
                    };
                    json!({"type":"touch","action":action,"x":unit("x")?,"y":unit("y")?})
                }
                "button" => match event["button"].as_str() {
                    Some(b @ ("home" | "back" | "recents" | "power")) => json!({"type":b}),
                    _ => return Err("invalid_device_input".into()),
                },
                "key" => json!({"type":"key","keycode":event["keycode"].as_u64().filter(|k| *k > 0 && *k < 1000).ok_or("invalid_device_input")?}),
                "text" => {
                    let text = event["text"].as_str().filter(|t| !t.is_empty() && t.len() <= 1024).ok_or("invalid_device_input")?;
                    json!({"type":"text","text":text})
                }
                "keyframe" => json!({"type":"reset-video","ack":false}),
                _ => return Err("invalid_device_input".into()),
            };
            Ok(tungstenite::Message::Text(body.to_string().into()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn connection() -> Connection {
        Connection::new("http://127.0.0.1:3773", None, Credential::bearer("test-token-1234").unwrap()).unwrap()
    }
    fn access() -> HubAccess {
        HubAccess { base: "/api/device-hub".into(), host_id: "local".into(), ticket: "tick".into() }
    }
    #[test]
    fn urls_follow_the_native_allow_list() {
        let c = connection();
        let a = access();
        let ws = c.hub_url(&a, "/vendor/serve-emu/ws", Some("emulator-5554"), true).unwrap();
        assert_eq!(ws.as_str(), "ws://127.0.0.1:3773/api/device-hub/vendor/serve-emu/ws?device=emulator-5554&frame-meta=1&hostId=local&wsTicket=tick");
        let mjpeg = c.hub_url(&a, "/vendor/serve-sim/helper/ABC-123/stream.mjpeg", None, false).unwrap();
        assert_eq!(mjpeg.as_str(), "http://127.0.0.1:3773/api/device-hub/vendor/serve-sim/helper/ABC-123/stream.mjpeg?hostId=local&wsTicket=tick");
    }
    #[test]
    fn identifiers_and_bases_are_strict() {
        assert!(hub_id("emulator-5554", 256).is_ok());
        assert!(hub_id("../x", 256).is_err());
        assert!(hub_id("a/b", 256).is_err());
        assert!(hub_id("", 256).is_err());
        assert!(hub_base("/api/device-hub").is_ok());
        assert!(hub_base("/api/../x").is_err());
        assert!(hub_base("/other").is_err());
        assert!(hub_base("/api/device-hub/").is_err());
        let c = connection();
        assert!(c.device_mjpeg(&access(), "../../api/x").is_err());
    }
    #[test]
    fn mjpeg_frames_split_on_jpeg_markers() {
        let mut stream = b"--frame\r\nContent-Type: image/jpeg\r\n\r\n".to_vec();
        stream.extend_from_slice(&[0xff, 0xd8, 1, 2, 3, 0xff, 0xd9]);
        stream.extend_from_slice(b"\r\n--frame\r\nContent-Type: image/jpeg\r\n\r\n");
        stream.extend_from_slice(&[0xff, 0xd8, 9, 0xff, 0xd9]);
        let frames: Vec<_> = MjpegFrames::new(std::io::Cursor::new(stream)).map(|f| f.unwrap()).collect();
        assert_eq!(frames, vec![vec![0xff, 0xd8, 1, 2, 3, 0xff, 0xd9], vec![0xff, 0xd8, 9, 0xff, 0xd9]]);
    }
    #[test]
    fn semu_packets_expose_keyframes() {
        let mut packet = vec![0x53, 0x45, 0x4d, 0x55, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 42];
        packet.extend_from_slice(&[0, 0, 0, 1, 0x65]);
        assert_eq!(parse_semu(&packet), Some((true, &[0, 0, 0, 1, 0x65][..])));
        packet[5] = 0;
        assert_eq!(parse_semu(&packet).unwrap().0, false);
        assert!(parse_semu(b"nope").is_none());
    }
    #[test]
    fn input_matches_each_platform_wire_format() {
        let touch = json!({"type":"touch","phase":"down","x":0.25,"y":0.5});
        match input_message(Platform::Ios, &touch).unwrap() {
            tungstenite::Message::Binary(b) => {
                assert_eq!(b[0], 0x03);
                assert_eq!(serde_json::from_slice::<Value>(&b[1..]).unwrap(), json!({"type":"begin","x":0.25,"y":0.5}));
            }
            other => panic!("{other:?}"),
        }
        match input_message(Platform::Android, &touch).unwrap() {
            tungstenite::Message::Text(t) => assert_eq!(serde_json::from_str::<Value>(&t).unwrap(), json!({"type":"touch","action":"down","x":0.25,"y":0.5})),
            other => panic!("{other:?}"),
        }
        match input_message(Platform::Ios, &json!({"type":"button","button":"recents"})).unwrap() {
            tungstenite::Message::Binary(b) => assert_eq!(&b[1..], br#"{"button":"app_switcher"}"#),
            other => panic!("{other:?}"),
        }
        assert!(input_message(Platform::Ios, &json!({"type":"touch","phase":"down","x":1.5,"y":0})).is_err());
        assert!(input_message(Platform::Ios, &json!({"type":"button","button":"back"})).is_err());
        assert!(input_message(Platform::Android, &json!({"type":"shell","cmd":"rm"})).is_err());
    }

    /// A local stand-in for the native proxy: ticket, MJPEG, screenshot and
    /// the input socket, checking the query each request carries.
    #[test]
    fn talks_to_a_hub_over_http_and_websocket() {
        use std::io::{BufRead, BufReader, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let mut seen = vec![];
            for _ in 0..4 {
                let (stream, _) = listener.accept().unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let path = line.split_whitespace().nth(1).unwrap().to_string();
                let mut headers = String::new();
                loop {
                    let mut h = String::new();
                    reader.read_line(&mut h).unwrap();
                    if h == "\r\n" { break }
                    headers.push_str(&h);
                }
                seen.push(path.clone());
                let mut out = stream;
                if path.starts_with("/api/auth/websocket-ticket") {
                    let body = r#"{"ticket":"tick-1"}"#;
                    write!(out, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}", body.len(), body).unwrap();
                } else if path.contains("stream.mjpeg") {
                    write!(out, "HTTP/1.1 200 OK\r\nContent-Type: multipart/x-mixed-replace; boundary=f\r\nConnection: close\r\n\r\n--f\r\nContent-Type: image/jpeg\r\n\r\n").unwrap();
                    out.write_all(&[0xff, 0xd8, 7, 0xff, 0xd9]).unwrap();
                    write!(out, "\r\n--f\r\n").unwrap();
                } else if path.contains("/api/screenshot") {
                    let png = b"\x89PNG fake";
                    write!(out, "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\n\r\n", png.len()).unwrap();
                    out.write_all(png).unwrap();
                } else if path.contains("/helper/ws") {
                    // Hand the upgrade to tungstenite with the request already read.
                    let key = headers.lines().find_map(|h| h.strip_prefix("Sec-WebSocket-Key: ").or_else(|| h.strip_prefix("sec-websocket-key: "))).unwrap().trim().to_string();
                    let accept = tungstenite::handshake::derive_accept_key(key.as_bytes());
                    write!(out, "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n").unwrap();
                    let mut ws = tungstenite::WebSocket::from_raw_socket(out, tungstenite::protocol::Role::Server, None);
                    let mut config = vec![0x82];
                    config.extend_from_slice(br#"{"width":390,"height":844}"#);
                    ws.send(tungstenite::Message::Binary(config.into())).unwrap();
                    let message = ws.read().unwrap();
                    seen.push(format!("input:{}", message.into_data().first().copied().unwrap_or(0)));
                }
            }
            seen
        });
        let c = Connection::new(&format!("http://127.0.0.1:{port}"), None, Credential::bearer("test-token-1234").unwrap()).unwrap();
        let access = c.device_hub("/api/device-hub", "local").unwrap();
        let frames: Vec<_> = MjpegFrames::new(c.device_mjpeg(&access, "UDID-1").unwrap()).collect::<Result<_>>().unwrap();
        assert_eq!(frames, vec![vec![0xff, 0xd8, 7, 0xff, 0xd9]]);
        assert_eq!(c.device_screenshot(&access, Platform::Ios, "UDID-1").unwrap(), b"\x89PNG fake");
        let mut ws = c.device_socket(&access, Platform::Ios, "UDID-1").unwrap();
        let config = loop {
            match ws.read() {
                Ok(m) => break m.into_data(),
                Err(tungstenite::Error::Io(e)) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => continue,
                Err(e) => panic!("{e}"),
            }
        };
        assert_eq!(config[0], 0x82);
        ws.send(input_message(Platform::Ios, &json!({"type":"touch","phase":"down","x":0.5,"y":0.5})).unwrap()).unwrap();
        let seen = server.join().unwrap();
        assert_eq!(seen[0], "/api/auth/websocket-ticket");
        assert_eq!(seen[1], "/api/device-hub/vendor/serve-sim/helper/UDID-1/stream.mjpeg?hostId=local&wsTicket=tick-1");
        assert_eq!(seen[2], "/api/device-hub/vendor/serve-sim/api/screenshot?device=UDID-1&hostId=local&wsTicket=tick-1");
        assert_eq!(seen[3], "/api/device-hub/vendor/serve-sim/helper/ws?device=UDID-1&hostId=local&wsTicket=tick-1");
        assert_eq!(seen[4], "input:3");
    }
}
