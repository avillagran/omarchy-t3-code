//! On-demand offscreen Qt frontend transport. Rust owns lifetime, private frames,
//! scoped reads and cancellation. No browser library enters the shell or daemon.
use omarchy_t3_client::*;
use serde_json::{json,Value};
use std::os::unix::process::CommandExt;
use std::{fs,io::{BufRead,BufReader,Write},os::unix::fs::PermissionsExt,process::{Child,ChildStdin,Command,Stdio},sync::{mpsc,Arc,atomic::{AtomicU64,Ordering}},thread};
#[derive(Clone)]pub struct Manager{tx:mpsc::Sender<Value>,generation:Arc<AtomicU64>}
impl Manager{
 pub fn send(&self,mut input:Value){if matches!(text(&input,"op"),"document-open"|"document-close"){self.generation.fetch_add(1,Ordering::SeqCst);}input["generation"]=json!(self.generation.load(Ordering::SeqCst));let _=self.tx.send(input);}
}
struct Session{child:Child,stdin:ChildStdin,dir:std::path::PathBuf,id:String}
impl Drop for Session{fn drop(&mut self){let _=writeln!(self.stdin,"{{\"type\":\"close\"}}");let until=std::time::Instant::now()+std::time::Duration::from_millis(1500);while std::time::Instant::now()<until{if self.child.try_wait().ok().flatten().is_some(){break}thread::sleep(std::time::Duration::from_millis(10));}if self.child.try_wait().ok().flatten().is_none(){unsafe{libc::kill(-(self.child.id() as i32),libc::SIGKILL);}let _=self.child.kill();}let _=self.child.wait();let _=fs::remove_dir_all(&self.dir);}}
fn launch(exe:&std::path::Path,doc:&Value,id:&str,generation:u64,current:Arc<AtomicU64>)->Result<Session>{
 let root=exe.parent().ok_or("viewer_unavailable")?;
 let qml=doc["qmlPreview"]==true;
 if !root.join(if qml{"libt3-qml-preview.so"}else{"libt3-document.so"}).is_file(){return Err("viewer_unavailable".into())}
 // QML previews write uncompressed frames: keep them in memory (tmpfs), not on disk.
 let base=if qml{std::env::var_os("XDG_RUNTIME_DIR").map(std::path::PathBuf::from).filter(|p|p.is_absolute()&&p.is_dir()).map(|p|p.join("omarchy-t3-code")).unwrap_or_else(||xdg("XDG_CACHE_HOME",".cache"))}else{xdg("XDG_CACHE_HOME",".cache")};
 let dir=base.join("document-sessions").join(uuid::Uuid::new_v4().to_string());fs::create_dir_all(&dir).map_err(|_|"viewer_unavailable")?;fs::set_permissions(&dir,fs::Permissions::from_mode(0o700)).map_err(|_|"viewer_unavailable")?;
 let mut payload=doc.clone();payload["offscreen"]=json!(true);payload["frameDirectory"]=json!(dir);payload["width"]=json!(900);payload["height"]=json!(620);
 let mut command=if qml{qml_preview::command(exe,&dir)}else{let mut c=Command::new(exe);c.arg("document-render").process_group(0).env("QT_QPA_PLATFORM","offscreen").env("QT_QUICK_BACKEND","software").env("QT_SCALE_FACTOR","1").env("QT_SCREEN_SCALE_FACTORS","1").env("QTWEBENGINE_CHROMIUM_FLAGS","--disable-gpu").env_remove("T3_DOCUMENT_VERIFY").env_remove("T3_DOCUMENT_GRAB");c};
 let mut child=command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().map_err(|_|"viewer_unavailable")?;
 let stdin=child.stdin.take().ok_or("viewer_unavailable")?;let stdout=child.stdout.take().ok_or("viewer_unavailable")?;
 let mut session=Session{child,stdin,dir:dir.clone(),id:id.into()};let bytes=payload.to_string().into_bytes();session.stdin.write_all(&(bytes.len() as u64).to_be_bytes()).and_then(|_|session.stdin.write_all(&bytes)).map_err(|_|"viewer_unavailable")?;
 let internal=doc["internalReview"]==true;let id=id.to_string();thread::spawn(move||{
  // A renderer may say why it is about to exit (QML previews: stuck in a loop).
  let mut reason="viewer_closed".to_string();
  for line in BufReader::new(stdout).lines(){let Ok(line)=line else{break};if current.load(Ordering::SeqCst)!=generation{break}let Ok(v)=serde_json::from_str::<Value>(&line)else{continue};if let Some(e)=v["error"].as_str().filter(|e|e.starts_with("qml_preview_")&&e.len()<64){reason=e.to_string();continue}if internal&&v["reviewReady"]==true{super::emit("document-ready",json!({"requestId":id,"error":text(&v,"reviewError")}));continue}if v["appRequest"].is_object(){super::emit("document-app-request",json!({"requestId":id,"request":v["appRequest"]}));continue}if let Some(value)=v["clipboard"].as_str().filter(|s|s.len()<=120000){super::emit("document-clipboard",json!({"requestId":id,"text":value}));continue}if let Some(height)=v["contentHeight"].as_u64().filter(|h|*h>0&&*h<=100000){super::emit("document-size",json!({"requestId":id,"height":height}));continue}let name=text(&v,"frame");if !matches!(name,"frame-0.png"|"frame-1.png")&&!(qml&&matches!(name,"frame-0.bmp"|"frame-1.bmp")){continue}let path=dir.join(name);if !fs::metadata(&path).is_ok_and(|m|m.is_file()&&m.len()<=20*1024*1024){continue}let Ok(url)=Url::from_file_path(path)else{continue};super::emit("document-frame",json!({"requestId":id,"url":url.to_string(),"revision":v["revision"],"width":v["width"],"height":v["height"]}));
  }
  if current.load(Ordering::SeqCst)==generation{super::emit("document-frame",json!({"requestId":id,"error":reason}));}
 });Ok(session)
}
pub fn start()->Manager{start_with(std::env::current_exe().expect("current executable"))}
pub fn start_with(exe:std::path::PathBuf)->Manager{
 let(tx,rx)=mpsc::channel::<Value>();let generation=Arc::new(AtomicU64::new(0));let current=generation.clone();
 thread::spawn(move||{let mut session:Option<Session>=None;
  for input in rx{let stamp=input["generation"].as_u64().unwrap_or(0);if stamp!=current.load(Ordering::SeqCst){continue}
   match text(&input,"op"){
    "document-open"=>{
     session=None;let result=write::execute(&input);if stamp!=current.load(Ordering::SeqCst){continue}
     let result=result.and_then(|mut doc|{if doc["html"]==true{session=Some(launch(&exe,&doc,text(&input,"requestId"),stamp,current.clone())?)}if doc["internalReview"]==true||doc["generated"]==true{doc["contents"]=json!("");}Ok(doc)});
     super::emit("action",json!({"op":"document-open","requestId":input["requestId"],"result":result.as_ref().ok(),"error":result.as_ref().err()}));
    },
    "document-input"=>{if let Some(s)=session.as_mut(){if s.id==text(&input,"requestId"){let v=&input["event"];if (v.to_string().len()<=8192||text(v,"type")=="app-response"&&v.to_string().len()<=2*1024*1024)&&matches!(text(v,"type"),"app-response"|"press"|"release"|"move"|"double"|"wheel"|"key"|"keyup"|"resize"|"zoom"|"capture"|"review-options"){let _=writeln!(s.stdin,"{v}");}}}},
    "document-close"=>{session=None;},_=>{}
   }
  }
 });Manager{tx,generation}
}

