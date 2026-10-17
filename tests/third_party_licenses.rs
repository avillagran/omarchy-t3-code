use omarchy_t3_client::third_party_licenses as licenses;
use serde_json::json;
#[test]
fn compiled_notices_match_locked_dependencies_and_current_build_target() {
 use sha2::{Digest,Sha256};
 let sources:serde_json::Value=serde_json::from_str(include_str!("../licenses/third-party-licenses.sources.json")).unwrap();
 let digest=|b:&[u8]|format!("{:x}",Sha256::digest(b));
 assert_eq!(sources["cargoLockSha256"],digest(include_bytes!("../Cargo.lock")),"regenerate notices after changing dependencies");
 assert_eq!(sources["outputSha256"],digest(include_bytes!("../licenses/third-party-licenses.json")));
 assert!(sources["target"].as_str().unwrap().starts_with(std::env::consts::ARCH),"run tools/build-licenses.cjs for this target before compiling");
 assert_eq!(sources["entries"].as_u64().unwrap() as usize,licenses::read().unwrap()["entries"].as_array().unwrap().len());
}
#[test]
fn actual_compiled_manifest_has_metadata_and_lazy_exact_notices_without_environment() {
 let metadata=licenses::read().unwrap();let entries=metadata["entries"].as_array().unwrap();assert!(entries.len()>200);assert_eq!(metadata["scope"],"application");
 assert!(entries.iter().all(|e|e.get("noticeText").is_none()));
 for name in ["T3 Code","Lucide icons","@pierre/diffs","t3-protocol","serde","ot3","Rust standard library","Lucide action icons"]{
  let e=entries.iter().find(|e|e["name"]==name).unwrap();let text=licenses::notice(&json!({"manifestId":metadata["manifestId"],"entryKey":e["entryKey"]})).unwrap();assert!(!text["noticeText"].as_str().unwrap().is_empty());assert_eq!(text["entryKey"],e["entryKey"]);
 }
 assert_eq!(licenses::notice(&json!({"manifestId":"old","entryKey":entries[0]["entryKey"]})).unwrap_err(),"license_manifest_changed");
 assert_eq!(licenses::notice(&json!({"manifestId":metadata["manifestId"],"entryKey":"missing"})).unwrap_err(),"unknown_license_entry");
 assert!(omarchy_t3_client::write::execute(&json!({"op":"licenses-read"})).is_ok());
}
#[test]
fn native_shape_duplicate_and_url_rules_preserve_plain_notice_text() {
 let e=json!({"kind":"package","name":"Example <script> & ñ /","version":null,"license":"MIT OR Apache-2.0","bundles":["desktop","web"],"noticeText":"<script>plain legal text</script>\nCopyright exact","sourceUrl":"https://example.com/source","unexpected":"omit"});
 let m=json!({"schemaVersion":1,"entries":[e.clone()]});let decoded=licenses::decode(&m).unwrap();assert_eq!(decoded["entries"][0]["noticeText"],e["noticeText"]);assert!(decoded["entries"][0].get("unexpected").is_none());
 assert_eq!(licenses::entry_key(&e),"%5B%22package%22%2C%22Example%20%3Cscript%3E%20%26%20%C3%B1%20%2F%22%2Cnull%5D");
 for field in ["kind","name","version","license","bundles","noticeText","sourceUrl"]{let mut broken=e.clone();broken.as_object_mut().unwrap().remove(field);assert!(licenses::decode(&json!({"schemaVersion":1,"entries":[broken]})).is_err(),"{field}");}
 for bad in ["javascript:alert(1)","file:///etc/passwd","data:text/html,x"]{let mut broken=e.clone();broken["sourceUrl"]=json!(bad);assert!(licenses::decode(&json!({"schemaVersion":1,"entries":[broken]})).is_err());}
 assert_eq!(licenses::decode(&json!({"schemaVersion":1,"entries":[e.clone(),e]})).unwrap_err(),"duplicate_license_entry");
}
