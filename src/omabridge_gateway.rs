//! Public addresses for this device's T3 Code, for apps that cannot run
//! OmaBridge (T3's own mobile and desktop apps). A paired device with a public
//! IP that lets this one manage it opens an HTTPS gateway into this device's
//! T3 Code; T3's own pairing still guards it. Needs `omabridge manage`, so it
//! is reported unsupported with an OmaBridge release that predates it.
//! Only ids, names, addresses and states leave this module, never keys.
use crate::*;
use crate::omabridge_route::valid_peer;
/// The device's own OmaBridge, as the Control Panel and the OmaBridge section use it.
fn manage(request:&Value,seconds:u32)->Result<Value>{
 let (doc,status)=crate::omabridge_device::manage(request,seconds);
 if status==0{return Ok(doc)}
 let code=doc["error"].as_str().filter(|c|valid_code(c)).unwrap_or("omabridge_command_failed");
 Err(match code{"omabridge_outdated"=>"omabridge_manage_unsupported","omabridge_missing"=>"omabridge_unavailable","timeout"=>"omabridge_peer_unreachable",c=>c}.into())
}
fn valid_code(code:&str)->bool{!code.is_empty()&&code.len()<=64&&code.bytes().all(|b|b.is_ascii_lowercase()||b==b'_')}
fn valid_host(host:&str)->bool{
 host.len()<=253&&host.contains('.')&&host.split('.').all(|l|!l.is_empty()&&l.len()<=63&&!l.starts_with('-')&&!l.ends_with('-')&&l.bytes().all(|b|b.is_ascii_lowercase()||b.is_ascii_digit()||b==b'-'))
}
fn valid_url(url:&str)->bool{reqwest::Url::parse(url).is_ok_and(|u|u.scheme()=="https"&&u.host_str().is_some()&&u.username().is_empty()&&u.password().is_none()&&u.query().is_none()&&u.fragment().is_none())}
fn gateway(g:&Value)->Option<Value>{
 let url=text(g,"url");if !valid_url(url){return None}
 Some(json!({"url":url,"running":g["running"]==true,"error":g["error"].as_str().filter(|e|valid_code(e)).unwrap_or_default()}))
}
fn peers(doc:&Value)->Vec<Value>{doc["peers"].as_array().cloned().unwrap_or_default().into_iter().filter(|p|valid_peer(text(p,"id"))).collect()}
fn strings(v:&Value)->Vec<String>{v.as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect()}
/// This device's public addresses, read from each device it manages (in
/// parallel: an unreachable one only costs its own timeout), and the ones this
/// device opens for others.
fn read()->Result<Value>{
 let local=manage(&json!({"action":"read"}),10)?;
 let own=text(&local["device"],"id").to_string();
 // Devices this one manages, and any it consented to expose it publicly
 // (managed elsewhere: their addresses cannot be read from here).
 let hosts:Vec<Value>=peers(&local).into_iter().filter(|p|p["managed"]==true||strings(&p["public"]).iter().any(|s|s=="t3")).collect();
 let remote:Vec<Result<Value>>=std::thread::scope(|scope|{
  let handles:Vec<_>=hosts.iter().map(|p|{let id=text(p,"id").to_string();let managed=p["managed"]==true;scope.spawn(move||if managed{manage(&json!({"action":"remote","peer":id,"command":{"action":"read"}}),25)}else{Err("omabridge_not_managed".into())})}).collect();
  handles.into_iter().map(|h|h.join().unwrap_or_else(|_|Err("local_error".into()))).collect()
 });
 let hosts:Vec<Value>=hosts.iter().zip(remote).map(|(p,r)|{
  let granted=strings(&p["public"]).iter().any(|s|s=="t3");
  let (addresses,zones,error)=match &r{
   Ok(doc)=>(doc["remote"]["gateways"].as_array().into_iter().flatten().filter(|g|g["peer"]==json!(own)&&g["service"]=="t3").filter_map(gateway).collect::<Vec<_>>(),strings(&doc["remote"]["gatewayZones"]),String::new()),
   Err(e)=>(vec![],vec![],e.clone()),
  };
  json!({"id":p["id"],"name":text(p,"name"),"managed":p["managed"]==true,"public":granted,"gateways":addresses,"zones":zones,"error":error})
 }).collect();
 let names:std::collections::HashMap<String,String>=peers(&local).iter().map(|p|(text(p,"id").to_string(),text(p,"name").to_string())).collect();
 let hosted:Vec<Value>=local["gateways"].as_array().into_iter().flatten().filter_map(|g|{let mut v=gateway(g)?;let peer=text(g,"peer");v["peer"]=json!(peer);v["name"]=json!(names.get(peer).cloned().unwrap_or_default());v["service"]=json!(text(g,"service"));Some(v)}).collect();
 Ok(json!({"supported":true,"device":own,"hosts":hosts,"hosted":hosted,"read":Utc::now().timestamp()}))
}
fn host_peer(local:&Value,peer:&str)->Result<Value>{
 if !valid_peer(peer){return Err("invalid_command".into())}
 let p=peers(local).into_iter().find(|p|text(p,"id")==peer).ok_or("omabridge_peer_not_found")?;
 if p["managed"]!=true{return Err("omabridge_not_managed".into())}
 Ok(p)
}
fn with(list:Vec<String>,item:&str,present:bool)->Vec<String>{let mut out:Vec<String>=list.into_iter().filter(|s|s!=item).collect();if present{out.push(item.into())}out}
/// Grant T3 Code to the gateway device and consent to its public exposure,
/// then ask that device to open the address. Grants it already had are kept.
fn open(input:&Value)->Result<Value>{
 let local=manage(&json!({"action":"read"}),10)?;
 let own=text(&local["device"],"id").to_string();if !valid_peer(&own){return Err("omabridge_identity_missing".into())}
 let p=host_peer(&local,text(input,"peer"))?;
 let host=text(input,"host").trim().to_ascii_lowercase();if !host.is_empty()&&!valid_host(&host){return Err("omabridge_name_invalid".into())}
 manage(&json!({"action":"grant-set","peer":p["id"],"services":with(strings(&p["services"]),"t3",true),"public":with(strings(&p["public"]),"t3",true),"confirmed":true}),15)?;
 let mut command=json!({"action":"gateway-set","peer":own,"service":"t3","enabled":true});
 if !host.is_empty(){command["host"]=json!(host)}
 let answer=manage(&json!({"action":"remote","peer":p["id"],"command":command,"confirmed":true}),30)?;
 let url=answer["remote"]["gateway"]["url"].as_str().filter(|u|valid_url(u)).map(str::to_string)
  .or_else(||answer["remote"]["gateways"].as_array().into_iter().flatten().find(|g|g["peer"]==json!(own)&&g["service"]=="t3").map(|g|text(g,"url").to_string()).filter(|u|valid_url(u)));
 Ok(json!({"url":url,"dns":answer["remote"]["gateway"]["dns"].as_bool()}))
}
/// Withdrawing consent here closes the address at once, even if the gateway
/// device is unreachable; it is then asked to stop listening as well.
fn close(input:&Value)->Result<Value>{
 let local=manage(&json!({"action":"read"}),10)?;
 let own=text(&local["device"],"id").to_string();
 let peer=text(input,"peer");if !valid_peer(peer){return Err("invalid_command".into())}
 let p=peers(&local).into_iter().find(|p|text(p,"id")==peer).ok_or("omabridge_peer_not_found")?;
 manage(&json!({"action":"grant-set","peer":peer,"services":strings(&p["services"]),"public":with(strings(&p["public"]),"t3",false),"confirmed":true}),15)?;
 let stopped=p["managed"]==true&&valid_peer(&own)&&manage(&json!({"action":"remote","peer":peer,"command":{"action":"gateway-set","peer":own,"service":"t3","enabled":false},"confirmed":true}),30).is_ok();
 Ok(json!({"stopped":stopped}))
}
fn check(input:&Value)->Result<Value>{
 let url=text(input,"url");if !valid_url(url){return Err("invalid_command".into())}
 let doc=manage(&json!({"action":"gateway-check","url":url}),20)?;
 let c=&doc["gatewayCheck"];
 Ok(json!({"url":url,"reachable":c["reachable"]==true,"sameEnvironment":c["sameEnvironment"].as_bool(),"checked":Utc::now().timestamp()}))
}
/// Gateway actions; `None` for anything else. Opening and closing change who
/// can reach this device, so both need confirmation.
pub fn execute(action:&str,input:&Value)->Result<Option<Value>>{
 if !matches!(action,"gateways-read"|"gateway-open"|"gateway-close"|"gateway-check"){return Ok(None)}
 if matches!(action,"gateway-open"|"gateway-close")&&input["confirmed"]!=true{return Err("confirmation_required".into())}
 let outcome=match action{
  "gateways-read"=>match read(){Ok(g)=>json!({"gateways":g}),Err(e) if e=="omabridge_manage_unsupported"=>json!({"gateways":{"supported":false}}),Err(e)=>return Err(e)},
  "gateway-check"=>json!({"gatewayCheck":check(input)?}),
  "gateway-open"=>{let opened=open(input)?;json!({"gatewayOpened":opened,"gateways":read().unwrap_or(json!(null))})},
  _=>{let closed=close(input)?;json!({"gatewayClosed":closed,"gateways":read().unwrap_or(json!(null))})},
 };
 let mut snapshot=crate::omabridge_route::snapshot()?;
 if let (Some(s),Some(o))=(snapshot.as_object_mut(),outcome.as_object()){for (k,v) in o{if !v.is_null(){s.insert(k.clone(),v.clone());}}}
 Ok(Some(snapshot))
}
#[cfg(test)]
mod tests{
 use super::*;
 #[test]
 fn names_and_addresses(){
  assert!(valid_host("laptop.vpn.example.net"));assert!(!valid_host("laptop"));assert!(!valid_host("-a.example.net"));assert!(!valid_host("a..net"));assert!(!valid_host("a.b/c"));
  assert!(valid_url("https://gw.example.net:55944/"));assert!(!valid_url("http://gw.example.net/"));assert!(!valid_url("https://u:p@gw.example.net/"));assert!(!valid_url("https://gw.example.net/?x=1"));
 }
 #[test]
 fn grants_keep_other_services(){
  assert_eq!(with(vec!["ssh".into(),"t3".into()],"t3",true),vec!["ssh".to_string(),"t3".into()]);
  assert_eq!(with(vec!["ssh".into(),"t3".into()],"t3",false),vec!["ssh".to_string()]);
 }
 #[test]
 fn gateways_drop_unsafe_fields(){
  assert_eq!(gateway(&json!({"url":"https://gw.example.net:4443/","running":true,"error":"Some <b>text</b>"})),Some(json!({"url":"https://gw.example.net:4443/","running":true,"error":""})));
  assert_eq!(gateway(&json!({"url":"javascript:alert(1)"})),None);
 }
}
