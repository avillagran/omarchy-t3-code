//! Preferences → Connections → **Sync this computer's setup**: leave a connected
//! host with this computer's working setup, only when the person asks for it.
//! Four items, each optional: the development vault (cloned or pulled there from
//! the same remote), the harnesses' global instructions, their skills, and their
//! configuration that is neither secret nor specific to this machine. Nothing is
//! deleted; what is replaced is first copied to a backup on the host.
//!
//! Credentials, sessions, history and caches are never read for sending: only the
//! allowlisted paths below are, and files that look like credentials are skipped.
//! Every sign-in stays per host (provider_login).
//!
//! Transport: the connection's SSH profile (one multiplexed OpenSSH master for the
//! whole sync, so a host that rate-limits port 22 sees a single connection), or the
//! same SSH through the peer's OmaBridge `ssh` service with the profile's host key
//! (`HostKeyAlias`). A preview is one SSH command (host-sync-probe.sh, read-only);
//! a sync is the probe again plus one command that receives a tar archive
//! (host-sync-apply.sh) and writes a file only if the host still has what the probe saw.
use crate::{json, text, Result};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;

const PROBE: &str = include_str!("host-sync-probe.sh");
const APPLY: &str = include_str!("host-sync-apply.sh");
const SEP: char = '\u{1f}';
pub const ITEMS: [&str; 4] = ["vault", "instructions", "skills", "config"];

/// Global instructions of each harness, where it reads them.
pub const INSTRUCTIONS: [&str; 4] = [".claude/CLAUDE.md", ".codex/AGENTS.md", ".config/opencode/AGENTS.md", ".gemini/GEMINI.md"];
/// Skill folders: each entry is one skill, copied with its links resolved.
pub const SKILL_ROOTS: [&str; 3] = [".claude/skills", ".codex/skills", ".agents/skills"];
/// Configuration files and folders, never the harnesses' state next to them.
pub const CONFIG_FILES: [&str; 6] = [".claude/settings.json", ".claude/keybindings.json", ".codex/config.toml", ".config/opencode/opencode.json", ".config/opencode/opencode.jsonc", ".gemini/settings.json"];
pub const CONFIG_DIRS: [&str; 14] = [
    ".claude/agents", ".claude/commands", ".claude/output-styles", ".claude/hooks", ".claude/themes",
    ".codex/prompts", ".codex/rules",
    ".config/opencode/agent", ".config/opencode/agents", ".config/opencode/command", ".config/opencode/commands", ".config/opencode/mode", ".config/opencode/plugin",
    ".gemini/commands",
];
/// Claude keeps user MCP servers in `~/.claude.json` next to this machine's
/// account and caches: only that key is merged, servers the host lacks are added.
pub const MERGES: [(&str, &str); 1] = [(".claude.json", "mcpServers")];
/// Kept out even inside an allowlisted folder: Claude's account-synced skills and
/// the theme Omarchy writes on every machine.
const EXCLUDED: [&str; 2] = [".claude/skills/synced", ".claude/themes/omarchy.json"];
const MAX_FILE: u64 = 8 << 20;
const MAX_TOTAL: u64 = 128 << 20;
const MAX_FILES: usize = 20_000;

/// A file to place on the host, relative to both homes.
#[derive(Clone, Debug)]
pub struct Entry {
    pub item: &'static str,
    pub path: String,
    pub bytes: Vec<u8>,
    pub mode: u32,
    /// The skill folder it belongs to (`.claude/skills/<name>`), else empty.
    pub group: String,
}
/// What this computer offers, before looking at the host.
#[derive(Default, Debug)]
pub struct Local {
    pub entries: Vec<Entry>,
    /// `{item, path, reason}` left here: `credentials`, `system`, `too-large`, `unreadable`.
    pub skipped: Vec<Value>,
    pub vault: Option<Vault>,
    pub vault_error: String,
}
#[derive(Clone, Debug)]
pub struct Vault {
    /// Relative to the home on both sides (`Obsidian/Dev`).
    pub path: String,
    pub url: String,
    pub name: String,
    pub email: String,
}

