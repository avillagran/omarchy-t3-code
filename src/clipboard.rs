//! On-demand Wayland clipboard access. No watcher, polling, clipboard log or staging files.
use crate::*;
use serde_json::{json,Value};
use std::{io::{Read,Cursor},process::{Command,Stdio}};
fn read(mime:Option<&str>,limit:usize)->Result<Vec<u8>>{
 let mut command=Command::new("timeout");command.args(["3","wl-paste"]);
 if let Some(mime)=mime{command.args(["--no-newline","--type",mime]);}else{command.arg("--list-types");}
 let mut child=command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().map_err(|_|"clipboard_unavailable")?;
 let mut bytes=Vec::new();let result=child.stdout.take().ok_or("clipboard_unavailable")?.take(limit as u64+1).read_to_end(&mut bytes);
 if result.is_err()||bytes.len()>limit{let _=child.kill();let _=child.wait();return Err("clipboard_too_large".into())}
 if !child.wait().map_err(|_|"clipboard_unavailable")?.success(){return Err("clipboard_unavailable".into())}Ok(bytes)
}
fn utf8(bytes:Vec<u8>)->Result<String>{String::from_utf8(bytes).map_err(|_|"invalid_clipboard_text".into())}
fn paths(mime:&str,value:&str)->Result<Vec<String>>{
 let mut files=vec![];
 for line in value.lines().map(str::trim).filter(|s|!s.is_empty()&&!s.starts_with('#')){
  if mime=="x-special/gnome-copied-files" && matches!(line,"copy"|"cut"){continue}
  let url=crate::Url::parse(line).map_err(|_|"invalid_clipboard_file")?;
  if url.scheme()!="file"||url.to_file_path().is_err(){return Err("invalid_clipboard_file".into())}files.push(url.to_string());
 }Ok(files)
}
pub fn paste(input:&Value)->Result<Value>{
 let types=utf8(read(None,16384)?)?;let types:Vec<&str>=types.lines().collect();
 if let Some(mime)=["text/uri-list","x-special/gnome-copied-files"].iter().find(|m|types.contains(m)){
  let urls=paths(mime,&utf8(read(Some(mime),120000)?)?)?;
  if urls.len()>input["capacity"].as_u64().unwrap_or(0) as usize{return Err("invalid_attachments".into())}
  let e=environments()?.into_iter().find(|e|e.id==text(input,"environment")).ok_or("unknown_environment")?;
  let mut attachments=vec![];let mut error=String::new();
  for url in urls{match write::upload(&e,&url){Ok(a)=>attachments.push(a),Err(e)=>{error=e;break}}}
  return Ok(json!({"attachments":attachments,"error":error}))
 }
 if let Some(mime)=["image/png","image/jpeg","image/webp","image/gif"].iter().find(|m|types.contains(m)){
  if input["capacity"].as_u64().unwrap_or(0)==0{return Err("invalid_attachments".into())}
  let bytes=read(Some(mime),10*1024*1024)?;if bytes.is_empty(){return Err("invalid_file".into())}
  let ext=match *mime{"image/jpeg"=>"jpg","image/webp"=>"webp","image/gif"=>"gif",_=>"png"};
  let name=format!("clipboard-{}.{}",uuid::Uuid::new_v4(),ext);
  let e=environments()?.into_iter().find(|e|e.id==text(input,"environment")).ok_or("unknown_environment")?;
  let connection=e.connection(true)?;
  let a=connection.upload_attachment(&name,mime,bytes.len() as u64,Cursor::new(bytes))?;
  return Ok(json!({"attachments":[a]}))
 }
 let mime=["text/plain;charset=utf-8","text/plain","UTF8_STRING"].iter().find(|m|types.contains(m)).ok_or("clipboard_unsupported")?;
 let raw=utf8(read(Some(mime),480000)?)?;Ok(json!({"text":message_text::plain_qt(&raw).unwrap_or(raw)}))
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn copied_paths_are_local_and_keep_escaping(){assert_eq!(paths("text/uri-list","# comment\nfile:///tmp/a%20b.png\r\n").unwrap(),vec!["file:///tmp/a%20b.png"]);assert_eq!(paths("x-special/gnome-copied-files","copy\nfile:///tmp/a.txt").unwrap().len(),1);assert!(paths("text/uri-list","https://example.com/a").is_err());}
}

