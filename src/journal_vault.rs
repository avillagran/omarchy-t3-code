//! Preferences → Memory/Obsidian: the development vault on this computer
//! (docs/JOURNAL_VAULT.md). A git repository of Obsidian notes shared by every
//! machine; ot3 clones it here, keeps it in sync and points local agents at it.
//!
//! "This computer" is always the device ot3 runs on. Connected hosts are only read
//! through T3's `filesystem.getMetadata` / `projects.readFile` RPCs: nothing is
//! written, cloned or run on them. No credential is read from or copied to a host.
use crate::{home, json, save_json, text, xdg, Environment, Result};
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashSet},
    fs,
    io::Write,
    os::unix::fs::PermissionsExt,
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

pub const BLOCK_START: &str = "<!-- dev-vault-journal:start -->";
pub const BLOCK_END: &str = "<!-- dev-vault-journal:end -->";
const DEVICES_START: &str = "<!-- ot3-devices:start -->";
const DEVICES_END: &str = "<!-- ot3-devices:end -->";
pub const DEFAULT_PATH: &str = "~/Obsidian/Dev";
/// Files a host may hold its vault location in; nothing else is read on a host.
const HOST_CONFIG: &str = "~/.config/omarchy-t3-code/vault.json";
const HOST_INSTRUCTIONS: [&str; 2] = ["~/.claude/CLAUDE.md", "~/.codex/AGENTS.md"];
const DEFAULT_SSH: &str = "ssh -o BatchMode=yes -o ConnectTimeout=10";
const MAX_CANDIDATES: usize = 4;

/// Where this computer keeps its own files. Tests point it at a temporary HOME.
#[derive(Clone, Debug)]
pub struct Paths {
    pub home: PathBuf,
    pub config: PathBuf,
    pub state: PathBuf,
}
impl Paths {
    pub fn current() -> Self {
        Self {
            home: home(),
            config: xdg("XDG_CONFIG_HOME", ".config").join("vault.json"),
            state: xdg("XDG_STATE_HOME", ".local/state").join("vault-state.json"),
        }
    }
    pub fn under(home: &Path) -> Self {
        Self {
            home: home.into(),
            config: home.join(".config/omarchy-t3-code/vault.json"),
            state: home.join(".local/state/omarchy-t3-code/vault-state.json"),
        }
    }
    pub fn expand(&self, path: &str) -> PathBuf {
        match path.strip_prefix("~/") {
            Some(rest) => self.home.join(rest),
            None if path == "~" => self.home.clone(),
            None => PathBuf::from(path),
        }
    }
}

// ---------------------------------------------------------------- remotes

/// The URL without any password or token. `https://token@host/x` becomes
/// `https://host/x`; an SSH user (`git@`, `ssh://me@`) is not a secret and stays.
pub fn redact_url(url: &str) -> String {
    let url = url.trim();
    for scheme in ["https://", "http://"] {
        if let Some(rest) = url.strip_prefix(scheme) {
            let (authority, path) = rest.split_once('/').map_or((rest, ""), |(a, p)| (a, p));
            let host = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
            return if path.is_empty() { format!("{scheme}{host}") } else { format!("{scheme}{host}/{path}") };
        }
    }
    if let Some(rest) = url.strip_prefix("ssh://") {
        let (authority, path) = rest.split_once('/').map_or((rest, ""), |(a, p)| (a, p));
        let authority = match authority.rsplit_once('@') {
            Some((user, host)) => format!("{}@{host}", user.split(':').next().unwrap_or("")),
            None => authority.into(),
        };
        return format!("ssh://{authority}/{path}");
    }
    url.into()
}
pub fn has_credentials(url: &str) -> bool {
    redact_url(url) != url.trim()
}
/// Only plain git transports: no option-like value and no `ext::`/helper URLs.
pub fn valid_remote_url(url: &str) -> bool {
    let url = url.trim();
    if url.is_empty() || url.len() > 1024 || url.starts_with('-') || url.chars().any(|c| c.is_control() || c.is_whitespace()) || url.contains("::") {
        return false;
    }
    url.starts_with("https://")
        || url.starts_with("ssh://")
        || url.starts_with("file://")
        || url.starts_with('/')
        || url.starts_with("~/")
        || url.split_once(':').is_some_and(|(authority, path)| !authority.contains('/') && !authority.is_empty() && !path.is_empty())
}
/// A local path (or `file://`) rather than a network remote.
pub fn is_path(url: &str) -> bool {
    let url = url.trim();
    url.starts_with('/') || url.starts_with("~/") || url.starts_with("file://")
}
/// Same repository, whatever the spelling: `github.com/owner/repo` for GitHub over
/// HTTPS or SSH, `host/path` for other SSH or HTTPS remotes, `path:<p>` for paths.
pub fn canonical_remote(url: &str) -> Option<String> {
    let url = redact_url(url);
    let url = url.trim().trim_end_matches('/');
    if url.is_empty() || url.chars().any(char::is_control) {
        return None;
    }
    let tidy = |s: &str| s.trim_end_matches('/').trim_end_matches(".git").trim_end_matches('/').to_string();
    if is_path(url) {
        return Some(format!("path:{}", tidy(url.trim_start_matches("file://"))));
    }
    let (host, path) = if let Some(rest) = url.strip_prefix("https://").or(url.strip_prefix("http://")) {
        rest.split_once('/')?
    } else if let Some(rest) = url.strip_prefix("ssh://") {
        let (authority, path) = rest.split_once('/')?;
        (authority.rsplit_once('@').map_or(authority, |(_, h)| h), path)
    } else if let Some((authority, path)) = url.split_once(':') {
        // scp-like `git@github.com:owner/repo.git`.
        (authority.rsplit_once('@').map_or(authority, |(_, h)| h), path)
    } else {
        return None;
    };
    let host = host.split(':').next().unwrap_or(host).to_ascii_lowercase();
    let path = tidy(path.trim_start_matches('/').trim_start_matches("~/"));
    if host.is_empty() || path.is_empty() {
        return None;
    }
    Some(format!("{host}/{path}"))
}
pub fn kind_of(url: &str) -> &'static str {
    if canonical_remote(url).is_some_and(|c| c.starts_with("github.com/")) {
        "github-private"
    } else if is_path(url) {
        "local-bare"
    } else {
        "custom-url"
    }
}
/// `url` of `[remote "origin"]` in a `.git/config`.
pub fn origin_url(git_config: &str) -> Option<String> {
    let mut inside = false;
    for line in git_config.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            inside = line.replace(' ', "").replace('\t', "") == "[remote\"origin\"]";
            continue;
        }
        if inside {
            if let Some((key, value)) = line.split_once('=') {
                if key.trim().eq_ignore_ascii_case("url") {
                    let value = value.trim().trim_matches('"');
                    if !value.is_empty() {
                        return Some(value.into());
                    }
                }
            }
        }
    }
    None
}

// ---------------------------------------------------------------- host detection

/// Read-only access to a host's files. In production this is T3's RPC on that
/// environment; it never writes.
pub trait HostFiles {
    /// One kind (`file`, `directory`, `other`) or None per path, in order.
    fn metadata(&self, paths: &[String]) -> Result<Vec<Option<String>>>;
    fn read(&self, path: &str) -> Result<String>;
}
/// Read `path`; when a `~/` path fails and the host's home is known, read it again
/// as an absolute path.
pub fn read_with_home(read: impl Fn(&str) -> Result<String>, home: Option<&str>, path: &str) -> Result<String> {
    match (read(path), path.strip_prefix("~/"), home) {
        (Err(_), Some(rest), Some(home)) => read(&format!("{home}/{rest}")),
        (result, _, _) => result,
    }
}
/// T3 on the host. `home` is the host's home folder when known: servers that
/// expand `~/` in getMetadata but not in readFile (node, Oct 2026) need it.
pub struct RpcFiles<'a>(pub &'a Environment, pub Option<String>);
impl RpcFiles<'_> {
    fn read_once(&self, path: &str) -> Result<String> {
        let v = crate::write::rpc(self.0, "projects.readFile", json!({"cwd":"/","relativePath":path,"allowMissing":true}))?;
        if v["truncated"] == true {
            return Err("vault_host_file_too_large".into());
        }
        Ok(text(&v, "contents").into())
    }
}
impl HostFiles for RpcFiles<'_> {
    fn metadata(&self, paths: &[String]) -> Result<Vec<Option<String>>> {
        let v = crate::write::rpc(self.0, "filesystem.getMetadata", json!({"paths":paths}))?;
        let entries = v["entries"].as_array().ok_or("vault_host_unsupported")?;
        Ok(entries.iter().map(|e| e["kind"].as_str().map(str::to_owned)).collect())
    }
    fn read(&self, path: &str) -> Result<String> {
        read_with_home(|p| self.read_once(p), self.1.as_deref(), path)
    }
}
fn safe_host_path(p: &str) -> bool {
    (p.starts_with("~/") || p.starts_with('/'))
        && p.len() <= 512
        && !p.chars().any(char::is_control)
        && !p.split('/').any(|s| s == "..")
}
/// Vault folders named in an agent instruction file: inside the marked block, or
/// a `~/…` / `/…` path in backticks on a line that speaks of a vault.
pub fn instruction_paths(contents: &str) -> Vec<String> {
    let mut found = vec![];
    let block = contents
        .find(BLOCK_START)
        .and_then(|s| contents[s..].find(BLOCK_END).map(|e| &contents[s..s + e]));
    let mut push = |p: &str| {
        let p = p.trim().trim_end_matches('/').trim_end_matches(['.', ',', ';', ':']);
        if safe_host_path(p) && !found.iter().any(|f| f == p) {
            found.push(p.to_string());
        }
    };
    for (scope, any_line) in [(block.unwrap_or(""), true), (contents, false)] {
        for line in scope.lines() {
            if !any_line && !line.to_lowercase().contains("vault") {
                continue;
            }
            for (i, part) in line.split('`').enumerate() {
                if i % 2 == 1 && (part.starts_with("~/") || part.starts_with('/')) && !part.contains(' ') {
                    push(part);
                }
            }
        }
    }
    found
}
/// Home folder of a host whose server does not expand `~/`, from its projects.
pub fn home_from_projects(roots: &[String]) -> Option<String> {
    for root in roots {
        let parts: Vec<&str> = root.split('/').collect();
        match parts.as_slice() {
            ["", "home" | "Users", user, ..] if !user.is_empty() => return Some(format!("/{}/{user}", parts[1])),
            ["", "root", ..] => return Some("/root".into()),
            _ => {}
        }
    }
    None
}
fn host_error(error: &str) -> &'static str {
    if error.starts_with("rpc_rejected") || error.contains("unknown_method") || error.contains("not_supported") {
        "unsupported"
    } else {
        "offline"
    }
}
/// What one host reports: `online` (with zero or more vaults), `offline` or
/// `unsupported`. An unreachable host is never reported as having no vault.
pub fn host_vaults(files: &dyn HostFiles, home: Option<&str>) -> Value {
    let expand = |p: &str| match (p.strip_prefix("~/"), home) {
        (Some(rest), Some(h)) => format!("{h}/{rest}"),
        _ => p.to_string(),
    };
    let sources: Vec<String> = std::iter::once(HOST_CONFIG).chain(HOST_INSTRUCTIONS).map(expand).collect();
    let kinds = match files.metadata(&sources) {
        Ok(k) => k,
        Err(e) => return json!({"state":host_error(&e),"error":e,"vaults":[]}),
    };
    let mut instructions: Vec<Option<String>> = vec![None, None];
    let mut candidates: Vec<(String, &str)> = vec![];
    let add = |p: String, from: &'static str, list: &mut Vec<(String, &str)>| {
        let p = expand(&p);
        if safe_host_path(&p) && !list.iter().any(|(c, _)| *c == p) && list.len() < MAX_CANDIDATES {
            list.push((p, from));
        }
    };
    for (i, path) in sources.iter().enumerate() {
        if kinds.get(i).cloned().flatten().as_deref() != Some("file") {
            continue;
        }
        let Ok(contents) = files.read(path) else { continue };
        if i == 0 {
            let config: Value = serde_json::from_str(&contents).unwrap_or(Value::Null);
            if config["enabled"] != false {
                if let Some(p) = config["path"].as_str() {
                    add(p.into(), "ot3", &mut candidates);
                }
            }
        } else {
            instructions[i - 1] = Some(contents.clone());
            for p in instruction_paths(&contents) {
                add(p, "instructions", &mut candidates);
            }
        }
    }
    add(DEFAULT_PATH.into(), "default", &mut candidates);
    let probes: Vec<String> = candidates
        .iter()
        .flat_map(|(p, _)| [format!("{p}/AGENTS.md"), format!("{p}/.git/config")])
        .collect();
    let kinds = match files.metadata(&probes) {
        Ok(k) => k,
        Err(e) => return json!({"state":host_error(&e),"error":e,"vaults":[]}),
    };
    let mut vaults = vec![];
    for (i, (path, from)) in candidates.iter().enumerate() {
        let is_file = |k: usize| kinds.get(k).cloned().flatten().as_deref() == Some("file");
        if !is_file(i * 2) {
            continue;
        }
        let git = is_file(i * 2 + 1);
        // Why a remote is missing, so the page can tell "no remote" from "unreadable".
        let (remote, remote_error) = if !git {
            (None, Some("no-git".to_string()))
        } else {
            match files.read(&probes[i * 2 + 1]) {
                Ok(c) => match origin_url(&c) {
                    Some(u) => (Some(u), None),
                    None => (None, Some(if c.is_empty() { "empty-read".into() } else { "no-origin".into() })),
                },
                Err(e) => (None, Some(e)),
            }
        };
        // Whether this host's agents get the current protocol for this vault.
        let state = |c: &Option<String>| c.as_deref().map(|c| block_state(c, path)).unwrap_or("missing");
        vaults.push(json!({
            "instructions":{"claude":state(&instructions[0]),"codex":state(&instructions[1])},
            "path":path,"from":from,"git":git,"remoteError":remote_error,
            "remote":remote.as_deref().map(redact_url),
            "canonical":remote.as_deref().and_then(canonical_remote),
            "kind":remote.as_deref().map(kind_of),
        }));
    }
    json!({"state":"online","vaults":vaults})
}
/// Vaults of every host, grouped by remote: clones of one remote are one vault.
/// A path remote is only the same repository on the same host.
pub fn group(hosts: &[Value]) -> Vec<Value> {
    let mut groups: BTreeMap<String, Value> = BTreeMap::new();
    for host in hosts {
        for vault in host["vaults"].as_array().into_iter().flatten() {
            let key = match vault["canonical"].as_str() {
                Some(c) if c.starts_with("path:") => format!("{}@{c}", text(host, "environment")),
                Some(c) => c.to_string(),
                None => format!("{}@local:{}", text(host, "environment"), text(vault, "path")),
            };
            let entry = groups.entry(key.clone()).or_insert_with(|| {
                json!({"id":key,"remote":vault["remote"],"kind":vault["kind"],"shareable":vault["remote"].is_string(),"hosts":[]})
            });
            entry["hosts"].as_array_mut().unwrap().push(json!({
                "environment":host["environment"],"name":host["name"],"path":vault["path"],"remote":vault["remote"],
            }));
        }
    }
    groups.into_values().collect()
}

