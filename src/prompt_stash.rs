//! Port of T3's promptStashStore / ComposerStashMenu semantics: local,
//! provider-agnostic prompts, restored into the current composer. Project scope
//! and existing per-thread drafts are desktop adapters. No server mutation.
use crate::drafts;
use omarchy_t3_client::*;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{fs, io::Read, os::{fd::AsRawFd, unix::fs::{OpenOptionsExt, PermissionsExt}}, path::PathBuf};
const MAX_ENTRIES:usize=20;
struct Store {path:PathBuf,data:Value,_lock:fs::File}
fn has_content(d:&Value)->bool{!text(d,"text").trim().is_empty()||d["attachments"].as_array().is_some_and(|a|!a.is_empty())}
fn locked(d:&Value)->bool{d["attempt"].is_object()||d["queueEdit"].is_object()||d["attachments"].as_array().into_iter().flatten().any(|f|f["uploading"]==true||f["error"].is_string())}
fn empty(d:&Value)->Value{let mut d=d.clone();d["text"]=json!("");d["attachments"]=json!([]);d}
fn snippet(s:&str)->String{s.trim().chars().take(300).collect()}
fn fingerprint(d:&Value)->String{format!("{:x}",Sha256::digest(d.to_string()))}
impl Store{
 fn open(env:&str,project:&str)->Result<Self>{
  if !valid_id(env)||!valid_id(project){return Err("invalid_project".into())}
  let dir=xdg("XDG_STATE_HOME",".local/state").join("prompt-stash");fs::create_dir_all(&dir).map_err(|_|"draft_save_failed")?;fs::set_permissions(&dir,fs::Permissions::from_mode(0o700)).map_err(|_|"draft_save_failed")?;
  let id=fingerprint(&json!([env,project]));
  let lock=fs::OpenOptions::new().read(true).write(true).create(true).mode(0o600).open(dir.join(format!("{id}.lock"))).map_err(|_|"draft_save_failed")?;
  if unsafe{libc::flock(lock.as_raw_fd(),libc::LOCK_EX|libc::LOCK_NB)}!=0{return Err("drafts_busy".into())}
  let path=dir.join(format!("{id}.json"));
  let data=match fs::File::open(&path){
   Ok(f)=>{let mut bytes=vec![];f.take(8*1024*1024+1).read_to_end(&mut bytes).map_err(|_|"draft_read_failed")?;if bytes.len()>8*1024*1024{return Err("draft_read_failed".into())}let v:Value=serde_json::from_slice(&bytes).map_err(|_|"draft_read_failed")?;
    if v["version"]!=1||v["environment"]!=env||v["projectId"]!=project||!v["entries"].is_array(){return Err("draft_read_failed".into())}v},
   Err(e)if e.kind()==std::io::ErrorKind::NotFound=>json!({"version":1,"environment":env,"projectId":project,"entries":[],"pending":null}),
   Err(_)=>return Err("draft_read_failed".into())
  };Ok(Self{path,data,_lock:lock})
 }
 fn commit(&self)->Result<()>{if self.data.to_string().len()>8*1024*1024{return Err("draft_too_large".into())}save_json(&self.path,&self.data).map_err(|_|"draft_save_failed".into())}
 fn key(&self,key:&str)->Result<()>{let(env,id)=key.split_once('/').ok_or("invalid_draft_key")?;if env!=text(&self.data,"environment")||!valid_id(id){return Err("invalid_draft_key".into())}Ok(())}
 fn recover(&mut self)->Result<Value>{
  let pending=self.data["pending"].clone();if !pending.is_object(){return Ok(json!({}))}
  // Validate the complete transaction before touching either composer. External
  // edits make recovery stop rather than clearing a newer prompt.
  for c in pending["changes"].as_array().ok_or("draft_read_failed")?{let key=text(c,"key");self.key(key)?;let now=drafts::load(key)?;if now!=c["before"]&&now!=c["after"]{return Err("draft_changed".into())}}
  let mut changes=json!({});
  for c in pending["changes"].as_array().unwrap(){let key=text(c,"key");drafts::save(key,&c["after"])?;changes[key]=c["after"].clone();}
  self.data["entries"]=pending["entries"].clone();self.data["lastRequestId"]=pending["requestId"].clone();self.data["lastChanges"]=changes.clone();self.data["pending"]=Value::Null;self.commit()?;Ok(changes)
 }
 fn list(&self,keys:&[Value])->Result<Vec<Value>>{
  let mut result=self.data["entries"].as_array().unwrap().iter().map(|e|json!({"id":e["id"],"prompt":snippet(text(e,"prompt")),"attachmentCount":e["attachments"].as_array().map_or(0,Vec::len),"createdAt":e["createdAt"],"locked":false})).collect::<Vec<_>>();
  for key in keys{let key=key.as_str().ok_or("invalid_draft_key")?;self.key(key)?;let d=drafts::load(key)?;
   if has_content(&d){result.push(json!({"id":format!("thread:{key}"),"sourceKey":key,"prompt":snippet(text(&d,"text")),"attachmentCount":d["attachments"].as_array().map_or(0,Vec::len),"fingerprint":fingerprint(&d),"locked":locked(&d)}));}
  }Ok(result)
 }
}
// Before a composer is read after restart, finish any interrupted transfer
// involving it. An unrecoverable conflict blocks loading instead of exposing a
// half-applied prompt that a subsequent autosave could overwrite.
pub fn recover_before_load(key:&str,project:&str)->Result<()> {
 if project.is_empty(){return Ok(())}
 let env=key.split_once('/').ok_or("invalid_draft_key")?.0;
 let mut store=Store::open(env,project)?;store.key(key)?;store.recover()?;Ok(())
}
pub fn execute(cmd:&Value)->Result<Value>{
 let mut store=Store::open(text(cmd,"environment"),text(cmd,"projectId"))?;
 let keys=cmd["keys"].as_array().ok_or("invalid_draft_key")?;if keys.len()>5000{return Err("draft_too_large".into())}
 for key in keys{store.key(key.as_str().ok_or("invalid_draft_key")?)?;}
 let key=text(cmd,"key");store.key(key)?;if !keys.iter().any(|k|k==key){return Err("invalid_draft_key".into())}
 let request=text(cmd,"requestId");uuid::Uuid::parse_str(request).map_err(|_|"invalid_request_id")?;
 let mut changes=store.recover()?;
 if store.data["lastRequestId"]==request{changes=store.data["lastChanges"].clone();}
 else if text(cmd,"action")!="list"{
  let current=drafts::load(key)?;let expected=drafts::clean(&cmd["draft"])?;
  if current!=expected{return Err("draft_changed".into())}if locked(&current){return Err("draft_locked".into())}
  let mut entries=store.data["entries"].as_array().unwrap().clone();let mut steps=vec![];
  match text(cmd,"action"){
   "stash"=>{
    if !has_content(&current){return Err("draft_empty".into())}
    // Native cap, but never silently evict an existing user prompt.
    if entries.len()>=MAX_ENTRIES{return Err("draft_stash_full".into())}
    entries.insert(0,json!({"id":request,"createdAt":chrono::Utc::now().to_rfc3339(),"prompt":current["text"],"attachments":current["attachments"]}));
    steps.push(json!({"key":key,"before":current,"after":empty(&current)}));
   },
   "restore"|"delete"=>{
    let id=text(cmd,"id");let mut source=None;
    let chosen=if let Some(source_key)=id.strip_prefix("thread:"){
     if !keys.iter().any(|k|k==source_key){return Err("invalid_draft_key".into())}store.key(source_key)?;
     let d=drafts::load(source_key)?;
     if fingerprint(&d)!=text(cmd,"fingerprint"){return Err("draft_changed".into())}
     if locked(&d){return Err("draft_locked".into())}
     source=Some((source_key.to_owned(),d.clone()));json!({"prompt":d["text"],"attachments":d["attachments"]})
    }else{let index=entries.iter().position(|e|e["id"]==id).ok_or("draft_not_found")?;entries.remove(index)};
    if text(cmd,"action")=="restore"{
     if source.as_ref().is_some_and(|(src,_)|src==key){return Ok(json!({"entries":store.list(keys)?,"changes":changes}))}
     let mut next=current.clone();let prompt=text(&chosen,"prompt");let before=text(&current,"text");
     next["text"]=json!(if prompt.is_empty(){before.to_owned()}else if before.trim().is_empty(){prompt.to_owned()}else{format!("{}\n\n{prompt}",before.trim_end())});
     let files=next["attachments"].as_array_mut().unwrap();
     for f in chosen["attachments"].as_array().into_iter().flatten(){if !files.iter().any(|existing|existing["attachment"]["id"].is_string()&&existing["attachment"]["id"]==f["attachment"]["id"]){files.push(f.clone());}}
     if files.len()>100{return Err("invalid_attachments".into())}next=drafts::clean(&next)?;
     steps.push(json!({"key":key,"before":current,"after":next}));
    }
    if let Some((source_key,d))=source{steps.push(json!({"key":source_key,"before":d,"after":empty(&d)}));}
   },
   _=>return Err("invalid_draft_action".into())
  }
  store.data["pending"]=json!({"requestId":request,"entries":entries,"changes":steps});store.commit()?;changes=store.recover()?;
 }
 Ok(json!({"entries":store.list(keys)?,"changes":changes}))
}

pub fn legacy_entries(environment:&str,project:&str)->Result<Vec<Value>>{
 let mut store=Store::open(environment,project)?;store.recover()?;
 Ok(store.data["entries"].as_array().cloned().unwrap_or_default())
}
