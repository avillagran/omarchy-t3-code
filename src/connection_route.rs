//! On-demand native load balancing. No mutation occurs while choosing a host.
use crate::*;
use serde_json::{json,Value};
/// Candidate list is filtered against the current project identity before any
/// resource/model probe. A draft with uploaded assets or an explicit host stays put.
pub fn eligible_projects(projects:&[Value],source_environment:&str,source_project:&str)->Vec<Value>{
 let Some(source)=projects.iter().find(|p|p["environmentId"]==source_environment&&p["id"]==source_project)else{return vec![]};
 let grouping=|p:&Value|p["repositoryIdentity"]["origin"]["canonicalKey"].as_str().or(p["repositoryIdentity"]["canonicalKey"].as_str()).filter(|v|!v.trim().is_empty()).map(str::to_owned);
 let Some(identity)=grouping(source)else{return vec![]};
 projects.iter().filter(|p|!p["isScratch"].as_bool().unwrap_or(false)&&grouping(p).as_ref()==Some(&identity)).cloned().collect()
}
pub fn provider_eligible(config:&Value,project:&str,instance:&str,model:&str)->bool{
 let Some(provider)=config["providers"].as_array().into_iter().flatten().find(|p|p["instanceId"]==instance)else{return false};
 let enabled=config["settings"]["projectSettingsOverrides"][project]["providerInstances"][instance]["enabled"].as_bool().or(config["settings"]["providerInstances"][instance]["enabled"].as_bool()).unwrap_or(provider["enabled"]==true);
 enabled&&provider["installed"]!=false&&provider["status"]!="error"&&provider["auth"]["status"]!="unauthenticated"&&provider["availability"]!="unavailable"&&provider["models"].as_array().is_some_and(|models|models.iter().any(|m|m["slug"]==model))
}
pub fn choose(input:&Value)->Result<Value>{
 let preferences=connection_preferences::read_client_preferences()?;
 let fallback=json!({"environment":input["environment"],"projectId":input["projectId"],"modelSelection":input["modelSelection"],"balanced":false});
 if preferences["loadBalancingEnabled"]!=true||input["automatic"]!=true||input["attachments"].as_array().is_some_and(|v|!v.is_empty())||input["explicitEnvironment"]==true{return Ok(fallback)}
 let known=environments()?;let client=Client::new()?;
 let mut projects=vec![];
 // Use native snapshots rather than trusting a GUI-supplied repository match.
 for e in known.iter().filter(|e|e.connection_error.is_none()){
  if let Ok(page)=client.get(e,"/api/projects",false){for p in page["projects"].as_array().into_iter().flatten(){let mut p=p.clone();p["environmentId"]=json!(e.id);projects.push(p);}}
 }
 let eligible=eligible_projects(&projects,text(input,"environment"),text(input,"projectId"));
 if eligible.iter().map(|p|text(p,"environmentId")).collect::<std::collections::HashSet<_>>().len()<2{return Ok(fallback)}
 let primary=fs::read_to_string(t3_home().join("userdata/environment-id")).unwrap_or_default();
 let provider=text(&input["modelSelection"],"instanceId");let model=text(&input["modelSelection"],"model");
 let results=std::thread::scope(|scope|{
  let mut handles=vec![];
  for p in &eligible{
   let Some(e)=known.iter().find(|e|e.id==text(p,"environmentId"))else{continue};
   let canonical=if e.id=="local"{primary.trim()}else{&e.id};let weight=preferences["loadBalancingWeights"][canonical].as_u64().unwrap_or(50);
   if weight==0{continue}
   handles.push(scope.spawn(move||{
    let config=write::rpc(e,"server.getConfig",json!({})).ok()?;
    if !provider_eligible(&config,text(p,"id"),provider,model){return None}
    let resources=write::rpc(e,"server.getHostResources",json!({})).ok()?;
    Some((json!({"environmentId":e.id,"weight":weight,"receivedAt":Utc::now().timestamp_millis(),"resources":resources}),json!({"environment":e.id,"projectId":p["id"],"modelSelection":input["modelSelection"],"balanced":true})))
   }));
  }
  handles.into_iter().filter_map(|h|h.join().ok().flatten()).collect::<Vec<_>>()
 });
 let candidates=results.iter().map(|r|r.0.clone()).collect::<Vec<_>>();
 let Some(selected)=connection_preferences::choose_load_balanced_environment(&candidates,Utc::now().timestamp_millis())else{return Err("no_available_balanced_environment".into())};
 results.into_iter().find(|r|r.0["environmentId"]==selected).map(|r|r.1).ok_or("no_available_balanced_environment".into())
}
