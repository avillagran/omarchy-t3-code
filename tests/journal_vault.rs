//! Memory/Obsidian on temporary bare repositories, clones and isolated HOMEs.
use omarchy_t3_client::journal_vault::{self as v, HostFiles, HostTransport, Paths, Task};
use serde_json::{json, Value};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::Once,
    time::{Duration, Instant},
};

static GIT: Once = Once::new();
/// One isolated git identity for the whole test process; nothing of the user's.
fn root(name: &str) -> PathBuf {
    let base = std::env::temp_dir().join("ot3-journal-vault-tests");
    GIT.call_once(|| {
        fs::create_dir_all(&base).unwrap();
        let config = base.join("gitconfig");
        fs::write(&config, "[user]\n\tname = Test\n\temail = test@example.invalid\n[init]\n\tdefaultBranch = main\n[commit]\n\tgpgsign = false\n").unwrap();
        std::env::set_var("GIT_CONFIG_GLOBAL", &config);
        std::env::set_var("GIT_CONFIG_NOSYSTEM", "1");
    });
    let dir = base.join(format!("{name}-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    dir
}
fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git").arg("-C").arg(dir).args(args).output().unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).trim().into()
}
fn bare(root: &Path) -> String {
    let path = root.join("remote/dev.git");
    fs::create_dir_all(&path).unwrap();
    git(&path, &["init", "-q", "--bare", "-b", "main"]);
    path.display().to_string()
}
/// A device with its own HOME and a vault created there, pushed to `url`.
fn device_with_vault(root: &Path, name: &str, url: &str) -> (Paths, PathBuf) {
    let paths = Paths::under(&root.join(name));
    let vault = paths.home.join("Obsidian/Dev");
    v::create(&vault, Some(url), None).unwrap();
    (paths, vault)
}
fn setup(paths: &Paths, url: &str) -> Value {
    v::setup(paths, &json!({"path":"~/Obsidian/Dev","url":url})).unwrap()
}
fn step<'a>(result: &'a Value, id: &str) -> &'a Value {
    result["steps"].as_array().unwrap().iter().find(|s| s["id"] == id).unwrap_or_else(|| panic!("no step {id}: {result}"))
}
fn files_under(dir: &Path) -> Vec<String> {
    let mut out = vec![];
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.file_name().is_some_and(|n| n == ".git") {
                continue;
            }
            if p.is_dir() {
                stack.push(p);
            } else {
                out.push(p.strip_prefix(dir).unwrap().display().to_string());
            }
        }
    }
    out.sort();
    out
}

#[test]
fn create_clone_pull_and_push_between_devices() {
    let root = root("sync");
    let url = bare(&root);
    let (_, node_vault) = device_with_vault(&root, "node", &url);
    assert!(node_vault.join("AGENTS.md").is_file() && node_vault.join("Templates/Project-MEMORY.md").is_file());
    assert_eq!(git(&node_vault, &["log", "--oneline"]).lines().count(), 1);

    let mac = Paths::under(&root.join("mbp-m2"));
    let result = setup(&mac, &url);
    for id in ["folder", "access", "clone", "instructions", "device"] {
        assert_eq!(step(&result, id)["state"], "done", "{id}: {result}");
    }
    let mac_vault = mac.home.join("Obsidian/Dev");
    assert_eq!(v::load_config(&mac)["remote"]["kind"], "local-bare");
    // The device row reached the remote.
    git(&node_vault, &["pull", "-q", "--rebase"]);
    assert!(fs::read_to_string(node_vault.join("Machines.md")).unwrap().contains(&v::device_id()));

    fs::write(node_vault.join("Ideas.md"), "- una idea\n").unwrap();
    assert_eq!(v::commit_push(&node_vault, "journal(dev-vault): idea").unwrap()["pushed"], true);
    v::pull(&mac_vault).unwrap();
    assert_eq!(fs::read_to_string(mac_vault.join("Ideas.md")).unwrap(), "- una idea\n");
    let status = v::status(&mac_vault);
    assert_eq!((status["ahead"].as_u64(), status["behind"].as_u64(), status["changes"].as_u64()), (Some(0), Some(0), Some(0)));
    // Nothing to commit: no empty commit, no push.
    assert_eq!(v::commit_push(&mac_vault, "x").unwrap(), json!({"committed":false,"pushed":false}));
}