/// Each visible inline page has an independent renderer; modal previews keep
/// their own slot. Leaving the viewport cancels pending reads and frees Qt.
/// QML previews (Qt Quick only, 30-70 MB each) have their own, larger budget
/// than WebEngine pages, so a message with several QML blocks still renders.
pub struct Pool{modal:Manager,modal_id:String,review:Manager,review_id:String,inline:std::collections::HashMap<String,Manager>,qml:std::collections::HashSet<String>}
impl Pool{
 pub fn new()->Self{Self{modal:start(),modal_id:String::new(),review:start(),review_id:String::new(),inline:std::collections::HashMap::new(),qml:std::collections::HashSet::new()}}
 pub fn send(&mut self,input:Value){
  let id=text(&input,"requestId").to_string();
  match text(&input,"op"){
   "document-open" if input["reviewSnapshotId"].is_string()=>{self.review_id=id;self.review.send(input)},
   "document-open" if input["inline"]==true=>{
    let qml=input["attachment"]["generated"]=="qml";let used=self.inline.keys().filter(|k|self.qml.contains(*k)==qml).count();
    if !self.inline.contains_key(&id)&&used>=if qml{8}else{4}{super::emit("action",json!({"op":"document-open","requestId":id,"error":"viewer_unavailable"}));return}
    if qml{self.qml.insert(id.clone());}
    self.inline.entry(id).or_insert_with(start).send(input);
   },
   "document-open"=>{self.modal_id=id;self.modal.send(input)},
   "document-close" if id.is_empty()=>{self.modal_id.clear();self.review_id.clear();self.qml.clear();self.review.send(json!({"op":"document-close"}));for(_,manager)in self.inline.drain(){manager.send(json!({"op":"document-close"}));}self.modal.send(input)},
   "document-close" if id==self.review_id=>{self.review_id.clear();self.review.send(input)},
   "document-close"=>{self.qml.remove(&id);if let Some(manager)=self.inline.remove(&id){manager.send(input)}else if id==self.modal_id{self.modal_id.clear();self.modal.send(input)}},
   _ if id==self.review_id=>self.review.send(input),
   _=>{if let Some(manager)=self.inline.get(&id){manager.send(input)}else if id==self.modal_id{self.modal.send(input)}},
  }
 }
}
