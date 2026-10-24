//! Private immutable attachment cache. UI only receives local file URLs.
use crate::*;
use serde_json::{json,Value};
use std::{io::Write,os::unix::fs::{OpenOptionsExt,PermissionsExt,MetadataExt}};
pub fn load(input:&Value)->Result<Value>{
 let a=&input["attachment"];let env=text(input,"environment");let id=text(a,"id");let mime=text(a,"mimeType");
 if !valid_id(env)||!valid_id(id){return Err("invalid_image".into())}
 let workspace=!text(a,"source").is_empty();
 let ext=if workspace{"image"}else{match mime{"image/png"=>"png","image/jpeg"=>"jpg","image/gif"=>"gif","image/webp"=>"webp","image/heic"=>"heic","image/heif"=>"heif","image/avif"=>"avif",_=>return Err("invalid_image".into())}};
 let root=xdg("XDG_CACHE_HOME",".cache").join("images");std::fs::create_dir_all(&root).map_err(|_|"image_cache_failed")?;std::fs::set_permissions(&root,std::fs::Permissions::from_mode(0o700)).map_err(|_|"image_cache_failed")?;
 let file=root.join(format!("{:x}.{ext}",Sha256::digest(format!("{env}/{}/{id}/{}",if workspace{text(input,"thread")}else{""},if workspace{uuid::Uuid::new_v4().to_string()}else{String::new()}).as_bytes())));
 let cached=!workspace && std::fs::symlink_metadata(&file).is_ok_and(|m|m.is_file()&&m.uid()==unsafe{libc::geteuid()}&&m.mode()&0o077==0&&m.len()>0&&m.len()<=10*1024*1024);
 if !cached{
  let e=environments()?.into_iter().find(|e|e.id==env).ok_or("unknown_environment")?;
  let c=e.connection(true)?;
  let bytes=if workspace{let source=text(a,"source");if source.starts_with("https://")||source.starts_with("http://")||source.starts_with("data:"){direct(source)?}else{let path=markdown_images::workspace_path(source)?;c.media_image(text(input,"thread"),&path)?.0}}else{c.attachment_image(id,text(a,"name"),mime)?};
  let tmp=root.join(format!(".{}.tmp",uuid::Uuid::new_v4()));let mut f=std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&tmp).map_err(|_|"image_cache_failed")?;if f.write_all(&bytes).and_then(|_|std::fs::rename(&tmp,&file)).is_err(){let _=std::fs::remove_file(tmp);return Err("image_cache_failed".into())}
  // Keep at most128 images /128MiB. Never inspect paths outside this private cache.
  let mut entries=std::fs::read_dir(&root).map_err(|_|"image_cache_failed")?.flatten().filter_map(|e|e.metadata().ok().filter(|m|m.is_file()).map(|m|(e.path(),m.len(),m.modified().unwrap_or(std::time::UNIX_EPOCH)))).collect::<Vec<_>>();entries.sort_by_key(|e|std::cmp::Reverse(e.2));let mut total=0u64;for (i,(path,size,_)) in entries.into_iter().enumerate(){total+=size;if i>=128||total>128*1024*1024{let _=std::fs::remove_file(path);}}
 }
 // HEIC/AVIF from phones: the image crate cannot size them, Qt's kimageformats draws them.
 let dimensions=image::ImageReader::open(&file).ok().and_then(|r|r.with_guessed_format().ok()).and_then(|r|r.into_dimensions().ok()).unwrap_or((640,480));
 if dimensions.0==0||dimensions.1==0||u64::from(dimensions.0)*u64::from(dimensions.1)>40_000_000{return Err("invalid_image".into())}
 Ok(json!({"width":dimensions.0,"height":dimensions.1,"url":crate::Url::from_file_path(file).map_err(|_|"image_cache_failed")?.to_string()}))
}

// Public Markdown media carries no environment credentials. Host-local files
// always go through media_image, including file: URLs and absolute paths.
fn direct(source:&str)->Result<Vec<u8>>{
 use base64::Engine;
 let bytes=if source.starts_with("data:"){
  let (header,body)=source.split_once(',').ok_or("invalid_image")?;
  if !matches!(header,"data:image/png;base64"|"data:image/jpeg;base64"|"data:image/gif;base64"|"data:image/webp;base64")||body.len()>14*1024*1024{return Err("invalid_image".into())}
  base64::engine::general_purpose::STANDARD.decode(body).map_err(|_|"invalid_image")?
 }else{
  let u=crate::Url::parse(source).map_err(|_|"invalid_image")?;
  if !u.username().is_empty()||u.password().is_some(){return Err("invalid_image".into())}
  let http=reqwest::blocking::Client::builder().no_proxy().redirect(reqwest::redirect::Policy::limited(3)).timeout(std::time::Duration::from_secs(20)).build().map_err(|_|"image_unreachable")?;
  let response=http.get(u).send().map_err(|_|"image_unreachable")?;
  if !response.status().is_success(){return Err("image_unavailable".into())}
  let mut bytes=Vec::new();response.take(10*1024*1024+1).read_to_end(&mut bytes).map_err(|_|"image_unavailable")?;bytes
 };
 if bytes.len()>10*1024*1024{return Err("image_limit_10MiB".into())}
 if image::guess_format(&bytes).ok().is_none_or(|f|!matches!(f,image::ImageFormat::Png|image::ImageFormat::Jpeg|image::ImageFormat::Gif|image::ImageFormat::WebP)){return Err("invalid_image".into())}
 Ok(bytes)
}
