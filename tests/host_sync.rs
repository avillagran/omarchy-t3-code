//! Sync this computer's setup to a host, with two temporary homes: the "host"
//! runs the same scripts in a local shell with its own HOME.
use omarchy_t3_client::host_sync::{self as h, Session};
use serde_json::{json, Value};
use std::{
    fs,
    os::unix::fs::{symlink, PermissionsExt},
    path::{Path, PathBuf},
    process::Command,
};

fn root(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ot3-host-sync-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}
fn write(path: &Path, body: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, body).unwrap();
}
fn item<'a>(v: &'a Value, id: &str) -> &'a Value {
    v["items"].as_array().unwrap().iter().find(|i| i["id"] == id).unwrap()
}
fn entry<'a>(v: &'a Value, id: &str, path: &str) -> &'a Value {
    item(v, id)["entries"].as_array().unwrap().iter().find(|e| e["path"] == path).unwrap_or_else(|| panic!("{path} in {}", item(v, id)))
}
fn git(dir: &Path, args: &[&str], cfg: &Path) {
    let ok = Command::new("git").arg("-C").arg(dir).args(args).env("GIT_CONFIG_GLOBAL", cfg).env("GIT_CONFIG_NOSYSTEM", "1").output().unwrap();
    assert!(ok.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&ok.stderr));
}

/// This computer: instructions with its home in them, skills (own, linked into
/// the home, the system's, hidden), settings with and without a token, an agent,
/// and files that must never leave (credentials, sessions).
fn this_computer(base: &Path) -> PathBuf {
    let home = base.join("here");
    let me = home.display().to_string();
    write(&home.join(".claude/CLAUDE.md"), &format!("Vault: {me}/Obsidian/Dev\nNot {me}ow\n"));
    write(&home.join(".codex/AGENTS.md"), "codex rules\n");
    write(&home.join(".claude/.credentials.json"), "{\"claudeAiOauth\":{\"accessToken\":\"x\"}}");
    write(&home.join(".codex/auth.json"), "{\"OPENAI_API_KEY\":\"sk-proj-abcdefghijklmnopqrstuvwxyz\"}");
    write(&home.join(".claude/projects/p/session.jsonl"), "history");
    write(&home.join(".claude/skills/mine/SKILL.md"), "---\nname: mine\n---\n");
    write(&home.join(".claude/skills/mine/scripts/run.sh"), &format!("#!/bin/sh\nexec {me}/bin/tool\n"));
    fs::set_permissions(home.join(".claude/skills/mine/scripts/run.sh"), fs::Permissions::from_mode(0o755)).unwrap();
    write(&home.join("Work/skills/linked/SKILL.md"), "linked skill\n");
    symlink(home.join("Work/skills/linked"), home.join(".claude/skills/linked")).unwrap();
    write(&base.join("system/omarchy/SKILL.md"), "omarchy\n");
    symlink(base.join("system/omarchy"), home.join(".claude/skills/omarchy")).unwrap();
    write(&home.join(".codex/skills/.system/x/SKILL.md"), "codex own\n");
    write(&home.join(".claude/skills/synced/bucket/SKILL.md"), "account sync\n");
    write(&home.join(".claude/settings.json"), &format!("{{\"model\":\"opus\",\"statusLine\":{{\"command\":\"{me}/bin/status\"}}}}\n"));
    write(&home.join(".config/opencode/opencode.json"), "{\"provider\":{\"x\":{\"options\":{\"apiKey\":\"literal-secret-value\"}}}}\n");
    write(&home.join(".claude/agents/reviewer.md"), "agent\n");
    write(&home.join(".claude/themes/omarchy.json"), "{}\n");
    write(&home.join(".claude.json"), &format!("{{\"oauthAccount\":{{\"email\":\"me@x\"}},\"mcpServers\":{{\"docs\":{{\"command\":\"{me}/bin/docs\"}}}}}}\n"));
    home
}
/// The host: its own CLAUDE.md, Omarchy's skill as a link, a skill of its own
/// linked in, and a ~/.claude.json with its account and one MCP server.
fn the_host(base: &Path) -> PathBuf {
    let home = base.join("there");
    write(&home.join(".claude/CLAUDE.md"), "host's own instructions\n");
    write(&base.join("hostsystem/omarchy/SKILL.md"), "host omarchy\n");
    symlink(base.join("hostsystem/omarchy"), home.join(".claude/skills/omarchy")).ok();
    fs::create_dir_all(home.join(".claude/skills")).unwrap();
    write(&home.join("dotfiles/linked/SKILL.md"), "host's linked\n");
    symlink(home.join("dotfiles/linked"), home.join(".claude/skills/linked")).unwrap();
    write(&home.join(".claude.json"), "{\"oauthAccount\":{\"email\":\"host@x\"},\"mcpServers\":{\"local\":{\"command\":\"x\"}}}\n");
    home
}

