use omarchy_t3_client::{markdown_render,qml_preview,write};
use serde_json::{json,Value};
use std::{io::{BufRead,BufReader,Write},process::{Command,Stdio},time::{Duration,Instant}};

#[test]
fn allowed_modules_pass_with_versions_aliases_and_pragmas(){
 let source="pragma ComponentBehavior: Bound\nimport QtQuick 2.15\nimport QtQuick.Controls as C\nimport QtQuick.Layouts 1.15 as L\nimport QtQuick.Shapes\nimport QtQuick.Effects;\nimport QtQml\nRectangle { width: 100; height: 100 }";
 assert!(qml_preview::validate(source).is_ok());
 assert_eq!(qml_preview::imports(source),["QtQuick","QtQuick.Controls","QtQuick.Layouts","QtQuick.Shapes","QtQuick.Effects","QtQml"]);
}
#[test]
fn shell_system_and_unlisted_modules_are_refused(){
 for module in ["Quickshell","Quickshell.Io","Quickshell.Wayland","QtCore","QtQuick.LocalStorage","QtQuick.Dialogs","QtQml.XmlListModel","QtWebEngine","QtNetwork","QtQuick.Controls.Material","qs.Commons","Qt.labs.platform"]{
  let source=format!("import QtQuick\nimport {module} 1.0\nItem {{}}");
  assert_eq!(qml_preview::validate(&source).unwrap_err(),format!("qml_import_blocked:{module}"),"{module}");
 }
}
#[test]
fn file_and_directory_imports_are_refused(){
 assert_eq!(qml_preview::validate("import QtQuick\nimport \"helpers.js\" as H\nItem {}").unwrap_err(),"qml_import_blocked:\"helpers.js\"");
 assert_eq!(qml_preview::validate("import QtQuick\nimport '../ui/qml'\nItem {}").unwrap_err(),"qml_import_blocked:'../ui/qml'");
 assert_eq!(qml_preview::validate("import QtQuick\nimport \"/usr/share/omarchy/shell/Commons\"\nItem {}").unwrap_err(),"qml_import_blocked:\"/usr/share/omarchy/shell/Commons\"");
}
#[test]
fn imports_inside_dynamic_object_strings_are_checked(){
 let one_line="import QtQuick\nItem { Component.onCompleted: Qt.createQmlObject(\"import Quickshell.Io; Process { command: ['sh'] }\", this) }";
 assert_eq!(qml_preview::validate(one_line).unwrap_err(),"qml_import_blocked:Quickshell.Io");
 let escaped="import QtQuick\nItem { Component.onCompleted: Qt.createQmlObject('import QtCore\\nSettings {}', this) }";
 assert_eq!(qml_preview::validate(escaped).unwrap_err(),"qml_import_blocked:QtCore");
}
#[test]
fn comments_and_prose_do_not_count_as_imports(){
 let source="// import Quickshell.Io\n/* import QtCore\n import Quickshell */\nimport QtQuick\nText { text: \"import data\"; property string hint: \"Please import Files first\" }";
 assert!(qml_preview::validate(source).is_ok(),"{:?}",qml_preview::validate(source));
 assert!(qml_preview::validate("import QtQuick\nItem { property var reimport: 1; function importAll(){} }").is_ok());
}
#[test]
fn empty_oversized_and_binary_sources_are_refused(){
 assert_eq!(qml_preview::validate("  \n").unwrap_err(),"qml_preview_empty");
 assert_eq!(qml_preview::validate(&format!("import QtQuick\nItem {{}}\n//{}","x".repeat(70*1024))).unwrap_err(),"qml_preview_too_large");
 assert_eq!(qml_preview::validate("import QtQuick\0\nItem {}").unwrap_err(),"qml_preview_invalid");
}
#[test]
fn previews_open_without_an_environment_and_blocked_ones_never_reach_qt(){
 let ok=write::execute(&json!({"op":"document-open","attachment":{"generated":"qml","source":"import QtQuick\nRectangle { width: 10; height: 10 }","labels":{"errors":"Errores"}},"theme":{"appearance":"dark"}})).unwrap();
 assert_eq!(ok["qmlPreview"],true);assert_eq!(ok["html"],true);assert_eq!(ok["generated"],true);assert_eq!(ok["labels"]["errors"],"Errores");
 assert!(ok["contents"].as_str().unwrap().contains("Rectangle"));
 let blocked=write::execute(&json!({"op":"document-open","attachment":{"generated":"qml","source":"import Quickshell.Io\nProcess {}"}}));
 assert_eq!(blocked.unwrap_err(),"qml_import_blocked:Quickshell.Io");
}
#[test]
fn qml_code_blocks_are_marked_for_an_in_place_preview(){
 let blocks=vec![json!({"language":"qml","code":"import QtQuick\nItem {}"}),json!({"language":"rust","code":"fn main(){}"}),json!({"language":"QML","code":"Item {}"})];
 let html=markdown_render("Antes\n\n```qml\nimport QtQuick\nItem {}\n```\n\nEntre\n\n```rust\nfn main(){}\n```\n\n```QML\nItem {}\n```\n\nDespués",Some(&blocks),false);
 let open=html.find("<!--ot3-qml:0-->").expect("start marker");let close=html.find("<!--/ot3-qml:0-->").expect("end marker");
 let inside=&html[open..close];assert!(inside.contains("Item {}")&&inside.contains("ot3-code-copy://0"),"{inside}");
 assert!(html[..open].contains("Antes")&&html[close..].contains("Entre"),"{html}");
 assert!(!html.contains("ot3-qml:1"),"other languages stay as code: {html}");
 assert!(html.contains("<!--ot3-qml:2-->")&&html.contains("<!--/ot3-qml:2-->"),"language match ignores case");
 assert!(html.rfind("<!--/ot3-qml:2-->").unwrap()<html.rfind("Después").unwrap());
 assert!(!html.contains("ot3-code-preview://"),"no separate preview link: {html}");
 // Without code-block metadata (plans, documents) nothing is marked.
 assert!(!markdown_render("```qml\nItem {}\n```",None,false).contains("ot3-qml"));
}