/// How this computer reaches a host for git: its T3 SSH profile, and the OmaBridge
/// peer carrying that environment when there is one.
#[derive(Clone, Debug, Default)]
pub struct HostTransport {
    pub ssh_args: Option<Vec<String>>,
    pub omabridge: Option<(PathBuf, PathBuf, String)>,
}
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}
/// The remote as this computer must use it. Network URLs stay as they are; a path
/// on the host is a bare repository there, reached over SSH (direct or OmaBridge).
pub fn clone_source(remote: &str, transport: &HostTransport) -> Result<Value> {
    if has_credentials(remote) {
        return Err("vault_url_has_credentials".into());
    }
    if !valid_remote_url(remote) {
        return Err("vault_invalid_remote".into());
    }
    if !is_path(remote) {
        return Ok(json!({"url":remote,"kind":kind_of(remote),"transport":"direct","sshCommand":null}));
    }
    let args = transport.ssh_args.as_ref().ok_or("vault_remote_needs_ssh")?;
    let split = args.iter().position(|a| a == "--").ok_or("vault_remote_needs_ssh")?;
    let destination = args.get(split + 1).ok_or("vault_remote_needs_ssh")?;
    let option = |flag: &str| args[..split].windows(2).find(|w| w[0] == flag).map(|w| w[1].clone());
    let user = option("-l").map(|u| format!("{u}@")).unwrap_or_default();
    let port = option("-p").map(|p| format!(":{p}")).unwrap_or_default();
    let path = remote.trim().trim_start_matches("file://");
    // ssh://host/~/path is relative to the remote user's home.
    let path = path.strip_prefix('/').map(|p| format!("/{p}")).unwrap_or_else(|| format!("/{path}"));
    let url = format!("ssh://{user}{destination}{port}{path}");
    match &transport.omabridge {
        Some((bin, state_dir, peer)) => {
            let proxy = format!(
                "{} --state-dir {} --name ot3 connect --peer {} --service ssh",
                shell_quote(&bin.display().to_string()),
                shell_quote(&state_dir.display().to_string()),
                peer
            );
            let command = format!("ssh -o BatchMode=yes -o ConnectTimeout=20 -o {}", shell_quote(&format!("ProxyCommand={proxy}")));
            Ok(json!({"url":url,"kind":"local-bare","transport":"omabridge","sshCommand":command}))
        }
        None => Ok(json!({"url":url,"kind":"local-bare","transport":"ssh","sshCommand":null})),
    }
}

// ---------------------------------------------------------------- git on this computer

pub struct GitOutput {
    pub ok: bool,
    pub stdout: String,
    pub stderr: String,
}
fn repo_ssh(dir: &Path) -> bool {
    run_git(Some(dir), &["config", "--get", "core.sshCommand"], Some(DEFAULT_SSH), 10).ok
}
/// git on this computer, never prompting: a missing credential fails at once.
pub fn run_git(dir: Option<&Path>, args: &[&str], ssh: Option<&str>, seconds: u32) -> GitOutput {
    let mut c = Command::new("timeout");
    c.arg(format!("{seconds}s")).arg("git");
    if let Some(d) = dir {
        c.arg("-C").arg(d);
    }
    c.args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "never")
        .env("LC_ALL", "C")
        .env_remove("GIT_ASKPASS")
        .env_remove("SSH_ASKPASS")
        .stdin(Stdio::null());
    // A clone's own core.sshCommand (OmaBridge) must win over the default.
    match ssh {
        Some(s) => {
            c.env("GIT_SSH_COMMAND", s);
        }
        None if dir.is_some_and(|d| d.join(".git").exists() && repo_ssh(d)) => {
            c.env_remove("GIT_SSH_COMMAND");
        }
        None => {
            c.env("GIT_SSH_COMMAND", DEFAULT_SSH);
        }
    }
    match c.output() {
        Ok(o) => GitOutput {
            ok: o.status.success(),
            stdout: String::from_utf8_lossy(&o.stdout).trim().into(),
            stderr: if o.status.code() == Some(124) { "timed out".into() } else { String::from_utf8_lossy(&o.stderr).trim().into() },
        },
        Err(_) => GitOutput { ok: false, stdout: String::new(), stderr: "git unavailable".into() },
    }
}
/// A stable error code for a failed remote operation.
pub fn classify(stderr: &str) -> &'static str {
    let s = stderr.to_lowercase();
    if s.contains("git unavailable") {
        "vault_git_missing"
    } else if s.contains("please tell me who you are") || s.contains("empty ident") {
        "vault_git_identity_missing"
    } else if [
        "authentication failed",
        "could not read username",
        "could not read password",
        "terminal prompts disabled",
        "permission denied (publickey",
        "permission denied, please try again",
        "host key verification failed",
        "repository not found",
        "error: 403",
        "returned error: 401",
        "returned error: 403",
        "invalid username or password",
    ]
    .iter()
    .any(|p| s.contains(p))
    {
        "vault_auth_missing"
    } else if [
        "could not resolve host",
        "could not resolve hostname",
        "connection refused",
        "timed out",
        "network is unreachable",
        "no route to host",
        "connection closed",
        "connection reset",
        "failed to connect",
        "unable to access",
    ]
    .iter()
    .any(|p| s.contains(p))
    {
        "vault_remote_unreachable"
    } else if s.contains("does not appear to be a git repository") || s.contains("not found") || s.contains("no such file") {
        "vault_remote_missing"
    } else if s.contains("conflict") {
        "vault_conflict"
    } else {
        "vault_git_failed"
    }
}
/// How to connect this computer's own authentication for a remote. Nothing is
/// copied from a host: the user signs in here.
pub fn auth_help(url: &str) -> Value {
    if canonical_remote(url).is_some_and(|c| c.starts_with("github.com/")) {
        let gh = which("gh").is_some();
        let https = !url.contains("git@") && !url.starts_with("ssh://");
        let steps: Vec<&str> = if https {
            vec!["gh auth login", "gh auth setup-git"]
        } else {
            vec!["gh auth login", "gh ssh-key add ~/.ssh/id_ed25519.pub"]
        };
        json!({"provider":"github","ghInstalled":gh,"steps":steps})
    } else {
        let host = canonical_remote(url).and_then(|c| c.split('/').next().map(str::to_owned)).unwrap_or_default();
        json!({"provider":"ssh","steps":["ssh-keygen -t ed25519", format!("ssh-copy-id {host}")]})
    }
}
pub fn which(name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
        .map(|d| d.join(name))
        .find(|p| fs::metadata(p).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0))
}
/// `git ls-remote` with this computer's credentials.
pub fn check_access(url: &str, ssh: Option<&str>) -> Result<()> {
    let out = run_git(None, &["ls-remote", "--exit-code", "--heads", "--", url], ssh, 30);
    // An empty repository has no heads (exit 2) but is reachable.
    if out.ok || out.stderr.is_empty() && !out.stdout.contains("fatal") {
        return Ok(());
    }
    Err(classify(&out.stderr).into())
}

