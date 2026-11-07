//! Device screens for the side panel (Devices tab). One screen at a time:
//! `device-screen-open` opens the native session (`device.open`) when asked,
//! then streams frames as private files the panel shows like browser frames,
//! and `device-screen-input` forwards touches, buttons and keys. Frames come
//! from the iOS MJPEG stream, the Android H.264 socket decoded by ffmpeg when
//! it is installed, or periodic screenshots otherwise. Hiding the panel stops
//! the view; only `device-screen-close` with `endSession` closes the session.
use omarchy_t3_client::*;
use protocol::api::device as device_api;
use protocol::device_hub::{self, MjpegFrames, Platform};
use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc, Arc, Mutex,
    },
    time::{Duration, Instant},
};

mod decoder;

struct Session {
    request_id: String,
    cancel: Arc<AtomicBool>,
    input: mpsc::Sender<Value>,
    target: Target,
    decoder: decoder::Slot,
    /// The device id the session resolved to (an emulator serial once booted).
    device: Arc<Mutex<String>>,
}
#[derive(Clone)]
struct Target {
    request_id: String,
    environment: String,
    thread: String,
    host_id: String,
    device_id: String,
    platform: Platform,
    hub_base: String,
}

type DeviceKey = (String, String, String);
type ActiveActions = Arc<Mutex<std::collections::HashSet<DeviceKey>>>;

#[derive(Default)]
pub struct Manager {
    current: Option<Session>,
    actions: ActiveActions,
}

struct ActionLease {
    active: ActiveActions,
    key: DeviceKey,
}
impl ActionLease {
    fn acquire(active: &ActiveActions, key: DeviceKey) -> Option<Self> {
        let mut pending=active.lock().unwrap();
        if pending.len()>=16 || !pending.insert(key.clone()) { return None; }
        Some(Self{active:active.clone(),key})
    }
}
impl Drop for ActionLease {
    fn drop(&mut self) { self.active.lock().unwrap().remove(&self.key); }
}

fn surface(request_id: &str, data: Value) {
    let mut data = data;
    data["requestId"] = json!(request_id);
    crate::emit("device-surface", data);
}

impl Manager {
    pub fn request(&mut self, v: &Value, visible: bool) {
        let request_id = text(v, "requestId").to_string();
        match text(v, "op") {
            "device-screen-open" => {
                self.stop();
                if !visible {
                    surface(&request_id, json!({"kind":"error","error":"panel_hidden"}));
                    return;
                }
                match target(v) {
                    Ok(target) => self.current = Some(start(target, v["open"] == true)),
                    Err(error) => surface(&request_id, json!({"kind":"error","error":error})),
                }
            }
            "device-screen-input" => {
                if let Some(session) = self.current.as_ref().filter(|s| s.request_id == request_id) {
                    let _ = session.input.send(v["event"].clone());
                }
            }
            // Settings and actions on the open device (device.action / device.detail).
            "device-screen-action" | "device-screen-detail" => {
                let Some(session) = self.current.as_ref().filter(|s| s.request_id == request_id) else { return };
                let target = session.target.clone();
                let device = session.device.lock().unwrap().clone();
                let detail = text(v, "op") == "device-screen-detail";
                let action = v["action"].clone();
                let key=(target.environment.clone(),target.host_id.clone(),device.clone());
                let Some(lease)=ActionLease::acquire(&self.actions,key) else {
                    surface(&target.request_id,json!({"kind":"action-error","error":"device_action_pending"}));
                    return;
                };
                std::thread::spawn(move || {
                    // Hiding or reopening the viewer does not cancel a host command.
                    // Keep its reservation until the RPC settles, including failures.
                    let _lease=lease;
                    let result = (|| -> Result<Value> {
                        let env = crate::env_by_id(&target.environment)?;
                        let c = env.connection(true)?;
                        if detail {
                            return device_api::detail(&c, &device, Some(&target.host_id));
                        }
                        let mut payload = action_payload(&action)?;
                        payload["hostId"] = json!(target.host_id);
                        payload["deviceId"] = json!(device);
                        // The native action already returns the updated detail. A second
                        // read could fail after a successful nonrepeatable operation.
                        device_api::action(&c, &device_api::DeviceAction::from_json(&payload).map_err(|_| "invalid_device_action")?)
                    })();
                    match result {
                        Ok(detail) => surface(&target.request_id, json!({"kind":"detail","detail":detail})),
                        Err(error) => surface(&target.request_id, json!({"kind":"action-error","error":error})),
                    }
                });
            }
            "device-screen-close" => {
                let Some(session) = self.current.as_ref().filter(|s| s.request_id == request_id) else { return };
                let mut target = session.target.clone();
                target.device_id = session.device.lock().unwrap().clone();
                let shutdown = v["shutdown"] == true;
                self.stop();
                if v["endSession"] == true {
                    std::thread::spawn(move || {
                        let result = (|| -> Result<Value> {
                            let env = crate::env_by_id(&target.environment)?;
                            let c = env.connection(true)?;
                            device_api::close(&c, &target.thread, Some(&target.host_id), Some(&target.device_id), Some(shutdown))
                        })();
                        // Native T3 gives up waiting on a shutdown after 15 s while the
                        // device keeps powering off; report it as still shutting down.
                        let error = result.err().map(|e| if shutdown && protocol::error_code(&e) == "rpc_rejected" { "device_shutdown_pending".to_string() } else { e });
                        surface(&target.request_id, json!({"kind":"closed","error":error}));
                    });
                } else {
                    surface(&target.request_id, json!({"kind":"stopped"}));
                }
            }
            _ => {}
        }
    }
    /// Stops the current view (panel hidden, chat switched or a new screen).
    pub fn stop(&mut self) {
        if let Some(session) = self.current.take() {
            session.cancel.store(true, Ordering::Relaxed);
            decoder::stop(&session.decoder);
        }
    }
}