#[test]
fn a_rebase_conflict_is_aborted_and_local_commits_kept() {
    let root = root("conflict");
    let url = bare(&root);
    let (_, a) = device_with_vault(&root, "a", &url);
    let b_paths = Paths::under(&root.join("b"));
    setup(&b_paths, &url);
    let b = b_paths.home.join("Obsidian/Dev");
    v::pull(&a).unwrap();
    fs::write(a.join("Ideas.md"), "desde a\n").unwrap();
    v::commit_push(&a, "a").unwrap();
    fs::write(b.join("Ideas.md"), "desde b\n").unwrap();
    assert_eq!(v::commit_push(&b, "b").unwrap_err(), "vault_conflict");
    assert!(!b.join(".git/rebase-merge").exists() && !b.join(".git/rebase-apply").exists());
    assert_eq!(fs::read_to_string(b.join("Ideas.md")).unwrap(), "desde b\n");
    assert_eq!(git(&b, &["log", "-1", "--format=%s"]), "b");
    assert_eq!(v::status(&b)["ahead"], 1);
}

#[test]
fn instruction_block_is_idempotent_and_keeps_foreign_content() {
    let root = root("instructions");
    let home = root.join("home");
    let vault = home.join("Obsidian/Dev");
    fs::create_dir_all(home.join(".claude")).unwrap();
    let foreign = "# Mis reglas\n\n<!-- other-tool:start -->\nno tocar\n<!-- other-tool:end -->\n\nFin.";
    fs::write(home.join(".claude/CLAUDE.md"), foreign).unwrap();
    // ~/.codex/AGENTS.md is a symlink to a regular file elsewhere.
    fs::create_dir_all(root.join("dotfiles")).unwrap();
    fs::write(root.join("dotfiles/AGENTS.md"), "codex\n").unwrap();
    fs::create_dir_all(home.join(".codex")).unwrap();
    std::os::unix::fs::symlink(root.join("dotfiles/AGENTS.md"), home.join(".codex/AGENTS.md")).unwrap();

    let first = v::install_instructions(&home, &vault).unwrap();
    assert!(first.iter().all(|f| f["changed"] == true));
    let once = fs::read_to_string(home.join(".claude/CLAUDE.md")).unwrap();
    assert!(once.starts_with(foreign) && once.contains("`~/Obsidian/Dev`") && once.contains(v::BLOCK_END));
    assert!(fs::symlink_metadata(home.join(".codex/AGENTS.md")).unwrap().file_type().is_symlink());
    assert!(fs::read_to_string(root.join("dotfiles/AGENTS.md")).unwrap().starts_with("codex\n\n<!-- dev-vault-journal:start -->"));
    let second = v::install_instructions(&home, &vault).unwrap();
    assert!(second.iter().all(|f| f["changed"] == false));
    assert_eq!(fs::read_to_string(home.join(".claude/CLAUDE.md")).unwrap(), once);
    assert!(v::instructions_installed(&home, &vault));

    // A moved vault replaces only the block.
    v::install_instructions(&home, &home.join("Notes/Dev")).unwrap();
    let moved = fs::read_to_string(home.join(".claude/CLAUDE.md")).unwrap();
    assert!(moved.starts_with(foreign) && moved.contains("`~/Notes/Dev`") && !moved.contains("`~/Obsidian/Dev`"));
    assert_eq!(moved.matches(v::BLOCK_START).count(), 1);
    // A lone marker is not guessed at.
    assert_eq!(v::upsert_block(&format!("x\n{}\n", v::BLOCK_START), "b").unwrap_err(), "vault_instructions_malformed");
}

