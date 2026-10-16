//! Rust document adapter. Qt and Chromium are loaded only in a separate viewer
//! process, never in the persistent backend or the desktop shell.
use crate::*;
use std::os::fd::FromRawFd;

pub fn read(e:&Environment,input:&Value)->Result<Value>{
 let resource=if input["attachment"].is_object(){let a=&input["attachment"];let id=text(a,"id");if !valid_id(id){return Err("invalid_file".into())}json!({"_tag":"attachment","attachmentId":id,"fileName":text(a,"name"),"mimeType":text(a,"mimeType"),"disposition":"inline"})}else{protocol::documents::resource(text(input,"thread"),text(input,"path"))?};
 let name=if input["attachment"].is_object(){text(&input["attachment"],"name")}else{text(input,"path")};
 let c=if let Some(c)=&e.connection{c.as_ref().clone()}else{protocol::Connection::new(&e.base_url,None,protocol::Credential::bearer(&environment_token(e,true)?)?)?};
 let mut doc=c.document(resource)?;doc["labels"]=input["labels"].clone();doc["title"]=json!(name);doc["theme"]=input["theme"].clone();doc["html"]=json!(name.to_ascii_lowercase().ends_with(".html")||name.to_ascii_lowercase().ends_with(".htm")||text(&input["attachment"],"mimeType")=="text/html");Ok(doc)
}
/// Tiny Qt frontend bridge loaded explicitly so the Rust daemon has no Qt dependency.
pub fn renderer()->Result<()>{viewer_mode(true)}
pub fn viewer()->Result<()>{viewer_mode(false)}
fn viewer_mode(offscreen:bool)->Result<()>{
 let exe=std::env::current_exe().map_err(|_|"viewer_unavailable")?;let root=exe.parent().ok_or("viewer_unavailable")?;
 let mut payload=String::new();if offscreen {let fd=unsafe{libc::dup(libc::STDIN_FILENO)};if fd<0{return Err("invalid_file".into())}let mut input=unsafe{std::fs::File::from_raw_fd(fd)};let mut header=[0u8;8];input.read_exact(&mut header).map_err(|_|"invalid_file")?;let size=u64::from_be_bytes(header);if size>32*1024*1024{return Err("invalid_file".into())}let mut bytes=vec![0;size as usize];input.read_exact(&mut bytes).map_err(|_|"invalid_file")?;payload=String::from_utf8(bytes).map_err(|_|"invalid_file")?;}else{std::io::stdin().take(32*1024*1024).read_to_string(&mut payload).map_err(|_|"invalid_file")?;}
 if payload.len()>32*1024*1024{return Err("invalid_file".into())}
 let payload=std::ffi::CString::new(payload).map_err(|_|"invalid_file")?;
 let lib=std::ffi::CString::new(root.join("libt3-document.so").to_string_lossy().as_bytes()).unwrap();
 let qml=std::ffi::CString::new(root.join(if offscreen{"../ui/document/T3DocumentSurface.qml"}else{"../ui/document/T3DocumentWindow.qml"}).to_string_lossy().as_bytes()).unwrap();
 unsafe{let handle=libc::dlopen(lib.as_ptr(),libc::RTLD_NOW|libc::RTLD_LOCAL);if handle.is_null(){eprintln!("Qt frontend: {}",std::ffi::CStr::from_ptr(libc::dlerror()).to_string_lossy());return Err("viewer_unavailable".into())}
 let symbol=libc::dlsym(handle,c"t3_document_main".as_ptr());if symbol.is_null(){eprintln!("Qt symbol: {}",std::ffi::CStr::from_ptr(libc::dlerror()).to_string_lossy());return Err("viewer_unavailable".into())}
 let run:unsafe extern "C" fn(*const libc::c_char,*const libc::c_char)->libc::c_int=std::mem::transmute(symbol);
 if run(payload.as_ptr(),qml.as_ptr())!=0{return Err("viewer_unavailable".into())}}
 Ok(())
}
