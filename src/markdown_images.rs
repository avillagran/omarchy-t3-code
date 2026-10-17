//! Markdown image destinations stay opaque to QML; host files use native asset grants.
use crate::*;
pub fn descriptors(input:&str)->Vec<Value>{
 use pulldown_cmark::{Event,Options,Parser,Tag,TagEnd};
 let mut rows=Vec::new();let mut current:Option<(String,String)>=None;
 for event in Parser::new_ext(input,Options::ENABLE_TABLES|Options::ENABLE_STRIKETHROUGH){match event{
  Event::Start(Tag::Image{dest_url,..})=>current=Some((dest_url.to_string(),String::new())),
  Event::Text(t)|Event::Code(t)=>if let Some((_,alt))=&mut current{alt.push_str(&t)},
  Event::End(TagEnd::Image)=>if let Some((source,alt))=current.take(){let id=format!("markdown-{:x}",Sha256::digest(source.as_bytes()));rows.push(json!({"id":id,"type":"image","name":if alt.is_empty(){"Image"}else{&alt},"source":source}));},
  _=>{}
 }}rows
}
/// Native markdownImages.ts contract: filesystem references belong to the
/// selected environment, never the machine running the native image component.
pub fn workspace_path(source:&str)->Result<String>{
 let source=source.trim();
 if source.is_empty()||source.len()>4096||source.chars().any(char::is_control)||source.starts_with(['#','?']){return Err("invalid_image".into())}
 if source.to_ascii_lowercase().starts_with("file:"){
  return reqwest::Url::parse(source).ok().and_then(|u|u.to_file_path().ok()).and_then(|p|p.to_str().map(String::from)).filter(|p|!p.chars().any(char::is_control)).ok_or("invalid_image".into());
 }
 if source.starts_with("//")||source.starts_with("~/")||source.contains(':'){return Err("invalid_image".into())}
 let raw=source.split(['#','?']).next().unwrap_or("");let mut bytes=Vec::new();let b=raw.as_bytes();let mut i=0;
 while i<b.len(){if b[i]==b'%'&&i+2<b.len(){if let (Some(a),Some(c))=((b[i+1] as char).to_digit(16),(b[i+2] as char).to_digit(16)){bytes.push((a*16+c) as u8);i+=3;continue}}bytes.push(b[i]);i+=1}
 let path=String::from_utf8(bytes).map_err(|_|"invalid_image")?;
 if path.is_empty()||path.chars().any(char::is_control)||path.contains(':')||path.starts_with("~/"){return Err("invalid_image".into())}Ok(path)
}