#[test]
fn project_stub_and_index_stay_inside_the_vault() {
    let root = root("projects");
    let vault = root.join("vault");
    v::create(&vault, None, None).unwrap();
    fs::write(vault.join("MEMORY.md"), "# MEMORY\n\n## Proyectos\n- [[Projects/old/MEMORY|old]] — viejo\n\n## Otros\n- algo\n").unwrap();
    assert!(v::ensure_project(&vault, "ot3", "omarchy-t3-code", "2026-10-23").unwrap());
    let note = fs::read_to_string(vault.join("Projects/ot3/MEMORY.md")).unwrap();
    assert!(note.contains("project: ot3") && note.contains("updated: 2026-10-23") && !note.contains("<slug>"));
    assert!(vault.join("Projects/ot3/Sessions/.gitkeep").is_file());
    let index = fs::read_to_string(vault.join("MEMORY.md")).unwrap();
    assert_eq!(index, "# MEMORY\n\n## Proyectos\n- [[Projects/old/MEMORY|old]] — viejo\n- [[Projects/ot3/MEMORY|ot3]] — omarchy-t3-code\n\n## Otros\n- algo\n");
    // Again: nothing rewritten, no second index line.
    fs::write(vault.join("Projects/ot3/MEMORY.md"), "editado\n").unwrap();
    assert!(!v::ensure_project(&vault, "ot3", "omarchy-t3-code", "2026-10-24").unwrap());
    assert_eq!(fs::read_to_string(vault.join("Projects/ot3/MEMORY.md")).unwrap(), "editado\n");
    assert_eq!(fs::read_to_string(vault.join("MEMORY.md")).unwrap(), index);

    for bad in ["../x", "a/b", "", ".hidden", ".."] {
        assert_eq!(v::ensure_project(&vault, bad, "t", "d").unwrap_err(), "vault_invalid_slug", "{bad}");
    }
    // Projects/ pointing outside the vault: nothing is written there.
    let outside = root.join("outside");
    fs::create_dir_all(&outside).unwrap();
    let other = root.join("vault2");
    v::create(&other, None, None).unwrap();
    fs::remove_dir_all(other.join("Projects")).unwrap();
    std::os::unix::fs::symlink(&outside, other.join("Projects")).unwrap();
    assert_eq!(v::ensure_project(&other, "evil", "t", "d").unwrap_err(), "vault_path_escape");
    assert!(files_under(&outside).is_empty() && fs::read_dir(&outside).unwrap().next().is_none());

    assert_eq!(v::project_slug(&json!({}), "My Project", "/home/me/Work/omarchy-t3-code").as_deref(), Some("omarchy-t3-code"));
    assert_eq!(v::project_slug(&json!({"projects":{"T":"mapped"}}), "T", "/x/y").as_deref(), Some("mapped"));
    assert_eq!(v::project_slug(&json!({}), "Hola Mundo", "").as_deref(), Some("Hola-Mundo"));
}

#[test]
fn device_rows_are_upserted_and_the_note_kept() {
    let root = root("machines");
    let vault = root.join("vault");
    v::create(&vault, None, None).unwrap();
    let before = fs::read_to_string(vault.join("Machines.md")).unwrap();
    let mac = json!({"id":"abc","name":"mbp-m2","environment":"","system":"linux aarch64","path":"~/Obsidian/Dev","updated":"2026-10-23"});
    assert!(v::register_device(&vault, &mac).unwrap());
    assert!(!v::register_device(&vault, &mac).unwrap());
    let other = json!({"id":"def","name":"node | x","environment":"8b55","system":"linux x86_64","path":"~/Obsidian/Dev","updated":"2026-10-23"});
    assert!(v::register_device(&vault, &other).unwrap());
    let mut renamed = mac.clone();
    renamed["name"] = json!("mbp");
    assert!(v::register_device(&vault, &renamed).unwrap());
    let after = fs::read_to_string(vault.join("Machines.md")).unwrap();
    assert!(after.starts_with(before.trim_end()));
    assert_eq!(after.matches("| `abc` |").count(), 1);
    assert!(after.contains("| mbp | `abc` | — |") && after.contains("| node   x | `def` | `8b55` |"));
}