/// What is at the chosen folder on this computer.
pub fn inspect(path: &Path, expected: Option<&str>) -> Value {
    let Ok(meta) = fs::metadata(path) else {
        return json!({"state":"missing"});
    };
    if !meta.is_dir() {
        return json!({"state":"occupied","reason":"not-directory"});
    }
    let empty = fs::read_dir(path).map(|mut d| d.next().is_none()).unwrap_or(false);
    if empty {
        return json!({"state":"empty"});
    }
    if !path.join(".git").exists() {
        return json!({"state":"occupied","reason":"not-git"});
    }
    let remote = run_git(Some(path), &["remote", "get-url", "origin"], None, 10);
    let remote = remote.ok.then(|| remote.stdout).filter(|r| !r.is_empty());
    let canonical = remote.as_deref().and_then(canonical_remote);
    let shown = remote.as_deref().map(redact_url);
    match (canonical, expected.and_then(canonical_remote)) {
        (Some(a), Some(b)) if a == b => json!({"state":"clone","remote":shown,"canonical":a}),
        (Some(_), Some(_)) => json!({"state":"occupied","reason":"other-remote","remote":shown}),
        (None, _) => json!({"state":"occupied","reason":"no-remote"}),
        (Some(a), None) => json!({"state":"clone","remote":shown,"canonical":a}),
    }
}
/// Clone into a missing or empty folder, or reuse a clone of the same remote.
/// An occupied folder is never touched.
pub fn clone(url: &str, path: &Path, ssh: Option<&str>) -> Result<Value> {
    let found = inspect(path, Some(url));
    match text(&found, "state") {
        "clone" => return Ok(json!({"reused":true})),
        "missing" | "empty" => {}
        _ => return Err(format!("vault_path_occupied: {}", text(&found, "reason"))),
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|_| "vault_path_unwritable")?;
    }
    let target = path.to_string_lossy().to_string();
    let out = run_git(None, &["clone", "--", url, &target], ssh, 300);
    if !out.ok {
        // A partial clone into a folder that did not exist is ours to remove.
        if text(&found, "state") == "missing" {
            let _ = fs::remove_dir_all(path);
        }
        return Err(classify(&out.stderr).into());
    }
    if let Some(command) = ssh {
        run_git(Some(path), &["config", "core.sshCommand", command], None, 10);
    }
    Ok(json!({"reused":false}))
}
fn rebasing(path: &Path) -> bool {
    path.join(".git/rebase-merge").exists() || path.join(".git/rebase-apply").exists()
}
/// `pull --rebase`; a stopped rebase is aborted so local commits stay intact.
pub fn pull(path: &Path) -> Result<Value> {
    if rebasing(path) {
        return Err("vault_conflict".into());
    }
    let out = run_git(Some(path), &["pull", "--rebase", "--autostash", "--quiet"], None, 120);
    if out.ok {
        return Ok(json!({"pulled":true}));
    }
    if rebasing(path) {
        if resolve_managed(path) {
            return Ok(json!({"pulled":true,"merged":true}));
        }
        run_git(Some(path), &["rebase", "--abort"], None, 30);
        return Err("vault_conflict".into());
    }
    Err(classify(&out.stderr).into())
}
/// A rebase stopped only on ot3-managed rows (Threads.md, the device table) is
/// finished by merging rows from both sides; anything else is left to the person.
fn resolve_managed(path: &Path) -> bool {
    for _ in 0..20 {
        if !rebasing(path) {
            return true;
        }
        let conflicted = run_git(Some(path), &["diff", "--name-only", "--diff-filter=U"], None, 30).stdout;
        let files: Vec<&str> = conflicted.lines().filter(|l| !l.is_empty()).collect();
        if files.is_empty() || !files.iter().all(|f| managed(f)) {
            return false;
        }
        for file in &files {
            // In a rebase, stage 2 is the remote side and stage 3 this computer's commit.
            let theirs = run_git(Some(path), &["show", &format!(":2:{file}")], None, 30);
            let ours = run_git(Some(path), &["show", &format!(":3:{file}")], None, 30);
            if !theirs.ok || !ours.ok {
                return false;
            }
            let Some(merged) = merge_managed(file, &format!("{}\n", theirs.stdout), &format!("{}\n", ours.stdout)) else { return false };
            let target = path.join(file);
            if confined(path, &target).is_err() || replace(&target, &merged).is_err() || !run_git(Some(path), &["add", "--", file], None, 30).ok {
                return false;
            }
        }
        let next = run_git(Some(path), &["-c", "core.editor=true", "rebase", "--continue"], None, 60);
        if !next.ok && !rebasing(path) {
            return false;
        }
    }
    false
}
fn managed(file: &str) -> bool {
    file == "Machines.md" || file.strip_prefix("Projects/").and_then(|r| r.strip_suffix("/Threads.md")).is_some_and(valid_slug)
}
/// Both sides' rows of the managed table in `file`, on the remote side's text.
pub fn merge_managed(file: &str, theirs: &str, ours: &str) -> Option<String> {
    if file == "Machines.md" {
        let mut merged = theirs.to_string();
        for row in marked_rows(ours, DEVICES_START, DEVICES_END)? {
            let id = row.split('`').nth(1)?.to_string();
            merged = upsert_row(&merged, DEVICES_START, DEVICES_END, DEVICES_HEADER, &row, &format!("| `{id}` |"))?;
        }
        return Some(merged);
    }
    let rows: Vec<ThreadRow> = marked_rows(ours, THREADS_START, THREADS_END)?.iter().filter_map(|r| ThreadRow::parse(r)).collect();
    Some(upsert_threads(theirs, file.split('/').nth(1)?, &rows))
}
fn marked_rows(doc: &str, start: &str, end: &str) -> Option<Vec<String>> {
    let (s, e) = (doc.find(start)?, doc.find(end)?);
    (e > s).then(|| doc[s + start.len()..e].lines().filter(|l| l.starts_with("| ") && !l.starts_with("| Dispositivo") && !l.starts_with("| Actualizado") && !l.starts_with("|---")).map(str::to_owned).collect())
}
fn upsert_row(doc: &str, start: &str, end: &str, header: &str, row: &str, key: &str) -> Option<String> {
    let mut rows = marked_rows(doc, start, end)?;
    match rows.iter().position(|r| r.contains(key)) {
        Some(i) => rows[i] = row.to_string(),
        None => rows.push(row.to_string()),
    }
    let (s, e) = (doc.find(start)?, doc.find(end)?);
    Some(format!("{}{start}\n{header}\n{}\n{end}{}", &doc[..s], rows.join("\n"), &doc[e + end.len()..]))
}

// ---------------------------------------------------------------- thread registry

pub const THREADS_START: &str = "<!-- ot3-threads:start -->";
pub const THREADS_END: &str = "<!-- ot3-threads:end -->";
const THREADS_HEADER: &str = "| Actualizado | Máquina | Hilo | Estado |\n|---|---|---|---|";
const DEVICES_HEADER: &str = "| Dispositivo (ot3) | Id | Environment local | Sistema | Vault | Actualizado |\n|---|---|---|---|---|---|";
/// Only the most recent threads of a project are kept: agents search the file, never
/// load history whole. Older rows stay in the file's git history.
pub const MAX_THREADS: usize = 25;
#[derive(Clone, Debug, PartialEq)]
pub struct ThreadRow {
    pub updated: String,
    pub machine: String,
    pub id: String,
    pub title: String,
    pub status: String,
}
fn cell(s: &str, max: usize) -> String {
    let clean: String = s.chars().map(|c| if c.is_control() || "|[]`".contains(c) { ' ' } else { c }).collect();
    let clean = clean.split_whitespace().collect::<Vec<_>>().join(" ");
    if clean.chars().count() > max { format!("{}…", clean.chars().take(max).collect::<String>()) } else { clean }
}
impl ThreadRow {
    /// A settled, titled, top-level thread of a real project; subagents, scratch
    /// projects and untitled threads are not history worth keeping.
    pub fn from_thread(t: &Value, machine: &str) -> Option<ThreadRow> {
        let title = text(t, "title").trim();
        let id = text(t, "id");
        if t["working"] == true || !t["parentThreadId"].is_null() || title.is_empty() || title.eq_ignore_ascii_case("New thread") || id.is_empty() || id.contains(|c: char| c.is_whitespace() || c == ')') {
            return None;
        }
        let at = t["updatedAt"].as_i64().filter(|v| *v > 0)?;
        let updated = chrono::DateTime::from_timestamp_millis(at)?.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M").to_string();
        let status = match text(t, "status") { "" => "settled", s => s };
        Some(ThreadRow { updated, machine: cell(machine, 40), id: id.into(), title: cell(title, 90), status: cell(status, 20) })
    }
    pub fn line(&self) -> String {
        format!("| {} | {} | [{}](t3-thread://v1/{}) | {} |", self.updated, self.machine, self.title, self.id, self.status)
    }
    pub fn parse(line: &str) -> Option<ThreadRow> {
        let cells: Vec<&str> = line.trim().trim_matches('|').split(" | ").map(str::trim).collect();
        let [updated, machine, link, status] = cells.as_slice() else { return None };
        let (title, rest) = link.strip_prefix('[')?.split_once("](t3-thread://v1/")?;
        Some(ThreadRow { updated: updated.to_string(), machine: machine.to_string(), id: rest.strip_suffix(')')?.to_string(), title: title.into(), status: status.to_string() })
    }
}
/// Merge `rows` into a project's Threads.md (created when missing): one row per
/// thread, newest first, at most MAX_THREADS.
pub fn upsert_threads(doc: &str, slug: &str, rows: &[ThreadRow]) -> String {
    let doc = if doc.contains(THREADS_START) && doc.contains(THREADS_END) {
        doc.to_string()
    } else if doc.trim().is_empty() {
        format!("---\nproject: {slug}\nmanaged-by: omarchy-t3-code\n---\n# Hilos — {slug}\n\nLos {MAX_THREADS} hilos más recientes de este proyecto en todas las máquinas, mantenidos por Omarchy T3 Code. \
Cada enlace `t3-thread://` se abre en la máquina indicada. Hilos anteriores: `git log -p -- Projects/{slug}/Threads.md`.\n\n{THREADS_START}\n{THREADS_HEADER}\n{THREADS_END}\n")
    } else {
        format!("{}\n\n{THREADS_START}\n{THREADS_HEADER}\n{THREADS_END}\n", doc.trim_end())
    };
    let mut all: Vec<ThreadRow> = marked_rows(&doc, THREADS_START, THREADS_END).unwrap_or_default().iter().filter_map(|r| ThreadRow::parse(r)).collect();
    for row in rows {
        match all.iter().position(|r| r.id == row.id) {
            Some(i) if all[i].updated > row.updated => {}
            Some(i) => all[i] = row.clone(),
            None => all.push(row.clone()),
        }
    }
    all.sort_by(|a, b| b.updated.cmp(&a.updated).then(a.id.cmp(&b.id)));
    all.truncate(MAX_THREADS);
    let (s, e) = (doc.find(THREADS_START).unwrap(), doc.find(THREADS_END).unwrap());
    let body = all.iter().map(ThreadRow::line).collect::<Vec<_>>();
    let rows = if body.is_empty() { String::new() } else { format!("{}\n", body.join("\n")) };
    format!("{}{THREADS_START}\n{THREADS_HEADER}\n{rows}{THREADS_END}{}", &doc[..s], &doc[e + THREADS_END.len()..])
}
/// Write rows to `Projects/<slug>/Threads.md`; true when the file changed.
pub fn record_threads(vault: &Path, slug: &str, rows: &[ThreadRow]) -> Result<bool> {
    if !valid_slug(slug) {
        return Err("vault_invalid_slug".into());
    }
    let dir = vault.join("Projects").join(slug);
    confined(vault, &dir)?;
    fs::create_dir_all(&dir).map_err(|_| "vault_write_failed")?;
    confined(vault, &dir)?;
    let file = dir.join("Threads.md");
    let current = fs::read_to_string(&file).unwrap_or_default();
    let next = upsert_threads(&current, slug, rows);
    if next == current {
        return Ok(false);
    }
    if current.is_empty() {
        write_new(&file, &next)?;
    } else {
        replace(&file, &next)?;
    }
    Ok(true)
}

/// Commit whatever changed, rebase on the remote and push.
pub fn commit_push(path: &Path, message: &str) -> Result<Value> {
    if rebasing(path) {
        return Err("vault_conflict".into());
    }
    let mut committed = false;
    if !run_git(Some(path), &["status", "--porcelain"], None, 30).stdout.is_empty() {
        let add = run_git(Some(path), &["add", "-A"], None, 60);
        if !add.ok {
            return Err(classify(&add.stderr).into());
        }
        let out = run_git(Some(path), &["commit", "-q", "-m", message], None, 60);
        if !out.ok {
            return Err(classify(&format!("{}\n{}", out.stderr, out.stdout)).into());
        }
        committed = true;
    }
    let ahead = status(path)["ahead"].as_u64().unwrap_or(0);
    if ahead == 0 {
        return Ok(json!({"committed":committed,"pushed":false}));
    }
    pull(path)?;
    let out = run_git(Some(path), &["push", "--quiet"], None, 120);
    if !out.ok {
        return Err(classify(&out.stderr).into());
    }
    Ok(json!({"committed":committed,"pushed":true}))
}
/// Local sync state, without contacting the remote.
pub fn status(path: &Path) -> Value {
    if !path.join(".git").exists() {
        return json!({"git":false});
    }
    let branch = run_git(Some(path), &["rev-parse", "--abbrev-ref", "HEAD"], None, 10).stdout;
    let counts = run_git(Some(path), &["rev-list", "--left-right", "--count", "HEAD...@{u}"], None, 10);
    let (ahead, behind, upstream) = match counts.stdout.split_whitespace().collect::<Vec<_>>().as_slice() {
        [a, b] if counts.ok => (a.parse::<u64>().unwrap_or(0), b.parse::<u64>().unwrap_or(0), true),
        _ => (run_git(Some(path), &["rev-list", "--count", "HEAD"], None, 10).stdout.parse().unwrap_or(0), 0, false),
    };
    let changes = run_git(Some(path), &["status", "--porcelain"], None, 30).stdout.lines().count();
    let remote = run_git(Some(path), &["remote", "get-url", "origin"], None, 10);
    json!({
        "git":true,"branch":branch,"upstream":upstream,"ahead":ahead,"behind":behind,"changes":changes,
        "conflict":rebasing(path),"remote":remote.ok.then(|| redact_url(&remote.stdout)),
    })
}

