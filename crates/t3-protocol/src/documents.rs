//! On-demand file previews using the same scoped asset grants as T3's file panel.
use crate::*;

pub fn resource(thread: &str, path: &str) -> Result<Value> {
    if thread.is_empty() || path.is_empty() || path.len()>1024 || path.chars().any(char::is_control) {return Err("invalid_file".into())}
    let lower=path.to_ascii_lowercase();
    let media=[".html",".htm",".png",".jpg",".jpeg",".webp",".gif"].iter().any(|ext|lower.ends_with(ext));
    Ok(json!({"_tag":if media{"media-file"}else{"workspace-file"},"threadId":thread,"path":path}))
}
impl Connection {
    /// Bounded UTF-8 document read. The bearer credential is used only for the
    /// RPC grant; the signed GET stays on the environment's origin.
    pub fn document(&self, resource:Value)->Result<Value>{
        if !matches!(resource["_tag"].as_str(),Some("media-file"|"workspace-file"|"attachment")){return Err("invalid_file".into())}
        let grant=self.rpc("assets.createUrl",json!({"resource":resource}))?;
        let url=endpoint(&self.base,field(&grant,"relativeUrl")?)?;
        if !url.path().starts_with("/api/assets/"){return Err("invalid_asset_url".into())}
        let response=self.http.get(url.clone()).timeout(Duration::from_secs(30)).send().map_err(|_|"document_unreachable")?;
        if !response.status().is_success(){return Err("document_unavailable".into())}
        let mime=response.headers().get("content-type").and_then(|v|v.to_str().ok()).unwrap_or("").split(';').next().unwrap_or("").to_string();
        let mut bytes=vec![];response.take(4*1024*1024+1).read_to_end(&mut bytes).map_err(|_|"document_unavailable")?;
        if bytes.len()>4*1024*1024{return Err("document_limit_4MiB".into())}
        if matches!(mime.as_str(),"image/png"|"image/jpeg"|"image/gif"|"image/webp"){
            let valid=match mime.as_str(){"image/png"=>bytes.starts_with(b"\x89PNG\r\n\x1a\n"),"image/jpeg"=>bytes.starts_with(b"\xff\xd8\xff"),"image/gif"=>bytes.starts_with(b"GIF87a")||bytes.starts_with(b"GIF89a"),"image/webp"=>bytes.starts_with(b"RIFF")&&bytes.get(8..12)==Some(b"WEBP"),_=>false};
            if !valid{return Err("invalid_image".into())}
            return Ok(json!({"contents":"","imageData":format!("data:{mime};base64,{}",base64::engine::general_purpose::STANDARD.encode(bytes)),"baseUrl":url.as_str()}))
        }
        let contents=String::from_utf8(bytes).map_err(|_|"unsupported_document")?;
        Ok(json!({"contents":contents,"baseUrl":url.as_str()}))
    }
}

pub fn html_render(item:&Value)->Option<Value>{
 let name=item["toolName"].as_str()?;
 if name!="html_render" && !name.ends_with("__html_render") && !name.ends_with(".html_render"){return None}
 fn read(v:&Value,depth:u8)->Option<Value>{
  if depth>6 || v["isError"]==true || v["is_error"]==true || !v["error"].is_null(){return None}
  if let Some(s)=v.as_str(){if s.len()>128*1024{return None}return read(&serde_json::from_str::<Value>(s).ok()?,depth+1)}
  if let Some(a)=v.as_array(){return a.iter().take(32).find_map(|v|read(&v["text"],depth+1))}
  for key in ["structuredContent","content"]{if !v[key].is_null(){return read(&v[key],depth+1)}}
  let h=&v["htmlRender"];let id=h["attachmentId"].as_str()?;let title=h["title"].as_str()?;
  if id.is_empty()||id.len()>256{return None}
  Some(json!({"type":"file","id":id,"name":format!("{}.html",title.chars().filter(|c|!c.is_control()&&!"/\\".contains(*c)).take(120).collect::<String>()),"mimeType":"text/html"}))
 }
 read(&item["output"],0)
}
