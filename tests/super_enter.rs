use std::{fs, os::unix::fs::PermissionsExt, process::Command};
#[test]
fn legacy_super_enter_never_submits_and_always_opens_terminal() {
    let root = std::env::temp_dir().join(format!("t3-shortcut-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&root).unwrap();
    for (name, body) in [
        (
            "omarchy-shell",
            "#!/bin/sh\nprintf '%s' \"$TEST_CONSUMED\"\n",
        ),
        (
            "omarchy-launch-terminal",
            "#!/bin/sh\ntouch \"$TEST_TERMINAL_MARK\"\n",
        ),
    ] {
        let p = root.join(name);
        fs::write(&p, body).unwrap();
        fs::set_permissions(p, fs::Permissions::from_mode(0o700)).unwrap()
    }
    let marker = root.join("terminal");
    for consumed in ["true", "false"] {
        let _ = fs::remove_file(&marker);
        let status = Command::new(env!("CARGO_BIN_EXE_omarchy-t3-client"))
            .arg("super-enter")
            .env(
                "PATH",
                format!("{}:{}", root.display(), std::env::var("PATH").unwrap()),
            )
            .env("TEST_CONSUMED", consumed)
            .env("TEST_TERMINAL_MARK", &marker)
            .status()
            .unwrap();
        assert!(status.success());
        assert_eq!(marker.exists(), true);
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn agent_binding_preserves_omarchy_terminal_shortcut() {
    let source = include_str!("../integrations/hyprland/bindings.lua");
    assert!(!source.contains("hl.unbind(\"SUPER + RETURN\")"));
    assert!(!source.contains("super-enter"));
    assert!(source.contains("SUPER + SHIFT + A"));
}
