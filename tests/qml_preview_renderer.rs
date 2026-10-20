//! The QML preview renderer process itself: limits, idle cost and frame rate.
//! Needs `tools/build-document-viewer` and a built client in bin/; skipped otherwise.
use omarchy_t3_client::qml_preview;
use serde_json::{json,Value};
use std::{io::{BufRead,BufReader,Write},process::{Child,ChildStdin,Stdio},sync::mpsc,time::{Duration,Instant}};

struct Renderer{child:Child,stdin:Option<ChildStdin>,lines:mpsc::Receiver<(Instant,String)>,frames:std::path::PathBuf,started:Instant}
impl Renderer{
 fn exe()->Option<std::path::PathBuf>{
  let root=std::path::Path::new(env!("CARGO_MANIFEST_DIR"));let exe=root.join("bin/omarchy-t3-client");
  (exe.is_file()&&root.join("bin/libt3-qml-preview.so").is_file()).then_some(exe)
 }
 fn start(name:&str,source:&str)->Option<Self>{
  let Some(exe)=Self::exe()else{eprintln!("QML renderer not built; skipped");return None};
  let frames=std::env::temp_dir().join(format!("ot3-qml-{name}-{}",std::process::id()));let _=std::fs::remove_dir_all(&frames);std::fs::create_dir_all(&frames).unwrap();
  let mut child=qml_preview::command(&exe,&frames).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().unwrap();
  let payload=json!({"qmlPreview":true,"contents":source,"theme":{"appearance":"dark"},"offscreen":true,"frameDirectory":frames,"width":730,"height":330}).to_string().into_bytes();
  let mut stdin=child.stdin.take().unwrap();stdin.write_all(&(payload.len() as u64).to_be_bytes()).unwrap();stdin.write_all(&payload).unwrap();
  let(tx,lines)=mpsc::channel();let stdout=child.stdout.take().unwrap();
  std::thread::spawn(move||{for line in BufReader::new(stdout).lines().map_while(Result::ok){let _=tx.send((Instant::now(),line));}});
  Some(Self{child,stdin:Some(stdin),lines,frames,started:Instant::now()})
 }
 fn pid(&self)->u32{self.child.id()}
 /// Lines until `stop` matches one or `limit` passes.
 fn read(&self,limit:Duration,stop:impl Fn(&Value)->bool)->Vec<(Instant,Value)>{
  let until=Instant::now()+limit;let mut out=vec![];
  while let Some(left)=until.checked_duration_since(Instant::now()){
   match self.lines.recv_timeout(left.min(Duration::from_millis(100))){
    Ok((at,line))=>{let v=serde_json::from_str(&line).unwrap_or(json!({"line":line}));let done=stop(&v);out.push((at,v));if done{break}}
    Err(mpsc::RecvTimeoutError::Disconnected)=>break,Err(_)=>{}
   }
  }
  out
 }
 fn first_frame(&self)->Option<Duration>{self.read(Duration::from_secs(15),|v|v["frame"].is_string()).last().filter(|(_,v)|v["frame"].is_string()).map(|(at,_)|at.duration_since(self.started))}
 fn status(&self,key:&str)->u64{std::fs::read_to_string(format!("/proc/{}/status",self.pid())).ok().and_then(|s|s.lines().find(|l|l.starts_with(key)).and_then(|l|l.split_whitespace().nth(1)?.parse().ok())).unwrap_or(0)}
 fn cpu_ticks(&self)->u64{std::fs::read_to_string(format!("/proc/{}/stat",self.pid())).ok().map(|s|{let f:Vec<&str>=s.rsplit_once(')').unwrap().1.split_whitespace().collect();f[11].parse::<u64>().unwrap()+f[12].parse::<u64>().unwrap()}).unwrap_or(0)}
 fn pss_kb(&self)->u64{std::fs::read_to_string(format!("/proc/{}/smaps_rollup",self.pid())).ok().and_then(|s|s.lines().find(|l|l.starts_with("Pss:")).and_then(|l|l.split_whitespace().nth(1)?.parse().ok())).unwrap_or(0)}
 fn close(&mut self)->Option<Duration>{
  let at=Instant::now();if let Some(mut stdin)=self.stdin.take(){let _=writeln!(stdin,"{{\"type\":\"close\"}}");}
  while at.elapsed()<Duration::from_secs(3){if self.child.try_wait().ok().flatten().is_some(){return Some(at.elapsed())}std::thread::sleep(Duration::from_millis(10));}
  None
 }
}
impl Drop for Renderer{fn drop(&mut self){let _=self.child.kill();let _=self.child.wait();let _=std::fs::remove_dir_all(&self.frames);}}
fn ticks_per_second()->f64{unsafe{libc::sysconf(libc::_SC_CLK_TCK) as f64}}

