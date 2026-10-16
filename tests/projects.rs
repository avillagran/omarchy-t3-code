use omarchy_t3_client::protocol::projects::command;
use serde_json::json;
#[test]
fn project_mutations_preserve_native_scope_and_explicit_delete_confirmation(){
 let rename=command(&json!({"action":"project-rename","projectId":"project-1","title":" New title "})).unwrap();assert_eq!(rename,json!({"type":"project.update","projectId":"project-1","title":"New title"}));
 assert!(command(&json!({"action":"project-rename","projectId":"project-1","title":" "})).is_err());
 assert!(command(&json!({"action":"project-remove","projectId":"project-1"})).is_err());
 assert_eq!(command(&json!({"action":"project-remove","projectId":"project-1","confirmed":true})).unwrap()["force"],false);
 assert_eq!(command(&json!({"action":"project-remove","projectId":"project-1","confirmed":true,"force":true})).unwrap()["force"],true);
}
#[test]
fn project_defaults_allow_native_fields_but_never_connection_or_filesystem_changes(){
 let patch=json!({"title":"Name","defaultModelSelection":{"instanceId":"codex","model":"gpt-6.1-sol","options":[]},"defaultThreadEnvMode":"worktree","autoPull":true});
 let c=command(&json!({"action":"project-settings","projectId":"p","patch":patch})).unwrap();assert_eq!(c["defaultThreadEnvMode"],"worktree");assert_eq!(c["autoPull"],true);
 for patch in [json!({"workspaceRoot":"/other"}),json!({"environment":"other"}),json!({"defaultThreadEnvMode":"remote"}),json!({"autoPull":"true"})]{assert!(command(&json!({"action":"project-settings","projectId":"p","patch":patch})).is_err())}
}
#[test]
fn native_client_grouping_is_read_only_and_exports_no_other_preferences(){
 use std::fs;let root=std::env::temp_dir().join(format!("t3-project-pref-{}",uuid::Uuid::new_v4()));fs::create_dir_all(root.join("userdata")).unwrap();let file=root.join("userdata/client-settings.json");let raw=json!({"sidebarProjectGroupingMode":"repository_path","sidebarProjectGroupingOverrides":{"remote:/repo":"separate"},"unrelated":"must-not-export"}).to_string();fs::write(&file,&raw).unwrap();let old=std::env::var_os("T3CODE_HOME");std::env::set_var("T3CODE_HOME",&root);
 let value=omarchy_t3_client::native_project_grouping();assert_eq!(value["mode"],"repository_path");assert_eq!(value["overrides"]["remote:/repo"],"separate");assert!(!value.to_string().contains("unrelated"));assert_eq!(fs::read_to_string(&file).unwrap(),raw);if let Some(old)=old{std::env::set_var("T3CODE_HOME",old)}else{std::env::remove_var("T3CODE_HOME")}fs::remove_dir_all(root).unwrap();
}
