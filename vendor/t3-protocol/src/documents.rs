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
        let grant=crate::api::assets::grant(self,resource)?;
        let url=endpoint(&self.base,field(&grant,"relativeUrl")?)?;
        if !url.path().starts_with("/api/assets/"){return Err("invalid_asset_url".into())}
        let response=self.http.get(url.clone()).timeout(Duration::from_secs(30)).send().map_err(|_|"document_unreachable")?;
        if !response.status().is_success(){return Err("document_unavailable".into())}
        let mime=response.headers().get("content-type").and_then(|v|v.to_str().ok()).unwrap_or("").split(';').next().unwrap_or("").to_string();
        let mut bytes=vec![];
        read_limited(response,4*1024*1024,&mut bytes).map_err(|e|match e{BodyError::TooLarge=>"document_limit_4MiB",BodyError::Unreadable=>"document_unavailable"})?;
        if matches!(mime.as_str(),"image/png"|"image/jpeg"|"image/gif"|"image/webp"){
            let valid=match mime.as_str(){"image/png"=>bytes.starts_with(b"\x89PNG\r\n\x1a\n"),"image/jpeg"=>bytes.starts_with(b"\xff\xd8\xff"),"image/gif"=>bytes.starts_with(b"GIF87a")||bytes.starts_with(b"GIF89a"),"image/webp"=>bytes.starts_with(b"RIFF")&&bytes.get(8..12)==Some(b"WEBP"),_=>false};
            if !valid{return Err("invalid_image".into())}
            return Ok(json!({"contents":"","imageData":format!("data:{mime};base64,{}",base64::engine::general_purpose::STANDARD.encode(bytes)),"baseUrl":url.as_str()}))
        }
        let contents=String::from_utf8(bytes).map_err(|_|"unsupported_document")?;
        Ok(json!({"contents":contents,"baseUrl":url.as_str()}))
    }
}

/// MCP app of a completed tool call (T3 mcpAppFromToolItem): the app's HTML is an attachment,
/// and the call is `server.tool`. Returns the app reference plus a file attachment for it.
pub fn mcp_app(item:&Value)->Option<Value>{
 if item["status"]!="completed"&&!item["status"].is_null(){return None}
 fn read(v:&Value,depth:u8)->Option<Value>{
  if depth>6 || v["isError"]==true || v["is_error"]==true{return None}
  if let Some(s)=v.as_str(){if s.len()>128*1024{return None}return read(&serde_json::from_str::<Value>(s).ok()?,depth+1)}
  if let Some(a)=v.as_array(){return a.iter().take(32).find_map(|v|read(&v["text"],depth+1))}
  if v["t3McpApp"].is_object(){return Some(v["t3McpApp"].clone())}
  for key in ["structuredContent","content"]{if !v[key].is_null(){return read(&v[key],depth+1)}}
  None
 }
 let app=read(&item["output"],0)?;
 let bounded=|k:&str|app[k].as_str().filter(|s|!s.trim().is_empty()&&s.len()<=256).map(str::to_string);
 let (id,server,tool)=(bounded("attachmentId")?,bounded("server")?,bounded("tool")?);
 let uri=app["resourceUri"].as_str().filter(|u|u.starts_with("ui://")&&u.len()<=2048)?;
 if item["toolName"].as_str()!=Some(format!("{server}.{tool}").as_str()){return None}
 Some(json!({"type":"file","id":id,"name":format!("{}.html",tool.chars().filter(|c|!c.is_control()&&!"/\\".contains(*c)).take(120).collect::<String>()),"mimeType":"text/html","mcpApp":{"attachmentId":id,"server":server,"tool":tool,"resourceUri":uri,"itemId":item["id"],"threadId":item["threadId"]}}))
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
#[cfg(test)]
mod mcp_app_tests {
    use serde_json::json;
    #[test]
    fn reads_app_reference_of_matching_tool() {
        let item = json!({"id":"i1","threadId":"t","status":"completed","toolName":"weather.forecast","output":{"structuredContent":{"t3McpApp":{"attachmentId":"att","server":"weather","tool":"forecast","resourceUri":"ui://weather/forecast"}}}});
        let a = super::mcp_app(&item).unwrap();
        assert_eq!((a["id"].as_str(), a["mcpApp"]["itemId"].as_str(), a["name"].as_str()), (Some("att"), Some("i1"), Some("forecast.html")));
        let mut other = item.clone();
        other["toolName"] = json!("weather.other");
        assert!(super::mcp_app(&other).is_none());
        let mut bad = item;
        bad["output"]["structuredContent"]["t3McpApp"]["resourceUri"] = json!("https://x");
        assert!(super::mcp_app(&bad).is_none());
    }
}

impl Connection {
    /// Text-only JSON read; image/media bytes use the existing binary asset channel.
    pub fn document_text(&self,resource:Value)->Result<Value>{
        if !matches!(resource["_tag"].as_str(),Some("media-file"|"workspace-file"|"attachment")){return Err("invalid_file".into())}
        let asset=self.asset_bytes(resource,4*1024*1024)?;
        if asset.mime.starts_with("image/")||asset.mime.starts_with("video/")||asset.mime.starts_with("audio/"){return Err("binary_operation".into())}
        let contents=String::from_utf8(asset.bytes).map_err(|_|"unsupported_document")?;
        Ok(json!({"contents":contents,"mimeType":asset.mime}))
    }
}
