//! Explicit T3 desktop launch, then web fallback. Never passes a credential in either URL.
use crate::*;
use std::process::{Command,Stdio};
pub fn app_uri(environment:&str,thread:&str)->Result<String>{
 if !valid_id(environment)||!valid_id(thread){return Err("invalid_id".into())}
 let mut u=crate::Url::parse("t3code://threads/").unwrap();u.path_segments_mut().map_err(|_|"invalid_id")?.push(environment).push(thread);Ok(u.to_string())
}
pub fn open(environment:&str,thread:&str,web:&str)->Result<&'static str>{
 let uri=app_uri(environment,thread)?;
 for app in ["com.t3tools.T3Code","t3code"]{
  if Command::new("timeout").args(["4","gtk-launch",app,&uri]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s|s.success()){return Ok("application")}
 }
 Command::new("xdg-open").arg(web).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().map_err(|_|"local_error")?;
 Ok("browser")
}
