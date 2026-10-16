//! Private immutable attachment cache. UI only receives local file URLs.
use crate::*;
use serde_json::{json,Value};
use std::{io::Write,os::unix::fs::{OpenOptionsExt,PermissionsExt,MetadataExt}};
pub fn load(input:&Value)->Result<Value>{
 let a=&input["attachment"];let env=text(input,"environment");let id=text(a,"id");let mime=text(a,"mimeType");
 if !valid_id(env)||!valid_id(id){return Err("invalid_image".into())}
 let ext=match mime{"image/png"=>"png","image/jpeg"=>"jpg","image/gif"=>"gif","image/webp"=>"webp",_=>return Err("invalid_image".into())};
 let root=xdg("XDG_CACHE_HOME",".cache").join("images");std::fs::create_dir_all(&root).map_err(|_|"image_cache_failed")?;std::fs::set_permissions(&root,std::fs::Permissions::from_mode(0o700)).map_err(|_|"image_cache_failed")?;
 let file=root.join(format!("{:x}.{ext}",Sha256::digest(format!("{env}/{id}").as_bytes())));
 let cached=std::fs::symlink_metadata(&file).is_ok_and(|m|m.is_file()&&m.uid()==unsafe{libc::geteuid()}&&m.mode()&0o077==0&&m.len()>0&&m.len()<=10*1024*1024);
 if !cached{
  let e=environments()?.into_iter().find(|e|e.id==env).ok_or("unknown_environment")?;
  let c=if let Some(c)=&e.connection{c.as_ref().clone()}else{protocol::Connection::new(&e.base_url,None,protocol::Credential::bearer(&environment_token(&e,true)?)?)?};
  let bytes=c.attachment_image(id,text(a,"name"),mime)?;
  let tmp=root.join(format!(".{}.tmp",uuid::Uuid::new_v4()));let mut f=std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&tmp).map_err(|_|"image_cache_failed")?;if f.write_all(&bytes).and_then(|_|std::fs::rename(&tmp,&file)).is_err(){let _=std::fs::remove_file(tmp);return Err("image_cache_failed".into())}
  // Keep at most128 images /128MiB. Never inspect paths outside this private cache.
  let mut entries=std::fs::read_dir(&root).map_err(|_|"image_cache_failed")?.flatten().filter_map(|e|e.metadata().ok().filter(|m|m.is_file()).map(|m|(e.path(),m.len(),m.modified().unwrap_or(std::time::UNIX_EPOCH)))).collect::<Vec<_>>();entries.sort_by_key(|e|std::cmp::Reverse(e.2));let mut total=0u64;for (i,(path,size,_)) in entries.into_iter().enumerate(){total+=size;if i>=128||total>128*1024*1024{let _=std::fs::remove_file(path);}}
 }
 Ok(json!({"url":reqwest::Url::from_file_path(file).map_err(|_|"image_cache_failed")?.to_string()}))
}
