//! Effect RPC streams. A host worker owns retry/backoff and snapshot cursors.
use super::*;
/// Same liveness budget as T3's client runtime (rpc/protocol.ts): a ping every 5 s and a
/// socket dropped after 15-20 s without any frame, so a suspended host or a changed network
/// is detected within seconds instead of a minute.
pub const STREAM_PING_INTERVAL:Duration=Duration::from_secs(5);
pub const STREAM_IDLE_TIMEOUT:Duration=Duration::from_secs(20);
pub const STREAM_PROBE_TIMEOUT:Duration=Duration::from_secs(3);
/// Largest single frame (and so item) a subscription accepts: 64 MiB only for a thread
/// subscription that asked for bounded snapshots, the generic 8 MiB otherwise.
pub(crate) fn item_limit(tag:&str,payload:&Value)->u64{
 if tag=="orchestration.subscribeThread" && payload["acceptBoundedSnapshot"]==true {THREAD_SNAPSHOT_MAX_BYTES}else{MAX_BYTES}
}
/// Where `item` sits in its stream's sequence: `snapshotSequence` of a shell or
/// thread `snapshot`, else the item's `sequence` (also for thread events this
/// build cannot decode, which still move the cursor). Shell and thread resume
/// from it with `afterSequence`; terminal and worktree setup items carry it only
/// to drop replays. `None` for `synchronized` and for streams without one.
pub fn cursor_of(tag:&str,item:&Value)->Option<u64>{
 match tag{
  "orchestration.subscribeShell" if item["kind"]=="snapshot"=>item["snapshot"]["snapshotSequence"].as_u64(),
  "orchestration.subscribeThread" if item["kind"]=="snapshot"=>item["snapshotSequence"].as_u64(),
  "orchestration.subscribeShell"|"orchestration.subscribeThread"|"subscribeWorktreeSetup"=>item["sequence"].as_u64(),
  "terminal.attach"=>match item["type"].as_str(){
   Some("snapshot"|"started"|"restarted")=>item["snapshot"]["sequence"].as_u64().or(item["sequence"].as_u64()),
   _=>item["sequence"].as_u64(),
  },
  _=>None,
 }
}
/// The cursor once `item` is applied on top of `cursor`: an item that replaces the
/// host's state sets it (as ot3 does on a thread snapshot); any other only moves it on.
pub fn advance(tag:&str,cursor:Option<u64>,item:&Value)->Option<u64>{
 let at=cursor_of(tag,item);
 if start_of(tag,item)==Some(Start::Snapshot){at.or(cursor)}else{cursor.max(at)}
}
/// What the first item of a subscription means for the state the host holds.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Start{
 /// It replaces that state: a `snapshot`, a `replace` frame, or a stream whose every item is the whole state.
 Snapshot,
 /// The server resumed from `afterSequence`: apply the items on top of the state held at the cursor.
 Resume,
 /// The stream only carries events from now on; there is no state to replace.
 Events,
}
impl Start{
 pub fn as_str(self)->&'static str{match self{Start::Snapshot=>"snapshot",Start::Resume=>"resume",Start::Events=>"events"}}
}
/// `Start` for `item` as the first of a `tag` subscription; for `Snapshot`, also what
/// the item does whenever it arrives. `None` for a tag the typed API does not know.
pub fn start_of(tag:&str,item:&Value)->Option<Start>{
 let replaced=match tag{
  "orchestration.subscribeShell"|"orchestration.subscribeThread"=>item["kind"]=="snapshot",
  "orchestration.subscribeCommandOutput"=>item["kind"]=="replace",
  "terminal.attach"=>matches!(item["type"].as_str(),Some("snapshot"|"started"|"restarted")),
  "subscribeServerConfig"=>item["type"]=="snapshot",
  "subscribeDeviceState"|"subscribeDiscoveredLocalServers"|"subscribeWorktreeSetup"|"server.getStorageCleanupReport"|"provider.install.subscribe"|"provider.auth.subscribe"=>true,
  "subscribePreviewEvents"|"git.runStackedAction"=>return Some(Start::Events),
  _=>return None,
 };
 Some(if replaced{Start::Snapshot}else{Start::Resume})
}
/// A stream read as a request with a deadline: its first item `accept` takes, or the
/// last item seen when `timeout` runs out or the server ends the stream first.
/// `Ok(None)` when nothing arrived in time; an error only if none did before it.
/// A JSON `null` item (no storage report yet) is `Some(Value::Null)`.
pub fn first_item(conn:&Connection,spec:&crate::api::streams::StreamSpec,timeout:Duration,mut accept:impl FnMut(&Value)->bool)->Result<Option<Value>>{
 let started=Instant::now();let done=std::cell::Cell::new(false);let mut last=None;
 let result=spec.subscribe(conn,||done.get()||started.elapsed()>=timeout,||false,|item|{
  if let Some(v)=item{if accept(&v){done.set(true)}last=Some(v);}
  Ok(())
 });
 match (last,result){(Some(v),_)=>Ok(Some(v)),(None,Err(e))=>Err(e),(None,Ok(()))=>Ok(None)}
}
/// What a subscription worker sees, one level finer than `subscribe`'s `Option<Value>`
/// (where both `ChunkEnd` and `Idle` arrive as `None`).
pub(crate) enum Tick {
 Item(Value),
 /// The server's Chunk frame ended after the items just delivered.
 ChunkEnd,
 /// A 100 ms read tick completed no frame: everything received so far was delivered
 /// (a large frame may still be arriving).
 Idle,
}
impl Connection {
 /// `None` flushes a chunk boundary/idle tick. Cancel is checked at most100ms after a read.
 pub fn subscribe(&self,tag:&str,payload:Value,cancel:impl Fn()->bool,receive:impl FnMut(Option<Value>)->Result<()>)->Result<()>{self.subscribe_probed(tag,payload,cancel,||false,receive)}
 /// `probe` returning true (e.g. after a resume from suspend) pings at once and drops the
 /// socket unless a frame arrives within STREAM_PROBE_TIMEOUT.
 pub fn subscribe_probed(&self,tag:&str,payload:Value,cancel:impl Fn()->bool,mut probe:impl FnMut()->bool,mut receive:impl FnMut(Option<Value>)->Result<()>)->Result<()>{
  self.subscribe_ticks(tag,payload,cancel,||probe().then(Instant::now),|tick|match tick{Tick::Item(item)=>receive(Some(item)),Tick::ChunkEnd|Tick::Idle=>receive(None)})
 }
 /// `subscribe_probed` telling chunk ends and idle ticks apart, for the foreign-host inbox.
 /// `probe` returns when a probe was requested: the STREAM_PROBE_TIMEOUT window starts
 /// there (or when the socket opened, if later), not when this loop noticed it.
 pub(crate) fn subscribe_ticks(&self,tag:&str,payload:Value,cancel:impl Fn()->bool,mut probe:impl FnMut()->Option<Instant>,mut receive:impl FnMut(Tick)->Result<()>)->Result<()>{
  if cancel(){return Ok(())}
  let mut ws=self.websocket_with_limit(item_limit(tag,&payload))?;
  if cancel(){let _=ws.close(None);return Ok(())}
  match ws.get_mut(){tungstenite::stream::MaybeTlsStream::Plain(s)=>s.set_read_timeout(Some(Duration::from_millis(100))),tungstenite::stream::MaybeTlsStream::Rustls(s)=>s.sock.set_read_timeout(Some(Duration::from_millis(100))),_=>return Err("unsupported_tls".into())}.map_err(|_|"disconnected")?;
  ws.send(Message::Text(json!({"_tag":"Request","id":"1","tag":tag,"payload":payload,"headers":[]}).to_string().into())).map_err(|_|"disconnected")?;
  let opened=Instant::now();let mut ping=opened;let mut seen=opened;let mut probing:Option<Instant>=None;
  while !cancel(){
   match ws.read(){
    Ok(Message::Text(t))=>{seen=Instant::now();probing=None;let mut v:Value=serde_json::from_str(&t).map_err(|_|"malformed_stream")?;drop(t);
     let kind=v["_tag"].as_str().unwrap_or("").to_owned();
     match kind.as_str(){
      "Chunk" if v["requestId"]=="1"=>{let Value::Array(values)=v["values"].take() else {return Err("malformed_stream".into())};for item in values{receive(Tick::Item(item))?;}receive(Tick::ChunkEnd)?;ws.send(Message::Text(json!({"_tag":"Ack","requestId":"1"}).to_string().into())).map_err(|_|"disconnected")?;},
      "Exit" if v["requestId"]=="1"=>return Err("stream_closed".into()),
      "Ping"=>ws.send(Message::Text(json!({"_tag":"Pong"}).to_string().into())).map_err(|_|"disconnected")?,
      _=>{}
     }
    },
    Ok(Message::Ping(p))=>ws.send(Message::Pong(p)).map_err(|_|"disconnected")?,
    Ok(Message::Close(_))=>return Err("disconnected".into()),
    Ok(_)=>{},
    Err(tungstenite::Error::Io(e)) if matches!(e.kind(),std::io::ErrorKind::WouldBlock|std::io::ErrorKind::TimedOut)=>receive(Tick::Idle)?,
    Err(_)=>return Err("disconnected".into())
   }
   if let Some(asked)=probe(){ws.send(Message::Text(json!({"_tag":"Ping"}).to_string().into())).map_err(|_|"disconnected")?;ping=Instant::now();probing.get_or_insert(asked.max(opened));}
   if probing.is_some_and(|at|at.elapsed()>STREAM_PROBE_TIMEOUT){return Err("stream_timeout".into())}
   if ping.elapsed()>=STREAM_PING_INTERVAL{ws.send(Message::Text(json!({"_tag":"Ping"}).to_string().into())).map_err(|_|"disconnected")?;ping=Instant::now();}
   if seen.elapsed()>STREAM_IDLE_TIMEOUT{return Err("stream_timeout".into())}
  }
  let _=ws.send(Message::Text(json!({"_tag":"Interrupt","requestId":"1"}).to_string().into()));let _=ws.close(None);Ok(())
 }
}