const STATIC:&str="import QtQuick\nimport QtQuick.Controls\nRectangle { width: 400; height: 200; color: omarchy.surface\n Button { anchors.centerIn: parent; text: \"Hola\" } }";
const SPINNER:&str="import QtQuick\nRectangle { width: 200; height: 200; color: omarchy.surface\n Rectangle { width: 60; height: 60; anchors.centerIn: parent; color: omarchy.accent\n  RotationAnimation on rotation { from: 0; to: 360; duration: 1000; loops: Animation.Infinite } } }";

#[test]
fn static_previews_start_fast_stay_idle_and_light(){
 let Some(mut r)=Renderer::start("static",STATIC)else{return};
 let first=r.first_frame().expect("a first frame");
 assert!(first<Duration::from_secs(3),"first frame took {first:?}");
 std::thread::sleep(Duration::from_millis(500));let _=r.read(Duration::ZERO,|_|false);
 let ticks=r.cpu_ticks();let frames=r.read(Duration::from_secs(2),|_|false).into_iter().filter(|(_,v)|v["frame"].is_string()).count();
 let cpu=(r.cpu_ticks()-ticks) as f64/ticks_per_second()/2.0*100.0;
 assert!(frames<=1,"a static preview keeps capturing ({frames} frames in 2 s)");
 assert!(cpu<5.0,"a static preview uses {cpu:.1}% CPU while idle");
 let pss=r.pss_kb()/1024;assert!(pss>0&&pss<150,"static preview PSS {pss} MB");
 let closed=r.close().expect("close request ends the renderer");assert!(closed<Duration::from_secs(1),"close took {closed:?}");
}
#[test]
fn animations_are_captured_at_most_twenty_times_a_second_as_bmp(){
 let Some(r)=Renderer::start("spinner",SPINNER)else{return};
 r.first_frame().expect("a first frame");
 let frames:Vec<_>=r.read(Duration::from_secs(2),|_|false).into_iter().filter(|(_,v)|v["frame"].is_string()).collect();
 let fps=frames.len() as f64/2.0;
 assert!((10.0..=22.0).contains(&fps),"animated preview captured at {fps} fps");
 let(_,last)=frames.last().unwrap();let name=last["frame"].as_str().unwrap();
 assert!(matches!(name,"frame-0.bmp"|"frame-1.bmp"),"{name}");
 let bytes=std::fs::read(r.frames.join(name)).unwrap();assert_eq!(&bytes[..2],b"BM");
 assert!(bytes.len() as u64>=last["width"].as_u64().unwrap()*last["height"].as_u64().unwrap()*3,"uncompressed frame");
}
#[test]
fn endless_loops_are_reported_and_end_the_renderer(){
 // At creation: no frame ever comes.
 let Some(mut r)=Renderer::start("loop","import QtQuick\nItem { width: 10; height: 10; Component.onCompleted: { while (true) {} } }")else{return};
 let lines=r.read(Duration::from_secs(10),|v|v["error"].is_string());
 assert_eq!(lines.last().map(|(_,v)|v["error"].clone()),Some(json!("qml_preview_unresponsive")),"{lines:?}");
 let status=r.child.wait().unwrap();assert_eq!(status.code(),Some(3));
 let reported=lines.last().unwrap().0.duration_since(r.started);
 assert!(reported<Duration::from_secs(8),"reported after {reported:?}");
 // Later, from a timer: the first frame arrives, then the watchdog fires.
 let Some(mut r)=Renderer::start("late-loop","import QtQuick\nRectangle { width: 10; height: 10\n Timer { interval: 300; running: true; onTriggered: { for (;;) {} } } }")else{return};
 r.first_frame().expect("a frame before the loop starts");
 let lines=r.read(Duration::from_secs(10),|v|v["error"].is_string());
 assert_eq!(lines.last().map(|(_,v)|v["error"].clone()),Some(json!("qml_preview_unresponsive")));
 assert_eq!(r.child.wait().unwrap().code(),Some(3));
}
#[test]
fn memory_hungry_components_stay_under_the_heap_limit(){
 let Some(mut r)=Renderer::start("memory","import QtQuick\nItem { width: 10; height: 10\n property var keep: []\n Component.onCompleted: { for (var i = 0; i < 400; i++) keep.push(new ArrayBuffer(8 * 1024 * 1024)) } }")else{return};
 let limit_mb=qml_preview::MEMORY_LIMIT/1024/1024;let mut peak=0;let until=Instant::now()+Duration::from_secs(6);
 while Instant::now()<until{peak=peak.max(r.status("VmHWM:")/1024);if r.child.try_wait().ok().flatten().is_some(){break}let _=r.read(Duration::from_millis(100),|_|false);}
 // The data limit applies to the heap; RSS adds Qt's shared libraries (~60 MB).
 assert!(peak<limit_mb+200,"renderer peaked at {peak} MB with a {limit_mb} MB heap limit");
}
#[test]
fn renderer_runs_niced_with_a_heap_limit_and_a_bare_environment(){
 let Some(r)=Renderer::start("limits",STATIC)else{return};r.first_frame().expect("a first frame");
 let stat=std::fs::read_to_string(format!("/proc/{}/stat",r.pid())).unwrap();let nice:i64=stat.rsplit_once(')').unwrap().1.split_whitespace().nth(16).unwrap().parse().unwrap();
 assert_eq!(nice,10);
 let limits=std::fs::read_to_string(format!("/proc/{}/limits",r.pid())).unwrap();
 let data=limits.lines().find(|l|l.starts_with("Max data size")).unwrap();assert!(data.contains(&qml_preview::MEMORY_LIMIT.to_string()),"{data}");
 let environ=std::fs::read(format!("/proc/{}/environ",r.pid())).unwrap();
 let keys:Vec<String>=environ.split(|b|*b==0).filter(|e|!e.is_empty()).map(|e|String::from_utf8_lossy(e).split('=').next().unwrap().to_string()).collect();
 let allowed=["HOME","LANG","LC_ALL","LC_CTYPE","XDG_CACHE_HOME","PATH","QML_DISABLE_DISK_CACHE","QT_QUICK_CONTROLS_STYLE","QT_QPA_PLATFORM","QT_QUICK_BACKEND","QT_SCALE_FACTOR","QT_SCREEN_SCALE_FACTORS"];
 for key in &keys{assert!(allowed.contains(&key.as_str()),"inherited {key}")}
 assert!(String::from_utf8_lossy(&environ).contains("PATH=/nonexistent"));
}
#[test]
fn window_roots_are_reported_instead_of_opened(){
 let Some(r)=Renderer::start("window","import QtQuick\nWindow { width: 200; height: 200; visible: true }")else{return};
 let lines=r.read(Duration::from_secs(15),|v|v["frame"].is_string());
 assert!(lines.iter().any(|(_,v)|v["frame"].is_string()),"the error is drawn in a frame");
 // Error-only previews ask for a short page: one message line.
 assert!(lines.iter().any(|(_,v)|v["contentHeight"]==json!(104)),"{lines:?}");
}
#[test]
fn four_previews_together_stay_within_a_memory_budget(){
 let renderers:Vec<_>=(0..4).filter_map(|i|Renderer::start(&format!("pool-{i}"),STATIC)).collect();
 if renderers.len()<4{return}
 for r in &renderers{r.first_frame().expect("a first frame");}
 let pss:u64=renderers.iter().map(|r|r.pss_kb()).sum::<u64>()/1024;
 assert!(pss<400,"four inline previews use {pss} MB PSS");
}
