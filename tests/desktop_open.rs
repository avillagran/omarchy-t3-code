use omarchy_t3_client::desktop_open;
use std::{fs,os::unix::fs::PermissionsExt};
#[test]
fn opening_prefers_desktop_and_only_falls_back_when_unavailable(){
 let root=std::env::temp_dir().join(format!("t3-open-{}",uuid::Uuid::new_v4()));fs::create_dir_all(&root).unwrap();
 for (name,body) in [("gtk-launch","#!/bin/sh\nprintf '%s %s\\n' \"$1\" \"$2\" >> \"$TEST_OPEN_LOG\"\nif [ \"$TEST_OPEN_FAIL\" = all ] || { [ \"$TEST_OPEN_FAIL\" = first ] && [ \"$1\" = com.t3tools.T3Code ]; }; then exit 1; fi\n"),("xdg-open","#!/bin/sh\nprintf 'browser %s\\n' \"$1\" >> \"$TEST_OPEN_LOG\"\n")]{let p=root.join(name);fs::write(&p,body).unwrap();fs::set_permissions(p,fs::Permissions::from_mode(0o700)).unwrap()}
 std::env::set_var("PATH",format!("{}:{}",root.display(),std::env::var("PATH").unwrap()));let log=root.join("log");std::env::set_var("TEST_OPEN_LOG",&log);
 assert_eq!(desktop_open::app_uri("env-1","thread-1").unwrap(),"t3code://threads/env-1/thread-1");assert!(desktop_open::app_uri("../bad","one").is_err());
 for failure in ["none","first","all"]{fs::write(&log,"").unwrap();std::env::set_var("TEST_OPEN_FAIL",failure);assert_eq!(desktop_open::open("env-1","thread-1","https://example.invalid/#/env-1/thread-1").unwrap(),if failure=="all"{"browser"}else{"application"});if failure=="all"{for _ in 0..30{if fs::read_to_string(&log).unwrap().contains("browser"){break}std::thread::sleep(std::time::Duration::from_millis(10));}}let output=fs::read_to_string(&log).unwrap();assert!(output.contains("t3code://threads/env-1/thread-1"));assert_eq!(output.contains("browser"),failure=="all");if failure=="first"{assert!(output.contains("t3code t3code://"));}}
 fs::remove_dir_all(root).unwrap();
}
