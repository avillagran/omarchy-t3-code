use omarchy_t3_client::scroll;use std::{fs,os::unix::fs::PermissionsExt};
#[test]
fn compositor_probe_is_read_only_and_missing_patch_uses_local_fallback(){
 let root=std::env::temp_dir().join(format!("t3-scroll-{}",uuid::Uuid::new_v4()));fs::create_dir_all(&root).unwrap();let exe=root.join("hyprctl");let log=root.join("calls");std::env::set_var("T3_SCROLL_LOG",&log);std::env::set_var("PATH",format!("{}:{}",root.display(),std::env::var("PATH").unwrap()));
 std::env::remove_var("HYPRLAND_INSTANCE_SIGNATURE");fs::write(&exe,"#!/bin/sh\nprintf '%s %s %s\\n' \"$1\" \"$2\" \"$3\" >> \"$T3_SCROLL_LOG\"\ncase \"$2\" in input:touchpad:scroll_accel_profile) n=2;; input:touchpad:scroll_decel) n=600;; *) n=0;; esac\nprintf '{\"option\":\"%s\",\"int\":%s}' \"$2\" \"$n\"\n").unwrap();fs::set_permissions(&exe,fs::Permissions::from_mode(0o700)).unwrap();let v=scroll::read();assert_eq!(v["touchpadNative"],true);assert_eq!(v["mouseNative"],false);let calls=fs::read_to_string(&log).unwrap();assert_eq!(calls.lines().count(),5);assert!(calls.lines().all(|l|l.starts_with("getoption ")&&l.ends_with(" -j")));
 fs::write(&exe,"#!/bin/sh\nexit 1\n").unwrap();let v=scroll::read();assert_eq!(v["touchpadNative"],false);assert_eq!(v["mouseNative"],false);fs::remove_dir_all(root).unwrap();
}