// ---------------------------------------------------------------- files in the vault

/// `path` must stay inside `vault`, following any symlink already on disk.
fn confined(vault: &Path, path: &Path) -> Result<()> {
    if path.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err("vault_path_escape".into());
    }
    let root = fs::canonicalize(vault).map_err(|_| "vault_path_missing")?;
    let mut probe = path.to_path_buf();
    while !probe.exists() {
        if !probe.pop() {
            return Err("vault_path_escape".into());
        }
    }
    let real = fs::canonicalize(&probe).map_err(|_| "vault_path_escape")?;
    if real.starts_with(&root) {
        Ok(())
    } else {
        Err("vault_path_escape".into())
    }
}
pub fn valid_slug(slug: &str) -> bool {
    !slug.is_empty()
        && slug.len() <= 80
        && slug != "."
        && slug != ".."
        && !slug.starts_with('.')
        && slug.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}
/// Slug of a T3 project: its mapped name, else its folder name, else its title.
pub fn project_slug(config: &Value, title: &str, workspace_root: &str) -> Option<String> {
    if let Some(s) = config["projects"][title].as_str() {
        return valid_slug(s).then(|| s.into());
    }
    let folder = workspace_root.trim_end_matches('/').rsplit('/').next().unwrap_or("");
    for candidate in [folder, title] {
        let s: String = candidate
            .trim()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || "._-".contains(c) { c } else { '-' })
            .collect();
        let s = s.trim_matches('-').trim_start_matches('.').to_string();
        if valid_slug(&s) {
            return Some(s);
        }
    }
    None
}
fn write_new(path: &Path, contents: &str) -> Result<bool> {
    match fs::OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(mut f) => f.write_all(contents.as_bytes()).map(|_| true).map_err(|_| "vault_write_failed".into()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
        Err(_) => Err("vault_write_failed".into()),
    }
}
/// Replace a file's contents through a temporary file in the same folder.
fn replace(path: &Path, contents: &str) -> Result<()> {
    let parent = path.parent().ok_or("vault_write_failed")?;
    let tmp = parent.join(format!(".ot3-{}", uuid::Uuid::new_v4()));
    let mode = fs::metadata(path).map(|m| m.permissions().mode()).unwrap_or(0o644);
    let result = fs::write(&tmp, contents)
        .and_then(|_| fs::set_permissions(&tmp, fs::Permissions::from_mode(mode)))
        .and_then(|_| fs::rename(&tmp, path));
    let _ = fs::remove_file(&tmp);
    result.map_err(|_| "vault_write_failed".into())
}

pub const PROJECT_TEMPLATE: &str = include_str!("../integrations/dev-vault/Templates/Project-MEMORY.md");
/// `Projects/<slug>/MEMORY.md` from the vault's template, and its line in the
/// `## Proyectos` index. Existing notes are never rewritten.
pub fn ensure_project(vault: &Path, slug: &str, title: &str, today: &str) -> Result<bool> {
    if !valid_slug(slug) {
        return Err("vault_invalid_slug".into());
    }
    let dir = vault.join("Projects").join(slug);
    confined(vault, &dir)?;
    fs::create_dir_all(dir.join("Sessions")).map_err(|_| "vault_write_failed")?;
    confined(vault, &dir)?;
    let template = fs::read_to_string(vault.join("Templates/Project-MEMORY.md")).unwrap_or_else(|_| PROJECT_TEMPLATE.into());
    let note = template.replace("<slug>", slug).replace("YYYY-MM-DD", today);
    let created = write_new(&dir.join("MEMORY.md"), &note)?;
    write_new(&dir.join("Sessions/.gitkeep"), "")?;
    let index = vault.join("MEMORY.md");
    if let Ok(current) = fs::read_to_string(&index) {
        confined(vault, &index)?;
        if !current.contains(&format!("[[Projects/{slug}/MEMORY")) {
            let line = format!("- [[Projects/{slug}/MEMORY|{slug}]] — {}\n", title.replace(['\n', '\r'], " "));
            let next = insert_in_section(&current, "## Proyectos", &line);
            replace(&index, &next)?;
        }
    }
    Ok(created)
}
/// Append `line` at the end of the `heading` section, adding the section if absent.
fn insert_in_section(doc: &str, heading: &str, line: &str) -> String {
    let Some(start) = doc.lines().scan(0, |at, l| {
        let here = *at;
        *at += l.len() + 1;
        Some((here, l))
    })
    .find(|(_, l)| l.trim() == heading)
    .map(|(at, _)| at) else {
        let sep = if doc.ends_with("\n\n") { "" } else if doc.ends_with('\n') { "\n" } else { "\n\n" };
        return format!("{doc}{sep}{heading}\n{line}");
    };
    let body = start + heading.len();
    let end = doc[body..].find("\n## ").map(|i| body + i + 1).unwrap_or(doc.len());
    let (before, after) = doc.split_at(end);
    let before = before.trim_end_matches('\n');
    let gap = if after.is_empty() { "" } else { "\n" };
    format!("{before}\n{line}{gap}{after}")
}

/// This device's row in `Machines.md`, inside an ot3-managed table. Other rows and
/// the rest of the note are kept.
pub fn register_device(vault: &Path, device: &Value) -> Result<bool> {
    let file = vault.join("Machines.md");
    confined(vault, &file)?;
    let current = fs::read_to_string(&file).unwrap_or_else(|_| "# Machines\n".into());
    let cell = |k: &str| text(device, k).replace(['|', '\n', '\r', '`'], " ").trim().to_string();
    let id = cell("id");
    if id.is_empty() {
        return Err("vault_invalid_device".into());
    }
    let row = format!(
        "| {} | `{id}` | {} | {} | `{}` | {} |",
        cell("name"),
        if cell("environment").is_empty() { "—".into() } else { format!("`{}`", cell("environment")) },
        cell("system"),
        cell("path"),
        cell("updated"),
    );
    let header = DEVICES_HEADER;
    let (mut rows, before, after): (Vec<String>, String, String) = match (current.find(DEVICES_START), current.find(DEVICES_END)) {
        (Some(s), Some(e)) if e > s => {
            let inner = &current[s + DEVICES_START.len()..e];
            let rows = inner.lines().filter(|l| l.starts_with("| ") && !l.starts_with("| Dispositivo") && !l.starts_with("|---")).map(str::to_owned).collect();
            (rows, current[..s].to_string(), current[e + DEVICES_END.len()..].to_string())
        }
        (None, None) => {
            let sep = if current.ends_with('\n') { "\n" } else { "\n\n" };
            (vec![], format!("{current}{sep}## Dispositivos ot3\n\n"), "\n".into())
        }
        _ => return Err("vault_machines_malformed".into()),
    };
    let marker = format!("| `{id}` |");
    match rows.iter().position(|r| r.contains(&marker)) {
        Some(i) if rows[i] == row => return Ok(false),
        Some(i) => rows[i] = row,
        None => rows.push(row),
    }
    let next = format!("{before}{DEVICES_START}\n{header}\n{}\n{DEVICES_END}{after}", rows.join("\n"));
    // Same device, newer date only: keep the file when nothing else changed.
    replace(&file, &next)?;
    Ok(true)
}

/// Embedded templates for a new vault (structure of `~/Obsidian/Dev`).
pub const TEMPLATES: &[(&str, &str)] = &[
    ("AGENTS.md", include_str!("../integrations/dev-vault/AGENTS.md")),
    ("MEMORY.md", include_str!("../integrations/dev-vault/MEMORY.md")),
    ("Machines.md", include_str!("../integrations/dev-vault/Machines.md")),
    ("Memory/User.md", include_str!("../integrations/dev-vault/Memory/User.md")),
    ("Memory/Rules.md", include_str!("../integrations/dev-vault/Memory/Rules.md")),
    ("Templates/Project-MEMORY.md", PROJECT_TEMPLATE),
    ("Templates/Session.md", include_str!("../integrations/dev-vault/Templates/Session.md")),
    (".gitignore", include_str!("../integrations/dev-vault/gitignore")),
];
/// A new vault in a missing or empty folder: templates, first commit, and the
/// remote when given (`github` creates a private repository with `gh`).
pub fn create(path: &Path, remote: Option<&str>, github: Option<&str>) -> Result<Value> {
    match text(&inspect(path, None), "state") {
        "missing" | "empty" => {}
        _ => return Err("vault_path_occupied".into()),
    }
    if remote.is_some_and(has_credentials) {
        return Err("vault_url_has_credentials".into());
    }
    if remote.is_some_and(|u| !valid_remote_url(u)) {
        return Err("vault_invalid_remote".into());
    }
    if github.is_some_and(|n| n.is_empty() || n.len() > 100 || !n.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-/".contains(&b)) || n.starts_with(['-', '/'])) {
        return Err("vault_invalid_repository".into());
    }
    fs::create_dir_all(path).map_err(|_| "vault_path_unwritable")?;
    for (name, contents) in TEMPLATES {
        let file = path.join(name);
        fs::create_dir_all(file.parent().unwrap()).map_err(|_| "vault_write_failed")?;
        write_new(&file, contents)?;
    }
    fs::create_dir_all(path.join("Projects")).map_err(|_| "vault_write_failed")?;
    write_new(&path.join("Projects/.gitkeep"), "")?;
    for args in [&["init", "-q", "-b", "main"][..], &["add", "-A"], &["commit", "-q", "-m", "journal: create the development vault"]] {
        let out = run_git(Some(path), args, None, 60);
        if !out.ok {
            return Err(classify(&format!("{}\n{}", out.stderr, out.stdout)).into());
        }
    }
    if let Some(name) = github {
        let out = Command::new("timeout")
            .args(["120s", "gh", "repo", "create", name, "--private", "--source"])
            .arg(path)
            .args(["--remote", "origin", "--push"])
            .env("GH_PROMPT_DISABLED", "1")
            .stdin(Stdio::null())
            .output()
            .map_err(|_| "vault_gh_missing")?;
        if !out.status.success() {
            return Err(classify(&String::from_utf8_lossy(&out.stderr)).replace("vault_git_failed", "vault_gh_failed"));
        }
        return Ok(json!({"pushed":true}));
    }
    if let Some(url) = remote {
        run_git(Some(path), &["remote", "add", "origin", url], None, 10);
        let out = run_git(Some(path), &["push", "-u", "origin", "main"], None, 120);
        if !out.ok {
            return Err(classify(&out.stderr).into());
        }
        return Ok(json!({"pushed":true}));
    }
    Ok(json!({"pushed":false}))
}

// ---------------------------------------------------------------- agent instructions