/// A host's files in memory; records every call so tests can prove it is only read.
struct FakeHost {
    files: BTreeMap<String, String>,
    dirs: Vec<String>,
    offline: Option<String>,
    calls: RefCell<Vec<String>>,
}
impl FakeHost {
    fn new(files: &[(&str, &str)]) -> Self {
        FakeHost { files: files.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(), dirs: vec![], offline: None, calls: RefCell::new(vec![]) }
    }
}
impl HostFiles for FakeHost {
    fn metadata(&self, paths: &[String]) -> Result<Vec<Option<String>>, String> {
        self.calls.borrow_mut().push(format!("metadata {}", paths.len()));
        if let Some(e) = &self.offline {
            return Err(e.clone());
        }
        Ok(paths.iter().map(|p| if self.files.contains_key(p) { Some("file".into()) } else if self.dirs.contains(p) { Some("directory".into()) } else { None }).collect())
    }
    fn read(&self, path: &str) -> Result<String, String> {
        self.calls.borrow_mut().push(format!("read {path}"));
        self.files.get(path).cloned().ok_or("missing".into())
    }
}
const GIT_CONFIG: &str = "[core]\n\tbare = false\n[remote \"origin\"]\n\turl = https://user:ghp_secret@github.com/avillagran/dev-vault.git\n\tfetch = +refs/heads/*:refs/remotes/origin/*\n";