fn landlock()->bool{unsafe{libc::syscall(444,std::ptr::null::<u8>(),0usize,1u32)>=1}}
fn sandboxed_shell(script:&str,extra:Option<&std::path::Path>)->std::process::Output{
 use std::os::unix::process::CommandExt;
 let mut sandbox=qml_preview::Sandbox::new();sandbox.allow("/usr",qml_preview::READ);sandbox.allow("/usr/bin/bash",qml_preview::READ|1);sandbox.allow_loader();
 if let Some(dir)=extra{sandbox.allow(dir,qml_preview::ALL)}
 let mut command=Command::new("/usr/bin/bash");command.args(["-c",script]).env_clear();
 unsafe{command.pre_exec(move||sandbox.enter());}
 command.output().unwrap()
}
#[test]
fn landlock_keeps_reads_writes_launches_and_tcp_inside_the_allowed_paths(){
 if !landlock(){eprintln!("Landlock unavailable; skipped");return}
 let outside=std::env::temp_dir().join(format!("ot3-qml-secret-{}",std::process::id()));std::fs::create_dir_all(&outside).unwrap();
 std::fs::write(outside.join("secret.txt"),"secret").unwrap();
 let frames=std::env::temp_dir().join(format!("ot3-qml-frames-{}",std::process::id()));std::fs::create_dir_all(&frames).unwrap();
 // Reads under /usr work; reads elsewhere fail.
 let read=sandboxed_shell(&format!("read -r line < /usr/lib/os-release && echo usr-ok; read -r x < {} && echo LEAK || echo blocked",outside.join("secret.txt").display()),None);
 let out=String::from_utf8_lossy(&read.stdout);assert!(out.contains("usr-ok"),"{out}");assert!(out.contains("blocked")&&!out.contains("LEAK"),"{out}");
 // The frame directory stays writable.
 let write=sandboxed_shell(&format!("echo frame > {0}/frame.png && cat_ok=1; read -r v < {0}/frame.png && echo \"$v\"",frames.display()),Some(&frames));
 assert!(String::from_utf8_lossy(&write.stdout).contains("frame"),"{:?}",write);
 // Only the renderer binary itself may be executed: no helpers, no xdg-open.
 let launch=sandboxed_shell("/usr/bin/true && echo LAUNCHED || echo refused",None);
 let out=String::from_utf8_lossy(&launch.stdout);assert!(out.contains("refused")&&!out.contains("LAUNCHED"),"{out}");
 // No TCP connections, even to localhost.
 let listener=std::net::TcpListener::bind("127.0.0.1:0").unwrap();let port=listener.local_addr().unwrap().port();
 let net=sandboxed_shell(&format!("(exec 3<>/dev/tcp/127.0.0.1/{port}) 2>/dev/null && echo CONNECTED || echo refused"),None);
 let out=String::from_utf8_lossy(&net.stdout);assert!(out.contains("refused")&&!out.contains("CONNECTED"),"{out}");
 let _=std::fs::remove_dir_all(outside);let _=std::fs::remove_dir_all(frames);
}