pub fn instruction_block(vault: &Path, home: &Path) -> String {
    let shown = vault.strip_prefix(home).map(|r| format!("~/{}", r.display())).unwrap_or_else(|_| vault.display().to_string());
    block_for(&shown)
}
/// The protocol for agents, for a vault shown as `shown` (`~/Obsidian/Dev`). It asks
/// for targeted searches: the vault is never loaded whole into a prompt.
pub fn block_for(shown: &str) -> String {
    format!(
        "{BLOCK_START}\n## Bitácora central de desarrollo (Dev Vault)\n\n\
El vault git `{shown}` es la fuente de verdad de estado, decisiones y avances de cada proyecto, \
compartida entre todas las máquinas. Protocolo completo: `{shown}/AGENTS.md`.\n\n\
- Al empezar: `git -C {shown} pull --rebase --quiet` y leer `Projects/<slug>/MEMORY.md` \
(`<slug>` = carpeta del repo; crearlo desde `Templates/Project-MEMORY.md` si falta).\n\
- Para buscar historial, decisiones o un hilo anterior: `rg -n -i '<término>' {shown} --glob '*.md'` \
y leer solo las notas encontradas. Nunca cargar el vault completo.\n\
- Hilos anteriores del proyecto en todas las máquinas: `Projects/<slug>/Threads.md` \
(lo mantiene Omarchy T3 Code; no editarlo).\n\
- Al terminar o tras un avance significativo: actualizar ese MEMORY (estado, decisiones fechadas, pendientes, \
hilos relevantes con máquina y enlace `t3-thread://`), una nota en `Sessions/` si hubo trabajo sustancial, \
y commit y push.\n\
- Nombres de archivos en inglés; sin secretos ni volcados de datos. Si el vault no existe en esta máquina, avisar \
en lugar de improvisar otra memoria.\n\
(Bloque mantenido por Omarchy T3 Code, Preferencias → Memoria/Obsidian.)\n{BLOCK_END}\n"
    )
}
/// Whether an instruction file holds the current block for `shown`.
pub fn block_state(contents: &str, shown: &str) -> &'static str {
    if contents.contains(&block_for(shown)) {
        "current"
    } else if contents.contains(BLOCK_START) {
        "outdated"
    } else {
        "missing"
    }
}
/// Put `block` in place of the marked block, or after the content. Everything else,
/// including other marked blocks, is kept byte for byte.
pub fn upsert_block(existing: &str, block: &str) -> Result<String> {
    match (existing.find(BLOCK_START), existing.find(BLOCK_END)) {
        (Some(s), Some(e)) if e > s => {
            let mut end = e + BLOCK_END.len();
            if existing[end..].starts_with('\n') {
                end += 1;
            }
            Ok(format!("{}{block}{}", &existing[..s], &existing[end..]))
        }
        (None, None) if existing.is_empty() => Ok(block.into()),
        (None, None) => {
            let sep = if existing.ends_with("\n\n") { "" } else if existing.ends_with('\n') { "\n" } else { "\n\n" };
            Ok(format!("{existing}{sep}{block}"))
        }
        _ => Err("vault_instructions_malformed".into()),
    }
}
/// The block in `~/.claude/CLAUDE.md` and `~/.codex/AGENTS.md` of this computer.
pub fn install_instructions(home: &Path, vault: &Path) -> Result<Vec<Value>> {
    let block = instruction_block(vault, home);
    let mut done = vec![];
    for name in [".claude/CLAUDE.md", ".codex/AGENTS.md"] {
        let mut file = home.join(name);
        if let Ok(meta) = fs::symlink_metadata(&file) {
            if meta.file_type().is_symlink() {
                file = fs::canonicalize(&file).map_err(|_| "vault_instructions_unwritable")?;
                if !fs::metadata(&file).is_ok_and(|m| m.is_file()) {
                    return Err("vault_instructions_unwritable".into());
                }
            } else if !meta.is_file() {
                return Err("vault_instructions_unwritable".into());
            }
        }
        let current = fs::read_to_string(&file).unwrap_or_default();
        let next = upsert_block(&current, &block)?;
        let changed = next != current;
        if changed {
            fs::create_dir_all(file.parent().unwrap()).map_err(|_| "vault_instructions_unwritable")?;
            replace(&file, &next)?;
        }
        done.push(json!({"file":home.join(name).display().to_string(),"changed":changed}));
    }
    Ok(done)
}
/// Renew this computer's block when ot3's protocol text changed. Only a block
/// already there is rewritten; a file without one is left to the setup.
pub fn refresh_instructions(paths: &Paths, config: &Value) -> bool {
    let vault = PathBuf::from(text(config, "path"));
    if config["enabled"] != true || config["agentInstructions"] != true || !vault.join(".git").is_dir() || instructions_installed(&paths.home, &vault) {
        return false;
    }
    let outdated = HOST_FILES.iter().any(|n| fs::read_to_string(paths.home.join(n)).is_ok_and(|c| c.contains(BLOCK_START)));
    outdated && install_instructions(&paths.home, &vault).is_ok()
}
pub fn instructions_installed(home: &Path, vault: &Path) -> bool {
    let block = instruction_block(vault, home);
    [".claude/CLAUDE.md", ".codex/AGENTS.md"].iter().all(|n| fs::read_to_string(home.join(n)).is_ok_and(|c| c.contains(&block)))
}

/// Install the agent protocol on a host, over that environment's SSH profile, only
/// when the person asks. Each file is read, the block replaced as on this computer,
/// and written back only if it did not change in between.
pub const HOST_FILES: [&str; 2] = [".claude/CLAUDE.md", ".codex/AGENTS.md"];
pub fn install_on_host(environment: &str, shown: &str) -> Result<Vec<Value>> {
    if !safe_host_path(shown) {
        return Err("vault_invalid_path".into());
    }
    let args = crate::connections::ssh_args_for(environment).map_err(|_| "vault_remote_needs_ssh")?;
    let ssh = |script: &str, extra: &[&str], input: Option<&[u8]>| -> Result<(i32, Vec<u8>)> {
        let mut remote = format!("sh -c {} sh", shell_quote(script));
        for a in extra {
            remote.push(' ');
            remote.push_str(&shell_quote(a));
        }
        let mut child = Command::new("timeout")
            .args(["30s", "ssh"])
            .args(&args)
            .arg(remote)
            .stdin(if input.is_some() { Stdio::piped() } else { Stdio::null() })
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| "ssh_unavailable")?;
        if let Some(bytes) = input {
            child.stdin.take().ok_or("ssh_unavailable")?.write_all(bytes).map_err(|_| "ssh_unavailable")?;
        }
        let out = child.wait_with_output().map_err(|_| "ssh_unavailable")?;
        Ok((out.status.code().unwrap_or(-1), out.stdout))
    };
    let block = block_for(shown);
    let mut done = vec![];
    for name in HOST_FILES {
        // 4: absent. Symlinks are followed to a regular file, as on this computer.
        let (code, current) = ssh(r#"f="$HOME/$1"; [ -L "$f" ] && f=$(readlink -f "$f"); [ -f "$f" ] || exit 4; cat -- "$f""#, &[name], None)?;
        let current = match code {
            0 => String::from_utf8(current).map_err(|_| "vault_instructions_unwritable")?,
            4 => String::new(),
            124 => return Err("ssh_timeout".into()),
            _ => return Err("ssh_unavailable".into()),
        };
        let next = upsert_block(&current, &block)?;
        if next == current {
            done.push(json!({"file":format!("~/{name}"),"changed":false}));
            continue;
        }
        use sha2::{Digest, Sha256};
        let expected = if code == 4 { "none".to_string() } else { format!("{:x}", Sha256::digest(current.as_bytes())) };
        let write = r#"set -eu; f="$HOME/$1"; [ -L "$f" ] && f=$(readlink -f "$f"); mkdir -p "$(dirname "$f")"
cur=none; [ -f "$f" ] && cur=$(sha256sum "$f" | cut -d' ' -f1); [ "$cur" = "$2" ] || exit 3
tmp="$f.ot3-$$"; cat > "$tmp"; [ -f "$f" ] && chmod --reference="$f" "$tmp" 2>/dev/null || true; mv -f "$tmp" "$f""#;
        match ssh(write, &[name, &expected], Some(next.as_bytes()))?.0 {
            0 => done.push(json!({"file":format!("~/{name}"),"changed":true})),
            3 => return Err("vault_instructions_changed".into()),
            _ => return Err("vault_instructions_unwritable".into()),
        }
    }
    Ok(done)
}

// ---------------------------------------------------------------- search

const SEARCH_MAX_FILES: usize = 5000;
const SEARCH_MAX_BYTES: u64 = 512 * 1024;
/// Notes matching every word of `query` (case-insensitive), best first: a match in
/// the path counts more than one in the text. Each hit carries its first matching
/// line and, when that line links a thread, the thread id.
pub fn search(vault: &Path, query: &str, limit: usize) -> Vec<Value> {
    let words: Vec<String> = query.split_whitespace().map(|w| w.to_lowercase()).filter(|w| !w.is_empty()).take(8).collect();
    if words.is_empty() || !vault.is_dir() {
        return vec![];
    }
    let mut hits: Vec<(i64, Value)> = vec![];
    let mut stack = vec![vault.to_path_buf()];
    let mut seen = 0;
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            let Ok(kind) = entry.file_type() else { continue };
            if kind.is_symlink() || name.starts_with('.') {
                continue;
            }
            if kind.is_dir() {
                stack.push(path);
                continue;
            }
            if !name.ends_with(".md") || entry.metadata().map(|m| m.len() > SEARCH_MAX_BYTES).unwrap_or(true) {
                continue;
            }
            seen += 1;
            if seen > SEARCH_MAX_FILES {
                break;
            }
            let Ok(body) = fs::read_to_string(&path) else { continue };
            let rel = path.strip_prefix(vault).unwrap_or(&path).display().to_string();
            let lower = body.to_lowercase();
            let rel_lower = rel.to_lowercase();
            if !words.iter().all(|w| lower.contains(w) || rel_lower.contains(w)) {
                continue;
            }
            let mut score = words.iter().map(|w| if rel_lower.contains(w) { 50 } else { 0 } + lower.matches(w.as_str()).count().min(20) as i64).sum::<i64>();
            // The line that matches most words, for the snippet.
            let (line_no, line) = body
                .lines()
                .enumerate()
                .max_by_key(|(i, l)| (words.iter().filter(|w| l.to_lowercase().contains(*w)).count(), -(*i as i64)))
                .unwrap_or((0, ""));
            if rel.ends_with("/MEMORY.md") || rel == "MEMORY.md" {
                score += 10;
            }
            let thread = line.split("t3-thread://v1/").nth(1).map(|r| r.split([')', ' ', '`', '|']).next().unwrap_or("").to_string()).filter(|t| !t.is_empty());
            let title = body.lines().find_map(|l| l.strip_prefix("# ")).unwrap_or(&rel).trim().to_string();
            hits.push((score, json!({"file":rel,"title":title,"line":line_no+1,"snippet":cell(line,160),"thread":thread,"path":path.display().to_string()})));
        }
    }
    hits.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| text(&a.1, "file").cmp(text(&b.1, "file"))));
    hits.into_iter().take(limit.clamp(1, 50)).map(|(_, v)| v).collect()
}

// ---------------------------------------------------------------- Obsidian