#[test]
fn hosts_are_read_and_vaults_grouped_by_remote() {
    // node: set up by hand, found from the instructions it gives its agents.
    let node = FakeHost::new(&[
        ("~/.claude/CLAUDE.md", "## Bitácora central de desarrollo (Dev Vault)\n\nEl vault git `~/Obsidian/Dev` es la fuente de verdad.\n"),
        ("~/Obsidian/Dev/AGENTS.md", "# AGENTS"),
        ("~/Obsidian/Dev/.git/config", GIT_CONFIG),
    ]);
    // omarchy: an ot3 that saved its own vault.json, same remote spelled over SSH.
    let omarchy = FakeHost::new(&[
        ("~/.config/omarchy-t3-code/vault.json", r#"{"enabled":true,"path":"/home/a/Notes/Dev"}"#),
        ("/home/a/Notes/Dev/AGENTS.md", "# AGENTS"),
        ("/home/a/Notes/Dev/.git/config", "[remote \"origin\"]\n\turl = git@github.com:avillagran/dev-vault\n"),
    ]);
    // work: another vault; lab: a folder with notes but no AGENTS.md is not a vault.
    let work = FakeHost::new(&[("~/Obsidian/Dev/AGENTS.md", "x"), ("~/Obsidian/Dev/.git/config", "[remote \"origin\"]\n url = https://gitlab.com/me/work-vault.git\n")]);
    let lab = FakeHost::new(&[("~/Obsidian/Dev/Home.md", "x")]);
    let mut hosts = vec![];
    for (id, name, host) in [("n", "node", &node), ("o", "omarchy", &omarchy), ("w", "work", &work), ("l", "lab", &lab)] {
        let mut h = v::host_vaults(host, None);
        h["environment"] = json!(id);
        h["name"] = json!(name);
        hosts.push(h);
    }
    assert_eq!(hosts[0]["state"], "online");
    let found = &hosts[0]["vaults"][0];
    assert_eq!((found["path"].as_str(), found["from"].as_str()), (Some("~/Obsidian/Dev"), Some("instructions")));
    // The token never leaves the host's file.
    assert_eq!(found["remote"], "https://github.com/avillagran/dev-vault.git");
    assert!(!hosts.iter().any(|h| h.to_string().contains("ghp_secret")));
    assert_eq!(hosts[1]["vaults"][0]["path"], "/home/a/Notes/Dev");
    assert_eq!(hosts[3]["state"], "online");
    assert!(hosts[3]["vaults"].as_array().unwrap().is_empty());

    let groups = v::group(&hosts);
    assert_eq!(groups.len(), 2, "{groups:?}");
    let shared = groups.iter().find(|g| g["id"] == "github.com/avillagran/dev-vault").unwrap();
    assert_eq!(shared["hosts"].as_array().unwrap().len(), 2);
    assert_eq!(shared["kind"], "github-private");
    assert!(groups.iter().any(|g| g["id"] == "gitlab.com/me/work-vault"));
    // Hosts are only ever read.
    for host in [&node, &omarchy, &work, &lab] {
        assert!(host.calls.borrow().iter().all(|c| c.starts_with("metadata ") || c.starts_with("read ")));
    }
}

#[test]
fn an_unreachable_host_is_not_a_host_without_vault() {
    let mut host = FakeHost::new(&[("~/Obsidian/Dev/AGENTS.md", "x")]);
    host.offline = Some("ssh_unavailable".into());
    let report = v::host_vaults(&host, None);
    assert_eq!((report["state"].as_str(), report["error"].as_str()), (Some("offline"), Some("ssh_unavailable")));
    host.offline = Some("rpc_rejected: unknown method".into());
    assert_eq!(v::host_vaults(&host, None)["state"], "unsupported");
    // Servers that do not expand ~/: the home their projects show is used.
    let old = FakeHost::new(&[("/home/node/Obsidian/Dev/AGENTS.md", "x")]);
    assert!(v::host_vaults(&old, None)["vaults"].as_array().unwrap().is_empty());
    let home = v::home_from_projects(&["/srv/x".into(), "/home/node/Desarrollo/ggup".into()]).unwrap();
    assert_eq!(v::host_vaults(&old, Some(&home))["vaults"][0]["path"], "/home/node/Obsidian/Dev");
    // node (Oct 2026): getMetadata expands ~/ but readFile does not.
    let server = |p: &str| -> Result<String, String> {
        if p.starts_with("~/") { Err("rpc_rejected: Failed to read workspace file".into()) } else if p == "/home/node/Obsidian/Dev/.git/config" { Ok(GIT_CONFIG.into()) } else { Err("missing".into()) }
    };
    assert!(v::read_with_home(server, None, "~/Obsidian/Dev/.git/config").is_err());
    assert_eq!(v::read_with_home(server, Some("/home/node"), "~/Obsidian/Dev/.git/config").unwrap(), GIT_CONFIG);
    // A vault without origin is reported, but cannot be shared.
    let local = FakeHost::new(&[("~/Obsidian/Dev/AGENTS.md", "x"), ("~/Obsidian/Dev/.git/config", "[core]\n")]);
    let mut h = v::host_vaults(&local, None);
    h["environment"] = json!("x");
    assert_eq!(v::group(&[h])[0]["shareable"], false);
}

#[test]
fn an_existing_clone_is_reused_and_the_wizard_is_idempotent() {
    let root = root("reuse");
    let url = bare(&root);
    device_with_vault(&root, "node", &url);
    let mac = Paths::under(&root.join("mac"));
    // Already cloned by hand, spelled differently.
    git(&root, &["clone", "-q", &format!("file://{url}"), &mac.home.join("Obsidian/Dev").display().to_string()]);
    let first = setup(&mac, &url);
    assert_eq!(step(&first, "folder")["reuse"], true);
    assert_eq!(step(&first, "clone")["reused"], true);
    let vault = mac.home.join("Obsidian/Dev");
    let claude = fs::read_to_string(mac.home.join(".claude/CLAUDE.md")).unwrap();
    let machines = fs::read_to_string(vault.join("Machines.md")).unwrap();
    let commits = git(&vault, &["rev-list", "--count", "HEAD"]);
    let second = setup(&mac, &url);
    assert_eq!(step(&second, "clone")["reused"], true);
    assert_eq!(fs::read_to_string(mac.home.join(".claude/CLAUDE.md")).unwrap(), claude);
    assert_eq!(fs::read_to_string(vault.join("Machines.md")).unwrap(), machines);
    assert_eq!(git(&vault, &["rev-list", "--count", "HEAD"]), commits, "no new commit when nothing changed");
    // Repair restores a deleted template and pushes it.
    fs::remove_file(vault.join("Templates/Session.md")).unwrap();
    git(&vault, &["commit", "-qam", "oops"]);
    git(&vault, &["push", "-q"]);
    let repaired = v::repair(&mac).unwrap();
    assert!(repaired["fixed"].as_array().unwrap().contains(&json!("Templates/Session.md")), "{repaired}");
    assert_eq!(v::status(&vault)["ahead"], 0);
}

#[test]
fn an_occupied_folder_is_never_overwritten() {
    let root = root("occupied");
    let url = bare(&root);
    device_with_vault(&root, "node", &url);
    let mac = Paths::under(&root.join("mac"));
    let target = mac.home.join("Obsidian/Dev");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("personal.md"), "mío\n").unwrap();
    let result = setup(&mac, &url);
    assert_eq!(result["complete"], false);
    assert_eq!(step(&result, "folder")["error"], "vault_path_occupied: not-git");
    assert_eq!(files_under(&target), vec!["personal.md".to_string()]);
    assert!(!mac.config.exists(), "nothing saved for a refused folder");
    // A clone of another remote is occupied too; another folder works.
    let other = bare(&root.join("x"));
    device_with_vault(&root, "y", &other);
    git(&root, &["clone", "-q", &other, &mac.home.join("Notes").display().to_string()]);
    assert_eq!(v::inspect(&mac.home.join("Notes"), Some(&url))["reason"], "other-remote");
    let elsewhere = v::setup(&mac, &json!({"path":"~/Vaults/Dev","url":url})).unwrap();
    assert_eq!(step(&elsewhere, "clone")["state"], "done");
    assert_eq!(fs::read_to_string(target.join("personal.md")).unwrap(), "mío\n");
    // A file at the path.
    fs::write(mac.home.join("file"), "x").unwrap();
    assert_eq!(v::inspect(&mac.home.join("file"), None)["reason"], "not-directory");
    // The home folder itself is refused.
    assert_eq!(v::setup(&mac, &json!({"path":"~","url":url})).unwrap_err(), "vault_invalid_path");
}

