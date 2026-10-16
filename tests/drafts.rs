use serde_json::{json, Value};
use std::{
    io::Write,
    os::unix::fs::PermissionsExt,
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
#[test]
fn drafts_survive_restart_isolate_chats_and_preserve_uncertain_identity() {
    let root = std::env::temp_dir().join(format!("t3-drafts-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let key = "fixture/one";
    let request = uuid::Uuid::new_v4().to_string();
    let text = "Prompt seguro\ncon acentos: ñ y emojis 🦀";
    let draft = json!({"text":text,"attachments":[],"delivery":"auto","attempt":{"op":"send","environment":"fixture","thread":"one","requestId":request,"text":text,"attachments":[],"modelSelection":null,"delivery":"auto","nextThread":{"projectId":"p","modelSelection":{"instanceId":"codex","model":"fixture"},"runtimeMode":"approval-required","branch":"main","worktreePath":null}},"token":"must-not-persist"});
    let events = run(
        &root,
        &[
            json!({"op":"draft-save","key":key,"revision":"one","draft":draft}),
            json!({"op":"draft-save","key":"fixture/two","revision":"two","draft":{"text":"Other thread"}}),
        ],
    );
    assert!(events
        .iter()
        .filter(|e| e["type"] == "draft-saved")
        .all(|e| e["data"]["error"].is_null()));
    let events = run(
        &root,
        &[
            json!({"op":"draft-load","key":key}),
            json!({"op":"draft-load","key":"fixture/two"}),
            json!({"op":"draft-save","key":"../escape","revision":"bad","draft":{"text":"Invalid"}}),
        ],
    );
    let data = events
        .iter()
        .find(|e| e["type"] == "draft-loaded" && e["data"]["key"] == key)
        .unwrap()["data"]["draft"]
        .clone();
    assert_eq!(data["text"], text);
    assert_eq!(data["attempt"]["requestId"], request);
    assert_eq!(data["attempt"]["nextThread"]["projectId"], "p");
    assert!(!data.to_string().contains("must-not-persist"));
    assert_eq!(
        events
            .iter()
            .find(|e| e["data"]["key"] == "fixture/two")
            .unwrap()["data"]["draft"]["text"],
        "Other thread"
    );
    assert_eq!(
        events
            .iter()
            .find(|e| e["data"]["revision"] == "bad")
            .unwrap()["data"]["error"],
        "invalid_draft_key"
    );
    let dir = root.join("state/omarchy-t3-code/drafts");
    assert_eq!(
        std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
        0o700
    );
    for file in std::fs::read_dir(&dir).unwrap() {
        assert_eq!(
            file.unwrap().metadata().unwrap().permissions().mode() & 0o777,
            0o600
        )
    }
    let events = run(
        &root,
        &[
            json!({"op":"draft-save","key":key,"revision":"oversize","draft":{"text":"a".repeat(120001)}}),
            json!({"op":"draft-load","key":key}),
        ],
    );
    assert_eq!(
        events.iter().find(|e| e["type"] == "draft-saved").unwrap()["data"]["error"],
        "draft_too_large"
    );
    assert_eq!(
        events.iter().find(|e| e["type"] == "draft-loaded").unwrap()["data"]["draft"]["text"],
        text
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn queued_edit_survives_restart_with_original_draft_and_retry(){
 let root=std::env::temp_dir().join(format!("t3-queue-draft-{}",uuid::Uuid::new_v4()));std::fs::create_dir_all(&root).unwrap();
 let request=uuid::Uuid::new_v4().to_string();let draft=json!({"text":"Edit queue","attachments":[{"name":"kept.txt","retained":true,"attachment":{"id":"file","type":"file","name":"kept.txt","mimeType":"text/plain","sizeBytes":2}}],"queueEdit":{"runId":"queued","messageId":"message","backup":{"text":"Normal unsent draft","attachments":[],"delivery":"auto","modelSelection":null}},"attempt":{"op":"queue","action":"edit","runId":"queued","thread":"one","environment":"fixture","requestId":request,"text":"Edit queue","attachments":[]}});
 let events=run(&root,&[json!({"op":"draft-save","key":"fixture/one","revision":"edit","draft":draft})]);assert!(events.iter().find(|e|e["type"]=="draft-saved").unwrap()["data"]["error"].is_null());
 let events=run(&root,&[json!({"op":"draft-load","key":"fixture/one"})]);let loaded=&events.iter().find(|e|e["type"]=="draft-loaded").unwrap()["data"]["draft"];
 assert_eq!(loaded["text"],"Edit queue");assert_eq!(loaded["queueEdit"]["backup"]["text"],"Normal unsent draft");assert_eq!(loaded["attachments"][0]["retained"],true);assert_eq!(loaded["attempt"]["requestId"],request);assert_eq!(loaded["attempt"]["action"],"edit");std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn questionnaire_answers_and_stable_retry_survive_restart_without_touching_prompt(){
 let root=std::env::temp_dir().join(format!("t3-question-draft-{}",uuid::Uuid::new_v4()));std::fs::create_dir_all(&root).unwrap();
 let questions=json!({"request":{"index":1,"answers":{"first":{"selected":[" value "]},"second":{"custom":"unfinished"}},"attempt":{"op":"question","action":"respond","requestId":uuid::Uuid::new_v4().to_string(),"runtimeRequestId":"request","answers":{"first":" value ","second":"unfinished"}}}});
 let saved=run(&root,&[json!({"op":"draft-save","key":"fixture/one","revision":"q","draft":{"text":"Do not lose this prompt","questionnaires":questions}})]);
 assert!(saved.iter().find(|e|e["type"]=="draft-saved").unwrap()["data"]["error"].is_null());
 let events=run(&root,&[json!({"op":"draft-load","key":"fixture/one"})]);let loaded=&events.iter().find(|e|e["type"]=="draft-loaded").unwrap()["data"]["draft"];
 assert_eq!(loaded["text"],"Do not lose this prompt");assert_eq!(loaded["questionnaires"],questions);std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn permission_selection_and_send_intent_survive_restart(){
 for mode in ["approval-required","auto-accept-edits","auto","full-access"]{
  let root=std::env::temp_dir().join(format!("ot3-mode-draft-{}",uuid::Uuid::new_v4()));std::fs::create_dir_all(&root).unwrap();
  let id=uuid::Uuid::new_v4().to_string();let draft=json!({"text":"Safe draft","runtimeMode":mode,"attachments":[],"attempt":{"op":"send","environment":"fixture","thread":"one","requestId":id,"text":"Safe draft","attachments":[],"runtimeMode":mode}});
  let saved=run(&root,&[json!({"op":"draft-save","key":"fixture/one","revision":"mode","draft":draft})]);assert!(saved.iter().find(|e|e["type"]=="draft-saved").unwrap()["data"]["error"].is_null());
  let loaded=run(&root,&[json!({"op":"draft-load","key":"fixture/one"})]);let d=&loaded.iter().find(|e|e["type"]=="draft-loaded").unwrap()["data"]["draft"];assert_eq!(d["runtimeMode"],mode);assert_eq!(d["attempt"]["runtimeMode"],mode);assert_eq!(d["attempt"]["requestId"],id);std::fs::remove_dir_all(root).unwrap();
 }
}
