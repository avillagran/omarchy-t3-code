//! One cancellable live reader for the visible chat. No socket for hidden panels.
//! A reconnect to the same chat resumes after the last applied sequence with the
//! envelope already held; the bounded HTTP snapshot is read again only when the
//! stream failed for a reason other than transport. Retries follow `reconnect`.
use omarchy_t3_client::*;
use serde_json::{json,Value};
use std::{sync::{Arc,mpsc,atomic::{AtomicU64,AtomicBool,Ordering}},time::{Duration,Instant}};
#[derive(Clone)]pub struct Control{generation:Arc<AtomicU64>,pub connected:Arc<AtomicBool>,sender:mpsc::Sender<(u64,Option<(String,String,i64)>)>}
impl Control{
 pub fn new(client:Client)->Self{
  let (sender,receiver)=mpsc::channel();let this=Self{generation:Arc::new(AtomicU64::new(0)),connected:Arc::new(AtomicBool::new(false)),sender};let worker=this.clone();
  std::thread::spawn(move||{let mut pending=None;loop{
   let (generation,target)=match pending.take().or_else(||receiver.recv().ok()){Some(t)=>t,None=>break};let Some((env,thread,serial))=target else{continue};let mut backoff=reconnect::Backoff::default();let mut waiter=reconnect::Waiter::default();let mut woke=false;let mut rendered=RenderCache::new();
   // Survives reconnects of this target: the envelope and the sequence it reflects.
   let mut held:Option<Value>=None;let mut sequence=0u64;
   'target: while worker.generation.load(Ordering::Relaxed)==generation{
    worker.connected.store(false,Ordering::Relaxed);crate::emit("live-state",json!({"serial":serial,"connected":false}));
    let mut probed=false;
    let result=(||->Result<()>{
     let e=environments()?.into_iter().find(|e|e.id==env).ok_or("unknown_environment")?;
     // After a resume or network change, check the server answers before reconnecting.
     if woke && !reconnect::reachable(&e){return Err("unreachable".into())}
     if held.is_none(){
      let fetched=client.bounded_thread(&e,&thread)?;
      if worker.generation.load(Ordering::Relaxed)!=generation{return Ok(())}
      crate::emit("messages",json!({"serial":serial,"result":normalize_messages_cached(&fetched,&mut rendered)?}));
      sequence=fetched["snapshotSequence"].as_u64().ok_or("missing_sequence")?;held=Some(fetched);
     }
     let envelope=held.as_mut().ok_or("missing_snapshot")?;
     let c=e.connection(true)?;
     let marker=server_capabilities::get(&env,&c)?.thread_completion_marker;
     let mut payload=json!({"threadId":thread,"afterSequence":sequence,"acceptBoundedSnapshot":true,"acceptCompactTurnItems":true});
     if marker{payload["requestCompletionMarker"]=json!(true);}
     let mut dirty=false;let mut emitted=Instant::now();
     // A wakeup checks the open socket instead of trusting it.
     let streamed=c.subscribe_probed("orchestration.subscribeThread",payload,||worker.generation.load(Ordering::Relaxed)!=generation,||{let w=waiter.take();probed|=w;w},|update|{
      if worker.generation.load(Ordering::Relaxed)!=generation{return Ok(())}
      // Without a completion marker the server gives no catch-up signal; like T3, the
      // resumed stream counts as live once subscribed.
      if (update.is_some()||!marker) && !worker.connected.swap(true,Ordering::Relaxed){backoff.connected();crate::emit("live-state",json!({"serial":serial,"connected":true}));}
      let boundary=update.is_none();
      if let Some(item)=update{
       match text(&item,"kind"){
        "snapshot"=>{*envelope=bounded_snapshot::restore(item.clone())?;sequence=item["snapshotSequence"].as_u64().unwrap_or(sequence);dirty=true;},
        "event"=>{let seq=item["sequence"].as_u64().unwrap_or(0);if seq>sequence{dirty|=protocol::projection::apply(&mut envelope["projection"],&item["event"]);sequence=seq;}},
        // Events this build cannot decode still move the resume cursor (T3 contracts).
        "unknown-event"=>{sequence=sequence.max(item["sequence"].as_u64().unwrap_or(0));},
        _=>{}
       }
      }
      // Only at chunk boundaries: a resumed replay never shows a half-applied state.
      if dirty && boundary && emitted.elapsed()>=Duration::from_millis(75){crate::emit("messages",json!({"serial":serial,"result":normalize_messages_cached(envelope,&mut rendered)?}));dirty=false;emitted=Instant::now();}
      Ok(())
     });
     // Flush what arrived inside the last throttle window before the stream dropped.
     if dirty && worker.generation.load(Ordering::Relaxed)==generation{crate::emit("messages",json!({"serial":serial,"result":normalize_messages_cached(envelope,&mut rendered)?}));}
     streamed
    })();
    worker.connected.store(false,Ordering::Relaxed);
    if worker.generation.load(Ordering::Relaxed)!=generation{break}
    if let Err(error)=&result{if !reconnect::transient(error){held=None;server_capabilities::invalidate(&env);}}
    crate::emit("live-state",json!({"serial":serial,"connected":false,"error":result.err()}));
    // Wait out the backoff, unless another chat is selected or the system wakes up;
    // a socket that failed its wakeup probe reconnects at once.
    woke=probed;let until=Instant::now()+backoff.next_delay();
    while !woke{
     let Some(left)=until.checked_duration_since(Instant::now()) else{break};
     match receiver.recv_timeout(left.min(Duration::from_millis(250))){Ok(next)=>{pending=Some(next);break 'target},Err(mpsc::RecvTimeoutError::Disconnected)=>return,Err(_)=>{woke=waiter.take();}}
    }
    if woke{backoff.reset();}
   }
  }});this
 }
 pub fn select(&self,target:Option<(String,String,i64)>){let generation=self.generation.fetch_add(1,Ordering::Relaxed)+1;self.connected.store(false,Ordering::Relaxed);let _=self.sender.send((generation,target));}
 pub fn ready(&self)->bool{self.connected.load(Ordering::Relaxed)}
}
