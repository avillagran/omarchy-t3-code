//! Read compositor options used by control-panel. Never changes desktop settings.
use serde_json::{json,Value};use std::process::{Command,Stdio};
fn option(key:&str)->Option<Value>{let out=Command::new("timeout").args(["1","hyprctl","getoption",key,"-j"]).stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;if !out.status.success(){return None}let v:Value=serde_json::from_slice(&out.stdout).ok()?;if v["option"]!=key{return None}Some(v)}
fn device(profile:Option<&Value>,decel:Option<&Value>)->bool{profile.and_then(|v|v["int"].as_i64()).is_some_and(|n|n>0)&&decel.and_then(|v|v["int"].as_i64()).is_some_and(|n|n>0)}
pub fn read()->Value{
 let touch=option("input:touchpad:scroll_accel_profile");let mouse=option("input:mouse:scroll_accel_profile");
 let td=if touch.is_some(){option("input:touchpad:scroll_decel")}else{None};let md=if mouse.is_some(){option("input:mouse:scroll_decel")}else{None};
 json!({"touchpadNative":device(touch.as_ref(),td.as_ref()),"mouseNative":device(mouse.as_ref(),md.as_ref())})
}
#[cfg(test)]mod tests{use super::*;#[test]fn only_effective_coast_suppresses_local_fallback(){assert!(!device(None,None));assert!(!device(Some(&json!({"int":0})),Some(&json!({"int":600}))));assert!(!device(Some(&json!({"int":2})),Some(&json!({"int":0}))));assert!(device(Some(&json!({"int":2})),Some(&json!({"int":600}))));}}