/// The device.action variants the panel offers, checked before they reach the hub.
pub fn action_payload(a: &Value) -> Result<Value> {
    if !a.is_object() || a.get("hostId").is_some() || a.get("deviceId").is_some() {
        return Err("invalid_device_action".into());
    }
    let mut input=a.clone();
    input["deviceId"]=json!("captured-device");
    let mut payload=device_api::DeviceAction::from_json(&input).and_then(|action|action.payload())
        .map_err(|_|"invalid_device_action".to_string())?;
    payload.as_object_mut().unwrap().remove("deviceId");
    Ok(payload)
}

fn target(v: &Value) -> Result<Target> {
    let field = |key: &str, max: usize| -> Result<String> {
        let value = text(v, key);
        if value.is_empty() || value.len() > max || value.chars().any(char::is_control) {
            return Err("invalid_device".into());
        }
        Ok(value.to_string())
    };
    Ok(Target {
        request_id: field("requestId", 128)?,
        environment: field("environment", 128)?,
        thread: field("thread", 2048)?,
        host_id: field("hostId", 128)?,
        device_id: field("deviceId", 256)?,
        platform: Platform::parse(text(v, "platform"))?,
        hub_base: if text(v, "hubBasePath").is_empty() { "/api/device-hub".into() } else { field("hubBasePath", 128)? },
    })
}

/// Private frame files, alternated so the panel never reads a half-written one.
struct Frames {
    dir: PathBuf,
    request_id: String,
    revision: AtomicU64,
    last: Mutex<Instant>,
}
impl Frames {
    fn new(request_id: &str) -> Result<Self> {
        let root = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
        let dir = root.join("omarchy-t3-code").join(format!("device-{}", uuid::Uuid::new_v4()));
        fs::DirBuilder::new().recursive(true).mode(0o700).create(&dir).map_err(|_| "device_storage_unavailable")?;
        Ok(Self { dir, request_id: request_id.into(), revision: AtomicU64::new(0), last: Mutex::new(Instant::now() - Duration::from_secs(1)) })
    }
    /// Publishes at most one frame per display refresh; older frames are dropped.
    fn publish(&self, bytes: &[u8], ext: &str) -> bool {
        self.publish_with(bytes, ext, || {})
    }
    fn publish_with(&self, bytes: &[u8], ext: &str, written_frame: impl FnOnce()) -> bool {
        {
            let mut last = self.last.lock().unwrap();
            if last.elapsed() < Duration::from_millis(8) {
                return false;
            }
            *last = Instant::now();
        }
        let Some((width, height)) = image_size(bytes) else { return false };
        let revision = self.revision.fetch_add(1, Ordering::Relaxed) + 1;
        let path = self.dir.join(format!("frame-{}.{ext}", revision % 2));
        let written = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&path)
            .and_then(|mut f| f.write_all(bytes));
        if written.is_ok() {
            written_frame();
            surface(&self.request_id, json!({"kind":"frame","url":format!("file://{}",path.display()),"revision":revision,"width":width,"height":height}));
        }
        written.is_ok()
    }
}
impl Drop for Frames {
    fn drop(&mut self) {
        let _ = crate::frame_files::reclaim(&self.dir);
    }
}

