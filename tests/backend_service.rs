use omarchy_t3_client::{backend_service,refresh_connections};use serde_json::json;use std::{fs,os::unix::fs::PermissionsExt};
#[test]fn service_start_respects_running_owner_and_profile(){
 let root=std::env::temp_dir().join(format!("t3-service-{}",uuid::Uuid::new_v4()));fs::create_dir_all(root.join(".t3/userdata")).unwrap();fs::create_dir_all(root.join(".t3/bin")).unwrap();std::env::set_var("HOME",&root);std::env::set_var("T3CODE_HOME",root.join(".t3"));
 let runtime=root.join(".t3/userdata/server-runtime.json");fs::write(&runtime,json!({"pid":std::process::id()}).to_string()).unwrap();assert_eq!(backend_service::start().unwrap()["status"],"already-running");fs::remove_file(runtime).unwrap();
 let cli=root.join(".t3/bin/t3");fs::write(&cli,"#!/bin/sh\n[ \"$1\" = service ] && [ \"$2\" = install ] && [ \"$3\" = --base-dir ] && [ \"$4\" = \"$HOME/.t3\" ]\n").unwrap();fs::set_permissions(&cli,fs::Permissions::from_mode(0o700)).unwrap();assert_eq!(backend_service::start().unwrap()["status"],"starting");
 fs::create_dir_all(root.join(".config/systemd/user")).unwrap();fs::write(root.join(".config/systemd/user/t3code.service"),"Environment=T3CODE_HOME=/other/profile\n").unwrap();assert_eq!(backend_service::start().unwrap_err(),"service_profile_mismatch");// A matching native unit is started through systemd; never reinstalled/repointed.
 let log=root.join("systemctl.log");std::env::set_var("T3_TEST_SYSTEMCTL_LOG",&log);fs::create_dir_all(root.join("test-bin")).unwrap();
 let systemctl=root.join("test-bin/systemctl");fs::write(&systemctl,"#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$T3_TEST_SYSTEMCTL_LOG\"\n").unwrap();fs::set_permissions(&systemctl,fs::Permissions::from_mode(0o700)).unwrap();
 let old_path=std::env::var("PATH").unwrap();std::env::set_var("PATH",format!("{}:{}",root.join("test-bin").display(),old_path));
 fs::write(root.join(".config/systemd/user/t3code.service"),format!("Environment=T3CODE_HOME=\"{}\"\n",root.join(".t3").display())).unwrap();
 assert_eq!(backend_service::start().unwrap()["status"],"starting");assert_eq!(fs::read_to_string(log).unwrap().trim(),"--user start t3code.service");
 std::env::set_var("PATH",old_path);refresh_connections();fs::remove_dir_all(root).unwrap();
}