#[test]
fn missing_local_authentication_is_explained_not_copied() {
    let root = root("auth");
    let mac = Paths::under(&root.join("mac"));
    // An SSH that always refuses the key, as with no key on this computer.
    let refuse = "sh -c 'echo \"git@example.invalid: Permission denied (publickey).\" >&2; exit 255' --";
    let result = v::setup(&mac, &json!({"path":"~/Obsidian/Dev","url":"git@example.invalid:me/dev-vault.git","sshCommand":refuse})).unwrap();
    let access = step(&result, "access");
    assert_eq!((access["state"].as_str(), access["error"].as_str()), (Some("failed"), Some("vault_auth_missing")));
    assert_eq!(access["help"]["provider"], "ssh");
    assert!(!mac.home.join("Obsidian/Dev").exists(), "no clone without access");
    for (stderr, code) in [
        ("fatal: could not read Username for 'https://github.com': terminal prompts disabled", "vault_auth_missing"),
        ("remote: Repository not found.\nfatal: repository 'https://github.com/a/b.git/' not found", "vault_auth_missing"),
        ("ssh: Could not resolve hostname node: Name or service not known", "vault_remote_unreachable"),
        ("fatal: '/srv/x.git' does not appear to be a git repository", "vault_remote_missing"),
        ("Author identity unknown\n*** Please tell me who you are.", "vault_git_identity_missing"),
    ] {
        assert_eq!(v::classify(stderr), code, "{stderr}");
    }
    let help = v::auth_help("https://github.com/avillagran/dev-vault.git");
    assert_eq!(help["steps"], json!(["gh auth login", "gh auth setup-git"]));
    // Credentials typed into a URL are refused, never stored.
    assert_eq!(v::setup(&mac, &json!({"path":"~/Obsidian/Dev","url":"https://x:tok@github.com/a/b.git"})).unwrap_err(), "vault_url_has_credentials");
    assert_eq!(v::save_config(&mac, &json!({"remote":{"kind":"custom-url","url":"https://tok@h/x"}})).unwrap_err(), "vault_url_has_credentials");
}