fn obsidian_config(home: &Path, how: &str) -> PathBuf {
    match how {
        "flatpak" => home.join(".var/app/md.obsidian.Obsidian/config/obsidian/obsidian.json"),
        "macos" => home.join("Library/Application Support/obsidian/obsidian.json"),
        _ => home.join(".config/obsidian/obsidian.json"),
    }
}
/// How Obsidian is installed on this computer, if it is.
pub fn obsidian(home: &Path) -> Option<&'static str> {
    if cfg!(target_os = "macos") && (Path::new("/Applications/Obsidian.app").exists() || home.join("Applications/Obsidian.app").exists()) {
        return Some("macos");
    }
    if which("obsidian").is_some() {
        return Some("native");
    }
    let flatpak = ["/var/lib/flatpak/app/md.obsidian.Obsidian"].iter().map(PathBuf::from).chain([home.join(".local/share/flatpak/app/md.obsidian.Obsidian")]);
    if flatpak.into_iter().any(|p| p.exists()) {
        return Some("flatpak");
    }
    let desktop = ["/usr/share/applications/obsidian.desktop"].iter().map(PathBuf::from).chain([home.join(".local/share/applications/obsidian.desktop")]);
    desktop.into_iter().any(|p| p.exists()).then_some("native")
}
/// Add the vault to Obsidian's vault list when absent; other vaults are kept.
pub fn register_obsidian(home: &Path, how: &str, vault: &Path) -> Result<bool> {
    let file = obsidian_config(home, how);
    let mut doc: Value = fs::read(&file).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_else(|| json!({}));
    if !doc.is_object() {
        return Err("vault_obsidian_config".into());
    }
    if !doc["vaults"].is_object() {
        doc["vaults"] = json!({});
    }
    let path = vault.display().to_string();
    if doc["vaults"].as_object().unwrap().values().any(|v| v["path"] == path.as_str()) {
        return Ok(false);
    }
    let id = format!("{:016x}", uuid::Uuid::new_v4().as_u128() as u64);
    doc["vaults"][id] = json!({"path":path,"ts":chrono::Utc::now().timestamp_millis()});
    fs::create_dir_all(file.parent().unwrap()).map_err(|_| "vault_obsidian_config")?;
    replace(&file, &doc.to_string())?;
    Ok(true)
}
pub fn obsidian_uri(vault: &Path) -> String {
    let path: String = vault
        .display()
        .to_string()
        .bytes()
        .map(|b| if b.is_ascii_alphanumeric() || b"-_.~/".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect();
    format!("obsidian://open?path={path}")
}
pub fn open_obsidian(home: &Path, vault: &Path) -> Result<()> {
    let how = obsidian(home).ok_or("vault_obsidian_missing")?;
    register_obsidian(home, how, vault)?;
    let opener = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
    Command::new(opener)
        .arg(obsidian_uri(vault))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|_| "vault_obsidian_open_failed".into())
}
pub fn install_help() -> Value {
    if which("omarchy-pkg-add").is_some() {
        json!({"command":"omarchy-pkg-add obsidian","url":"https://obsidian.md/download"})
    } else if cfg!(target_os = "macos") {
        json!({"command":"brew install --cask obsidian","url":"https://obsidian.md/download"})
    } else {
        json!({"command":null,"url":"https://obsidian.md/download"})
    }
}

// ---------------------------------------------------------------- configuration

pub fn default_config(paths: &Paths) -> Value {
    json!({
        "version":1,"enabled":false,"path":paths.expand(DEFAULT_PATH).display().to_string(),"remote":null,
        "sync":{"pullOnOpen":true,"pullOnThreadStart":true,"pushOnSettle":true,"recordThreads":true,"intervalMinutes":null},
        "agentInstructions":true,"projects":{},"device":{"name":device_name()},
    })
}
pub fn device_name() -> String {
    fs::read_to_string("/etc/hostname")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| Command::new("uname").arg("-n").output().ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()))
        .unwrap_or_else(|| "device".into())
}
pub fn load_config(paths: &Paths) -> Value {
    let mut config = default_config(paths);
    if let Ok(saved) = fs::read(&paths.config).map_err(|_| ()).and_then(|b| serde_json::from_slice::<Value>(&b).map_err(|_| ())) {
        if let Some(o) = saved.as_object() {
            for (k, v) in o {
                if k == "sync" && v.is_object() {
                    for (sk, sv) in v.as_object().unwrap() {
                        config["sync"][sk] = sv.clone();
                    }
                } else {
                    config[k] = v.clone();
                }
            }
        }
    }
    config
}
fn valid_local_path(paths: &Paths, p: &str) -> bool {
    let path = Path::new(p);
    path.is_absolute()
        && !p.chars().any(char::is_control)
        && !path.components().any(|c| matches!(c, Component::ParentDir))
        && path != paths.home
        && path != Path::new("/")
}
/// Merge a validated patch into `vault.json`; unknown saved fields are kept.
pub fn save_config(paths: &Paths, patch: &Value) -> Result<Value> {
    let mut config = load_config(paths);
    let object = patch.as_object().ok_or("vault_invalid_config")?;
    for (key, value) in object {
        match key.as_str() {
            "enabled" | "agentInstructions" if value.is_boolean() => config[key] = value.clone(),
            "path" => {
                let p = paths.expand(value.as_str().ok_or("vault_invalid_config")?.trim()).display().to_string();
                if !valid_local_path(paths, &p) {
                    return Err("vault_invalid_path".into());
                }
                config["path"] = json!(p);
            }
            "remote" if value.is_null() => config["remote"] = Value::Null,
            "remote" => {
                let url = text(value, "url");
                if has_credentials(url) {
                    return Err("vault_url_has_credentials".into());
                }
                if !valid_remote_url(url) {
                    return Err("vault_invalid_remote".into());
                }
                let kind = text(value, "kind");
                let transport = text(value, "transport");
                if !matches!(kind, "github-private" | "local-bare" | "custom-url") || !matches!(transport, "" | "direct" | "ssh" | "omabridge") {
                    return Err("vault_invalid_remote".into());
                }
                config["remote"] = json!({"kind":kind,"url":url,"environment":value["environment"].as_str(),"transport":if transport.is_empty(){"direct"}else{transport}});
            }
            "sync" => {
                for (sk, sv) in value.as_object().ok_or("vault_invalid_config")? {
                    match sk.as_str() {
                        "pullOnOpen" | "pullOnThreadStart" | "pushOnSettle" | "recordThreads" if sv.is_boolean() => config["sync"][sk] = sv.clone(),
                        "intervalMinutes" if sv.is_null() || sv.as_u64().is_some_and(|n| (5..=1440).contains(&n)) => config["sync"][sk] = sv.clone(),
                        _ => return Err("vault_invalid_config".into()),
                    }
                }
            }
            "projects" => {
                let map = value.as_object().ok_or("vault_invalid_config")?;
                if map.values().any(|v| !v.as_str().is_some_and(valid_slug)) {
                    return Err("vault_invalid_slug".into());
                }
                config["projects"] = value.clone();
            }
            "device" => {
                let name = text(value, "name").trim();
                if name.is_empty() || name.len() > 64 || name.chars().any(|c| c.is_control() || c == '|') {
                    return Err("vault_invalid_config".into());
                }
                config["device"]["name"] = json!(name);
            }
            _ => return Err("vault_invalid_config".into()),
        }
    }
    let mut saved: Value = fs::read(&paths.config).ok().and_then(|b| serde_json::from_slice(&b).ok()).filter(Value::is_object).unwrap_or_else(|| json!({}));
    for (k, v) in config.as_object().unwrap() {
        saved[k] = v.clone();
    }
    save_json(&paths.config, &saved)?;
    Ok(config)
}
fn record(paths: &Paths, key: &str, result: &Result<Value>) {
    let mut state: Value = fs::read(&paths.state).ok().and_then(|b| serde_json::from_slice(&b).ok()).filter(Value::is_object).unwrap_or_else(|| json!({}));
    state[key] = json!({"at":chrono::Utc::now().timestamp_millis(),"ok":result.is_ok(),"error":result.as_ref().err()});
    let _ = save_json(&paths.state, &state);
}
fn sync_state(paths: &Paths) -> Value {
    fs::read(&paths.state).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_else(|| json!({}))
}

/// Stable, non-identifying id of this device and user.
pub fn device_id() -> String {
    use sha2::{Digest, Sha256};
    let mut seed = fs::read("/etc/machine-id").unwrap_or_default();
    seed.extend(unsafe { libc::geteuid() }.to_le_bytes());
    seed.extend(b"dev-vault");
    format!("{:x}", Sha256::digest(&seed))[..12].to_string()
}
fn device(paths: &Paths, config: &Value, vault: &Path) -> Value {
    let environment = fs::read_to_string(crate::t3_home().join("userdata/environment-id")).unwrap_or_default();
    let shown = vault.strip_prefix(&paths.home).map(|r| format!("~/{}", r.display())).unwrap_or_else(|_| vault.display().to_string());
    json!({
        "id":device_id(),"name":text(&config["device"],"name"),"environment":environment.trim(),
        "system":format!("{} {}",std::env::consts::OS,std::env::consts::ARCH),"path":shown,
        "updated":chrono::Local::now().format("%Y-%m-%d").to_string(),
    })
}

// ---------------------------------------------------------------- status and wizard

// ---------------------------------------------------------------- git identity

/// The identity git would use for commits in `dir` (local, global or system).
pub fn identity(dir: &Path) -> Option<(String, String)> {
    let get = |k: &str| Some(run_git(Some(dir), &["config", "--get", k], Some(DEFAULT_SSH), 10)).filter(|o| o.ok && !o.stdout.is_empty()).map(|o| o.stdout);
    Some((get("user.name")?, get("user.email")?))
}
/// Who this person commits as, when the vault has no identity: the identities set in
/// their own checkouts on this computer (many people keep none globally), preferring
/// one that already authored commits in the vault.
pub fn suggest_identity(vault: &Path, roots: &[PathBuf]) -> Option<Value> {
    let mut score: BTreeMap<(String, String), u32> = BTreeMap::new();
    for root in roots.iter().take(60) {
        let local = |k: &str| Some(run_git(Some(root), &["config", "--local", "--get", k], Some(DEFAULT_SSH), 10)).filter(|o| o.ok && !o.stdout.is_empty()).map(|o| o.stdout);
        if let (Some(name), Some(email)) = (local("user.name"), local("user.email")) {
            *score.entry((name, email)).or_default() += 1;
        }
    }
    let log = run_git(Some(vault), &["log", "-200", "--format=%an%x09%ae"], Some(DEFAULT_SSH), 20).stdout;
    let authors: HashSet<(String, String)> = log.lines().filter_map(|l| l.split_once('\t')).map(|(n, e)| (n.to_string(), e.to_string())).collect();
    for (key, value) in score.iter_mut() {
        if authors.contains(key) {
            *value += 100;
        }
    }
    if score.is_empty() {
        // Nobody set one in a checkout: the vault's most recent author, if any.
        let (name, email) = log.lines().next()?.split_once('\t')?;
        return Some(json!({"name":name,"email":email,"from":"vault"}));
    }
    let ((name, email), points) = score.into_iter().max_by_key(|(_, v)| *v)?;
    Some(json!({"name":name,"email":email,"from":if points>=100{"vault-and-checkouts"}else{"checkouts"}}))
}
fn identity_state(vault: &Path, roots: &[PathBuf]) -> Value {
    match identity(vault) {
        Some((name, email)) => json!({"set":true,"name":name,"email":email}),
        None => json!({"set":false,"suggested":suggest_identity(vault, roots)}),
    }
}
/// Set the commit identity for the vault clone only; nothing global changes.
pub fn set_identity(vault: &Path, name: &str, email: &str) -> Result<()> {
    let (name, email) = (name.trim(), email.trim());
    let bad = |s: &str| s.is_empty() || s.len() > 200 || s.chars().any(|c| c.is_control() || "<>".contains(c));
    if bad(name) || bad(email) || !email.contains('@') || email.contains(char::is_whitespace) {
        return Err("vault_invalid_identity".into());
    }
    if !vault.join(".git").is_dir() {
        return Err("vault_not_cloned".into());
    }
    for (k, v) in [("user.name", name), ("user.email", email)] {
        if !run_git(Some(vault), &["config", "--local", k, v], Some(DEFAULT_SSH), 10).ok {
            return Err("vault_git_failed".into());
        }
    }
    Ok(())
}
/// After a setup whose device commit lacked an identity: when one identity both
/// wrote the vault and is set in this person's checkouts here, use it for the vault
/// and retry. Anything weaker is left to the page to confirm.
fn adopt_identity(paths: &Paths, result: &mut Value) {
    let Some(steps) = result["steps"].as_array_mut() else { return };
    let Some(device) = steps.iter_mut().find(|s| s["id"] == "device" && s["error"] == "vault_git_identity_missing") else { return };
    let config = load_config(paths);
    let vault = PathBuf::from(text(&config, "path"));
    let Some(suggested) = suggest_identity(&vault, &local_roots()).filter(|s| s["from"] == "vault-and-checkouts") else { return };
    if set_identity(&vault, text(&suggested, "name"), text(&suggested, "email")).is_err() {
        return;
    }
    let pushed = commit_push(&vault, &format!("journal(dev-vault): register {}", text(&config["device"], "name")));
    record(paths, "push", &pushed);
    *device = match pushed {
        Ok(_) => json!({"id":"device","state":"done"}),
        Err(e) => json!({"id":"device","state":"failed","error":e}),
    };
    let identity = json!({"id":"identity","state":"done","name":suggested["name"],"email":suggested["email"]});
    let at = steps.iter().position(|s| s["id"] == "device").unwrap_or(steps.len());
    steps.insert(at, identity);
    result["complete"] = json!(steps.iter().all(|s| matches!(text(s, "state"), "done" | "skipped")));
}
/// Paths of the projects T3 has on this computer, for the identity suggestion.
pub fn local_roots() -> Vec<PathBuf> {
    let Ok(environments) = crate::environments() else { return vec![] };
    let Ok(client) = crate::Client::new() else { return vec![] };
    let Some(local) = environments.iter().find(|e| e.id == "local") else { return vec![] };
    let Ok(page) = client.get(local, "/api/projects", false) else { return vec![] };
    page["projects"].as_array().into_iter().flatten().map(|p| PathBuf::from(text(p, "workspaceRoot"))).filter(|p| p.join(".git").exists()).collect()
}