#[test]
fn preview_then_sync_copies_only_the_setup_and_backs_up_what_it_replaces() {
    let base = root("files");
    let here = this_computer(&base);
    let there = the_host(&base);
    let host = [Session::Local { home: there.clone() }];
    let preview = h::preview_with(&host, &here, None).unwrap();
    assert_eq!(preview["host"]["home"], there.display().to_string());
    assert_eq!(entry(&preview, "instructions", "~/.claude/CLAUDE.md")["action"], "update");
    assert_eq!(entry(&preview, "instructions", "~/.codex/AGENTS.md")["action"], "add");
    assert_eq!(entry(&preview, "skills", "~/.claude/skills/mine")["action"], "add");
    assert_eq!(entry(&preview, "skills", "~/.claude/skills/mine")["files"], 2);
    assert_eq!(entry(&preview, "skills", "~/.claude/skills/linked")["action"], "kept", "a skill the host links itself is its own");
    assert_eq!(entry(&preview, "skills", "~/.claude/skills/omarchy")["reason"], "system", "Omarchy's link here is not sent");
    let skills = item(&preview, "skills").to_string();
    assert!(!skills.contains(".system") && !skills.contains("synced"), "hidden and account-synced skills stay: {skills}");
    assert_eq!(entry(&preview, "config", "~/.config/opencode/opencode.json")["reason"], "credentials");
    assert_eq!(entry(&preview, "config", "~/.claude.json")["action"], "update");
    let all = preview.to_string();
    for never in [".credentials.json", "auth.json", "projects/", "themes/omarchy.json"] {
        assert!(!all.contains(never), "{never} is never offered: {all}");
    }
    assert_eq!(item(&preview, "vault")["available"], false);

    let items: Vec<String> = ["instructions", "skills", "config"].iter().map(|s| s.to_string()).collect();
    let done = h::apply_with(&host, &here, None, &items).unwrap();
    assert_eq!(item(&done, "instructions")["status"], "done");
    assert_eq!(item(&done, "instructions")["counts"]["updated"], 1);
    assert_eq!(item(&done, "skills")["counts"]["kept"], 1);
    let claude = fs::read_to_string(there.join(".claude/CLAUDE.md")).unwrap();
    assert_eq!(claude, format!("Vault: {}/Obsidian/Dev\nNot {}ow\n", there.display(), here.display()), "only whole home paths change");
    let run = there.join(".claude/skills/mine/scripts/run.sh");
    assert!(fs::read_to_string(&run).unwrap().contains(&format!("{}/bin/tool", there.display())));
    assert_eq!(fs::metadata(&run).unwrap().permissions().mode() & 0o777, 0o755, "modes are kept");
    assert_eq!(fs::read_to_string(there.join("dotfiles/linked/SKILL.md")).unwrap(), "host's linked\n");
    assert_eq!(fs::read_to_string(base.join("hostsystem/omarchy/SKILL.md")).unwrap(), "host omarchy\n");
    assert!(fs::read_to_string(there.join(".claude/settings.json")).unwrap().contains(&format!("{}/bin/status", there.display())));
    assert!(!there.join(".config/opencode/opencode.json").exists() && !there.join(".claude/.credentials.json").exists() && !there.join(".codex/auth.json").exists());
    let merged: Value = serde_json::from_str(&fs::read_to_string(there.join(".claude.json")).unwrap()).unwrap();
    assert_eq!(merged["oauthAccount"]["email"], "host@x", "the host's account stays");
    assert_eq!(merged["mcpServers"]["local"]["command"], "x");
    assert_eq!(merged["mcpServers"]["docs"]["command"], format!("{}/bin/docs", there.display()));
    let backup = PathBuf::from(done["backup"].as_str().unwrap());
    assert!(backup.starts_with(there.join(".local/state/omarchy-t3-code/sync-backups")));
    assert_eq!(fs::read_to_string(backup.join(".claude/CLAUDE.md")).unwrap(), "host's own instructions\n");
    assert!(fs::read_to_string(backup.join(".claude.json")).unwrap().contains("host@x"));

    // Again: nothing to do, nothing written.
    let again = h::preview_with(&host, &here, None).unwrap();
    for id in ["instructions", "skills", "config"] {
        assert_eq!(item(&again, id)["pending"], 0, "{id}: {}", item(&again, id));
    }
    let done = h::apply_with(&host, &here, None, &items).unwrap();
    assert!(done["backup"].is_null());
    assert_eq!(item(&done, "skills")["status"], "unchanged");
    assert!(h::apply_with(&host, &here, None, &["passwords".to_string()]).is_err());
}

