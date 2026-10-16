//! Model picker catalog and settings contract, shared with Omi. No client-local favorites.
use serde_json::{json,Value};
use crate::{Connection,Result};
fn text<'a>(v:&'a Value,k:&str)->&'a str{v[k].as_str().unwrap_or("")}
pub fn preferences(settings:&Value)->Value{
 let favorites=settings["favorites"].as_array().into_iter().flatten().filter_map(|f|{let p=text(f,"provider");let m=text(f,"model");if p.is_empty()||m.is_empty(){None}else{Some(json!({"provider":p,"model":m}))}}).collect::<Vec<_>>();
 json!({"favorites":favorites,"favoritesReady":settings["favorites"].is_array()})
}
fn registry_icon(config:&Value,p:&Value)->String{
 if text(p,"driver")!="acpRegistry"{return String::new()}let cfg=&config["settings"]["providerInstances"][text(p,"instanceId")]["config"];if cfg["source"]=="local"{return String::new()}
 for raw in [text(cfg,"registryIconUrl"),text(p,"iconUrl")]{if let Ok(u)=reqwest::Url::parse(raw){if u.scheme()=="https"&&u.host_str()==Some("cdn.agentclientprotocol.com")&&u.username().is_empty()&&u.password().is_none()&&u.port().is_none()&&u.query().is_none()&&u.fragment().is_none()&&u.path().starts_with("/registry/v1/")&&u.path().ends_with(".svg"){return u.to_string()}}}
 let id=text(cfg,"agentId");if !id.is_empty()&&id.len()<=128&&id.chars().next().is_some_and(|c|c.is_ascii_lowercase()||c.is_ascii_digit())&&id.chars().all(|c|c.is_ascii_lowercase()||c.is_ascii_digit()||"._-".contains(c)){format!("https://cdn.agentclientprotocol.com/registry/v1/latest/{id}.svg")}else{String::new()}
}
pub fn catalog(config:&Value)->Value{
 let prefs=preferences(&config["settings"]);let mut models=vec![];let mut providers=vec![];
 for p in config["providers"].as_array().into_iter().flatten().filter(|p|p["enabled"]==true){
  let id=text(p,"instanceId");if id.is_empty(){continue}let ready=(text(p,"status").is_empty()||text(p,"status")=="ready")&&p["availability"]!="unavailable"&&p["installed"]!=false;
  providers.push(json!({"registryIcon":registry_icon(config,p),"instanceId":id,"name":p["displayName"],"driverKind":p["driver"],"ready":ready,"accentColor":p["accentColor"]}));
  let mp=&config["settings"]["providerModelPreferences"][id];let hidden=mp["hiddenModels"].as_array().cloned().unwrap_or_default();let order=mp["modelOrder"].as_array().cloned().unwrap_or_default();
  let mut list=vec![];
  for m in p["models"].as_array().into_iter().flatten(){
   let slug=text(m,"slug");if models.iter().chain(list.iter()).any(|v:&Value|v["instanceId"]==id&&v["model"]==slug){continue}if slug.is_empty()||hidden.iter().any(|v|v==slug){continue}
   list.push(json!({"registryIcon":registry_icon(config,p),"instanceId":id,"providerName":p["displayName"],"driverKind":p["driver"],"model":slug,"name":format!("{} · {}",text(p,"displayName"),text(m,"name")),"displayName":m["name"],"shortName":m["shortName"],"subProvider":m["subProvider"],"isLegacy":m["isLegacy"],"badge":m["badge"],"available":ready,"descriptors":m["capabilities"]["optionDescriptors"]}));
  }
  list.sort_by_key(|m|order.iter().position(|v|v==&m["model"]).unwrap_or(usize::MAX));models.extend(list);
 }
 json!({"models":models,"providers":providers,"favorites":prefs["favorites"],"favoritesReady":prefs["favoritesReady"],"capabilities":{"threadSettlement":config["environment"]["capabilities"]["threadSettlement"]==true,"threadSnooze":config["environment"]["capabilities"]["threadSnooze"]==true,"threadPinning":config["environment"]["capabilities"]["threadPinning"]==true,"threadAutoSettleOptOut":config["environment"]["capabilities"]["threadAutoSettleOptOut"]==true,"threadTitleRegeneration":config["environment"]["capabilities"]["threadTitleRegeneration"]==true}})
}
pub fn favorite_payload(provider:&str,model:&str,favorite:bool)->Result<Value>{
 if provider.is_empty()||provider.len()>512||model.is_empty()||model.len()>512||provider.chars().chain(model.chars()).any(char::is_control){return Err("invalid_model".into())}
 Ok(json!({"patch":{"setModelFavorites":[{"provider":provider,"model":model,"favorite":favorite}]}}))
}
impl Connection{
 pub fn model_catalog(&self)->Result<Value>{self.rpc("server.getConfig",json!({})).map(|c|catalog(&c))}
 pub fn set_model_favorite(&self,provider:&str,model:&str,favorite:bool)->Result<Value>{let patch=favorite_payload(provider,model,favorite)?;let current=self.rpc("server.getSettings",json!({}))?;if !current["favorites"].is_array(){return Err("model_preferences_uninitialized".into())}self.rpc("server.updateSettings",patch).map(|v|preferences(&v))}
}
#[cfg(test)]mod tests{use super::*;#[test]fn registry_icons_follow_official_metadata_only(){let mut config=json!({"settings":{"providerInstances":{"h":{"config":{"agentId":"hermes"}}}}});let mut p=json!({"driver":"acpRegistry","instanceId":"h","iconUrl":"https://user:secret@cdn.agentclientprotocol.com/registry/v1/x.svg"});assert_eq!(registry_icon(&config,&p),"https://cdn.agentclientprotocol.com/registry/v1/latest/hermes.svg");config["settings"]["providerInstances"]["h"]["config"]["agentId"]=json!("../private");assert_eq!(registry_icon(&config,&p),"");p["iconUrl"]=json!("https://cdn.agentclientprotocol.com.evil/registry/v1/x.svg");assert_eq!(registry_icon(&config,&p),"");p["iconUrl"]=json!("https://cdn.agentclientprotocol.com/registry/v1/latest/hermes.svg");assert!(!registry_icon(&config,&p).is_empty());config["settings"]["providerInstances"]["h"]["config"]["source"]=json!("local");assert_eq!(registry_icon(&config,&p),"");}
#[test]fn native_preferences_keep_identity_hidden_order_and_no_credentials(){let c=json!({"settings":{"favorites":[{"provider":"personal","model":"same"}],"providerModelPreferences":{"personal":{"hiddenModels":["hidden"],"modelOrder":["same","other"]}}},"providers":[{"enabled":true,"instanceId":"personal","displayName":"Personal","status":"ready","auth":{"token":"secret-do-not-export"},"models":[{"slug":"other","name":"Other"},{"slug":"hidden","name":"Hidden"},{"slug":"same","name":"Same"}]},{"enabled":true,"instanceId":"work","status":"warning","models":[{"slug":"same","name":"Work same"}]},{"enabled":false,"instanceId":"disabled","models":[{"slug":"bad"}]}]});let v=catalog(&c);assert_eq!(v["models"].as_array().unwrap().len(),3);assert_eq!(v["models"][0]["model"],"same");assert_eq!(v["models"][2]["available"],false);assert_eq!(v["favorites"][0]["provider"],"personal");assert!(!v.to_string().contains("secret-do-not-export"));assert_eq!(preferences(&json!({"favorites":null}))["favoritesReady"],false);let p=favorite_payload("personal","same",true).unwrap();assert_eq!(p["patch"]["setModelFavorites"][0]["favorite"],true);assert!(p["patch"].get("favorites").is_none());}}
