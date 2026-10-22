// Standalone integration test until the parent wires the module into lib.rs.
pub use omarchy_t3_client::{browser_preferences, read_catalog, t3_home, Result, MAX_BYTES};
#[path = "../src/connection_preferences.rs"]
mod preferences;
use preferences::*;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{fs, os::unix::fs::{symlink, PermissionsExt}};

fn temp_dir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("ot3-preference-fixture-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap(); dir
}
fn fixture() -> Value {
    json!({"schemaVersion":1,"targets":[
        {"_tag":"BearerConnectionTarget","environmentId":"remote","connectionId":"a","label":"Remote"},
        {"_tag":"SshConnectionTarget","environmentId":"remote","connectionId":"s","label":"Remote"},
        {"_tag":"BearerConnectionTarget","environmentId":"remote","connectionId":"learned","label":"Remote"},
        {"_tag":"RelayConnectionTarget","environmentId":"relay","label":"Relay"}],
        "profiles":[
        {"_tag":"BearerConnectionProfile","environmentId":"remote","connectionId":"a","httpBaseUrl":"https://HOST:443/path/","wsBaseUrl":"wss://host/socket/"},
        {"_tag":"SshConnectionProfile","environmentId":"remote","connectionId":"s","target":{"alias":"short","hostname":"host.example","username":"alice","port":22}},
        {"_tag":"BearerConnectionProfile","environmentId":"remote","connectionId":"learned","learned":true,"httpBaseUrl":"http://discovered/","wsBaseUrl":"ws://discovered/"}],
        "credentials":[{"connectionId":"a","credential":{"token":"PRIVATE-SECRET"}}],"remoteDpopTokens":[],
        "githubRoutingPermissions":[],"disabledEnvironmentIds":[],"futureField":{"keep":true}})
}
#[test]
fn client_settings_cas_preserves_unknown_fields_legacy_shape_and_symlinks() {
    let dir = temp_dir(); let path = dir.join("client-settings.json"); let link = dir.join("client-link.json");
    let raw = b"{\"future\":{\"untouched\":true},\"settings\":{\"chatWidth\":\"wide\",\"loadBalancingWeights\":{\"another\":75}}}\n";
    fs::write(&path, raw).unwrap(); symlink(&path, &link).unwrap();
    let rev = format!("{:x}", Sha256::digest(raw));
    let first = update_client_preferences_at(&link, &rev, &json!({"environmentId":"local","weight":100})).unwrap();
    assert_eq!(first["loadBalancingWeights"]["another"],75);
    assert_eq!(first["loadBalancingWeights"]["local"],100);
    assert!(update_client_preferences_at(&link, &rev, &json!({"loadBalancingEnabled":true})).unwrap_err().contains("changed_reload"));
    let second = update_client_preferences_at(&link, first["revision"].as_str().unwrap(), &json!({"loadBalancingEnabled":true})).unwrap();
    assert_eq!(second["loadBalancingEnabled"],true);
    assert!(link.symlink_metadata().unwrap().file_type().is_symlink());
    let persisted: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(persisted["future"]["untouched"],true);
    assert_eq!(persisted["settings"]["chatWidth"],"wide");
    assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o777,0o600);
    fs::remove_dir_all(dir).unwrap();
}
#[test]
fn invalid_settings_are_never_overwritten_and_nonpreferred_weights_are_rejected() {
    let dir=temp_dir();let path=dir.join("settings.json");
    fs::write(&path,b"{" ).unwrap();let hash=format!("{:x}",Sha256::digest(b"{"));
    assert_eq!(update_client_preferences_at(&path,&hash,&json!({"loadBalancingEnabled":false})).unwrap_err(),"invalid_client_settings");
    assert_eq!(fs::read(&path).unwrap(),b"{");
    fs::write(&path,b"{}" ).unwrap();let hash=format!("{:x}",Sha256::digest(b"{}"));
    for patch in [json!({"environmentId":"remote","weight":75}),json!({"environmentId":"remote","weight":101}),json!({"loadBalancingEnabled":"yes"}),json!({"bad":true})]{
        assert_eq!(update_client_preferences_at(&path,&hash,&patch).unwrap_err(),"invalid_client_preference");
    }
    assert_eq!(fs::read(&path).unwrap(),b"{}");fs::remove_dir_all(dir).unwrap();
}
#[test]
fn trust_key_covers_saved_routes_in_any_order_but_not_learned_routes() {
    let catalog=fixture();let key=github_connection_key(&catalog,"remote",None).unwrap();
    let mut reordered=catalog.clone();reordered["targets"].as_array_mut().unwrap().swap(0,1);
    assert_eq!(github_connection_key(&reordered,"remote",None),Some(key.clone()));
    reordered["profiles"][2]["httpBaseUrl"]=json!("http://changed-learned/");
    assert_eq!(github_connection_key(&reordered,"remote",None),Some(key.clone()));
    reordered["profiles"][0]["httpBaseUrl"]=json!("https://changed-address/");
    assert_ne!(github_connection_key(&reordered,"remote",None),Some(key.clone()));
    assert_eq!(set_github_permission(&reordered,None,"remote",&key,"read").unwrap_err(),"catalog_changed_reload");
    assert!(key.contains("https://host/path"));
    assert!(!key.contains("discovered"));
    let single=github_connection_key(&catalog,"relay",None).unwrap();
    assert_eq!(single,"[\"RelayConnectionTarget\",\"relay\"]");
}
#[test]
fn routing_projection_is_redacted_and_permission_transform_preserves_catalog() {
    let primary=json!({"_tag":"PrimaryConnectionTarget","environmentId":"local-real-id","httpBaseUrl":"http://localhost:3773","wsBaseUrl":"ws://localhost:3773"});
    assert_eq!(github_connection_key(&json!({"profiles":[]}),"local-real-id",Some(&primary)),Some("[\"PrimaryConnectionTarget\",\"local-real-id\",\"http://localhost:3773\",\"ws://localhost:3773\"]".into()));
    let catalog=fixture();
    let key=github_connection_key(&catalog,"remote",Some(&primary)).unwrap();
    let updated=set_github_permission(&catalog,Some(&primary),"remote",&key,"read-write").unwrap();
    assert_eq!(updated["futureField"],catalog["futureField"]);
    assert_eq!(updated["credentials"],catalog["credentials"]);
    assert_eq!(updated["githubRoutingPermissions"][0]["connectionKey"],key);
    let enabled=vec!["local-real-id".to_string(),"remote".to_string(),"relay".to_string()];
    let projection=github_preferences(&updated,Some(&primary),&enabled);
    assert_eq!(projection["environments"].as_array().unwrap().len(),3);
    assert_eq!(projection["environments"][1]["permission"],"read-write");
    assert!(!projection.to_string().contains("PRIVATE-SECRET"));
    assert!(!projection.to_string().contains("connectionKey"));
    let off=set_github_permission(&updated,Some(&primary),"remote",&key,"off").unwrap();
    assert!(off["githubRoutingPermissions"].as_array().unwrap().is_empty());
    assert!(set_github_permission(&catalog,None,"remote",&key,"admin").is_err());
    let mut disabled=updated.clone();disabled["disabledEnvironmentIds"]=json!(["remote"]);
    assert_eq!(github_preferences(&disabled,Some(&primary),&enabled)["environments"].as_array().unwrap().len(),2);
}
#[test]
fn selector_matches_native_capacity_freshness_and_manual_exclusion() {
    let now=100_000;
    let r=json!({"sampledAt":now,"cpuUtilization":0.2,"cpuCount":8,"availableMemoryBytes":8000,"totalMemoryBytes":16000});
    let candidates=vec![json!({"environmentId":"busy","resources":r,"weight":0}),json!({"environmentId":"stale","resources":r,"weight":100,"receivedAt":now-15_001}),json!({"environmentId":"preferred","resources":r,"weight":100,"receivedAt":now}),json!({"environmentId":"ordinary","resources":r,"weight":50,"receivedAt":now})];
    assert_eq!(choose_load_balanced_environment(&candidates,now),Some("preferred".into()));
    assert_eq!(choose_load_balanced_environment(&candidates[..2],now),None);
    let mut exhausted=candidates.clone();exhausted[2]["resources"]["cpuUtilization"]=json!(0.95);
    assert_eq!(choose_load_balanced_environment(&exhausted,now),Some("ordinary".into()));
    let settings=json!({"loadBalancingEnabled":true,"loadBalancingWeights":{"busy":0,"preferred":0,"ordinary":25}});
    assert_eq!(choose_shared_project_environment(&settings,&candidates,now),Some("ordinary".into()));
    assert_eq!(choose_shared_project_environment(&json!({"loadBalancingEnabled":false}),&candidates,now),None);
}

