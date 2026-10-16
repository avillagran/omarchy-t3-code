use serde_json::json;
use t3_protocol::lifecycle::describe;
#[test]fn native_and_legacy_handoff_models_are_preserved(){
 let runs=json!([{"id":"old","ordinal":1,"providerInstanceId":"a","modelSelection":{"model":"old-model"}},{"id":"target","ordinal":2,"providerInstanceId":"b","modelSelection":{"model":"target-model"}},{"id":"future","ordinal":3,"providerInstanceId":"a","modelSelection":{"model":"future-model"}}]);
 let item=json!({"type":"handoff","status":"completed","runId":"target","fromProviderInstanceIds":["a"],"toProviderInstanceId":"b"});
 let d=describe(&item,runs.as_array().unwrap()).unwrap();assert_eq!(d["from"][0]["model"],"old-model");assert_eq!(d["to"]["model"],"target-model");assert_eq!(d["active"],false);
 let mut stamped=item.clone();stamped["fromModelSelections"]=json!([{"instanceId":"a","model":"recorded"}]);stamped["toModel"]=json!("recorded-target");let d=describe(&json!({"item":stamped}),runs.as_array().unwrap()).unwrap();assert_eq!(d["from"][0]["model"],"recorded");assert_eq!(d["to"]["model"],"recorded-target");
 assert_eq!(describe(&json!({"type":"compaction","status":"running"}),&[]).unwrap()["active"],true);
 assert!(describe(&json!({"type":"command_execution"}),&[]).is_none());
}
