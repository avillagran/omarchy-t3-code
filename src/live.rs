//! One cancellable live reader for the visible chat. No socket for hidden panels.
//! A reconnect to the same chat resumes after the last applied sequence with the
//! envelope already held; the bounded HTTP snapshot is read again only when the
//! stream failed for a reason other than transport.
use omarchy_t3_client::*;
use serde_json::{json,Value};
use std::{sync::{Arc,mpsc,atomic::{AtomicU64,AtomicBool,Ordering}},time::{Duration,Instant}};
#[derive(Clone)]pub struct Control{generation:Arc<AtomicU64>,pub connected:Arc<AtomicBool>,sender:mpsc::Sender<(u64,Option<(String,String,i64)>)>}
impl Control{
 pub fn new(client:Client)->Self{
  let (sender,receiver)=mpsc::channel();let this=Self{generation:Arc::new(AtomicU64::new(0)),connected:Arc::new(AtomicBool::new(false)),sender};let worker=this.clone();
  std::thread::spawn(move||{let mut pending=None;loop{
   let (generation,target)=match pending.take().or_else(||receiver.recv().ok()){Some(t)=>t,None=>break};let Some((env,thread,serial))=target else{continue};let mut retry=1;let mut rendered=RenderCache::new();
   // Survives reconnects of this target: the envelope and the sequence it reflects.
   let mut held:Option<Value>=None;let mut sequence=0u64;
   while worker.generation.load(Ordering::Relaxed)==generation{
    worker.connected.store(false,Ordering::Relaxed);crate::emit("live-state",json!({"serial":serial,"connected":false}));
    let result=(||->Result<()>{
     let e=environments()?.into_iter().find(|e|e.id==env).ok_or("unknown_environment")?;
     if held.is_none(){
      let fetched=client.bounded_thread(&e,&thread)?;
      if worker.generation.load(Ordering::Relaxed)!=generation{return Ok(())}
      crate::emit("messages",json!({"serial":serial,"result":normalize_messages_cached(&fetched,&mut rendered)?}));
      sequence=fetched["snapshotSequence"].as_u64().ok_or("missing_sequence")?;held=Some(fetched);
     }
     let envelope=held.as_mut().ok_or("missing_snapshot")?;
     let c=if let Some(c)=e.connection{c.as_ref().clone()}else{protocol::Connection::new(&e.base_url,None,protocol::Credential::bearer(&environment_token(&e,true)?)?)?};
     let marker=server_capabilities::get(&env,&c)?.thread_completion_marker;
     let mut payload=json!({"threadId":thread,"afterSequence":sequence,"acceptBoundedSnapshot":true,"acceptCompactTurnItems":true});
     if marker{payload["requestCompletionMarker"]=json!(true);}
     let mut dirty=false;let mut emitted=Instant::now();
     c.subscribe("orchestration.subscribeThread",payload,||worker.generation.load(Ordering::Relaxed)!=generation,|update|{
      if worker.generation.load(Ordering::Relaxed)!=generation{return Ok(())}
      // Without a completion marker the server gives no catch-up signal; like T3, the
      // resumed stream counts as live once subscribed.
      if (update.is_some()||!marker) && !worker.connected.swap(true,Ordering::Relaxed){retry=1;crate::emit("live-state",json!({"serial":serial,"connected":true}));}
      if let Some(item)=update{
       match text(&item,"kind"){
        "snapshot"=>{*envelope=bounded_snapshot::restore(item.clone())?;sequence=item["snapshotSequence"].as_u64().unwrap_or(sequence);dirty=true;},
        "event"=>{let seq=item["sequence"].as_u64().unwrap_or(0);if seq>sequence{dirty|=protocol::projection::apply(&mut envelope["projection"],&item["event"]);sequence=seq;}},
        // Events this build cannot decode still move the resume cursor (T3 contracts).
        "unknown-event"=>{sequence=sequence.max(item["sequence"].as_u64().unwrap_or(0));},
        _=>{}
       }
      }
      if dirty && emitted.elapsed()>=Duration::from_millis(75){crate::emit("messages",json!({"serial":serial,"result":normalize_messages_cached(envelope,&mut rendered)?}));dirty=false;emitted=Instant::now();}
      Ok(())
     })?;
     // Flush what arrived inside the last throttle window before the stream ended.
     if dirty{crate::emit("messages",json!({"serial":serial,"result":normalize_messages_cached(envelope,&mut rendered)?}));}
     Ok(())
    })();
    worker.connected.store(false,Ordering::Relaxed);
    if worker.generation.load(Ordering::Relaxed)!=generation{break}
    if let Err(error)=&result{if !reconnect::transient(error){held=None;server_capabilities::invalidate(&env);}}
    crate::emit("live-state",json!({"serial":serial,"connected":false,"error":result.err()}));
    match receiver.recv_timeout(Duration::from_secs(retry)){Ok(next)=>{pending=Some(next);break},Err(mpsc::RecvTimeoutError::Disconnected)=>return,Err(_)=>{retry=(retry*2).min(15);}}
   }
  }});this
 }
 pub fn select(&self,target:Option<(String,String,i64)>){let generation=self.generation.fetch_add(1,Ordering::Relaxed)+1;self.connected.store(false,Ordering::Relaxed);let _=self.sender.send((generation,target));}
 pub fn ready(&self)->bool{self.connected.load(Ordering::Relaxed)}
}