/// Everything the page shows about this computer. `configured` is true only when
/// no step is pending.
pub fn read(paths: &Paths) -> Value {
    let config = load_config(paths);
    let vault = PathBuf::from(text(&config, "path"));
    let remote = config["remote"]["url"].as_str();
    let local = inspect(&vault, remote);
    let git = if text(&local, "state") == "clone" { status(&vault) } else { json!({"git":false}) };
    let instructions = config["agentInstructions"] != true || instructions_installed(&paths.home, &vault);
    let obsidian = obsidian(&paths.home);
    let identity = if text(&local, "state") == "clone" { identity_state(&vault, &[]) } else { json!({"set":false}) };
    let steps = json!({
        "remote":remote.is_some(),
        "clone":text(&local,"state")=="clone",
        "identity":identity["set"]==true,
        "instructions":instructions,
        "obsidian":obsidian.is_some(),
    });
    let configured = config["enabled"] == true && steps.as_object().unwrap().values().all(|v| v == true);
    json!({
        "config":config,"local":local,"status":git,"steps":steps,"configured":configured,"identity":identity,
        "obsidian":{"installed":obsidian.is_some(),"how":obsidian,"install":install_help()},
        "sync":sync_state(paths),"device":{"id":device_id(),"name":text(&config["device"],"name")},
    })
}

/// Set up this computer from a remote: check access with local credentials, clone
/// or reuse, install instructions, register the device, save. Each step reports
/// its own state; a failed step stops the wizard without undoing earlier ones.
pub fn setup(paths: &Paths, input: &Value) -> Result<Value> {
    let path = paths.expand(text(input, "path").trim()).display().to_string();
    if !valid_local_path(paths, &path) {
        return Err("vault_invalid_path".into());
    }
    let vault = PathBuf::from(&path);
    let url = text(input, "url").trim().to_string();
    if url.is_empty() {
        return Err("vault_remote_missing".into());
    }
    if has_credentials(&url) {
        return Err("vault_url_has_credentials".into());
    }
    if !valid_remote_url(&url) {
        return Err("vault_invalid_remote".into());
    }
    let ssh = input["sshCommand"].as_str();
    let mut steps = vec![];
    let fail = |steps: &mut Vec<Value>, id: &str, error: String| {
        steps.push(json!({"id":id,"state":"failed","error":error}));
    };
    let found = inspect(&vault, Some(&url));
    if text(&found, "state") == "occupied" {
        fail(&mut steps, "folder", format!("vault_path_occupied: {}", text(&found, "reason")));
        return Ok(json!({"steps":steps,"complete":false,"local":found}));
    }
    steps.push(json!({"id":"folder","state":"done","reuse":text(&found,"state")=="clone"}));
    if let Err(e) = check_access(&url, ssh) {
        let help = (e == "vault_auth_missing").then(|| auth_help(&url));
        steps.push(json!({"id":"access","state":"failed","error":e,"help":help}));
        return Ok(json!({"steps":steps,"complete":false}));
    }
    steps.push(json!({"id":"access","state":"done"}));
    match clone(&url, &vault, ssh) {
        Ok(v) => steps.push(json!({"id":"clone","state":"done","reused":v["reused"]})),
        Err(e) => {
            fail(&mut steps, "clone", e);
            return Ok(json!({"steps":steps,"complete":false}));
        }
    }
    let kind = input["kind"].as_str().map(str::to_owned).unwrap_or_else(|| kind_of(&url).into());
    let mut config = save_config(
        paths,
        &json!({"path":path,"enabled":true,"remote":{"kind":kind,"url":url,"environment":input["environment"],"transport":input["transport"].as_str().unwrap_or("direct")}}),
    )?;
    if let Some(agent) = input["agentInstructions"].as_bool() {
        config = save_config(paths, &json!({"agentInstructions":agent}))?;
    }
    if config["agentInstructions"] == true {
        match install_instructions(&paths.home, &vault) {
            Ok(files) => steps.push(json!({"id":"instructions","state":"done","files":files})),
            Err(e) => fail(&mut steps, "instructions", e),
        }
    } else {
        steps.push(json!({"id":"instructions","state":"skipped"}));
    }
    let registered = register_device(&vault, &device(paths, &config, &vault));
    let pushed = match registered {
        Ok(true) => commit_push(&vault, &format!("journal(dev-vault): register {}", text(&config["device"], "name"))),
        Ok(false) => Ok(json!({"committed":false,"pushed":false})),
        Err(e) => Err(e),
    };
    record(paths, "push", &pushed);
    match pushed {
        Ok(_) => steps.push(json!({"id":"device","state":"done"})),
        Err(e) => fail(&mut steps, "device", e),
    }
    let obsidian = obsidian(&paths.home);
    steps.push(json!({"id":"obsidian","state":if obsidian.is_some(){"done"}else{"pending"},"install":install_help()}));
    let complete = steps.iter().all(|s| matches!(text(s, "state"), "done" | "skipped"));
    Ok(json!({"steps":steps,"complete":complete}))
}
/// Repair an existing clone: remote, instructions, missing structure, device row.
pub fn repair(paths: &Paths) -> Result<Value> {
    let config = load_config(paths);
    let vault = PathBuf::from(text(&config, "path"));
    let url = config["remote"]["url"].as_str().ok_or("vault_remote_missing")?;
    if text(&inspect(&vault, Some(url)), "state") != "clone" {
        return Err("vault_not_cloned".into());
    }
    let mut fixed = vec![];
    for (name, contents) in TEMPLATES.iter().filter(|(n, _)| n.starts_with("Templates/") || *n == "AGENTS.md") {
        let file = vault.join(name);
        confined(&vault, &file)?;
        fs::create_dir_all(file.parent().unwrap()).map_err(|_| "vault_write_failed")?;
        if write_new(&file, contents)? {
            fixed.push(name.to_string());
        }
    }
    if config["agentInstructions"] == true && install_instructions(&paths.home, &vault)?.iter().any(|f| f["changed"] == true) {
        fixed.push("instructions".into());
    }
    if register_device(&vault, &device(paths, &config, &vault))? {
        fixed.push("Machines.md".into());
    }
    let pushed = commit_push(&vault, &format!("journal(dev-vault): repair {}", text(&config["device"], "name")));
    record(paths, "push", &pushed);
    Ok(json!({"fixed":fixed,"push":pushed.as_ref().ok(),"error":pushed.as_ref().err()}))
}

// ---------------------------------------------------------------- discovery over T3

/// Read every connected host but this computer, in parallel.
pub fn discover() -> Result<Value> {
    let environments = crate::environments()?;
    let primary = fs::read_to_string(crate::t3_home().join("userdata/environment-id")).unwrap_or_default();
    let client = crate::Client::new().ok();
    let client = &client;
    let hosts: Vec<Value> = std::thread::scope(|scope| {
        let handles: Vec<_> = environments
            .iter()
            .filter(|e| e.id != "local" && e.id != primary.trim())
            .map(|e| {
                scope.spawn(move || {
                    let mut v = if let Some(error) = &e.connection_error {
                        json!({"state":"offline","error":error,"vaults":[]})
                    } else {
                        // The host's home, from its projects, for servers that do not expand ~/ everywhere.
                        let home = client
                            .as_ref()
                            .and_then(|c| c.get(e, "/api/projects", false).ok())
                            .map(|page| page["projects"].as_array().into_iter().flatten().map(|p| text(p, "workspaceRoot").to_string()).collect::<Vec<_>>())
                            .as_deref()
                            .and_then(home_from_projects);
                        let files = RpcFiles(e, home.clone());
                        let mut v = host_vaults(&files, None);
                        // Servers before ~/ expansion at all: retry with absolute paths.
                        if v["state"] == "online" && v["vaults"].as_array().is_some_and(|a| a.is_empty()) {
                            if let Some(home) = home {
                                v = host_vaults(&files, Some(&home));
                            }
                        }
                        v
                    };
                    v["ssh"] = json!(crate::connections::ssh_args_for(&e.id).is_ok());
                    v["environment"] = json!(e.id);
                    v["name"] = json!(if e.name.is_empty() { &e.id } else { &e.name });
                    v
                })
            })
            .collect();
        handles.into_iter().filter_map(|h| h.join().ok()).collect()
    });
    Ok(json!({"hosts":hosts,"vaults":group(&hosts)}))
}
/// SSH profile and OmaBridge peer that carry an environment, for a path remote.
pub fn transport_for(environment: &str) -> HostTransport {
    let ssh_args = crate::connections::ssh_args_for(environment).ok();
    let omabridge = crate::omabridge_route::config().ok().and_then(|c| match c.plan(environment) {
        crate::omabridge_route::Plan::Pinned(r) if r.service == "ssh" => crate::omabridge_route::binary().map(|b| (b, r.state_dir, r.peer)),
        _ => None,
    });
    HostTransport { ssh_args, omabridge }
}

/// `obsidian://open` for one note of the vault.
pub fn note_uri(vault: &Path, file: &str) -> Option<String> {
    let path = vault.join(file);
    (file.ends_with(".md") && !file.split('/').any(|s| s == "..") && path.is_file()).then(|| obsidian_uri(&path))
}