#[test]
fn standalone_module_exports_bridge_functions_without_touching_live_native_storage() {
    let _: fn() -> Result<Value> = read_client_preferences;
    let _: fn(&str, &Value) -> Result<Value> = update_client_preferences;
    let _: fn(Option<&Value>, &[String]) -> Result<Value> = read_github_preferences;
    let mut disabled=fixture();disabled["disabledEnvironmentIds"]=json!(["remote"]);
    let key=github_connection_key(&disabled,"remote",None).unwrap();
    assert_eq!(set_github_permission(&disabled,None,"remote",&key,"read").unwrap_err(),"environment_disabled");
}

#[test]
fn legacy_plan_flag_uses_existing_native_client_file_and_cas(){
 let dir=temp_dir();let path=dir.join("settings.json");let raw=b"{\"settings\":{\"planModeEnabled\":false,\"fontSizeInterface\":14}}";fs::write(&path,raw).unwrap();let rev=format!("{:x}",Sha256::digest(raw));let view=update_client_preferences_at(&path,&rev,&json!({"planModeEnabled":true})).unwrap();assert_eq!(view["planModeEnabled"],true);let saved:Value=serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();assert_eq!(saved["settings"]["fontSizeInterface"],14);assert!(saved.get("planModeEnabled").is_none());assert_eq!(update_client_preferences_at(&path,&rev,&json!({"planModeEnabled":false})).unwrap_err(),"client_settings_changed_reload");fs::remove_dir_all(dir).unwrap();
}

