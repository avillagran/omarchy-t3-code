//! Effect RPC streams. A host worker owns retry/backoff and snapshot cursors.
use super::*;
impl Connection {
 /// `None` flushes a chunk boundary/idle tick. Cancel is checked at most100ms after a read.
 pub fn subscribe(&self,tag:&str,payload:Value,cancel:impl Fn()->bool,mut receive:impl FnMut(Option<Value>)->Result<()>)->Result<()>{
  if cancel(){return Ok(())}
  let limit=if tag=="orchestration.subscribeThread" && payload["acceptBoundedSnapshot"]==true {THREAD_SNAPSHOT_MAX_BYTES}else{MAX_BYTES};
  let mut ws=self.websocket_with_limit(limit)?;
  if cancel(){let _=ws.close(None);return Ok(())}
  match ws.get_mut(){tungstenite::stream::MaybeTlsStream::Plain(s)=>s.set_read_timeout(Some(Duration::from_millis(100))),tungstenite::stream::MaybeTlsStream::Rustls(s)=>s.sock.set_read_timeout(Some(Duration::from_millis(100))),_=>return Err("unsupported_tls".into())}.map_err(|_|"disconnected")?;
  ws.send(Message::Text(json!({"_tag":"Request","id":"1","tag":tag,"payload":payload,"headers":[]}).to_string().into())).map_err(|_|"disconnected")?;
  let mut ping=Instant::now();let mut seen=Instant::now();
  while !cancel(){
   match ws.read(){
    Ok(Message::Text(t))=>{seen=Instant::now();let v:Value=serde_json::from_str(&t).map_err(|_|"malformed_stream")?;
     match v["_tag"].as_str().unwrap_or(""){
      "Chunk" if v["requestId"]=="1"=>{for item in v["values"].as_array().ok_or("malformed_stream")?{receive(Some(item.clone()))?;}receive(None)?;ws.send(Message::Text(json!({"_tag":"Ack","requestId":"1"}).to_string().into())).map_err(|_|"disconnected")?;},
      "Exit" if v["requestId"]=="1"=>return Err("stream_closed".into()),
      "Ping"=>ws.send(Message::Text(json!({"_tag":"Pong"}).to_string().into())).map_err(|_|"disconnected")?,
      _=>{}
     }
    },
    Ok(Message::Ping(p))=>ws.send(Message::Pong(p)).map_err(|_|"disconnected")?,
    Ok(Message::Close(_))=>return Err("disconnected".into()),
    Ok(_)=>{},
    Err(tungstenite::Error::Io(e)) if matches!(e.kind(),std::io::ErrorKind::WouldBlock|std::io::ErrorKind::TimedOut)=>receive(None)?,
    Err(_)=>return Err("disconnected".into())
   }
   if ping.elapsed()>=Duration::from_secs(15){ws.send(Message::Text(json!({"_tag":"Ping"}).to_string().into())).map_err(|_|"disconnected")?;ping=Instant::now();}
   if seen.elapsed()>Duration::from_secs(60){return Err("stream_timeout".into())}
  }
  let _=ws.send(Message::Text(json!({"_tag":"Interrupt","requestId":"1"}).to_string().into()));let _=ws.close(None);Ok(())
 }
}
