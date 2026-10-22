//! Private cache of video attachments, as images.rs: the UI only ever gets a
//! local file URL, which Qt's MediaPlayer plays. T3 plays these in its image
//! viewer instead of opening them as documents.
use crate::*;
use serde_json::{json,Value};
use std::{io::Write,os::unix::fs::{OpenOptionsExt,PermissionsExt,MetadataExt}};
const LIMIT:u64=50*1024*1024;
/// T3 videoMimeType: a video/* type, or a generic type with a known extension.
pub fn mime(name:&str,mime:&str)->Option<&'static str>{
 let m=mime.split(';').next().unwrap_or("").trim().to_ascii_lowercase();
 let by_ext=|| name.rsplit_once('.').and_then(|(_,e)|match e.to_ascii_lowercase().as_str(){"mp4"|"m4v"=>Some("video/mp4"),"webm"=>Some("video/webm"),"mov"=>Some("video/quicktime"),"mkv"=>Some("video/x-matroska"),"ogv"=>Some("video/ogg"),"avi"=>Some("video/x-msvideo"),_=>None});
 match m.as_str(){"video/mp4"=>Some("video/mp4"),"video/webm"=>Some("video/webm"),"video/quicktime"=>Some("video/quicktime"),"video/x-matroska"=>Some("video/x-matroska"),"video/ogg"=>Some("video/ogg"),"video/x-msvideo"=>Some("video/x-msvideo"),""|"application/octet-stream"|"binary/octet-stream"=>by_ext(),_ if m.starts_with("video/")=>None,_=>None}
}
pub fn load(input:&Value)->Result<Value>{
 let a=&input["attachment"];let env=text(input,"environment");let id=text(a,"id");let name=text(a,"name");
 if !valid_id(env)||!valid_id(id){return Err("invalid_video".into())}
 let kind=mime(name,text(a,"mimeType")).ok_or("invalid_video")?;
 let ext=match kind{"video/mp4"=>"mp4","video/webm"=>"webm","video/quicktime"=>"mov","video/x-matroska"=>"mkv","video/ogg"=>"ogv",_=>"avi"};
 let root=xdg("XDG_CACHE_HOME",".cache").join("videos");std::fs::create_dir_all(&root).map_err(|_|"video_cache_failed")?;std::fs::set_permissions(&root,std::fs::Permissions::from_mode(0o700)).map_err(|_|"video_cache_failed")?;
 let file=root.join(format!("{:x}.{ext}",Sha256::digest(format!("{env}/{id}").as_bytes())));
 let cached=std::fs::symlink_metadata(&file).is_ok_and(|m|m.is_file()&&m.uid()==unsafe{libc::geteuid()}&&m.mode()&0o077==0&&m.len()>0&&m.len()<=LIMIT);
 if !cached{
  let e=environments()?.into_iter().find(|e|e.id==env).ok_or("unknown_environment")?;
  let bytes=attachment_bytes(&e,id,name,text(a,"mimeType"),LIMIT)?;
  let tmp=root.join(format!(".{}.tmp",uuid::Uuid::new_v4()));let mut f=std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&tmp).map_err(|_|"video_cache_failed")?;
  if f.write_all(&bytes).and_then(|_|std::fs::rename(&tmp,&file)).is_err(){let _=std::fs::remove_file(&tmp);return Err("video_cache_failed".into())}
  // Keep at most 16 videos / 512 MiB, oldest first.
  let mut entries=std::fs::read_dir(&root).map_err(|_|"video_cache_failed")?.flatten().filter_map(|e|e.metadata().ok().filter(|m|m.is_file()).map(|m|(e.path(),m.len(),m.modified().unwrap_or(std::time::UNIX_EPOCH)))).collect::<Vec<_>>();entries.sort_by_key(|e|std::cmp::Reverse(e.2));
  let mut total=0u64;for (i,(path,len,_)) in entries.iter().enumerate(){total+=len;if i>=16||total>512*1024*1024{let _=std::fs::remove_file(path);}}
 }
 Ok(json!({"mimeType":kind,"url":reqwest::Url::from_file_path(file).map_err(|_|"video_cache_failed")?.to_string()}))
}
#[cfg(test)]mod tests{use super::*;
 #[test]fn video_types_follow_t3(){assert_eq!(mime("a.mp4","video/mp4"),Some("video/mp4"));assert_eq!(mime("clip.MOV","application/octet-stream"),Some("video/quicktime"));assert_eq!(mime("x.mp4","text/plain"),None);assert_eq!(mime("x.txt",""),None);assert_eq!(mime("x.webm","video/webm; codecs=vp9"),Some("video/webm"));}
}
