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
 if !root.join("libt3-document.so").is_file(){return Err("viewer_unavailable".into())}
 let dir=xdg("XDG_CACHE_HOME",".cache").join("document-sessions").join(uuid::Uuid::new_v4().to_string());fs::create_dir_all(&dir).map_err(|_|"viewer_unavailable")?;fs::set_permissions(&dir,fs::Permissions::from_mode(0o700)).map_err(|_|"viewer_unavailable")?;
 let mut payload=doc.clone();payload["offscreen"]=json!(true);payload["frameDirectory"]=json!(dir);payload["width"]=json!(900);payload["height"]=json!(620);
 let mut child=Command::new(exe).arg("document-render").env("QT_QPA_PLATFORM","offscreen").process_group(0).env("QT_QUICK_BACKEND","software").env("QT_SCALE_FACTOR","1").env("QT_SCREEN_SCALE_FACTORS","1").env("QTWEBENGINE_CHROMIUM_FLAGS","--disable-gpu").env_remove("T3_DOCUMENT_VERIFY").env_remove("T3_DOCUMENT_GRAB").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().map_err(|_|"viewer_unavailable")?;
 let stdin=child.stdin.take().ok_or("viewer_unavailable")?;let stdout=child.stdout.take().ok_or("viewer_unavailable")?;
 let mut session=Session{child,stdin,dir:dir.clone(),id:id.into()};let bytes=payload.to_string().into_bytes();session.stdin.write_all(&(bytes.len() as u64).to_be_bytes()).and_then(|_|session.stdin.write_all(&bytes)).map_err(|_|"viewer_unavailable")?;
 let id=id.to_string();thread::spawn(move||{
  for line in BufReader::new(stdout).lines(){let Ok(line)=line else{break};if current.load(Ordering::SeqCst)!=generation{break}let Ok(v)=serde_json::from_str::<Value>(&line)else{continue};if let Some(value)=v["clipboard"].as_str().filter(|s|s.len()<=120000){super::emit("document-clipboard",json!({"requestId":id,"text":value}));continue}let name=text(&v,"frame");if !matches!(name,"frame-0.png"|"frame-1.png"){continue}let path=dir.join(name);if !fs::metadata(&path).is_ok_and(|m|m.is_file()&&m.len()<=20*1024*1024){continue}let Ok(url)=reqwest::Url::from_file_path(path)else{continue};super::emit("document-frame",json!({"requestId":id,"url":url.to_string(),"revision":v["revision"],"width":v["width"],"height":v["height"]}));
  }
  if current.load(Ordering::SeqCst)==generation{super::emit("document-frame",json!({"requestId":id,"error":"viewer_closed"}));}
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
     let result=result.and_then(|doc|{if doc["html"]==true{session=Some(launch(&exe,&doc,text(&input,"requestId"),stamp,current.clone())?)}Ok(doc)});
     super::emit("action",json!({"op":"document-open","requestId":input["requestId"],"result":result.as_ref().ok(),"error":result.as_ref().err()}));
    },
    "document-input"=>{if let Some(s)=session.as_mut(){if s.id==text(&input,"requestId"){let v=&input["event"];if v.to_string().len()<=8192&&matches!(text(v,"type"),"press"|"release"|"move"|"double"|"wheel"|"key"|"keyup"|"resize"|"zoom"|"capture"){let _=writeln!(s.stdin,"{v}");}}}},
    "document-close"=>{session=None;},_=>{}
   }
  }
 });Manager{tx,generation}
}
