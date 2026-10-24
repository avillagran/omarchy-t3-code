//! Rust document adapter. Qt and Chromium are loaded only in a separate viewer
//! process, never in the persistent backend or the desktop shell.
use crate::*;
use std::os::fd::FromRawFd;

pub fn read(e:&Environment,input:&Value)->Result<Value>{
 if input["reviewSnapshotId"].is_string(){return crate::review_document::open(e,input)}
 if input["attachment"]["generated"]=="mermaid"{return mermaid_page(input)}
 if input["attachment"]["generated"]=="qml"{return crate::qml_preview::page(input)}
 let resource=if input["attachment"].is_object(){let a=&input["attachment"];let id=text(a,"id");if !valid_id(id){return Err("invalid_file".into())}json!({"_tag":"attachment","attachmentId":id,"fileName":text(a,"name"),"mimeType":text(a,"mimeType"),"disposition":"inline"})}else{protocol::documents::resource(text(input,"thread"),text(input,"path"))?};
 let name=if input["attachment"].is_object(){text(&input["attachment"],"name")}else{text(input,"path")};
 let c=e.connection(true)?;
 let mut doc=if resource["_tag"]=="workspace-file" {
  // Asset grants only serve browser media. Text uses the native Files RPC,
  // including absolute host paths and the selected thread's worktree cwd.
  let cwd=text(input,"cwd");if cwd.is_empty(){return Err("invalid_workspace".into())}
  local_reference(e,cwd,text(input,"path"))?;
  let doc=c.rpc("projects.readFile",json!({"cwd":cwd,"relativePath":text(input,"path")}))?;
  if doc["truncated"]==true||text(&doc,"contents").len()>4*1024*1024{return Err("document_limit_4MiB".into())}doc
 }else{c.document(resource)?};
 doc["internalReview"]=json!(false);
 doc["markdown"]=json!([".md",".markdown",".mdown",".mkd"].iter().any(|ext|name.to_ascii_lowercase().ends_with(ext))||text(&input["attachment"],"mimeType")=="text/markdown");
 if doc["markdown"]==true{doc["rendered"]=json!(rendered_markdown(text(&doc,"contents")))}
 doc["labels"]=input["labels"].clone();doc["title"]=json!(name);doc["theme"]=input["theme"].clone();doc["html"]=json!(name.to_ascii_lowercase().ends_with(".html")||name.to_ascii_lowercase().ends_with(".htm")||text(&input["attachment"],"mimeType")=="text/html");Ok(doc)
}
/// The server only previews existing text files and answers everything else
/// with an opaque rejection. Local references are checked first: folders and
/// non-executable binaries (PDFs, archives…) open in their desktop app.
fn local_reference(e:&Environment,cwd:&str,path:&str)->Result<()>{
 if e.id!="local"{return Ok(())}
 let full=std::path::Path::new(cwd).join(path);
 let metadata=fs::metadata(&full).map_err(|_|"file_not_found")?;
 if !metadata.is_dir(){
  let mut head=Vec::with_capacity(8192);fs::File::open(&full).and_then(|f|f.take(8192).read_to_end(&mut head)).map_err(|_|"file_not_found")?;
  if !head.contains(&0){return Ok(())}
  use std::os::unix::fs::PermissionsExt;if metadata.permissions().mode()&0o111!=0{return Err("binary_file".into())}
 }
 std::process::Command::new("xdg-open").arg(&full).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn().map_err(|_|"local_error")?;
 Err("opened_externally".into())
}
/// Tiny Qt frontend bridge loaded explicitly so the Rust daemon has no Qt dependency.
pub fn renderer()->Result<()>{viewer_mode(true)}
pub fn viewer()->Result<()>{viewer_mode(false)}
fn viewer_mode(offscreen:bool)->Result<()>{
 let exe=std::env::current_exe().map_err(|_|"viewer_unavailable")?;let root=exe.parent().ok_or("viewer_unavailable")?;
 let mut payload=String::new();if offscreen {let fd=unsafe{libc::dup(libc::STDIN_FILENO)};if fd<0{return Err("invalid_file".into())}let mut input=unsafe{std::fs::File::from_raw_fd(fd)};let mut header=[0u8;8];input.read_exact(&mut header).map_err(|_|"invalid_file")?;let size=u64::from_be_bytes(header);if size>32*1024*1024{return Err("invalid_file".into())}let mut bytes=vec![0;size as usize];input.read_exact(&mut bytes).map_err(|_|"invalid_file")?;payload=String::from_utf8(bytes).map_err(|_|"invalid_file")?;}else{std::io::stdin().take(32*1024*1024).read_to_string(&mut payload).map_err(|_|"invalid_file")?;}
 if payload.len()>32*1024*1024{return Err("invalid_file".into())}
 // QML previews use their own small Qt library: no WebEngine in the sandboxed process.
 let qml_preview=offscreen&&serde_json::from_str::<Value>(&payload).is_ok_and(|v|v["qmlPreview"]==true);
 let payload=std::ffi::CString::new(payload).map_err(|_|"invalid_file")?;
 if qml_preview{return run_library(&root.join("libt3-qml-preview.so"),c"t3_qml_preview_main",&payload,&root.join("../ui/document/T3QmlPreview.qml"))}
 run_library(&root.join("libt3-document.so"),c"t3_document_main",&payload,&root.join(if offscreen{"../ui/document/T3DocumentSurface.qml"}else{"../ui/document/T3DocumentWindow.qml"}))
}
fn run_library(lib:&std::path::Path,symbol:&std::ffi::CStr,payload:&std::ffi::CStr,qml:&std::path::Path)->Result<()>{
 let lib=std::ffi::CString::new(lib.to_string_lossy().as_bytes()).map_err(|_|"viewer_unavailable")?;
 let qml=std::ffi::CString::new(qml.to_string_lossy().as_bytes()).map_err(|_|"viewer_unavailable")?;
 unsafe{let handle=libc::dlopen(lib.as_ptr(),libc::RTLD_NOW|libc::RTLD_LOCAL);if handle.is_null(){eprintln!("Qt frontend: {}",std::ffi::CStr::from_ptr(libc::dlerror()).to_string_lossy());return Err("viewer_unavailable".into())}
 let symbol=libc::dlsym(handle,symbol.as_ptr());if symbol.is_null(){eprintln!("Qt symbol: {}",std::ffi::CStr::from_ptr(libc::dlerror()).to_string_lossy());return Err("viewer_unavailable".into())}
 let run:unsafe extern "C" fn(*const libc::c_char,*const libc::c_char)->libc::c_int=std::mem::transmute(symbol);
 if run(payload.as_ptr(),qml.as_ptr())!=0{return Err("viewer_unavailable".into())}}
 Ok(())
}
/// A Mermaid code block drawn by the bundled mermaid.js (the version T3 pins), in the
/// same sandboxed page renderer as HTML renders. Strict security: no HTML labels or links.
pub fn mermaid_page(input:&Value)->Result<Value>{
 let exe=std::env::current_exe().map_err(|_|"viewer_unavailable")?;
 let script=exe.parent().and_then(|bin|bin.parent()).map(|root|root.join("ui/assets/mermaid/mermaid.min.js")).ok_or("viewer_unavailable")?;
 mermaid_page_with(input,&script)
}
fn mermaid_page_with(input:&Value,script:&std::path::Path)->Result<Value>{
 let source=input["attachment"]["source"].as_str().filter(|s|!s.trim().is_empty()&&s.len()<=64*1024).ok_or("invalid_file")?;
 let library=fs::read_to_string(script).map_err(|_|"viewer_unavailable")?;
 let dark=input["theme"]["appearance"]=="dark";
 let code=serde_json::to_string(source).map_err(|_|"invalid_file")?.replace("</","<\\/");
 let contents=format!("<!doctype html><html><head><meta charset=\"utf-8\"><style>body{{margin:0;padding:16px;display:flex;justify-content:center}}#d svg{{max-width:100%;height:auto}}#e{{color:#e5484d;font:13px monospace;white-space:pre-wrap}}</style></head><body><div id=\"d\"></div><pre id=\"e\"></pre><script>{}</script><script>mermaid.initialize({{startOnLoad:false,securityLevel:\"strict\",theme:\"{}\"}});mermaid.render(\"t3-diagram\",{}).then(function(r){{document.getElementById(\"d\").innerHTML=r.svg}}).catch(function(err){{document.getElementById(\"e\").textContent=String(err&&err.message||err)}});</script></body></html>",library.replace("</script","<\\/script"),if dark{"dark"}else{"default"},code);
 Ok(json!({"generated":true,"contents":contents,"baseUrl":"about:blank","html":true,"markdown":false,"internalReview":false,"title":"Diagram","labels":input["labels"],"theme":input["theme"]}))
}
#[cfg(test)]
mod mermaid_page_tests {
    #[test]
    fn writes_page_for_inspection() {
        if let Ok(out) = std::env::var("OT3_MERMAID_DUMP") {
            let input = serde_json::json!({"attachment":{"generated":"mermaid","source":"graph TD\n  A[Start] --> B{Works?}\n  B -->|yes| C[Done]"},"theme":{"appearance":"dark"}});
            // current_exe in tests is target/debug/deps/...; point at the repo copy instead.
            let doc = super::mermaid_page_with(&input, std::path::Path::new("ui/assets/mermaid/mermaid.min.js")).unwrap();
            std::fs::write(out, doc["contents"].as_str().unwrap()).unwrap();
        }
    }
}