/// Explicit user-requested copy only. Transfer bytes through stdin, never argv or
/// a signed attachment URL. wl-copy owns the Wayland selection after it forks.
fn offer(command:&mut Command,bytes:&[u8])->Result<()> {
 use std::io::Write;
 let mut child=command.stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().map_err(|_|"clipboard_unavailable")?;
 let result=child.stdin.take().ok_or("clipboard_unavailable").and_then(|mut pipe|pipe.write_all(bytes).map_err(|_|"clipboard_unavailable"));
 if result.is_err(){let _=child.kill();let _=child.wait();return Err("clipboard_unavailable".into())}
 if !child.wait().map_err(|_|"clipboard_unavailable")?.success(){return Err("clipboard_unavailable".into())}Ok(())
}
fn png_payload(bytes:Vec<u8>)->Result<Vec<u8>>{
 use image::{ImageReader,ImageFormat,Limits};
 let reader=ImageReader::new(Cursor::new(&bytes)).with_guessed_format().map_err(|_|"invalid_image")?;
 let format=reader.format().ok_or("invalid_image")?;
 if !matches!(format,ImageFormat::Png|ImageFormat::Jpeg|ImageFormat::Gif|ImageFormat::WebP){return Err("invalid_image".into())}
 let (w,h)=reader.into_dimensions().map_err(|_|"invalid_image")?;
 if w==0||h==0||u64::from(w)*u64::from(h)>16_000_000{return Err("clipboard_too_large".into())}
 // PNG is already the interoperable clipboard flavor: no pixel allocation.
 if format==ImageFormat::Png{return Ok(bytes)}
 let mut reader=ImageReader::with_format(Cursor::new(bytes),format);let mut limits=Limits::default();limits.max_alloc=Some(64*1024*1024);limits.max_image_width=Some(8192);limits.max_image_height=Some(8192);reader.limits(limits);
 let decoded=reader.decode().map_err(|_|"invalid_image")?;
 let mut out=Cursor::new(Vec::new());decoded.write_to(&mut out,ImageFormat::Png).map_err(|_|"invalid_image")?;
 Ok(out.into_inner())
}
pub fn copy(input:&Value)->Result<Value>{
 let (mime,bytes)=match text(input,"kind"){
  "text"=>{let text=input["text"].as_str().ok_or("invalid_clipboard_text")?;if text.len()>480000{return Err("clipboard_too_large".into())}("text/plain;charset=utf-8",text.as_bytes().to_vec())},
  "image"=>{
   // Resolve through the same authenticated attachment/cache path as previews.
   let loaded=images::load(input)?;let url=crate::Url::parse(text(&loaded,"url")).map_err(|_|"invalid_image")?;
   let path=url.to_file_path().map_err(|_|"invalid_image")?;
   use std::os::unix::fs::{OpenOptionsExt,MetadataExt};
   let mut file=std::fs::OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW).open(path).map_err(|_|"invalid_image")?;
   let meta=file.metadata().map_err(|_|"invalid_image")?;
   if !meta.is_file()||meta.uid()!=unsafe{libc::geteuid()}||meta.mode()&0o077!=0||meta.len()>10*1024*1024{return Err("invalid_image".into())}
   let mut bytes=Vec::new();file.by_ref().take(10*1024*1024+1).read_to_end(&mut bytes).map_err(|_|"invalid_image")?;
   if bytes.len()>10*1024*1024{return Err("clipboard_too_large".into())}
   ("image/png",png_payload(bytes)?)
  },
  _=>return Err("clipboard_unsupported".into())
 };
 let mut command=Command::new("timeout");command.args(["5","wl-copy","--type",mime]);offer(&mut command,&bytes)?;
 Ok(json!({"copied":true,"mimeType":mime}))
}
#[cfg(test)]mod copy_tests{
 use super::*;
 #[test]fn image_clipboard_normalizes_pixels_to_png(){
  let image=image::DynamicImage::new_rgb8(3,2);
  for format in [image::ImageFormat::Png,image::ImageFormat::Jpeg,image::ImageFormat::Gif,image::ImageFormat::WebP]{let mut data=Cursor::new(Vec::new());image.write_to(&mut data,format).unwrap();let png=png_payload(data.into_inner()).unwrap();assert_eq!(image::guess_format(&png).unwrap(),image::ImageFormat::Png);let decoded=image::load_from_memory(&png).unwrap();assert_eq!((decoded.width(),decoded.height()),(3,2));}
  assert!(png_payload(b"not an image".to_vec()).is_err());
 }
 #[test]fn explicit_copy_reports_process_failure_and_keeps_literal_text(){
  let mut command=Command::new("sh");command.args(["-c","test \"$(cat)\" = 'literal <b> & $not_a_variable'"]);offer(&mut command,b"literal <b> & $not_a_variable").unwrap();
  assert!(offer(Command::new("false").args([] as [&str;0]),b"x").is_err());
  assert!(copy(&json!({"kind":"text","text":"x".repeat(480001)})).is_err());
 }
}