#[test]
fn the_vault_is_cloned_then_pulled_from_the_same_remote() {
    let base = root("vault");
    let cfg = base.join("gitconfig");
    let bare = base.join("dev.git");
    // The remote is a network URL; git rewrites it to a local bare repository on both sides.
    let url = "https://example.invalid/me/dev.git";
    fs::write(&cfg, format!("[user]\n\tname = T\n\temail = t@x\n[init]\n\tdefaultBranch = main\n[url \"file://{}\"]\n\tinsteadOf = {url}\n", bare.display())).unwrap();
    std::env::set_var("GIT_CONFIG_GLOBAL", &cfg);
    std::env::set_var("GIT_CONFIG_NOSYSTEM", "1");
    fs::create_dir_all(&bare).unwrap();
    git(&bare, &["init", "-q", "--bare"], &cfg);
    let here = base.join("here");
    let vault = here.join("Obsidian/Dev");
    fs::create_dir_all(&here).unwrap();
    let out = Command::new("git").args(["clone", "-q", url]).arg(&vault).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    write(&vault.join("AGENTS.md"), "vault\n");
    git(&vault, &["config", "user.name", "Andrés V"], &cfg);
    git(&vault, &["config", "user.email", "a@x"], &cfg);
    git(&vault, &["add", "-A"], &cfg);
    git(&vault, &["commit", "-qm", "init"], &cfg);
    git(&vault, &["push", "-q", "origin", "HEAD:main"], &cfg);
    let config = json!({"path":vault.display().to_string(),"remote":{"url":url}});
    let there = base.join("there");
    fs::create_dir_all(&there).unwrap();
    let host = [Session::Local { home: there.clone() }];
    let vault_items = vec!["vault".to_string()];

    let preview = h::preview_with(&host, &here, Some(&config)).unwrap();
    let v = &item(&preview, "vault")["vault"];
    assert_eq!(v["action"], "clone", "{v}");
    assert_eq!(v["access"], "ok");
    assert_eq!(v["path"], "~/Obsidian/Dev");
    let done = h::apply_with(&host, &here, Some(&config), &vault_items).unwrap();
    assert_eq!(item(&done, "vault")["status"], "cloned", "{done}");
    let clone = there.join("Obsidian/Dev");
    assert_eq!(fs::read_to_string(clone.join("AGENTS.md")).unwrap(), "vault\n");
    let name = Command::new("git").arg("-C").arg(&clone).args(["config", "--local", "user.name"]).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&name.stdout).trim(), "Andrés V", "the commit identity comes along");

    write(&vault.join("MEMORY.md"), "more\n");
    git(&vault, &["add", "-A"], &cfg);
    git(&vault, &["commit", "-qm", "more"], &cfg);
    git(&vault, &["push", "-q", "origin", "HEAD:main"], &cfg);
    let preview = h::preview_with(&host, &here, Some(&config)).unwrap();
    assert_eq!(item(&preview, "vault")["vault"]["action"], "pull");
    let done = h::apply_with(&host, &here, Some(&config), &vault_items).unwrap();
    assert_eq!(item(&done, "vault")["status"], "pulled");
    assert!(clone.join("MEMORY.md").exists());

    // A folder with other files there is never overwritten.
    let other = base.join("other");
    write(&other.join("Obsidian/Dev/notes.md"), "mine\n");
    let host = [Session::Local { home: other.clone() }];
    assert_eq!(item(&h::preview_with(&host, &here, Some(&config)).unwrap(), "vault")["vault"]["action"], "occupied");
    assert_eq!(item(&h::apply_with(&host, &here, Some(&config), &vault_items).unwrap(), "vault")["status"], "occupied");
    assert_eq!(fs::read_to_string(other.join("Obsidian/Dev/notes.md")).unwrap(), "mine\n");
}
