use serde_json::{json, Value};
use std::{
    io::Write,
    process::{Command, Stdio},
};
fn run(root: &std::path::Path, commands: &[Value]) -> Vec<Value> {
    let mut p = Command::new(env!("CARGO_BIN_EXE_omarchy-t3-client"))
        .arg("watch")
        .env("HOME", root.join("home"))
        .env("T3CODE_HOME", root.join("native"))
        .env("XDG_CONFIG_HOME", root.join("config"))
        .env("XDG_STATE_HOME", root.join("state"))
        .env("XDG_DATA_HOME", root.join("data"))
        .env_remove("OMARCHY_T3_FIXTURE_ENVIRONMENTS")
        .env_remove("KIMI_CODING_API_KEY")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut stdin = p.stdin.take().unwrap();
    for c in commands {
        writeln!(stdin, "{c}").unwrap()
    }
    writeln!(stdin, "{}", json!({"op":"quit"})).unwrap();
    drop(stdin);
    let o = p.wait_with_output().unwrap();
    assert!(o.status.success());
    String::from_utf8(o.stdout)
        .unwrap()
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}
fn saved(root:&std::path::Path,key:&str,d:Value)->Value{
 let events=run(root,&[json!({"op":"draft-save","key":key,"draft":d,"revision":"save"}),json!({"op":"draft-load","key":key,"projectId":"project"})]);
 assert!(events.iter().find(|e|e["type"]=="draft-saved").unwrap()["data"]["error"].is_null());
 events.iter().find(|e|e["type"]=="draft-loaded").unwrap()["data"]["draft"].clone()
}
fn command(action:&str,key:&str,d:&Value)->Value{json!({"op":"prompt-stash","action":action,"environment":"fixture","projectId":"project","keys":["fixture/a","fixture/b"],"key":key,"requestId":uuid::Uuid::new_v4().to_string(),"draft":d})}
fn result(root:&std::path::Path,cmd:&Value)->Value{run(root,&[cmd.clone()]).into_iter().find(|e|e["type"]=="prompt-stash").unwrap()["data"].clone()}
fn loaded(root:&std::path::Path,key:&str)->Value{run(root,&[json!({"op":"draft-load","key":key,"projectId":"project"})]).into_iter().find(|e|e["type"]=="draft-loaded").unwrap()["data"]["draft"].clone()}
#[test]
fn project_drawer_restores_across_threads_without_changing_target_configuration(){
 let root=std::env::temp_dir().join(format!("ot3-stash-{}",uuid::Uuid::new_v4()));
 let a=saved(&root,"fixture/a",json!({"text":"Current prompt  ","runtimeMode":"approval-required","modelSelection":{"instanceId":"target","model":"model","options":[]}}));
 saved(&root,"fixture/b",json!({"text":"Other prompt","runtimeMode":"full-access","attachments":[{"name":"image.png","attachment":{"id":"asset","type":"image","name":"image.png","mimeType":"image/png","sizeBytes":12}}]}));
 saved(&root,"fixture/outside",json!({"text":"Other project"}));
 let list=result(&root,&command("list","fixture/a",&a));assert!(list["error"].is_null(),"{list}");assert_eq!(list["result"]["entries"].as_array().unwrap().len(),2);
 let entry=list["result"]["entries"].as_array().unwrap().iter().find(|e|e["sourceKey"]=="fixture/b").unwrap();
 let mut restore=command("restore","fixture/a",&a);restore["id"]=entry["id"].clone();restore["fingerprint"]=entry["fingerprint"].clone();
 let changed=result(&root,&restore);assert!(changed["error"].is_null(),"{changed}");
 let d=loaded(&root,"fixture/a");assert_eq!(d["text"],"Current prompt\n\nOther prompt");assert_eq!(d["attachments"][0]["attachment"]["id"],"asset");assert_eq!(d["runtimeMode"],"approval-required");assert_eq!(d["modelSelection"],a["modelSelection"]);assert_eq!(loaded(&root,"fixture/b")["text"],"");
 assert!(result(&root,&restore)["error"].is_null());assert_eq!(loaded(&root,"fixture/a"),d,"same request cannot append twice");
 assert_eq!(loaded(&root,"fixture/outside")["text"],"Other project");
 let stash=result(&root,&command("stash","fixture/a",&d));assert!(stash["error"].is_null(),"{stash}");assert_eq!(loaded(&root,"fixture/a")["text"],"");
 let mut list_cmd=command("list","fixture/b",&json!({}));list_cmd["keys"]=json!(["fixture/b"]);
 let list=result(&root,&list_cmd);let entry=&list["result"]["entries"][0];assert_eq!(entry["prompt"],d["text"],"stash outlives original thread membership");
 let mut restore=command("restore","fixture/b",&loaded(&root,"fixture/b"));restore["id"]=entry["id"].clone();restore["keys"]=json!(["fixture/b"]);
 assert!(result(&root,&restore)["error"].is_null());assert_eq!(loaded(&root,"fixture/b")["text"],d["text"]);
 std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn stale_locked_or_oversize_restore_never_clears_source(){
 let root=std::env::temp_dir().join(format!("ot3-stash-safe-{}",uuid::Uuid::new_v4()));
 let a=saved(&root,"fixture/a",json!({"text":"a".repeat(120000)}));let b=saved(&root,"fixture/b",json!({"text":"Keep me"}));
 let list=result(&root,&command("list","fixture/a",&a));let entry=list["result"]["entries"].as_array().unwrap().iter().find(|e|e["sourceKey"]=="fixture/b").unwrap();
 let mut restore=command("restore","fixture/a",&a);restore["id"]=entry["id"].clone();restore["fingerprint"]=entry["fingerprint"].clone();
 assert_eq!(result(&root,&restore)["error"],"draft_too_large");assert_eq!(loaded(&root,"fixture/b"),b);assert_eq!(loaded(&root,"fixture/a"),a);
 saved(&root,"fixture/b",json!({"text":"Newer edits"}));assert_eq!(result(&root,&restore)["error"],"draft_changed");assert_eq!(loaded(&root,"fixture/b")["text"],"Newer edits");
 let id=uuid::Uuid::new_v4().to_string();saved(&root,"fixture/b",json!({"text":"Pending","attempt":{"op":"send","environment":"fixture","thread":"b","requestId":id,"text":"Pending"}}));
 let list=result(&root,&command("list","fixture/a",&a));assert!(list["result"]["entries"].as_array().unwrap().iter().any(|e|e["sourceKey"]=="fixture/b"&&e["locked"]==true));
 let mut wrong=command("list","fixture/a",&a);wrong["keys"]=json!(["other/b","fixture/a"]);assert_eq!(result(&root,&wrong)["error"],"invalid_draft_key");
 std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn interrupted_take_recovers_durable_destination_and_removal(){
 let root=std::env::temp_dir().join(format!("ot3-stash-recover-{}",uuid::Uuid::new_v4()));
 let a=saved(&root,"fixture/a",json!({"text":"Recover this"}));let stash=command("stash","fixture/a",&a);assert!(result(&root,&stash)["error"].is_null());
 let dir=root.join("state/omarchy-t3-code/prompt-stash");let path=std::fs::read_dir(dir).unwrap().map(|e|e.unwrap().path()).find(|p|p.extension().is_some_and(|s|s=="json")).unwrap();
 let mut doc:Value=serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();let before=loaded(&root,"fixture/a");let mut after=before.clone();after["text"]=json!("Recover this");let id=uuid::Uuid::new_v4().to_string();
 doc["pending"]=json!({"requestId":id,"entries":[],"changes":[{"key":"fixture/a","before":before,"after":after}]});std::fs::write(&path,doc.to_string()).unwrap();
 // Simulate a process dying after writing the destination, before consuming the stash.
 saved(&root,"fixture/a",after.clone());let list=result(&root,&command("list","fixture/a",&after));assert!(list["error"].is_null(),"{list}");assert_eq!(loaded(&root,"fixture/a"),after);
 let doc:Value=serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();assert!(doc["pending"].is_null());assert_eq!(doc["entries"],json!([]));
 std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn unavailable_storage_keeps_both_prompts_and_retry_is_exact(){
 let root=std::env::temp_dir().join(format!("ot3-stash-failure-{}",uuid::Uuid::new_v4()));
 let a=saved(&root,"fixture/a",json!({"text":"Current"}));let b=saved(&root,"fixture/b",json!({"text":"Source"}));
 let list=result(&root,&command("list","fixture/a",&a));let entry=list["result"]["entries"].as_array().unwrap().iter().find(|e|e["sourceKey"]=="fixture/b").unwrap();
 let mut restore=command("restore","fixture/a",&a);restore["id"]=entry["id"].clone();restore["fingerprint"]=entry["fingerprint"].clone();
 let dir=root.join("state/omarchy-t3-code/drafts");
 let stash_dir=root.join("state/omarchy-t3-code/prompt-stash");let path=std::fs::read_dir(stash_dir).unwrap().map(|e|e.unwrap().path()).find(|p|p.extension().is_some_and(|s|s=="lock")).unwrap().with_extension("json");
 std::fs::create_dir(&path).unwrap();
 assert_eq!(result(&root,&restore)["error"],"draft_read_failed");
 let raw=std::fs::read_dir(&dir).unwrap().map(|e|e.unwrap().path()).filter(|p|p.extension().is_some_and(|x|x=="json")).map(|p|serde_json::from_slice::<Value>(&std::fs::read(p).unwrap()).unwrap()).collect::<Vec<_>>();
 assert_eq!(raw.iter().find(|d|d["key"]=="fixture/a").unwrap()["draft"],a);assert_eq!(raw.iter().find(|d|d["key"]=="fixture/b").unwrap()["draft"],b);
 std::fs::remove_dir(&path).unwrap();
 assert!(result(&root,&restore)["error"].is_null());assert_eq!(loaded(&root,"fixture/a")["text"],"Current\n\nSource");assert_eq!(loaded(&root,"fixture/b")["text"],"");
 assert!(result(&root,&restore)["error"].is_null());assert_eq!(loaded(&root,"fixture/a")["text"],"Current\n\nSource");std::fs::remove_dir_all(root).unwrap();
}