#[test]
fn native_diff_defaults_and_layout_cas_preserve_other_fields_and_local_view_options(){
 for wrapped in [false,true]{
  let dir=temp_dir();let path=dir.join("client-settings.json");let fields=json!({"diffLayout":"stacked","wordWrap":false,"diffIgnoreWhitespace":false,"diffFilesCollapsed":false,"futureNativeField":{"retained":[1,2,3]},"privateField":"SYNTHETIC-PRIVATE"});let original=if wrapped{json!({"schemaVersion":1,"settings":fields})}else{fields.clone()};let bytes=serde_json::to_vec(&original).unwrap();fs::write(&path,&bytes).unwrap();
  let rev=format!("{:x}",Sha256::digest(&bytes));let result=update_client_preferences_at(&path,&rev,&json!({"diffLayout":"split"})).unwrap();assert_eq!(result["diffLayout"],"split");assert_eq!(result["wordWrap"],false);assert_eq!(result["diffIgnoreWhitespace"],false);assert_eq!(result["diffFilesCollapsed"],false);assert!(!result.to_string().contains("SYNTHETIC-PRIVATE"));
  let saved:Value=serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();let mut expected=original.clone();if wrapped{expected["settings"]["diffLayout"]=json!("split")}else{expected["diffLayout"]=json!("split")};assert_eq!(saved,expected);
  let valid=result["revision"].as_str().unwrap();for value in [json!("unified"),json!(true),json!(1),Value::Null]{assert_eq!(update_client_preferences_at(&path,valid,&json!({"diffLayout":value})).unwrap_err(),"invalid_client_preference");}assert_eq!(update_client_preferences_at(&path,valid,&json!({"wordWrap":true})).unwrap_err(),"invalid_client_preference");assert_eq!(update_client_preferences_at(&path,&rev,&json!({"diffLayout":"stacked"})).unwrap_err(),"client_settings_changed_reload");
  fs::remove_dir_all(dir).unwrap();
 }
 let dir=temp_dir();let path=dir.join("client-settings.json");let raw=b"{}";fs::write(&path,raw).unwrap();let rev=format!("{:x}",Sha256::digest(raw));let default=update_client_preferences_at(&path,&rev,&json!({"diffLayout":"stacked"})).unwrap();assert_eq!(default["wordWrap"],true);assert_eq!(default["diffIgnoreWhitespace"],true);assert_eq!(default["diffFilesCollapsed"],true);fs::remove_dir_all(dir).unwrap();
}

#[test]
fn native_thread_delete_confirmation_defaults_and_explicit_false_are_read_only() {
    let dir=temp_dir(); let path=dir.join("client-settings.json");
    fs::write(&path,b"{}").unwrap();
    assert_eq!(read_client_preferences_at(&path).unwrap()["confirmThreadDelete"],true);
    fs::write(&path,b"{\"settings\":{\"confirmThreadDelete\":false}}").unwrap();
    let before=fs::read(&path).unwrap();
    let data=read_client_preferences_at(&path).unwrap();
    assert_eq!(data["confirmThreadDelete"],false);
    assert!(update_client_preferences_at(&path,data["revision"].as_str().unwrap(),&json!({"confirmThreadDelete":true})).is_err());
    assert_eq!(fs::read(&path).unwrap(),before);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn follow_up_behavior_reads_queue_by_default_and_writes_only_queue_or_steer(){
 let dir=temp_dir();let path=dir.join("settings.json");let raw=b"{\"settings\":{\"fontSizeInterface\":14}}";fs::write(&path,raw).unwrap();
 assert_eq!(read_client_preferences_at(&path).unwrap()["followUpBehavior"],"queue");
 let rev=format!("{:x}",Sha256::digest(raw));
 assert_eq!(update_client_preferences_at(&path,&rev,&json!({"followUpBehavior":"restart"})).unwrap_err(),"invalid_client_preference");
 let view=update_client_preferences_at(&path,&rev,&json!({"followUpBehavior":"steer"})).unwrap();assert_eq!(view["followUpBehavior"],"steer");
 let saved:Value=serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();assert_eq!(saved["settings"]["followUpBehavior"],"steer");assert_eq!(saved["settings"]["fontSizeInterface"],14);
 fs::remove_dir_all(dir).unwrap();
}
