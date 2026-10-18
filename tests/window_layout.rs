use omarchy_t3_client::window_layout;
use serde_json::json;
use std::{fs, os::unix::fs::PermissionsExt};
#[test]
fn layout_targets_only_verified_ot3_window_and_repeated_modes_do_not_toggle() {
    let root = std::env::temp_dir().join(format!("ot3-window-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&root).unwrap();
    let stub = root.join("hyprctl");
    let mark = root.join("calls");
    let clients = root.join("clients");
    fs::write(&clients,"[{\"pid\":77,\"title\":\"Other window\",\"address\":\"0xaaa\"},{\"pid\":88,\"title\":\"Omarchy T3 Code\",\"address\":\"0xbbb\",\"floating\":false}]").unwrap();
    fs::write(&stub,"#!/bin/sh\nif [ \"$1\" = getoption ]; then echo '{\"option\":\"general:border_size\",\"int\":3}'; elif [ \"$1\" = --batch ]; then printf '%s\\n' \"$2\" >> \"$OT3_LAYOUT_CALLS\"; elif [ \"$1\" = -j ]; then cat \"$OT3_LAYOUT_CLIENTS\"; else printf '%s\\n' \"$2\" >> \"$OT3_LAYOUT_CALLS\"; if grep -q false \"$OT3_LAYOUT_CLIENTS\"; then sed -i 's/false/true/' \"$OT3_LAYOUT_CLIENTS\"; else sed -i 's/true/false/' \"$OT3_LAYOUT_CLIENTS\"; fi; echo ok; fi\n").unwrap();
    fs::set_permissions(&stub, fs::Permissions::from_mode(0o700)).unwrap();
    let old = std::env::var("PATH").unwrap();
    std::env::set_var("PATH", format!("{}:{old}", root.display()));
    std::env::set_var("OT3_LAYOUT_CALLS", &mark);
    std::env::set_var("OT3_LAYOUT_CLIENTS", &clients);
    for mode in ["floating", "floating", "tiled", "tiled"] {
        window_layout::apply(&json!({"mode":mode,"pid":88})).unwrap();
    }
    assert!(window_layout::apply(&json!({"mode":"floating","pid":77})).is_err());
    assert!(window_layout::apply(&json!({"mode":"invalid","pid":88})).is_err());
    let calls = fs::read_to_string(mark).unwrap();
    assert_eq!(calls.matches("action = \"toggle\"").count(), 2);
    assert!(!calls.contains("0xaaa"));
    assert!(calls.contains("address:0xbbb"));
    // Every apply restores the user's border instead of the shell-wide rule.
    assert_eq!(calls.matches("prop = \"border_size\", value = \"3\"").count(), 4);
    assert_eq!(calls.matches("prop = \"no_follow_mouse\", value = \"false\"").count(), 4);
    std::env::set_var("PATH", old);
    fs::remove_dir_all(root).unwrap();
}