#[test]
fn mbp_m2_with_environment_node_writes_only_on_this_computer() {
    let root = root("client");
    let url = bare(&root);
    // node holds the vault; its filesystem is only what its T3 server answers.
    let node_files = FakeHost::new(&[
        ("~/.config/omarchy-t3-code/vault.json", &format!(r#"{{"path":"/home/node/Obsidian/Dev","remote":{{"url":"{url}"}}}}"#)),
        ("/home/node/Obsidian/Dev/AGENTS.md", "# AGENTS"),
        ("/home/node/Obsidian/Dev/.git/config", &format!("[remote \"origin\"]\n\turl = {url}\n")),
    ]);
    let before = node_files.files.clone();
    let mut node = v::host_vaults(&node_files, None);
    node["environment"] = json!("8b55eaad-3149-443c-953f-afd6dd002666");
    let group = &v::group(&[node])[0];
    assert_eq!(group["kind"], "local-bare");
    let remote = group["hosts"][0]["remote"].as_str().unwrap();

    // A bare repository on node is reached over node's SSH profile, or OmaBridge.
    let ssh = HostTransport { ssh_args: Some(vec!["-o".into(), "BatchMode=yes".into(), "-p".into(), "2222".into(), "-l".into(), "node".into(), "--".into(), "node.lan".into()]), omabridge: None };
    let source = v::clone_source(remote, &ssh).unwrap();
    assert_eq!(source["url"], format!("ssh://node@node.lan:2222{url}"));
    assert_eq!(source["transport"], "ssh");
    let bridged = HostTransport { omabridge: Some(("/opt/omabridge".into(), "/home/me/.local/share/omabridge".into(), "a".repeat(64))), ..ssh.clone() };
    let source = v::clone_source(remote, &bridged).unwrap();
    assert_eq!(source["transport"], "omabridge");
    assert!(source["sshCommand"].as_str().unwrap().contains(&format!("connect --peer {} --service ssh", "a".repeat(64))));
    assert_eq!(v::clone_source(remote, &HostTransport::default()).unwrap_err(), "vault_remote_needs_ssh");
    assert_eq!(v::clone_source("~/git/dev.git", &ssh).unwrap()["url"], "ssh://node@node.lan:2222/~/git/dev.git");

    // The test can't ssh into a node: clone the same bare repository directly, as the
    // URL above resolves to it. Every write lands in mbp-m2's HOME.
    device_with_vault(&root, "node-home", &url);
    let mac = Paths::under(&root.join("mbp-m2"));
    let result = v::setup(&mac, &json!({"path":"~/Obsidian/Dev","url":url,"environment":"8b55eaad-3149-443c-953f-afd6dd002666"})).unwrap();
    assert_eq!(step(&result, "clone")["state"], "done");
    assert_eq!(node_files.files, before);
    assert!(node_files.calls.borrow().iter().all(|c| c.starts_with("metadata ") || c.starts_with("read ")));
    let mut top: Vec<String> = fs::read_dir(&mac.home).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    top.sort();
    assert_eq!(top, [".claude", ".codex", ".config", ".local", "Obsidian"]);
    assert_eq!(files_under(&mac.home.join(".config")), ["omarchy-t3-code/vault.json"]);
    let config = v::load_config(&mac);
    assert_eq!(config["remote"]["environment"], "8b55eaad-3149-443c-953f-afd6dd002666");
    assert_eq!(config["path"], mac.home.join("Obsidian/Dev").display().to_string());
    // No credential in this computer's config.
    assert!(!fs::read_to_string(&mac.config).unwrap().contains("ghp_"));
}

#[test]
fn remotes_are_compared_without_credentials() {
    for (a, b) in [
        ("https://github.com/Me/Dev.git", "git@github.com:Me/Dev"),
        ("ssh://git@github.com/Me/Dev.git/", "https://tok@github.com/Me/Dev"),
        ("ssh://node@host:2222/srv/dev.git", "node@host:/srv/dev"),
    ] {
        assert_eq!(v::canonical_remote(a), v::canonical_remote(b), "{a} {b}");
    }
    assert_eq!(v::canonical_remote("/srv/dev.git").as_deref(), Some("path:/srv/dev"));
    assert_eq!(v::redact_url("https://user:pw@github.com/a/b.git"), "https://github.com/a/b.git");
    assert_eq!(v::redact_url("ssh://me:pw@host/x"), "ssh://me@host/x");
    assert_eq!(v::redact_url("git@github.com:a/b.git"), "git@github.com:a/b.git");
    assert!(!v::has_credentials("ssh://git@github.com/a/b"));
    assert_eq!(v::origin_url(GIT_CONFIG).as_deref(), Some("https://user:ghp_secret@github.com/avillagran/dev-vault.git"));
    assert_eq!(v::kind_of("https://gitlab.com/a/b"), "custom-url");
    for bad in ["-uhack", "ext::sh -c touch% /tmp/x", "https://h/x y", "", "fd::1"] {
        assert!(!v::valid_remote_url(bad), "{bad}");
    }
    for good in ["https://github.com/a/b.git", "git@github.com:a/b.git", "ssh://me@h:22/srv/x.git", "/srv/x.git", "file:///srv/x.git"] {
        assert!(v::valid_remote_url(good), "{good}");
    }
}

#[test]
fn automations_pull_on_open_and_start_push_after_settle_and_stub_new_projects() {
    let config = json!({"enabled":true,"remote":{"url":"x"},"sync":{"pullOnOpen":true,"pullOnThreadStart":true,"pushOnSettle":true,"intervalMinutes":null},"projects":{}});
    let project = |id: &str, root: &str| json!({"key":format!("e/{id}"),"id":id,"environmentId":"e","title":id,"workspaceRoot":root});
    let snap = |working: bool, projects: Vec<Value>| json!({"threads":[{"key":"e/t","project":"p1","working":working}],"projects":projects});
    let mut a = v::Automation::default();
    let t0 = Instant::now();
    // Existing projects get no note; opening pulls.
    assert_eq!(a.observe(&config, &snap(false, vec![project("p1", "/w/ot3")]), t0), vec![Task::Pull]);
    assert_eq!(a.observe(&config, &snap(true, vec![project("p1", "/w/ot3")]), t0 + Duration::from_secs(61)), vec![Task::Pull]);
    // Settled: the push waits SETTLE_DELAY with no thread working.
    assert!(a.observe(&config, &snap(false, vec![project("p1", "/w/ot3")]), t0 + Duration::from_secs(70)).is_empty());
    assert_eq!(a.next_due(), Some(t0 + Duration::from_secs(70) + v::SETTLE_DELAY));
    let tasks = a.observe(&config, &snap(false, vec![project("p1", "/w/ot3"), project("p2", "/w/new-one")]), t0 + Duration::from_secs(101));
    assert_eq!(tasks, vec![Task::Push("ot3".into()), Task::Project { slug: "new-one".into(), title: "p2".into() }]);
    // A restart within a minute does not pull twice.
    assert!(a.observe(&config, &snap(true, vec![]), t0 + Duration::from_secs(110)).is_empty());
    // Disabled: nothing.
    let mut off = v::Automation::default();
    assert!(off.observe(&json!({"enabled":false}), &snap(true, vec![]), t0).is_empty());
}

#[test]
fn configuration_is_validated_and_unknown_fields_kept() {
    let root = root("config");
    let paths = Paths::under(&root.join("home"));
    fs::create_dir_all(paths.config.parent().unwrap()).unwrap();
    fs::write(&paths.config, r#"{"version":1,"future":{"x":1}}"#).unwrap();
    let saved = v::save_config(&paths, &json!({"path":"~/Notes","sync":{"intervalMinutes":30},"projects":{"T3":"ot3"}})).unwrap();
    assert_eq!(saved["path"], paths.home.join("Notes").display().to_string());
    assert_eq!(saved["sync"]["pullOnOpen"], true);
    let raw: Value = serde_json::from_slice(&fs::read(&paths.config).unwrap()).unwrap();
    assert_eq!(raw["future"]["x"], 1);
    for bad in [json!({"path":"relative"}), json!({"path":"/a/../b"}), json!({"sync":{"intervalMinutes":1}}), json!({"projects":{"a":"../x"}}), json!({"nope":1}), json!({"remote":{"kind":"ftp","url":"x"}})] {
        assert!(v::save_config(&paths, &bad).is_err(), "{bad}");
    }
    let state = v::read(&paths);
    assert_eq!(state["configured"], false);
    assert_eq!(state["local"]["state"], "missing");
    assert_eq!(state["steps"]["clone"], false);
}
