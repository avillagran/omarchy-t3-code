//! Actual SDK HTTP/WebSocket plus the production Android decoder consumer.
use super::*;
use std::{io::{BufRead, BufReader}, net::TcpListener};
struct MediaHeaders;
impl tungstenite::handshake::server::Callback for MediaHeaders {
    fn on_request(self, request: &tungstenite::handshake::server::Request, response: tungstenite::handshake::server::Response) -> std::result::Result<tungstenite::handshake::server::Response, tungstenite::handshake::server::ErrorResponse> {
        assert!(request.uri().query().unwrap().contains("frame-meta=1"));
        assert!(request.uri().query().unwrap().contains("wsTicket=synthetic-media-only"));
        assert!(!request.headers().contains_key("authorization"));
        Ok(response)
    }
}
fn hub(payload:Vec<u8>)->(protocol::Connection,std::thread::JoinHandle<Vec<String>>) {
    assert!(std::path::Path::new("/run/.containerenv").is_file());
    let listener=TcpListener::bind("127.0.0.1:0").unwrap();let base=format!("http://{}",listener.local_addr().unwrap());
    let peer=std::thread::spawn(move||{
        let (mut http,_)=listener.accept().unwrap();let mut reader=BufReader::new(http.try_clone().unwrap());let mut line=String::new();reader.read_line(&mut line).unwrap();assert!(line.starts_with("POST /api/auth/websocket-ticket/media "));
        loop {let mut header=String::new();reader.read_line(&mut header).unwrap();if header=="\r\n"{break;}}
        let body=br#"{"ticket":"synthetic-media-only"}"#;write!(http,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len()).unwrap();http.write_all(body).unwrap();drop(http);
        let (stream,_)=listener.accept().unwrap();stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let mut socket=tungstenite::accept_hdr(stream,MediaHeaders).unwrap();
        socket.send(tungstenite::Message::Text(json!({"type":"config","width":480,"height":800}).to_string().into())).unwrap();
        let reset=socket.read().unwrap();assert!(reset.to_text().unwrap().contains("reset-video"));
        let mut packet=b"SEMU\x02\x01\0\0".to_vec();packet.extend_from_slice(&0u64.to_be_bytes());packet.extend_from_slice(&0f64.to_be_bytes());packet.extend_from_slice(&payload);
        let _=socket.send(tungstenite::Message::Binary(packet.into()));
        while let Ok(message)=socket.read(){if message.is_close(){break;}}
        listener.set_nonblocking(true).unwrap();assert_eq!(listener.accept().unwrap_err().kind(),std::io::ErrorKind::WouldBlock,"A failed decoder must not request a screenshot fallback");
        vec![line]
    });
    (protocol::Connection::new(&base,None,protocol::Credential::bearer("synthetic").unwrap()).unwrap(),peer)
}
fn screen(id:&str)->Target {target(&json!({"requestId":id,"environment":"private","thread":"private-thread","hostId":"local","deviceId":"emulator-synthetic","platform":"android"})).unwrap()}
#[test]
fn failed_decoder_clears_live_flags_reaps_and_never_substitutes_screenshots() {
    let (connection,peer)=hub(vec![0,0,0,1,0x65]);let access=connection.device_hub("/api/device-hub","local").unwrap();let target=screen("private-decoder-failure");let frames=Arc::new(Frames::new(&target.request_id).unwrap());let cancel=Arc::new(AtomicBool::new(false));let (_input,inputs)=mpsc::channel();let slot=decoder::Slot::default();
    let mut command=Command::new("python3");command.args(["-c","import sys;sys.stderr.write('codec fixture init failure\\n');sys.stderr.flush();sys.exit(234)"]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let result=android_view(AndroidView{connection:&connection,access:&access,target:&target,cancel:&cancel,inputs:&inputs,frames:&frames,decoder:&slot},command);
    assert_eq!(result.unwrap_err(),"device_decoder_failed");assert_eq!(frames.revision.load(Ordering::Relaxed),0);assert!(slot.lock().unwrap().is_none());peer.join().unwrap();
}
#[test]
fn real_synthetic_h264_reaches_the_frame_consumer_and_cancellation_reaps() {
    let source=Command::new("ffmpeg").args(["-loglevel","error","-filter_threads","1","-f","lavfi","-i","testsrc=size=480x800:rate=30","-t","2","-pix_fmt","yuv420p","-c:v","libx264","-threads","1","-f","h264","pipe:1"]).output().unwrap();assert!(source.status.success(),"{}",String::from_utf8_lossy(&source.stderr));assert!(!source.stdout.is_empty());
    let (connection,peer)=hub(source.stdout);let access=connection.device_hub("/api/device-hub","local").unwrap();let target=screen("private-decoder-live");let frames=Arc::new(Frames::new(&target.request_id).unwrap());let cancel=Arc::new(AtomicBool::new(false));let (_input,inputs)=mpsc::channel();let slot=decoder::Slot::default();
    let (observer_frames,observer_cancel)=(frames.clone(),cancel.clone());let observer=std::thread::spawn(move||{let deadline=Instant::now()+Duration::from_secs(5);while observer_frames.revision.load(Ordering::Relaxed)==0&&Instant::now()<deadline{std::thread::sleep(Duration::from_millis(5));}observer_cancel.store(true,Ordering::Relaxed);});
    let result=android_view(AndroidView{connection:&connection,access:&access,target:&target,cancel:&cancel,inputs:&inputs,frames:&frames,decoder:&slot},decoder_command());observer.join().unwrap();assert!(result.is_ok());assert!(frames.revision.load(Ordering::Relaxed)>0,"The real decoder must publish an actual JPEG");assert!(slot.lock().unwrap().is_none());peer.join().unwrap();
}
