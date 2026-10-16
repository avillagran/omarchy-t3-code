//! Minimal native settings surface shared with Omi. Never exports provider credentials.
use serde_json::{json,Value};
const BOOLS:&[(&str,&str)]=&[("enableProviderUpdateChecks","Check provider updates"),("continueThreadsAfterServerUpdate","Resume after server updates"),("sidebarAutoSettleKeepPullRequests","Keep threads with pull requests"),("sidebarAutoSettleOnMerge","Settle merged pull requests"),("snoozeLimitedThreads","Snooze rate-limited threads")];
pub fn snapshot(raw:&Value)->Value{
 let mut fields=Vec::new();
 for (key,label) in BOOLS{if let Some(value)=raw[*key].as_bool(){fields.push(json!({"key":key,"label":label,"kind":"bool","value":value}))}}
 for (key,label,choices) in [("responseStreamingMode","Response streaming",json!([{"id":"turn","name":"Turn"},{"id":"paragraph","name":"Paragraph"}])),("backgroundActivityProfile","Background activity",json!([{"id":"balanced","name":"Balanced"},{"id":"performance","name":"Performance"},{"id":"battery-saver","name":"Battery saver"},{"id":"custom","name":"Custom"}]))]{
  if raw[key].is_string(){let value=if key=="backgroundActivityProfile"{raw["backgroundActivity"]["profile"].as_str().unwrap_or_else(||raw[key].as_str().unwrap())}else{raw[key].as_str().unwrap()};fields.push(json!({"key":key,"label":label,"kind":"choice","value":value,"choices":choices}))}
 }
 json!({"fields":fields})
}
pub fn patch(input:&Value)->Result<Value,String>{
 let key=input["key"].as_str().ok_or("invalid_settings")?;let value=&input["value"];
 let valid=if BOOLS.iter().any(|(k,_)|*k==key){value.is_boolean()}else{match key{"responseStreamingMode"=>matches!(value.as_str(),Some("turn"|"paragraph")),"backgroundActivityProfile"=>matches!(value.as_str(),Some("balanced"|"performance"|"battery-saver"|"custom")),_=>false}};
 if !valid{return Err("invalid_settings".into())}
 if key=="backgroundActivityProfile"{return Ok(json!({"patch":{"backgroundActivity":{"schemaVersion":1,"profile":value}}}))}
 Ok(json!({"patch":{key:value}}))
}
#[cfg(test)]mod tests{use super::*;#[test]fn explicit_native_settings_only(){let s=snapshot(&json!({"enableProviderUpdateChecks":true,"providerInstances":{"secret":"do-not-export"},"responseStreamingMode":"paragraph"}));assert_eq!(s["fields"].as_array().unwrap().len(),2);assert!(!s.to_string().contains("do-not-export"));assert_eq!(patch(&json!({"key":"responseStreamingMode","value":"turn"})).unwrap()["patch"]["responseStreamingMode"],"turn");assert!(patch(&json!({"key":"providerInstances","value":{}})).is_err());assert!(patch(&json!({"key":"enableProviderUpdateChecks","value":"true"})).is_err());assert_eq!(patch(&json!({"key":"backgroundActivityProfile","value":"battery-saver"})).unwrap()["patch"]["backgroundActivity"]["profile"],"battery-saver");}}