/// Panel and CLI entry point: `vault-read`, `vault-discover`, `vault-action`.
pub fn execute(input: &Value) -> Result<Value> {
    let mut result = execute_inner(input)?;
    // The identity suggestion looks at this computer's T3 checkouts: only when missing.
    let state = if input["op"] == "vault-read" { Some(&mut result) } else { result.get_mut("state") };
    let Some(state) = state else { return Ok(result) };
    if state["identity"]["set"] == false && state["local"]["state"] == "clone" {
        let vault = PathBuf::from(text(&state["config"], "path"));
        state["identity"]["suggested"] = suggest_identity(&vault, &local_roots()).unwrap_or(Value::Null);
    }
    Ok(result)
}
fn execute_inner(input: &Value) -> Result<Value> {
    let paths = Paths::current();
    match text(input, "op") {
        "vault-read" => {
            refresh_instructions(&paths, &load_config(&paths));
            Ok(read(&paths))
        }
        "vault-discover" => discover(),
        "vault-action" if input["action"] == "search" => {
            let vault = PathBuf::from(text(&load_config(&paths), "path"));
            Ok(json!({"query":input["query"],"hits":search(&vault, text(input, "query"), input["limit"].as_u64().unwrap_or(20) as usize)}))
        }
        "vault-action" => {
            let _guard = Busy::take()?;
            match text(input, "action") {
                "inspect" => Ok(inspect(&paths.expand(text(input, "path")), input["url"].as_str())),
                "source" => {
                    let source = clone_source(text(input, "url"), &transport_for(text(input, "environment")))?;
                    Ok(source)
                }
                "setup" => {
                    let mut request = input.clone();
                    // A host path is re-resolved here, never taken from the panel as is.
                    if let Some(env) = input["environment"].as_str().filter(|_| is_path(text(input, "hostRemote"))) {
                        let source = clone_source(text(input, "hostRemote"), &transport_for(env))?;
                        request["url"] = source["url"].clone();
                        request["sshCommand"] = source["sshCommand"].clone();
                        request["transport"] = source["transport"].clone();
                        request["kind"] = source["kind"].clone();
                    } else {
                        request["sshCommand"] = Value::Null;
                    }
                    let mut result = setup(&paths, &request)?;
                    adopt_identity(&paths, &mut result);
                    Ok(json!({"setup":result,"state":read(&paths)}))
                }
                "create" => {
                    let path = paths.expand(text(input, "path").trim());
                    if !valid_local_path(&paths, &path.display().to_string()) {
                        return Err("vault_invalid_path".into());
                    }
                    let github = input["github"].as_str().filter(|s| !s.is_empty());
                    let remote = input["url"].as_str().filter(|s| !s.is_empty());
                    let created = create(&path, remote, github)?;
                    let url = github.map(|_| run_git(Some(&path), &["remote", "get-url", "origin"], None, 10).stdout).or(remote.map(str::to_owned));
                    let mut patch = json!({"path":path.display().to_string(),"enabled":true});
                    if let Some(url) = url.filter(|u| !u.is_empty()) {
                        patch["remote"] = json!({"kind":kind_of(&url),"url":url,"transport":"direct"});
                    }
                    let config = save_config(&paths, &patch)?;
                    if config["agentInstructions"] == true {
                        install_instructions(&paths.home, &path)?;
                    }
                    if register_device(&path, &device(&paths, &config, &path))? {
                        let pushed = commit_push(&path, &format!("journal(dev-vault): register {}", text(&config["device"], "name")));
                        record(&paths, "push", &pushed);
                    }
                    Ok(json!({"created":created,"state":read(&paths)}))
                }
                "repair" => Ok(json!({"repair":repair(&paths)?,"state":read(&paths)})),
                "pull" => {
                    let vault = PathBuf::from(text(&load_config(&paths), "path"));
                    let result = pull(&vault);
                    record(&paths, "pull", &result);
                    result?;
                    Ok(json!({"state":read(&paths)}))
                }
                "push" => {
                    let config = load_config(&paths);
                    let vault = PathBuf::from(text(&config, "path"));
                    // Sync now: bring the remote in first, then send what is local.
                    let result = pull(&vault).and_then(|_| commit_push(&vault, &format!("journal: sync from {}", text(&config["device"], "name"))));
                    record(&paths, "push", &result);
                    result?;
                    Ok(json!({"state":read(&paths)}))
                }
                "identity" => {
                    let config = load_config(&paths);
                    let vault = PathBuf::from(text(&config, "path"));
                    set_identity(&vault, text(input, "name"), text(input, "email"))?;
                    // What was waiting for an identity (the device row, notes) goes now.
                    let result = pull(&vault).and_then(|_| commit_push(&vault, &format!("journal: sync from {}", text(&config["device"], "name"))));
                    record(&paths, "push", &result);
                    Ok(json!({"state":read(&paths),"push":result.as_ref().ok(),"error":result.as_ref().err()}))
                }
                "open-note" => {
                    let vault = PathBuf::from(text(&load_config(&paths), "path"));
                    let uri = note_uri(&vault, text(input, "file")).ok_or("vault_note_missing")?;
                    obsidian(&paths.home).ok_or("vault_obsidian_missing")?;
                    register_obsidian(&paths.home, obsidian(&paths.home).unwrap(), &vault)?;
                    let opener = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
                    Command::new(opener).arg(uri).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().map_err(|_| "vault_obsidian_open_failed")?;
                    Ok(json!({"opened":true}))
                }
                "install-host" => {
                    let files = install_on_host(text(input, "environment"), text(input, "path"))?;
                    Ok(json!({"files":files}))
                }
                "open" => {
                    open_obsidian(&paths.home, Path::new(text(&load_config(&paths), "path")))?;
                    Ok(json!({"opened":true}))
                }
                "save" => {
                    let config = save_config(&paths, &input["patch"])?;
                    let vault = PathBuf::from(text(&config, "path"));
                    if input["patch"]["agentInstructions"] == true && text(&inspect(&vault, config["remote"]["url"].as_str()), "state") == "clone" {
                        install_instructions(&paths.home, &vault)?;
                    }
                    Ok(json!({"state":read(&paths)}))
                }
                _ => Err("invalid_command".into()),
            }
        }
        _ => Err("invalid_command".into()),
    }
}
/// One vault operation at a time on this computer, from the panel or automations.
static BUSY: AtomicBool = AtomicBool::new(false);
pub struct Busy;
impl Busy {
    pub fn take() -> Result<Busy> {
        if BUSY.swap(true, Ordering::SeqCst) {
            Err("vault_busy".into())
        } else {
            Ok(Busy)
        }
    }
}
impl Drop for Busy {
    fn drop(&mut self) {
        BUSY.store(false, Ordering::SeqCst);
    }
}

// ---------------------------------------------------------------- automations

#[derive(Debug, PartialEq)]
pub enum Task {
    Pull,
    Push(String),
    Project { slug: String, title: String },
    Record { slug: String, row: ThreadRow },
}
/// Decides what to do from successive snapshots: pull when ot3 opens and when a
/// thread starts, push once threads settle, stub notes for new projects.
#[derive(Default)]
pub struct Automation {
    started: bool,
    working: HashSet<String>,
    projects: HashSet<String>,
    environments: HashSet<String>,
    settled: Option<(Instant, String)>,
    rows: Vec<(String, ThreadRow)>,
    last_pull: Option<Instant>,
    last_interval: Option<Instant>,
}
pub const SETTLE_DELAY: Duration = Duration::from_secs(30);
const PULL_SPACING: Duration = Duration::from_secs(60);
impl Automation {
    pub fn observe(&mut self, config: &Value, snapshot: &Value, now: Instant) -> Vec<Task> {
        let mut tasks = vec![];
        if config["enabled"] != true || !config["remote"]["url"].is_string() {
            return tasks;
        }
        let sync = &config["sync"];
        let threads: Vec<&Value> = snapshot["threads"].as_array().into_iter().flatten().collect();
        let working: HashSet<String> = threads.iter().filter(|t| t["working"] == true).map(|t| text(t, "key").to_string()).collect();
        let slug_of = |project: &str| {
            snapshot["projects"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|p| text(p, "title") == project || text(p, "id") == project)
                .and_then(|p| project_slug(config, text(p, "title"), text(p, "workspaceRoot")))
        };
        let mut pull = false;
        if !self.started {
            self.started = true;
            pull = sync["pullOnOpen"] == true;
        } else if sync["pullOnThreadStart"] == true && working.iter().any(|k| !self.working.contains(k)) {
            pull = self.last_pull.is_none_or(|t| now.duration_since(t) >= PULL_SPACING);
        }
        let record = sync["recordThreads"] != false;
        if sync["pushOnSettle"] == true || record {
            let machine = |env: &str| {
                if env == "local" {
                    text(&config["device"], "name").to_string()
                } else {
                    snapshot["environments"].as_array().into_iter().flatten().find(|e| text(e, "id") == env).map(|e| text(e, "name").to_string()).filter(|n| !n.is_empty()).unwrap_or_else(|| env.to_string())
                }
            };
            for key in self.working.iter().filter(|k| !working.contains(*k)) {
                let Some(t) = threads.iter().find(|t| text(t, "key") == key) else { continue };
                let slug = slug_of(text(t, "project"));
                if record {
                    if let (Some(slug), Some(row)) = (&slug, ThreadRow::from_thread(t, &machine(text(t, "environmentId")))) {
                        self.rows.retain(|(_, r)| r.id != row.id);
                        self.rows.push((slug.clone(), row));
                    }
                }
                self.settled = Some((now, slug.unwrap_or_else(|| "dev-vault".into())));
            }
        }
        if let Some(minutes) = sync["intervalMinutes"].as_u64() {
            let every = Duration::from_secs(minutes * 60);
            match self.last_interval {
                None => self.last_interval = Some(now),
                Some(t) if now.duration_since(t) >= every => {
                    self.last_interval = Some(now);
                    pull = true;
                    self.settled.get_or_insert((now - SETTLE_DELAY, "dev-vault".into()));
                }
                _ => {}
            }
        }
        if pull {
            self.last_pull = Some(now);
            tasks.push(Task::Pull);
        }
        if let Some((at, slug)) = &self.settled {
            if working.is_empty() && now.duration_since(*at) >= SETTLE_DELAY {
                for (slug, row) in self.rows.drain(..) {
                    tasks.push(Task::Record { slug, row });
                }
                if sync["pushOnSettle"] == true {
                    tasks.push(Task::Push(slug.clone()));
                }
                self.settled = None;
            }
        }
        self.working = working;
        // An environment's first projects are what T3 already had: only projects
        // registered later get a note (and only when it is missing).
        let known: HashSet<String> = self.environments.clone();
        for p in snapshot["projects"].as_array().into_iter().flatten() {
            let environment = text(p, "environmentId").to_string();
            self.environments.insert(environment.clone());
            if !self.projects.insert(text(p, "key").to_string()) || !known.contains(&environment) || p["isScratch"] == true {
                continue;
            }
            if let Some(slug) = project_slug(config, text(p, "title"), text(p, "workspaceRoot")) {
                tasks.push(Task::Project { slug, title: text(p, "title").into() });
            }
        }
        tasks
    }
    /// When the loop must look again for a pending push.
    pub fn next_due(&self) -> Option<Instant> {
        self.settled.as_ref().map(|(at, _)| *at + SETTLE_DELAY)
    }
}
/// Run automation tasks on this computer, one at a time, off the caller's thread.
pub fn run_tasks(paths: Paths, tasks: Vec<Task>, done: impl FnOnce(Value) + Send + 'static) {
    if tasks.is_empty() {
        return;
    }
    std::thread::spawn(move || {
        let Ok(_guard) = Busy::take() else { return };
        let config = load_config(&paths);
        let vault = PathBuf::from(text(&config, "path"));
        if text(&inspect(&vault, config["remote"]["url"].as_str()), "state") != "clone" {
            return;
        }
        refresh_instructions(&paths, &config);
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let mut created = vec![];
        let mut recorded: Vec<String> = vec![];
        for task in &tasks {
            match task {
                Task::Pull => {
                    let result = pull(&vault);
                    record(&paths, "pull", &result);
                }
                Task::Project { slug, title } => {
                    if ensure_project(&vault, slug, title, &today).unwrap_or(false) {
                        created.push(slug.clone());
                    }
                }
                Task::Record { slug, row } => {
                    if record_threads(&vault, slug, std::slice::from_ref(row)).unwrap_or(false) && !recorded.contains(slug) {
                        recorded.push(slug.clone());
                    }
                }
                Task::Push(_) => {}
            }
        }
        let push = tasks.iter().find_map(|t| if let Task::Push(s) = t { Some(s.clone()) } else { None });
        let message = match (&push, created.as_slice()) {
            (Some(slug), _) => Some(format!("journal({slug}): sync from {}", text(&config["device"], "name"))),
            (None, []) if !recorded.is_empty() => Some(format!("journal({}): record threads", recorded.join(","))),
            (None, [one]) => Some(format!("journal({one}): add project memory")),
            (None, [_, ..]) => Some("journal: add project memories".into()),
            (None, []) => None,
        };
        if let Some(message) = message {
            let result = commit_push(&vault, &message);
            record(&paths, "push", &result);
        }
        done(read(&paths));
    });
}