/// Width and height from a JPEG SOF marker or a PNG IHDR chunk.
pub fn image_size(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() > 24 && bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        let w = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
        let h = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
        return Some((w, h));
    }
    if !bytes.starts_with(&[0xff, 0xd8]) {
        return None;
    }
    let mut i = 2;
    while i + 9 < bytes.len() {
        if bytes[i] != 0xff {
            i += 1;
            continue;
        }
        let marker = bytes[i + 1];
        if matches!(marker, 0xc0..=0xcf) && !matches!(marker, 0xc4 | 0xc8 | 0xcc) {
            let h = u16::from_be_bytes([bytes[i + 5], bytes[i + 6]]) as u32;
            let w = u16::from_be_bytes([bytes[i + 7], bytes[i + 8]]) as u32;
            return Some((w, h));
        }
        if marker == 0xd8 || marker == 0x01 || (0xd0..=0xd7).contains(&marker) || marker == 0xff {
            i += if marker == 0xff { 1 } else { 2 };
            continue;
        }
        let length = u16::from_be_bytes([bytes[i + 2], bytes[i + 3]]) as usize;
        i += 2 + length;
    }
    None
}

fn start(target: Target, open: bool) -> Session {
    let cancel = Arc::new(AtomicBool::new(false));
    let decoder = Arc::new(Mutex::new(None));
    let (input, inputs) = mpsc::channel::<Value>();
    let device = Arc::new(Mutex::new(target.device_id.clone()));
    let session = Session { request_id: target.request_id.clone(), cancel: cancel.clone(), input, target: target.clone(), decoder: decoder.clone(), device: device.clone() };
    std::thread::spawn(move || {
        let request_id = target.request_id.clone();
        let result = run(&target, open, &cancel, inputs, decoder, &device);
        if !cancel.load(Ordering::Relaxed) {
            if let Err(error) = result {
                surface(&request_id, json!({"kind":"error","error":error}));
            }
        }
    });
    session
}