/// The real renderer (needs `tools/build-document-viewer` and a built client in
/// bin/). A rich-text <img> bypasses Qt's URL interceptor, so this checks the
/// Landlock domain: the same component reads the file without it.
#[test]
fn renderer_cannot_read_files_outside_the_sandbox(){
 let root=std::path::Path::new(env!("CARGO_MANIFEST_DIR"));let exe=root.join("bin/omarchy-t3-client");
 if !landlock()||!exe.is_file()||!root.join("bin/libt3-qml-preview.so").is_file(){eprintln!("QML renderer not built; skipped");return}
 let outside=std::env::temp_dir().join(format!("ot3-qml-image-{}",std::process::id()));std::fs::create_dir_all(&outside).unwrap();
 let picture=outside.join("secret.png");image::RgbImage::from_pixel(40,300,image::Rgb([255,0,0])).save(&picture).unwrap();
 let source=format!("import QtQuick\nText {{ textFormat: Text.RichText; color: \"white\"; text: \"<img src='file://{}'>\" }}",picture.display());
 let measure=|sandboxed:bool|->u64{
  let frames=std::env::temp_dir().join(format!("ot3-qml-render-{}-{sandboxed}",std::process::id()));std::fs::create_dir_all(&frames).unwrap();
  let mut command=if sandboxed{qml_preview::command(&exe,&frames)}else{let mut c=Command::new(&exe);c.arg("document-render").env_clear().env("QT_QPA_PLATFORM","offscreen").env("QT_QUICK_BACKEND","software").env("QML_DISABLE_DISK_CACHE","1");c};
  let mut child=command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().unwrap();
  let payload=json!({"qmlPreview":true,"contents":source,"theme":{"appearance":"dark"},"offscreen":true,"frameDirectory":frames,"width":600,"height":400}).to_string().into_bytes();
  let mut stdin=child.stdin.take().unwrap();stdin.write_all(&(payload.len() as u64).to_be_bytes()).unwrap();stdin.write_all(&payload).unwrap();
  let (tx,rx)=std::sync::mpsc::channel();let stdout=child.stdout.take().unwrap();
  std::thread::spawn(move||{for line in BufReader::new(stdout).lines().map_while(Result::ok){let _=tx.send(line);}});
  let mut height=0;let mut framed=false;let until=Instant::now()+Duration::from_secs(15);
  while Instant::now()<until&&!framed{if let Ok(line)=rx.recv_timeout(Duration::from_millis(200)){let v:Value=serde_json::from_str(&line).unwrap_or(Value::Null);if let Some(h)=v["contentHeight"].as_u64(){height=height.max(h)}framed|=v["frame"].is_string();}}
  let _=writeln!(stdin,"{{\"type\":\"close\"}}");drop(stdin);let _=child.wait();let _=std::fs::remove_dir_all(frames);
  assert!(framed,"renderer produced no frame (sandboxed: {sandboxed})");height
 };
 let open=measure(false);let sandboxed=measure(true);let _=std::fs::remove_dir_all(outside);
 assert!(open>=300,"without the sandbox the probe image loads ({open})");
 assert!(sandboxed<200,"the sandboxed renderer must not read the file ({sandboxed})");
}
#[test]
fn whitespace_line_endings_and_statement_forms_are_parsed(){
 assert_eq!(qml_preview::imports("import\tQtQuick\r\nimport  QtQuick.Layouts\t1.15\r\nItem {}"),["QtQuick","QtQuick.Layouts"]);
 assert_eq!(qml_preview::imports("import QtQuick; import QtQml; Item {}"),["QtQuick","QtQml"]);
 assert_eq!(qml_preview::validate("import QtQuick\nimport Quickshell").unwrap_err(),"qml_import_blocked:Quickshell","an import at the very end counts");
 assert_eq!(qml_preview::validate("import QtQuick\r\nimport Quickshell.Io\r\nItem {}").unwrap_err(),"qml_import_blocked:Quickshell.Io");
 assert_eq!(qml_preview::validate("import QtQuick as Q\nimport QtCore as C\nQ.Item {}").unwrap_err(),"qml_import_blocked:QtCore");
}
#[test]
fn private_submodules_of_allowed_modules_are_refused(){
 for module in ["QtQuick.Controls.impl","QtQuick.Controls.Basic","QtQuick.Templates","QtQuick.Window","QtQml.Models","QtQml.WorkerScript","QtQuick.Particles","QtQuick.VectorImage"]{
  assert_eq!(qml_preview::validate(&format!("import QtQuick\nimport {module}\nItem {{}}")).unwrap_err(),format!("qml_import_blocked:{module}"));
 }
}
#[test]
fn strings_hide_neither_comments_nor_imports(){
 // A "//" inside a string is text, so the import after it on the next line still counts.
 assert_eq!(qml_preview::validate("import QtQuick\nText { text: \"http://x\" }\nimport QtCore\n").unwrap_err(),"qml_import_blocked:QtCore");
 // Comment markers inside strings do not open comments that would hide code.
 assert_eq!(qml_preview::validate("import QtQuick\nItem { property string a: \"/*\"\n Component.onCompleted: Qt.createQmlObject('import Quickshell; Item {}', this)\n property string b: \"*/\" }").unwrap_err(),"qml_import_blocked:Quickshell");
 // Escaped quotes do not end a string early.
 assert!(qml_preview::validate("import QtQuick\nText { text: \"say \\\"import data\\\" now\" }").is_ok());
}
#[test]
fn refused_module_names_are_bounded(){
 let long=format!("Q{}",("a".repeat(50)+".").repeat(10));
 let err=qml_preview::validate(&format!("import {long}\nItem {{}}")).unwrap_err();
 assert!(err.starts_with("qml_import_blocked:")&&err.len()<=19+120,"{}",err.len());
}
#[test]
fn validation_is_fast_on_the_largest_accepted_source(){
 let body="Rectangle { width: 10; height: 10; color: \"red\" } // import nothing here\n".repeat(1000);
 let source=format!("import QtQuick\nItem {{\n{}}}",&body[..60*1024]);
 let at=std::time::Instant::now();for _ in 0..20{assert!(qml_preview::validate(&source).is_ok())}
 assert!(at.elapsed()<std::time::Duration::from_millis(500),"20 validations of 60 KB took {:?}",at.elapsed());
}