fn safe_rel(p: &str) -> bool {
    !p.is_empty() && p.len() <= 1024 && !p.starts_with('/') && !p.chars().any(|c| c.is_control()) && p.split('/').all(|s| !s.is_empty() && s != "." && s != "..")
}
/// Names that hold credentials, keys, history or databases anywhere in a tree.
fn denied(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    matches!(n.as_str(), ".credentials.json" | "auth.json" | "credentials" | "credentials.json" | ".netrc" | ".git" | "node_modules" | "__pycache__" | ".venv" | ".ds_store")
        || n == ".env" || n.starts_with(".env.")
        || n.starts_with("id_rsa") || n.starts_with("id_ed25519") || n.starts_with("id_ecdsa") || n.starts_with("id_dsa")
        || n.starts_with("history")
        || [".pem", ".key", ".p12", ".pfx", ".keystore", ".sqlite", ".sqlite3", ".db", ".db-wal", ".db-shm", ".sqlite-wal", ".sqlite-shm", ".log"].iter().any(|s| n.ends_with(s))
}
/// A token or key written in the text: well-known prefixes anywhere, and for
/// configuration a secret-named key with a literal value (`${VAR}` and `$VAR` are fine).
pub fn has_secret(text: &str, config: bool) -> bool {
    const PREFIXES: [(&str, usize); 10] = [("sk-ant-", 20), ("sk-proj-", 20), ("ghp_", 30), ("gho_", 30), ("ghu_", 30), ("ghs_", 30), ("github_pat_", 30), ("xoxb-", 20), ("xoxp-", 20), ("glpat-", 20)];
    let token = |s: &str, min: usize| s.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-').count() >= min;
    for (prefix, min) in PREFIXES {
        let mut rest = text;
        while let Some(i) = rest.find(prefix) {
            if token(&rest[i + prefix.len()..], min) {
                return true;
            }
            rest = &rest[i + prefix.len()..];
        }
    }
    if text.contains("-----BEGIN") && text.contains("PRIVATE KEY") {
        return true;
    }
    if let Some(i) = text.find("AKIA") {
        if text[i + 4..].chars().take(16).filter(|c| c.is_ascii_uppercase() || c.is_ascii_digit()).count() == 16 {
            return true;
        }
    }
    if !config {
        return false;
    }
    const KEYS: [&str; 11] = ["apikey", "api_key", "api-key", "token", "secret", "password", "passwd", "authorization", "bearer", "private_key", "access_key"];
    let lower = text.to_ascii_lowercase();
    let word = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '-';
    for k in KEYS {
        for (i, _) in lower.match_indices(k) {
            let begin = lower[..i].rfind(|c: char| !word(c)).map_or(0, |b| b + 1);
            let rest = &lower[i..];
            let len = rest.find(|c: char| !word(c)).unwrap_or(rest.len());
            let name = &lower[begin..i + len];
            if ["max", "limit", "env", "file", "path", "count", "usage", "budget", "url"].iter().any(|w| name.contains(w)) || name.ends_with("tokens") {
                continue;
            }
            let rest = rest[len..].trim_start_matches(['"', '\'']).trim_start();
            let Some(rest) = rest.strip_prefix([':', '=']) else { continue };
            let rest = rest.trim_start();
            let Some(quote) = rest.chars().next().filter(|c| *c == '"' || *c == '\'') else { continue };
            let value = &rest[1..rest[1..].find(quote).map_or(rest.len(), |e| e + 1)];
            if value.len() >= 8 && !value.starts_with('$') && !value.starts_with('<') && !value.starts_with('{') && !value.contains("${") && !value.starts_with("env:") {
                return true;
            }
        }
    }
    false
}
/// `/home/<user>` of this computer becomes the host's home, only as a whole path
/// prefix (`/home/me/x`, `"/home/me"`, never `/home/meow`). Binary data is kept.
pub fn rewrite_home(bytes: &[u8], local: &str, host: &str) -> Vec<u8> {
    if local.is_empty() || local == host || local == "/" {
        return bytes.to_vec();
    }
    let Ok(text) = std::str::from_utf8(bytes) else { return bytes.to_vec() };
    if text.contains('\0') {
        return bytes.to_vec();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find(local) {
        let after = &rest[i + local.len()..];
        let boundary = after.chars().next().is_none_or(|c| !(c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.'));
        let before_ok = rest[..i].chars().last().is_none_or(|c| !(c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' || c == '/'));
        out.push_str(&rest[..i]);
        out.push_str(if boundary && before_ok { host } else { local });
        rest = after;
    }
    out.push_str(rest);
    out.into_bytes()
}
fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

// ---------------------------------------------------------------- this computer

struct Walk<'a> {
    home: &'a Path,
    real_home: PathBuf,
    local: &'a mut Local,
    total: u64,
    seen: HashSet<PathBuf>,
}
impl Walk<'_> {
    fn inside(&self, p: &Path) -> bool {
        fs::canonicalize(p).is_ok_and(|r| r.starts_with(&self.real_home))
    }
    fn skip(&mut self, item: &str, path: &str, reason: &str) {
        self.local.skipped.push(json!({"item":item,"path":path,"reason":reason}));
    }
    fn file(&mut self, item: &'static str, rel: &str, group: &str, config: bool) {
        if EXCLUDED.contains(&rel) {
            return;
        }
        let full = self.home.join(rel);
        if !self.inside(&full) {
            return self.skip(item, rel, "system");
        }
        let Ok(meta) = fs::metadata(&full) else { return self.skip(item, rel, "unreadable") };
        if !meta.is_file() {
            return;
        }
        if meta.len() > MAX_FILE || self.total + meta.len() > MAX_TOTAL || self.local.entries.len() >= MAX_FILES {
            return self.skip(item, rel, "too-large");
        }
        let Ok(bytes) = fs::read(&full) else { return self.skip(item, rel, "unreadable") };
        if std::str::from_utf8(&bytes).is_ok_and(|t| has_secret(t, config)) {
            return self.skip(item, rel, "credentials");
        }
        self.total += bytes.len() as u64;
        self.local.entries.push(Entry { item, path: rel.into(), bytes, mode: meta.permissions().mode() & 0o777, group: group.into() });
    }
    fn tree(&mut self, item: &'static str, rel: &str, group: &str, config: bool, depth: usize) {
        let full = self.home.join(rel);
        if depth > 16 || EXCLUDED.contains(&rel) || !self.inside(&full) {
            return;
        }
        let Ok(real) = fs::canonicalize(&full) else { return };
        if !self.seen.insert(real) {
            return;
        }
        let Ok(dir) = fs::read_dir(&full) else { return };
        let mut names: Vec<String> = dir.filter_map(|e| e.ok()?.file_name().into_string().ok()).collect();
        names.sort();
        for name in names {
            if denied(&name) || name.chars().any(|c| c.is_control()) {
                continue;
            }
            let child = format!("{rel}/{name}");
            match fs::metadata(self.home.join(&child)) {
                Ok(m) if m.is_dir() => self.tree(item, &child, group, config, depth + 1),
                Ok(m) if m.is_file() => self.file(item, &child, group, config),
                _ => {}
            }
        }
    }
}

/// Everything this computer would send, from its `home` (tests pass a temporary one).
pub fn collect(home: &Path, vault_config: Option<&Value>) -> Local {
    let mut local = Local::default();
    let real_home = fs::canonicalize(home).unwrap_or_else(|_| home.into());
    let mut w = Walk { home, real_home, local: &mut local, total: 0, seen: HashSet::new() };
    for rel in INSTRUCTIONS {
        w.file("instructions", rel, "", false);
    }
    for root in SKILL_ROOTS {
        let Ok(dir) = fs::read_dir(home.join(root)) else { continue };
        let mut names: Vec<String> = dir.filter_map(|e| e.ok()?.file_name().into_string().ok()).collect();
        names.sort();
        for name in names {
            let rel = format!("{root}/{name}");
            // Hidden entries are the harness's own (Codex's `.system`).
            if name.starts_with('.') || denied(&name) || EXCLUDED.contains(&rel.as_str()) || !safe_rel(&rel) {
                continue;
            }
            // A link out of this home (Omarchy's skills) is the system's: each host has its own.
            if !w.inside(&home.join(&rel)) {
                w.skip("skills", &rel, "system");
                continue;
            }
            match fs::metadata(home.join(&rel)) {
                Ok(m) if m.is_dir() => w.tree("skills", &rel, &rel, false, 0),
                Ok(m) if m.is_file() => w.file("skills", &rel, "", false),
                _ => {}
            }
        }
    }
    for rel in CONFIG_FILES {
        w.file("config", rel, "", true);
    }
    for rel in CONFIG_DIRS {
        if home.join(rel).is_dir() {
            w.tree("config", rel, "", true, 0);
        }
    }
    match vault_config.map(|c| local_vault(home, c)) {
        Some(Ok(v)) => local.vault = Some(v),
        Some(Err(e)) => local.vault_error = e,
        None => local.vault_error = "host_sync_vault_missing".into(),
    }
    local
}
/// The vault configured here, as the host must see it: the same path under its
/// home and the same network remote, without credentials.
fn local_vault(home: &Path, config: &Value) -> Result<Vault> {
    let path = PathBuf::from(text(config, "path"));
    if text(config, "path").is_empty() || !path.join(".git").exists() {
        return Err("host_sync_vault_missing".into());
    }
    let rel = path.strip_prefix(home).map_err(|_| "host_sync_vault_outside_home")?.to_string_lossy().to_string();
    if !safe_rel(&rel) {
        return Err("host_sync_vault_outside_home".into());
    }
    let git = |args: &[&str]| Command::new("git").arg("-C").arg(&path).args(args).stdin(Stdio::null()).stderr(Stdio::null()).output().ok().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
    let mut url = git(&["config", "--get", "remote.origin.url"]);
    if url.is_empty() {
        url = text(&config["remote"], "url").to_string();
    }
    let url = crate::journal_vault::redact_url(&url);
    let network = url.starts_with("https://") || url.starts_with("ssh://") || (url.contains('@') && url.contains(':') && !url.contains("://"));
    if url.is_empty() || !network || url.chars().any(|c| c.is_control() || c == SEP) {
        return Err("host_sync_vault_remote_unsupported".into());
    }
    let clean = |s: String| if s.chars().any(|c| c.is_control()) { String::new() } else { s };
    Ok(Vault { path: rel, url, name: clean(git(&["config", "--local", "user.name"])), email: clean(git(&["config", "--local", "user.email"])) })
}

// ---------------------------------------------------------------- transport

/// How a script reaches the host: SSH arguments ending in `-- <destination>`, or,
/// for tests, a local shell with another HOME.
#[derive(Clone, Debug)]
pub enum Session {
    Ssh { args: Vec<String>, transport: &'static str },
    Local { home: PathBuf },
}
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}
impl Session {
    pub fn transport(&self) -> &'static str {
        match self {
            Self::Ssh { transport, .. } => transport,
            Self::Local { .. } => "local",
        }
    }
    /// Runs `script` there with `input` on stdin: (exit code, stdout).
    pub fn run(&self, script: &str, input: &[u8], seconds: u32) -> Result<(i32, Vec<u8>)> {
        let mut cmd = Command::new("timeout");
        cmd.arg(format!("{seconds}s"));
        match self {
            Self::Ssh { args, .. } => {
                cmd.arg("ssh").args(args).arg(format!("sh -c {} sh", shell_quote(script)));
            }
            Self::Local { home } => {
                cmd.args(["sh", "-c", script, "sh"]).env("HOME", home);
            }
        }
        let mut child = cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().map_err(|_| "ssh_unavailable")?;
        let mut stdin = child.stdin.take().ok_or("ssh_unavailable")?;
        let input = input.to_vec();
        let writer = std::thread::spawn(move || {
            let _ = stdin.write_all(&input);
        });
        let out = child.wait_with_output().map_err(|_| "ssh_unavailable")?;
        let _ = writer.join();
        Ok((out.status.code().unwrap_or(-1), out.stdout))
    }
    /// Terminal command that runs `command` in a login shell there.
    pub fn terminal_args(&self) -> Option<Vec<String>> {
        match self {
            Self::Ssh { args, .. } => Some(args.clone()),
            Self::Local { .. } => None,
        }
    }
}
/// SSH through the peer's OmaBridge `ssh` service, still checked against the
/// profile's own host key.
pub(crate) fn bridged(direct: &[String], alias: &str, bin: &Path, state_dir: &Path, peer: &str) -> Result<Vec<String>> {
    if !crate::omabridge_route::valid_peer(peer) {
        return Err("omabridge_route_failed".into());
    }
    let split = direct.iter().position(|a| a == "--").ok_or("invalid_t3_ssh_profile")?;
    let proxy = format!("{} --state-dir {} --name ot3 connect --peer {peer} --service ssh", shell_quote(&bin.display().to_string()), shell_quote(&state_dir.display().to_string()));
    let mut args = direct[..split].to_vec();
    for o in [format!("HostKeyAlias={alias}"), "CheckHostIP=no".into(), "ProxyJump=none".into(), format!("ProxyCommand={proxy}")] {
        args.push("-o".into());
        args.push(o);
    }
    args.extend_from_slice(&direct[split..]);
    Ok(args)
}
/// The ways to reach an SSH environment, in the order to try them: the OmaBridge
/// peer carrying it now (or pinned), then its saved route, then other paired peers
/// granting `ssh` (each must still pass the host key check).
fn sessions(environment: &str) -> Result<Vec<Session>> {
    uuid::Uuid::parse_str(environment).map_err(|_| "invalid_t3_environment_id")?;
    let direct = crate::connections::ssh_args_for(environment).map_err(|_| "host_sync_needs_ssh")?;
    let mut out = vec![];
    let config = crate::omabridge_route::config().ok();
    let bin = crate::omabridge_route::binary();
    let alias = crate::connections::host_key_alias(&direct).ok();
    let mut peers: Vec<(String, PathBuf, bool)> = vec![];
    let mut direct_first = true;
    if let (Some(config), Some(_)) = (&config, &bin) {
        match config.plan(environment) {
            crate::omabridge_route::Plan::Pinned(r) if r.service == "ssh" => {
                direct_first = false;
                peers.push((r.peer.clone(), r.state_dir.clone(), r.only));
            }
            crate::omabridge_route::Plan::Auto => {
                if let Some(peer) = crate::omabridge_route::active_peer(environment, "ssh") {
                    direct_first = false;
                    peers.push((peer, config.state_dir(), false));
                }
                for r in crate::omabridge_route::auto_routes(config, "ssh") {
                    if !peers.iter().any(|(p, _, _)| *p == r.peer) {
                        peers.push((r.peer, r.state_dir, false));
                    }
                }
            }
            _ => {}
        }
    }
    let only = peers.first().is_some_and(|(_, _, only)| *only);
    if direct_first {
        out.push(Session::Ssh { args: direct.clone(), transport: "ssh" });
    }
    if let (Some(alias), Some(bin)) = (&alias, &bin) {
        for (peer, state_dir, _) in &peers {
            if let Ok(args) = bridged(&direct, alias, bin, state_dir, peer) {
                out.push(Session::Ssh { args, transport: "omabridge" });
            }
        }
    }
    if !direct_first && !only {
        out.push(Session::Ssh { args: direct, transport: "ssh" });
    }
    Ok(out)
}

// ---------------------------------------------------------------- the host

#[derive(Default, Debug)]
pub struct Probe {
    pub home: String,
    pub files: BTreeMap<String, String>,
    pub groups: BTreeMap<String, String>,
    /// path → (sha, contents) of a file to merge into, or its state.
    pub merges: BTreeMap<String, (String, Option<Vec<u8>>)>,
    pub vault: Option<String>,
    pub vault_origin: String,
    pub access: String,
    pub gh: String,
}
fn probe_input(local: &Local, merges: &[(String, String)]) -> String {
    let mut s = String::new();
    let mut groups = HashSet::new();
    for e in &local.entries {
        if !e.group.is_empty() && groups.insert(e.group.clone()) {
            s.push_str(&format!("G{SEP}{}\n", e.group));
        }
        s.push_str(&format!("F{SEP}{}\n", e.path));
    }
    for (path, _) in merges {
        s.push_str(&format!("J{SEP}{path}\n"));
    }
    if let Some(v) = &local.vault {
        s.push_str(&format!("V{SEP}{}{SEP}{}\n", v.path, v.url));
    }
    s
}
pub fn parse_probe(out: &[u8]) -> Result<Probe> {
    let text = std::str::from_utf8(out).map_err(|_| "host_sync_probe_failed")?;
    let mut p = Probe::default();
    for line in text.lines() {
        let f: Vec<&str> = line.split(SEP).collect();
        match (f[0], f.len()) {
            ("H", 2) => p.home = f[1].to_string(),
            ("F", 3) => {
                p.files.insert(f[1].into(), f[2].into());
            }
            ("G", 3) => {
                p.groups.insert(f[1].into(), f[2].into());
            }
            ("J", 3) => {
                p.merges.insert(f[1].into(), (f[2].into(), None));
            }
            ("J", 4) => {
                use base64::Engine;
                let bytes = base64::engine::general_purpose::STANDARD.decode(f[3]).map_err(|_| "host_sync_probe_failed")?;
                p.merges.insert(f[1].into(), (f[2].into(), Some(bytes)));
            }
            ("V", 3) => p.vault = Some(f[2].into()),
            ("V", 4) if f[2] == "clone" => {
                p.vault = Some("clone".into());
                p.vault_origin = crate::journal_vault::redact_url(f[3]);
            }
            ("A", 2) => p.access = f[1].into(),
            ("T", 2) => p.gh = f[1].into(),
            _ => {}
        }
    }
    if !p.home.starts_with('/') || p.home.chars().any(|c| c.is_control()) {
        return Err("host_sync_probe_failed".into());
    }
    Ok(p)
}
/// A planned write: the bytes as the host must get them and what it holds now.
#[derive(Clone, Debug)]
pub struct Planned {
    pub item: &'static str,
    pub path: String,
    pub group: String,
    pub bytes: Vec<u8>,
    pub mode: u32,
    pub expected: String,
    /// add, update, same, kept (the host's own) or skipped (not a file there).
    pub action: &'static str,
}
fn merged(local_home: &Path, path: &str, key: &str, host: &(String, Option<Vec<u8>>), host_home: &str) -> Option<Result<(Vec<u8>, bool)>> {
    let here: Value = serde_json::from_slice(&fs::read(local_home.join(path)).ok()?).ok()?;
    let servers = here.get(key)?.as_object()?.clone();
    if servers.is_empty() {
        return None;
    }
    let servers_text = serde_json::to_string(&servers).ok()?;
    if has_secret(&servers_text, true) {
        return Some(Err("credentials".into()));
    }
    let servers: Value = serde_json::from_slice(&rewrite_home(servers_text.as_bytes(), &local_home.display().to_string(), host_home)).ok()?;
    let mut doc = match &host.1 {
        Some(bytes) => match serde_json::from_slice::<Value>(bytes) {
            Ok(v) if v.is_object() => v,
            _ => return Some(Err("unreadable".into())),
        },
        None if host.0 == "absent" => json!({}),
        None => return Some(Err("not-a-file".into())),
    };
    if !doc[key].is_object() {
        doc[key] = json!({});
    }
    let mut changed = false;
    for (name, server) in servers.as_object()? {
        if doc[key].get(name).is_none() {
            doc[key][name] = server.clone();
            changed = true;
        }
    }
    let mut bytes = serde_json::to_vec_pretty(&doc).ok()?;
    bytes.push(b'\n');
    Some(Ok((bytes, changed)))
}
/// Compare what this computer offers with what the probe saw on the host.
pub fn plan(local_home: &Path, local: &Local, probe: &Probe) -> (Vec<Planned>, Vec<Value>) {
    let home = local_home.display().to_string();
    let mut out = vec![];
    let mut skipped = local.skipped.clone();
    for e in &local.entries {
        let bytes = rewrite_home(&e.bytes, &home, &probe.home);
        let now = probe.files.get(&e.path).cloned().unwrap_or_else(|| "other".into());
        let kept_group = !e.group.is_empty() && probe.groups.get(&e.group).is_some_and(|g| g == "link" || g == "other");
        let action = if kept_group || now == "system" {
            "kept"
        } else if now == "absent" {
            "add"
        } else if now == "dir" || now == "other" {
            "skipped"
        } else if now == sha(&bytes) {
            "same"
        } else {
            "update"
        };
        out.push(Planned { item: e.item, path: e.path.clone(), group: e.group.clone(), bytes, mode: e.mode, expected: now, action });
    }
    for (path, key) in MERGES {
        let Some(host) = probe.merges.get(path) else { continue };
        match merged(local_home, path, key, host, &probe.home) {
            None => {}
            Some(Err(reason)) => skipped.push(json!({"item":"config","path":path,"reason":reason})),
            Some(Ok((bytes, changed))) => out.push(Planned {
                item: "config",
                path: path.into(),
                group: String::new(),
                bytes,
                mode: 0o600,
                expected: host.0.clone(),
                action: if host.0 == "system" { "kept" } else if !changed { "same" } else if host.0 == "absent" { "add" } else { "update" },
            }),
        }
    }
    (out, skipped)
}
fn merge_paths(local_home: &Path) -> Vec<(String, String)> {
    MERGES.iter().filter(|(p, k)| fs::read(local_home.join(p)).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok()).is_some_and(|v| v[*k].as_object().is_some_and(|o| !o.is_empty()))).map(|(p, k)| (p.to_string(), k.to_string())).collect()
}
/// What the vault step would do there: clone, pull, or why it cannot.
fn vault_plan(local: &Local, probe: &Probe) -> Value {
    let Some(v) = &local.vault else { return json!({"available":false,"error":local.vault_error}) };
    let same = crate::journal_vault::canonical_remote(&probe.vault_origin).is_some() && crate::journal_vault::canonical_remote(&probe.vault_origin) == crate::journal_vault::canonical_remote(&v.url);
    let state = probe.vault.clone().unwrap_or_default();
    let action = match (state.as_str(), same) {
        ("missing" | "empty", _) => "clone",
        ("clone", true) => "pull",
        ("clone", false) => "occupied",
        _ => "occupied",
    };
    let github = crate::journal_vault::canonical_remote(&v.url).is_some_and(|c| c.starts_with("github.com/"));
    json!({"available":true,"path":format!("~/{}",v.path),"remote":v.url,"hostState":state,"hostRemote":probe.vault_origin,"action":action,
        "access":probe.access,"github":github,"gh":probe.gh,"identity":!v.name.is_empty()&&!v.email.is_empty()})
}
fn summary(planned: &[Planned], skipped: &[Value], vault: Value, probe: &Probe, transport: &str) -> Value {
    let mut items = vec![];
    for id in ["instructions", "skills", "config"] {
        let mut counts = BTreeMap::from([("add", 0), ("update", 0), ("same", 0), ("kept", 0), ("skipped", 0)]);
        let mut rows: BTreeMap<String, (&str, usize)> = BTreeMap::new();
        for p in planned.iter().filter(|p| p.item == id) {
            *counts.get_mut(p.action).unwrap() += 1;
            // A skill shows as one row: kept, else update when any file changes, else add.
            let key = if p.group.is_empty() { p.path.clone() } else { p.group.clone() };
            let row = rows.entry(key).or_insert((p.action, 0));
            row.1 += 1;
            let rank = |a: &str| ["same", "skipped", "add", "update", "kept"].iter().position(|x| *x == a).unwrap_or(0);
            if rank(p.action) > rank(row.0) {
                row.0 = p.action;
            }
        }
        let left: Vec<&Value> = skipped.iter().filter(|s| s["item"] == id).collect();
        counts.insert("left", left.len());
        let entries: Vec<Value> = rows.iter().map(|(path, (action, files))| json!({"path":format!("~/{path}"),"action":action,"files":files})).chain(left.iter().map(|s| json!({"path":format!("~/{}",text(s,"path")),"action":"left","reason":s["reason"]}))).take(400).collect();
        let pending = counts["add"] + counts["update"];
        items.push(json!({"id":id,"available":!rows.is_empty()||!left.is_empty(),"pending":pending,"counts":counts,"entries":entries}));
    }
    let pending = matches!(text(&vault, "action"), "clone" | "pull") && vault["access"] == "ok";
    items.insert(0, json!({"id":"vault","available":vault["available"]==true,"pending":usize::from(pending),"vault":vault}));
    json!({"host":{"home":probe.home},"transport":transport,"items":items})
}

/// Runs the probe through the first session that answers.
fn probe_with(sessions: &[Session], input: &str) -> Result<(Session, Probe)> {
    let mut error = String::from("host_sync_needs_ssh");
    for s in sessions {
        match s.run(PROBE, input.as_bytes(), 120) {
            Ok((0, out)) => return Ok((s.clone(), parse_probe(&out)?)),
            Ok((124, _)) => error = "ssh_timeout".into(),
            Ok((255, _)) => error = "ssh_unavailable".into(),
            Ok(_) => error = "host_sync_probe_failed".into(),
            Err(e) => error = e,
        }
    }
    Err(error)
}
/// Preview: what each item would add, update or keep on the host. Read-only.
pub fn preview_with(sessions: &[Session], local_home: &Path, vault_config: Option<&Value>) -> Result<Value> {
    let local = collect(local_home, vault_config);
    let merges = merge_paths(local_home);
    let (session, probe) = probe_with(sessions, &probe_input(&local, &merges))?;
    let (planned, skipped) = plan(local_home, &local, &probe);
    Ok(summary(&planned, &skipped, vault_plan(&local, &probe), &probe, session.transport()))
}
fn stamp() -> String {
    chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string()
}
/// Builds the archive host-sync-apply.sh reads, with GNU tar from a private folder.
fn archive(planned: &[&Planned], vault: Option<(&Vault, &str, &str)>, stamp: &str) -> Result<Vec<u8>> {
    let dir = tempfile_dir()?;
    let result = (|| -> Result<Vec<u8>> {
        let mut plan = String::new();
        let mut group = String::new();
        for p in planned {
            if !p.group.is_empty() && p.group != group {
                group = p.group.clone();
                plan.push_str(&format!("G{SEP}{group}\n"));
            }
            let file = dir.join("files").join(&p.path);
            fs::create_dir_all(file.parent().ok_or("local_error")?).map_err(|_| "local_error")?;
            fs::write(&file, &p.bytes).map_err(|_| "local_error")?;
            fs::set_permissions(&file, fs::Permissions::from_mode(p.mode | 0o600)).map_err(|_| "local_error")?;
            plan.push_str(&format!("F{SEP}{}{SEP}{}\n", p.path, p.expected));
        }
        if let Some((v, mode, origin)) = vault {
            plan.push_str(&format!("V{SEP}{}{SEP}{}{SEP}{mode}{SEP}{origin}{SEP}{}{SEP}{}\n", v.path, v.url, v.name, v.email));
        }
        fs::create_dir_all(dir.join("files")).map_err(|_| "local_error")?;
        fs::write(dir.join("plan"), plan).map_err(|_| "local_error")?;
        fs::write(dir.join("stamp"), stamp).map_err(|_| "local_error")?;
        let out = Command::new("tar").args(["--format=pax", "-C"]).arg(&dir).args(["-cf", "-", "stamp", "plan", "files"]).stdin(Stdio::null()).stderr(Stdio::null()).output().map_err(|_| "local_error")?;
        if !out.status.success() {
            return Err("local_error".into());
        }
        Ok(out.stdout)
    })();
    let _ = fs::remove_dir_all(&dir);
    result
}
fn tempfile_dir() -> Result<PathBuf> {
    use std::os::unix::fs::DirBuilderExt;
    let dir = std::env::temp_dir().join(format!("ot3-host-sync-{}", uuid::Uuid::new_v4()));
    fs::DirBuilder::new().mode(0o700).create(&dir).map_err(|_| "local_error")?;
    Ok(dir)
}
/// Sync the chosen items: probe again, then one command writes every file whose
/// host copy did not change since, after backing up what it replaces.
pub fn apply_with(sessions: &[Session], local_home: &Path, vault_config: Option<&Value>, items: &[String]) -> Result<Value> {
    let chosen = |id: &str| items.iter().any(|i| i == id);
    if items.is_empty() || items.iter().any(|i| !ITEMS.contains(&i.as_str())) {
        return Err("host_sync_invalid_items".into());
    }
    let local = collect(local_home, vault_config);
    let merges = merge_paths(local_home);
    let (session, probe) = probe_with(sessions, &probe_input(&local, &merges))?;
    let (planned, skipped) = plan(local_home, &local, &probe);
    let vault = vault_plan(&local, &probe);
    let writes: Vec<&Planned> = planned.iter().filter(|p| chosen(p.item) && matches!(p.action, "add" | "update")).collect();
    let vault_step = local.vault.as_ref().filter(|_| chosen("vault") && matches!(text(&vault, "action"), "clone" | "pull") && vault["access"] == "ok").map(|v| (v, text(&vault, "action"), probe.vault_origin.as_str()));
    let mut results: BTreeMap<String, String> = BTreeMap::new();
    let mut vault_result = String::new();
    let mut backup = Value::Null;
    if !writes.is_empty() || vault_step.is_some() {
        let tar = archive(&writes, vault_step, &stamp())?;
        let (code, out) = session.run(APPLY, &tar, 600)?;
        if code != 0 {
            return Err(match code {
                124 => "ssh_timeout",
                255 => "ssh_unavailable",
                _ => "host_sync_apply_failed",
            }
            .into());
        }
        for line in String::from_utf8_lossy(&out).lines() {
            let f: Vec<&str> = line.split(SEP).collect();
            match (f[0], f.len()) {
                ("F", 3) => {
                    results.insert(f[1].into(), f[2].into());
                }
                ("V", 3) => vault_result = f[2].into(),
                ("B", 2) => backup = json!(f[1]),
                _ => {}
            }
        }
    }
    let mut items_out = vec![];
    for id in ["instructions", "skills", "config"] {
        if !chosen(id) {
            continue;
        }
        let mut counts = BTreeMap::from([("added", 0), ("updated", 0), ("unchanged", 0), ("kept", 0), ("changed", 0), ("failed", 0), ("left", 0)]);
        for p in planned.iter().filter(|p| p.item == id) {
            let key = match (p.action, results.get(&p.path).map(String::as_str)) {
                ("add", Some("ok")) => "added",
                ("update", Some("ok")) => "updated",
                ("add" | "update", Some("kept")) | ("kept", _) => "kept",
                ("add" | "update", Some("changed")) => "changed",
                ("add" | "update", _) => "failed",
                ("same", _) => "unchanged",
                _ => "left",
            };
            *counts.get_mut(key).unwrap() += 1;
        }
        *counts.get_mut("left").unwrap() += skipped.iter().filter(|s| s["item"] == id).count();
        let status = if counts["failed"] + counts["changed"] > 0 { if counts["added"] + counts["updated"] > 0 { "partial" } else { "failed" } } else if counts["added"] + counts["updated"] > 0 { "done" } else { "unchanged" };
        items_out.push(json!({"id":id,"status":status,"counts":counts}));
    }
    if chosen("vault") {
        let status = match (vault_result.as_str(), text(&vault, "action")) {
            ("cloned", _) => "cloned",
            ("pulled", _) => "pulled",
            ("", _) if vault["available"] != true => "unavailable",
            ("", "clone" | "pull") => if vault["access"] == "auth" { "auth" } else { "unreachable" },
            ("", _) => "occupied",
            (other, _) => match other { "conflict" => "conflict", "occupied" => "occupied", "nogit" => "nogit", _ => "failed" },
        };
        items_out.insert(0, json!({"id":"vault","status":status,"vault":vault}));
    }
    Ok(json!({"at":chrono::Utc::now().to_rfc3339(),"transport":session.transport(),"host":{"home":probe.home},"backup":backup,"items":items_out}))
}

// ---------------------------------------------------------------- panel

static BUSY: Mutex<()> = Mutex::new(());
fn state_path() -> PathBuf {
    let base = std::env::var_os("XDG_STATE_HOME").map(PathBuf::from).filter(|p| p.is_absolute()).unwrap_or_else(|| crate::journal_vault::Paths::current().home.join(".local/state"));
    base.join("omarchy-t3-code/host-sync.json")
}
fn remember(environment: &str, result: &Value) {
    let path = state_path();
    let mut doc: Value = fs::read(&path).ok().and_then(|b| serde_json::from_slice(&b).ok()).filter(Value::is_object).unwrap_or_else(|| json!({}));
    doc[environment] = json!({"at":result["at"],"items":result["items"].as_array().map(|a| a.iter().map(|i| json!({"id":i["id"],"status":i["status"],"counts":i["counts"]})).collect::<Vec<_>>()),"backup":result["backup"]});
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let tmp = path.with_extension(format!("tmp-{}", uuid::Uuid::new_v4()));
    if fs::write(&tmp, serde_json::to_vec_pretty(&doc).unwrap_or_default()).is_ok() {
        let _ = fs::rename(&tmp, &path);
    }
}
fn last() -> Value {
    fs::read(state_path()).ok().and_then(|b| serde_json::from_slice(&b).ok()).filter(Value::is_object).unwrap_or_else(|| json!({}))
}
fn vault_config() -> Option<Value> {
    let paths = crate::journal_vault::Paths::current();
    paths.config.exists().then(|| crate::journal_vault::load_config(&paths))
}
/// The terminal on the host for `gh auth login` and `gh auth setup-git`.
fn github_login(environment: &str) -> Result<Value> {
    let list = sessions(environment)?;
    let (session, probe) = probe_with(&list, "")?;
    let _ = probe;
    let args = session.terminal_args().ok_or("host_sync_needs_ssh")?;
    crate::provider_login::open_terminal("gh auth login --hostname github.com --git-protocol https --web && gh auth setup-git", Some(&args))?;
    Ok(json!({"opened":true}))
}
/// `host-sync`: `read` (last results), `preview`, `apply` (confirmed) and
/// `github-login` for one SSH environment.
pub fn execute(input: &Value) -> Result<Value> {
    let environment = text(input, "environment");
    match text(input, "action") {
        "read" => Ok(json!({"last":last()})),
        "preview" => {
            let _busy = BUSY.try_lock().map_err(|_| "host_sync_busy")?;
            let home = crate::journal_vault::Paths::current().home;
            let mut result = preview_with(&sessions(environment)?, &home, vault_config().as_ref())?;
            result["environment"] = json!(environment);
            result["last"] = last()[environment].clone();
            Ok(result)
        }
        "apply" => {
            if input["confirmed"] != true {
                return Err("confirmation_required".into());
            }
            let items: Vec<String> = input["items"].as_array().into_iter().flatten().filter_map(|v| v.as_str().map(str::to_owned)).collect();
            let _busy = BUSY.try_lock().map_err(|_| "host_sync_busy")?;
            let home = crate::journal_vault::Paths::current().home;
            let mut result = apply_with(&sessions(environment)?, &home, vault_config().as_ref(), &items)?;
            result["environment"] = json!(environment);
            remember(environment, &result);
            Ok(result)
        }
        "github-login" => github_login(environment),
        _ => Err("invalid_command".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tokens_and_literal_secrets_are_found_but_references_are_not() {
        for text in ["{\"apiKey\":\"literal-secret-value\"}", "token = \"abcdefghijkl\"", "x sk-ant-api03-abcdefghijklmnopqrstuvwxyz", "ghp_abcdefghijklmnopqrstuvwxyz0123456789", "-----BEGIN OPENSSH PRIVATE KEY-----", "AKIAABCDEFGHIJKLMNOP"] {
            assert!(has_secret(text, true), "{text}");
        }
        for text in ["{\"apiKey\":\"${OPENAI_KEY}\"}", "bearer_token_env_var = \"GITHUB_TOKEN\"", "\"max_tokens\": \"12345678\"", "\"model\":\"opus\"", "password: <your password>", "\"tokenUrl\":\"https://x/oauth/token\""] {
            assert!(!has_secret(text, true), "{text}");
        }
        assert!(!has_secret("Set `token: \"abcdefghijkl\"` in your config", false), "prose in skills only fails on real tokens");
    }
    #[test]
    fn only_whole_home_paths_are_rewritten() {
        let out = rewrite_home(b"/home/me /home/me/x \"/home/me\" /home/meow /x/home/me ~/y", "/home/me", "/Users/me");
        assert_eq!(String::from_utf8(out).unwrap(), "/Users/me /Users/me/x \"/Users/me\" /home/meow /x/home/me ~/y");
        assert_eq!(rewrite_home(&[0xff, 0x00], "/home/me", "/x"), vec![0xff, 0x00]);
    }
    #[test]
    fn omabridge_keeps_the_profiles_host_key_check() {
        let direct: Vec<String> = ["-o", "BatchMode=yes", "-l", "node", "--", "node"].iter().map(|s| s.to_string()).collect();
        let peer = "a".repeat(64);
        let args = bridged(&direct, "node.local", Path::new("/opt/omabridge"), Path::new("/s"), &peer).unwrap();
        let joined = args.join(" ");
        assert!(joined.contains("HostKeyAlias=node.local") && joined.contains(&format!("ProxyCommand='/opt/omabridge' --state-dir '/s' --name ot3 connect --peer {peer} --service ssh")));
        assert!(!joined.contains("StrictHostKeyChecking") && !joined.contains("UserKnownHostsFile"));
        assert_eq!(&args[args.len() - 2..], ["--", "node"]);
        assert!(bridged(&direct, "node", Path::new("/b"), Path::new("/s"), "not-a-peer").is_err());
    }
    #[test]
    fn a_file_changed_on_the_host_after_the_probe_is_not_overwritten() {
        let base = std::env::temp_dir().join(format!("ot3-host-sync-cas-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let home = base.join("there");
        fs::create_dir_all(home.join(".codex")).unwrap();
        fs::write(home.join(".codex/AGENTS.md"), "before\n").unwrap();
        let p = Planned { item: "instructions", path: ".codex/AGENTS.md".into(), group: String::new(), bytes: b"new\n".to_vec(), mode: 0o644, expected: sha(b"before\n"), action: "update" };
        let added = Planned { path: ".codex/NEW.md".into(), expected: "absent".into(), action: "add", ..p.clone() };
        fs::write(home.join(".codex/AGENTS.md"), "edited meanwhile\n").unwrap();
        let tar = archive(&[&p, &added], None, "20261028T000000Z").unwrap();
        let (code, out) = Session::Local { home: home.clone() }.run(APPLY, &tar, 30).unwrap();
        assert_eq!(code, 0);
        let out = String::from_utf8(out).unwrap();
        assert!(out.contains(&format!("F{SEP}.codex/AGENTS.md{SEP}changed")) && out.contains(&format!("F{SEP}.codex/NEW.md{SEP}ok")), "{out}");
        assert_eq!(fs::read_to_string(home.join(".codex/AGENTS.md")).unwrap(), "edited meanwhile\n");
        assert!(safe_rel("a/b") && !safe_rel("../a") && !safe_rel("/a") && !safe_rel("a//b") && !safe_rel("a/\nb"));
        let _ = fs::remove_dir_all(&base);
    }
}
