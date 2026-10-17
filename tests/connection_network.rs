use omarchy_t3_client::{connection_network as network,*};
use serde_json::json;
use std::{fs,os::unix::fs::PermissionsExt};
#[test]
fn native_launch_parameters_are_bounded_and_tailscale_targets_are_not_hijacked(){
 let text=network::launch_override("local-only",true,8443).unwrap();assert!(text.contains("T3CODE_HOST=127.0.0.1")&&text.contains("T3CODE_TAILSCALE_SERVE=true")&&text.contains("T3CODE_TAILSCALE_SERVE_PORT=8443"));
 assert!(network::launch_override("0.0.0.0\nExecStart=bad",false,443).is_err());assert!(network::launch_override("local-only",false,0).is_err());
 let own=json!({"Web":{"host.tail.ts.net:443":{"Handlers":{"/":{"Proxy":"http://127.0.0.1:3773"}}}}});
 assert_eq!(network::serve_mapping(&own,"host.tail.ts.net.",443,3773),("active",Some("https://host.tail.ts.net/".into())));
 assert_eq!(network::serve_mapping(&own,"host.tail.ts.net",443,9999).0,"conflict");assert_eq!(network::serve_mapping(&own,"host.tail.ts.net",8443,3773).0,"absent");
}
#[test]
fn native_settings_preserve_fields_and_failed_activation_rolls_back(){
 let root=std::env::temp_dir().join(format!("ot3-network-{}",uuid::Uuid::new_v4()));fs::create_dir_all(root.join("native/userdata")).unwrap();fs::create_dir_all(root.join("bin")).unwrap();fs::create_dir_all(root.join(".config/systemd/user")).unwrap();
 std::env::set_var("HOME",&root);std::env::set_var("T3CODE_HOME",root.join("native"));std::env::set_var("PATH",format!("{}:{}",root.join("bin").display(),std::env::var("PATH").unwrap()));
 fs::write(root.join(".config/systemd/user/t3code.service"),format!("[Service]\nEnvironment=T3CODE_HOME={}\n",root.join("native").display())).unwrap();
 let settings=root.join("native/userdata/desktop-settings.json");let original=json!({"unknown":{"keep":42},"localEnvironmentEnabled":true});fs::write(&settings,original.to_string()).unwrap();
 for (name,source) in [("tailscale","#!/bin/sh\nprintf '{}'\n"),("ip","#!/bin/sh\nprintf '[]'\n"),("systemctl","#!/bin/sh\nexit 1\n")]{let p=root.join("bin").join(name);fs::write(&p,source).unwrap();fs::set_permissions(p,fs::Permissions::from_mode(0o700)).unwrap();}
 let before=network::execute(&json!({"action":"read"})).unwrap();assert_eq!(before["canManage"],true);
 assert_eq!(network::execute(&json!({"action":"apply","revision":before["revision"]})).unwrap_err(),"confirmation_required");
 assert_eq!(network::execute(&json!({"action":"apply","revision":"stale","confirmed":true})).unwrap_err(),"catalog_changed_reload");
 let error=network::execute(&json!({"action":"apply","revision":before["revision"],"confirmed":true,"mode":"local-only","tailscaleServeEnabled":false,"tailscaleServePort":443})).unwrap_err();assert_eq!(error,"network_command_failed");
 assert_eq!(read_json(&settings).unwrap(),original);assert!(!root.join(".config/systemd/user/t3code.service.d/ot3-network.conf").exists());assert_eq!(network::execute(&json!({"action":"read"})).unwrap()["revision"],before["revision"]);
 // Successful activation uses T3's own service and readiness helpers.
 let cli=root.join(".t3/bin/t3");fs::create_dir_all(cli.parent().unwrap()).unwrap();fs::write(&cli,"#!/bin/sh\nif [ \"$2\" = runtime-port ]; then printf 'ready 3773\\n'; fi\nexit 0\n").unwrap();fs::set_permissions(&cli,fs::Permissions::from_mode(0o700)).unwrap();
 let systemctl=root.join("bin/systemctl");fs::write(&systemctl,format!("#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\nif [ \"$2\" = restart ]; then printf '%s' '{{\"pid\":{},\"port\":3773,\"origin\":\"http://127.0.0.1:3773\",\"host\":\"127.0.0.1\"}}' > '{}'; fi\nexit 0\n",root.join("calls").display(),std::process::id(),root.join("native/userdata/server-runtime.json").display())).unwrap();
 let applied=network::execute(&json!({"action":"apply","revision":before["revision"],"confirmed":true,"mode":"local-only","tailscaleServeEnabled":false,"tailscaleServePort":8443})).unwrap();assert_eq!(applied["desiredMode"],"local-only");assert_eq!(read_json(&settings).unwrap()["unknown"]["keep"],42);assert!(fs::read_to_string(root.join(".config/systemd/user/t3code.service.d/ot3-network.conf")).unwrap().contains("T3CODE_TAILSCALE_SERVE_PORT=8443"));let calls=fs::read_to_string(root.join("calls")).unwrap();assert!(calls.contains("--user daemon-reload")&&calls.contains("--user restart t3code.service"));
 fs::remove_dir_all(root).unwrap();
}
