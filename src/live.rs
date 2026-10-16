//! One cancellable live reader for the visible chat. No socket for hidden panels.
use omarchy_t3_client::*;
use serde_json::json;
use std::{sync::{Arc,mpsc,atomic::{AtomicU64,AtomicBool,Ordering}},time::{Duration,Instant}};
#[derive(Clone)]pub struct Control{generation:Arc<AtomicU64>,pub connected:Arc<AtomicBool>,sender:mpsc::Sender<(u64,Option<(String,String,i64)>)>}
impl Control{
 pub fn new(client:Client)->Self{
  let (sender,receiver)=mpsc::channel();let this=Self{generation:Arc::new(AtomicU64::new(0)),connected:Arc::new(AtomicBool::new(false)),sender};let worker=this.clone();
  std::thread::spawn(move||{let mut pending=None;loop{
   let (generation,target)=match pending.take().or_else(||receiver.recv().ok()){Some(t)=>t,None=>break};let Some((env,thread,serial))=target else{continue};let mut retry=1;let mut rendered=RenderCache::new();
   while worker.generation.load(Ordering::Relaxed)==generation{
    worker.connected.store(false,Ordering::Relaxed);crate::emit("live-state",json!({"serial":serial,"connected":false}));
    let result=(||->Result<()>{
     let e=environments()?.into_iter().find(|e|e.id==env).ok_or("unknown_environment")?;
     let mut path=reqwest::Url::parse(&e.base_url).map_err(|_|"invalid_endpoint")?;path.path_segments_mut().map_err(|_|"invalid_endpoint")?.extend(["api","orchestration","threads",&thread,"bounded"]);
     let mut envelope=client.get(&e,path.path(),true)?;
     if worker.generation.load(Ordering::Relaxed)!=generation{return Ok(())}
     crate::emit("messages",json!({"serial":serial,"result":normalize_messages_cached(&envelope,&mut rendered)?}));
     let c=if let Some(c)=e.connection{c.as_ref().clone()}else{protocol::Connection::new(&e.base_url,None,protocol::Credential::bearer(&environment_token(&e,true)?)?)?};
     let mut sequence=envelope["snapshotSequence"].as_u64().ok_or("missing_sequence")?;let mut dirty=false;let mut emitted=Instant::now();
     c.subscribe("orchestration.subscribeThread",json!({"threadId":thread,"afterSequence":sequence,"requestCompletionMarker":true,"acceptBoundedSnapshot":true}),||worker.generation.load(Ordering::Relaxed)!=generation,|update|{
      if worker.generation.load(Ordering::Relaxed)!=generation{return Ok(())}
      if let Some(item)=update{
       if !worker.connected.swap(true,Ordering::Relaxed){crate::emit("live-state",json!({"serial":serial,"connected":true}));}
       match text(&item,"kind"){
        "snapshot"=>{envelope=item.clone();sequence=item["snapshotSequence"].as_u64().unwrap_or(sequence);dirty=true;},
        "event"=>{let seq=item["sequence"].as_u64().unwrap_or(0);if seq>sequence{dirty|=protocol::projection::apply(&mut envelope["projection"],&item["event"]);sequence=seq;}},
        _=>{}
       }
      }
      if dirty && emitted.elapsed()>=Duration::from_millis(75){crate::emit("messages",json!({"serial":serial,"result":normalize_messages_cached(&envelope,&mut rendered)?}));dirty=false;emitted=Instant::now();}
      Ok(())
     })
    })();
    worker.connected.store(false,Ordering::Relaxed);
    if worker.generation.load(Ordering::Relaxed)!=generation{break}
    crate::emit("live-state",json!({"serial":serial,"connected":false,"error":result.err()}));
    match receiver.recv_timeout(Duration::from_secs(retry)){Ok(next)=>{pending=Some(next);break},Err(mpsc::RecvTimeoutError::Disconnected)=>return,Err(_)=>{retry=(retry*2).min(15);}}
   }
  }});this
 }
 pub fn select(&self,target:Option<(String,String,i64)>){let generation=self.generation.fetch_add(1,Ordering::Relaxed)+1;self.connected.store(false,Ordering::Relaxed);let _=self.sender.send((generation,target));}
 pub fn ready(&self)->bool{self.connected.load(Ordering::Relaxed)}
}