fn run(target: &Target, open: bool, cancel: &Arc<AtomicBool>, inputs: mpsc::Receiver<Value>, decoder: decoder::Slot, device: &Arc<Mutex<String>>) -> Result<()> {
    let env = crate::env_by_id(&target.environment)?;
    let c = env.connection(true)?;
    let platform = match target.platform { Platform::Ios => "ios", Platform::Android => "android" };
    let mut screen_device = target.device_id.clone();
    if open {
        surface(&target.request_id, json!({"kind":"status","stage":"opening"}));
        let session = device_api::open(&c, &target.thread, &target.device_id, platform, Some(&target.host_id), Some(true))?;
        surface(&target.request_id, json!({"kind":"session","session":session}));
        // A booted device is addressed by its session id (an emulator serial
        // such as emulator-5554), not by the AVD name it was listed under.
        if let Some(id) = session["deviceId"].as_str().filter(|id| !id.is_empty() && id.len() <= 256) {
            screen_device = id.to_string();
            *device.lock().unwrap() = screen_device.clone();
        }
    }
    if cancel.load(Ordering::Relaxed) {
        return Ok(());
    }
    let target = &Target { device_id: screen_device, ..target.clone() };
    let frames = Arc::new(Frames::new(&target.request_id)?);
    surface(&target.request_id, json!({"kind":"status","stage":"connecting"}));
    // A device that just booted can refuse the stream for a while, so keep
    // retrying with a growing pause; any frame resets the count.
    let mut failures = 0u64;
    let mut seen = 0;
    while !cancel.load(Ordering::Relaxed) {
        let access = c.device_hub(&target.hub_base, &target.host_id)?;
        let result = match target.platform {
            Platform::Ios => ios(&c, &access, target, cancel, &inputs, &frames),
            Platform::Android => android(&c, &access, target, cancel, &inputs, &frames, &decoder),
        };
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        if result.as_ref().is_err_and(|error| error == "device_decoder_failed" || error == "device_diagnostics_unavailable") {
            return result;
        }
        let now = frames.revision.load(Ordering::Relaxed);
        if now > seen {
            seen = now;
            failures = 0;
        }
        failures += 1;
        if failures > 10 {
            return result;
        }
        surface(&target.request_id, json!({"kind":"status","stage":"reconnecting"}));
        for _ in 0..(5 * failures.min(6)) {
            if cancel.load(Ordering::Relaxed) {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    Ok(())
}

/// H.264 Annex-B on stdin to a JPEG stream on stdout. Short probing keeps the
/// first frame fast; the MJPEG encoder needs full-range YUV.
fn decoder_command() -> Command {
    let mut command = Command::new("ffmpeg");
    command
        .args(["-loglevel", "error", "-flags", "low_delay", "-probesize", "32", "-analyzeduration", "0", "-f", "h264", "-i", "pipe:0"])
        .args(["-f", "image2pipe", "-pix_fmt", "yuvj420p", "-c:v", "mjpeg", "-q:v", "4", "pipe:1"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}

fn would_block(error: &tungstenite::Error) -> bool {
    matches!(error, tungstenite::Error::Io(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut))
}

/// iOS: frames from the MJPEG stream on a helper thread; this thread keeps
/// the input socket, forwarding panel input and reporting screen config.
fn ios(c: &protocol::Connection, access: &device_hub::HubAccess, target: &Target, cancel: &Arc<AtomicBool>, inputs: &mpsc::Receiver<Value>, frames: &Arc<Frames>) -> Result<()> {
    let stream = c.device_mjpeg(access, &target.device_id)?;
    let (stream_cancel, stream_frames) = (cancel.clone(), frames.clone());
    let stream_done = Arc::new(AtomicBool::new(false));
    let done = stream_done.clone();
    std::thread::spawn(move || {
        for frame in MjpegFrames::new(stream) {
            if stream_cancel.load(Ordering::Relaxed) {
                break;
            }
            match frame {
                Ok(frame) => { stream_frames.publish(&frame, "jpg"); },
                Err(_) => break,
            }
        }
        done.store(true, Ordering::Relaxed);
    });
    let mut socket = c.device_socket(access, Platform::Ios, &target.device_id).ok();
    surface(&target.request_id, json!({"kind":"status","stage":"live","input":socket.is_some()}));
    while !cancel.load(Ordering::Relaxed) && !stream_done.load(Ordering::Relaxed) {
        let Some(ws) = socket.as_mut() else {
            if let Ok(event) = inputs.recv_timeout(Duration::from_millis(100)) {
                let _ = event;
                surface(&target.request_id, json!({"kind":"error","error":"device_input_unavailable"}));
            }
            continue;
        };
        while let Ok(event) = inputs.try_recv() {
            if let Ok(message) = device_hub::input_message(Platform::Ios, &event) {
                if ws.send(message).is_err() {
                    socket = None;
                    break;
                }
            }
        }
        let Some(ws) = socket.as_mut() else { continue };
        match ws.read() {
            Ok(tungstenite::Message::Binary(bytes)) if bytes.first() == Some(&0x82) => {
                if let Ok(config) = serde_json::from_slice::<Value>(&bytes[1..]) {
                    surface(&target.request_id, json!({"kind":"config","width":config["width"],"height":config["height"],"orientation":config["orientation"],"inputUnavailable":config["inputUnavailable"]==true}));
                }
            }
            Ok(_) => {}
            Err(e) if would_block(&e) => {}
            Err(_) => socket = None,
        }
    }
    Err("device_stream_ended".into())
}

/// Android: one socket carries H.264 down and JSON input up. Video is decoded
/// by ffmpeg when available; otherwise screenshots stand in for frames.
struct AndroidView<'a> {
    connection: &'a protocol::Connection, access: &'a device_hub::HubAccess, target: &'a Target,
    cancel: &'a Arc<AtomicBool>, inputs: &'a mpsc::Receiver<Value>, frames: &'a Arc<Frames>, decoder: &'a decoder::Slot,
}
fn android(c: &protocol::Connection, access: &device_hub::HubAccess, target: &Target, cancel: &Arc<AtomicBool>, inputs: &mpsc::Receiver<Value>, frames: &Arc<Frames>, decoder: &decoder::Slot) -> Result<()> {
    android_view(AndroidView { connection: c, access, target, cancel, inputs, frames, decoder }, decoder_command())
}
fn android_view(view: AndroidView<'_>, mut command: Command) -> Result<()> {
    let AndroidView { connection: c, access, target, cancel, inputs, frames, decoder } = view;
    let mut ws = c.device_socket(access, Platform::Android, &target.device_id)?;
    surface(&target.request_id, json!({"kind":"status","stage":"connecting","input":true,"decoder":false}));
    let mut running = match decoder::start(decoder, cancel, &mut command) {
        Ok(Some(running)) => Some(running),
        Ok(None) => return Ok(()),
        Err(decoder::StartError::Spawn(error)) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(decoder::StartError::Diagnostics(error)) => { surface(&target.request_id, json!({"kind":"status","stage":"error","input":false,"decoder":false})); eprintln!("device decoder diagnostics unavailable: {error}"); return Err("device_diagnostics_unavailable".into()); },
        Err(_) => { surface(&target.request_id, json!({"kind":"status","stage":"error","input":false,"decoder":false})); return Err("device_decoder_failed".into()); },
    };
    let mut feed = running.as_mut().and_then(|running| running.stdin.take());
    if let Some(running) = running.as_mut() {
        let stdout = running.stdout.take().expect("decoder stdout");
        let attempt = running.attempt.clone();
        let (decode_cancel, decode_frames, request_id) = (cancel.clone(), frames.clone(), target.request_id.clone());
        let reader = std::thread::Builder::new().name("device-decoder-frames".into()).spawn(move || {
            let mut reported = false;
            for frame in MjpegFrames::new(stdout) {
                if decode_cancel.load(Ordering::Relaxed) { break; }
                match frame {
                    Ok(frame) => attempt.publish(|| {
                        decode_frames.publish_with(&frame, "jpg", || {
                            if !reported {
                                reported = true;
                                surface(&request_id, json!({"kind":"status","stage":"live","input":true,"decoder":true}));
                            }
                        });
                    }),
                    Err(_) => break,
                }
            }
        });
        if reader.is_err() {
            surface(&target.request_id, json!({"kind":"status","stage":"error","input":false,"decoder":false}));
            return Err("device_decoder_failed".into());
        }
        let _ = ws.send(tungstenite::Message::Text(json!({"type":"reset-video","ack":false}).to_string().into()));
    } else {
        // A missing decoder remains a distinct, honestly reported screenshot mode.
        let (shot_cancel, shot_frames, shot_c, shot_access, device) = (cancel.clone(), frames.clone(), c.clone(), access.clone(), target.device_id.clone());
        std::thread::spawn(move || {
            while !shot_cancel.load(Ordering::Relaxed) {
                if let Ok(png) = shot_c.device_screenshot(&shot_access, Platform::Android, &device) { shot_frames.publish(&png, "png"); }
                std::thread::sleep(Duration::from_millis(250));
            }
        });
    }
    while !cancel.load(Ordering::Relaxed) {
        if running.as_ref().is_some_and(|running| running.attempt.exited()) {
            let detail = running.as_ref().unwrap().attempt.stop();
            surface(&target.request_id, json!({"kind":"status","stage":"error","input":false,"decoder":false}));
            if !detail.is_empty() { eprintln!("device decoder failed: {detail}"); }
            return Err("device_decoder_failed".into());
        }
        while let Ok(event) = inputs.try_recv() {
            if let Ok(message) = device_hub::input_message(Platform::Android, &event) { ws.send(message).map_err(|_| "device_stream_ended")?; }
        }
        match ws.read() {
            Ok(tungstenite::Message::Binary(packet)) => {
                let parsed = device_hub::parse_semu(&packet);
                if let Some(running) = running.as_ref() { running.attempt.packet(&packet, parsed); }
                if let (Some(stdin), Some((_, payload))) = (feed.as_mut(), parsed) {
                    if stdin.write_all(payload).is_err() {
                        if let Some(running) = running.as_ref() { let detail = running.attempt.stop(); if !detail.is_empty() {eprintln!("device decoder failed: {detail}");} }
                        surface(&target.request_id, json!({"kind":"status","stage":"error","input":false,"decoder":false}));
                        return Err("device_decoder_failed".into());
                    }
                }
            }
            Ok(tungstenite::Message::Text(text)) => {if let Some(running) = running.as_ref() {running.attempt.text(text.as_str());}},
            Ok(tungstenite::Message::Close(_)) => return Err("device_stream_ended".into()),
            Ok(_) => {},
            Err(e) if would_block(&e) => {},
            Err(_) => return Err("device_stream_ended".into()),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sizes_come_from_jpeg_and_png_headers() {
        let png = [b"\x89PNG\r\n\x1a\n".as_slice(), &[0, 0, 0, 13], b"IHDR", &640u32.to_be_bytes(), &480u32.to_be_bytes(), &[8, 6, 0, 0, 0]].concat();
        assert_eq!(image_size(&png), Some((640, 480)));
        let jpeg = [&[0xff, 0xd8, 0xff, 0xe0, 0, 4, 0, 0][..], &[0xff, 0xc0, 0, 11, 8, 0x03, 0x20, 0x01, 0xc2, 3, 0, 0, 0, 0, 0]].concat();
        assert_eq!(image_size(&jpeg), Some((450, 800)));
        assert_eq!(image_size(b"nope"), None);
    }
    #[test]
    fn android_video_decodes_to_jpeg_frames() {
        let Ok(source) = Command::new("ffmpeg")
            .args(["-loglevel", "error", "-f", "lavfi", "-i", "testsrc=size=360x780:rate=30", "-t", "0.5", "-c:v", "libx264", "-bsf:v", "h264_mp4toannexb", "-f", "h264", "pipe:1"])
            .stderr(Stdio::null())
            .output()
        else {
            return; // ffmpeg is optional; screenshots stand in without it.
        };
        let mut child = decoder_command().spawn().unwrap();
        let mut stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let feeder = std::thread::spawn(move || {
            for chunk in source.stdout.chunks(512) {
                stdin.write_all(chunk).unwrap();
            }
        });
        let frames: Vec<Vec<u8>> = MjpegFrames::new(stdout).map(|f| f.unwrap()).collect();
        feeder.join().unwrap();
        let _ = child.wait();
        assert!(frames.len() >= 10, "decoded {} frames", frames.len());
        assert_eq!(image_size(&frames[0]), Some((360, 780)));
    }
    #[test]
    fn targets_require_every_field() {
        let ok = json!({"requestId":"r","environment":"local","thread":"t","hostId":"local","deviceId":"emulator-5554","platform":"android"});
        assert!(target(&ok).is_ok());
        assert_eq!(target(&ok).unwrap().hub_base, "/api/device-hub");
        let mut bad = ok.clone();
        bad["platform"] = json!("windows");
        assert!(target(&bad).is_err());
        let mut missing = ok.clone();
        missing["deviceId"] = json!("");
        assert!(target(&missing).is_err());
    }
}
#[cfg(test)]
mod action_tests {
    use super::{action_payload, ActionLease, Manager};
    use serde_json::json;
    #[test]
    fn only_known_actions_pass() {
        assert_eq!(action_payload(&json!({"type":"setAppearance","value":"dark"})).unwrap()["value"], "dark");
        assert!(action_payload(&json!({"type":"setAppearance","value":"blue"})).is_err());
        assert_eq!(action_payload(&json!({"type":"setToggle","setting":"networkEnabled","value":false})).unwrap()["value"], false);
        assert!(action_payload(&json!({"type":"sendPush"})).is_err());
        assert_eq!(action_payload(&json!({"type":"openUrl","url":" https://t3.codes "})).unwrap()["url"], "https://t3.codes");
    }
    #[test]
    fn every_native_device_tool_action_retains_its_payload_without_editable_target_fields() {
        for action in [
            json!({"type":"setLocation","latitude":0.0,"longitude":-180.0}),
            json!({"type":"setPermission","appId":"example.app","permission":"camera","decision":"grant"}),
            json!({"type":"setLiquidGlass","value":"tinted"}),
            json!({"type":"setColorFilter","value":"blue-yellow"}),
            json!({"type":"launchApp","appId":"example.app"}),
            json!({"type":"terminateApp","appId":"example.app"}),
            json!({"type":"sendPush","appId":"example.app","payload":{"aps":{"alert":"Synthetic"}}}),
        ] { assert_eq!(action_payload(&action).unwrap(),action); }
        assert!(action_payload(&json!({"type":"setLocation","latitude":90.1,"longitude":0.0})).is_err());
        assert!(action_payload(&json!({"type":"launchApp","appId":"example.app","deviceId":"foreign"})).is_err());
        assert!(action_payload(&json!({"type":"launchApp","appId":"example.app","hostId":"foreign"})).is_err());
    }
    #[test]
    fn closing_a_view_does_not_release_an_inflight_device_command() {
        let mut manager=Manager::default();
        let key=("environment".into(),"host".into(),"device".into());
        let command=ActionLease::acquire(&manager.actions,key.clone()).unwrap();
        manager.stop();
        assert!(ActionLease::acquire(&manager.actions,key.clone()).is_none());
        let other=ActionLease::acquire(&manager.actions,("environment".into(),"host".into(),"other-device".into())).unwrap();
        drop(command);
        assert!(ActionLease::acquire(&manager.actions,key).is_some());
        drop(other);
        assert!(manager.actions.lock().unwrap().is_empty());
    }
}

#[cfg(test)]
mod flow_tests;
