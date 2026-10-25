//! `projects.*` RPCs. The existing JSON helpers in `crate::projects` stay
//! where they are and are re-exported here; `ProjectMutation::from_command`
//! types the output of `projects::command`.
use super::reader::{int, one_of, put, Reader};
use super::{api_fn, ids, send, ArgType, Function};
use crate::{Connection, Result};
use serde_json::{json, Value};

pub use crate::projects::{command, folder_path, folders};

/// T3 `PROJECT_READ_FILE_PATH_MAX_LENGTH`.
const READ_FILE_PATH_MAX: usize = 512;
/// T3 `PROJECT_SEARCH_ENTRIES_MAX_LIMIT`.
const SEARCH_LIMIT_MAX: u64 = 200;
const TITLE_MAX: usize = 255;
pub const ENTRY_KINDS: &[&str] = &["file", "directory"];
pub const MUTATION_TYPES: &[&str] = &["project.create", "project.update", "project.delete"];
pub const THREAD_ENV_MODES: &[&str] = &["local", "worktree"];
pub const SCRIPT_ICONS: &[&str] = &["play", "test", "lint", "configure", "build", "debug"];
pub const ICON_COLORS: &[&str] = &[
    "gray", "red", "orange", "amber", "yellow", "lime", "green", "emerald", "teal", "cyan", "sky", "blue", "indigo",
    "violet", "purple", "fuchsia", "pink", "rose",
];

pub const READ_FILE: Function = api_fn! {
    name: "projects.readFile", module: "projects", tag: Some("projects.readFile"),
    kind: Read, scope: Fixed("filesystem:read"),
    args: [(cwd, Path, required), (relativePath, Path, required), (allowMissing, Bool, optional)],
    payload: |a| read_file_payload(a.req("cwd")?, a.req("relativePath")?, a.bool("allowMissing")?),
};
pub const SEARCH_ENTRIES: Function = api_fn! {
    name: "projects.searchEntries", module: "projects", tag: Some("projects.searchEntries"),
    kind: Read, scope: Fixed("filesystem:read"),
    args: [(cwd, Path, required), (query, Text, required), (limit, Integer, required), (kind, Enum(ENTRY_KINDS), optional), (imageOnly, Bool, optional)],
    payload: |a| search_entries_payload(a.req("cwd")?, a.req("query")?, a.req_u64("limit")?, a.str("kind")?, a.bool("imageOnly")?),
};
/// Scripts also need `settings:write` (T3 `requiredScopesForProjectMutation`).
pub const MUTATE: Function = api_fn! {
    name: "projects.mutate", module: "projects", tag: Some("projects.mutate"),
    kind: MutationWithCommandId, scope: Dynamic(&["orchestration:operate", "settings:write"]),
    args: [(type, ArgType::Enum(MUTATION_TYPES), required), (commandId, Id, required), (projectId, Id, required)],
    payload: |a| ProjectMutation::from_json(a.all())?.payload(),
    open_args: true, variants: MUTATION_TYPES,
};
pub const CREATE_NEW: Function = api_fn! {
    name: "projects.createNew", module: "projects", tag: Some("projects.createNew"),
    kind: UnrepeatableMutation, scope: Fixed("orchestration:operate"),
    args: [(name, Text, required)],
    payload: |a| create_new_payload(a.req("name")?),
};
pub const FUNCTIONS: &[Function] = &[READ_FILE, SEARCH_ENTRIES, MUTATE, CREATE_NEW];

/// `projects.readFile`: `{relativePath, contents, byteLength, truncated, exists?}`.
/// `relative_path` is workspace-relative or an absolute host path.
pub fn read_file(conn: &Connection, cwd: &str, relative_path: &str, allow_missing: Option<bool>) -> Result<Value> {
    send(conn, &READ_FILE, read_file_payload(cwd, relative_path, allow_missing)?)
}
fn read_file_payload(cwd: &str, relative_path: &str, allow_missing: Option<bool>) -> Result<Value> {
    let path = ids::path("relativePath", relative_path)?;
    if path.chars().count() > READ_FILE_PATH_MAX {
        return Err(ids::invalid("relativePath"));
    }
    let mut payload = json!({"cwd":ids::path("cwd", cwd)?,"relativePath":path});
    if let Some(allow) = allow_missing {
        payload["allowMissing"] = json!(allow);
    }
    Ok(payload)
}
/// `projects.searchEntries`: `{entries[{path, kind, ignored?}], truncated}`. `query`
/// may be blank (≤ 256); `limit` is 1..=200. `composer::paths` post-processes it.
pub fn search_entries(conn: &Connection, cwd: &str, query: &str, limit: u64, kind: Option<&str>, image_only: Option<bool>) -> Result<Value> {
    send(conn, &SEARCH_ENTRIES, search_entries_payload(cwd, query, limit, kind, image_only)?)
}
fn search_entries_payload(cwd: &str, query: &str, limit: u64, kind: Option<&str>, image_only: Option<bool>) -> Result<Value> {
    let query = query.trim();
    if query.chars().count() > 256 || query.chars().any(char::is_control) {
        return Err(ids::invalid("query"));
    }
    let mut v = json!({"cwd":ids::path("cwd", cwd)?,"query":query,"limit":int("limit", limit, 1, SEARCH_LIMIT_MAX)?});
    put(&mut v, "kind", kind.map(|k| one_of("kind", k, ENTRY_KINDS)).transpose()?);
    put(&mut v, "imageOnly", image_only);
    Ok(v)
}

/// `project.update` fields. `Some(Value::Null)` / `Some(None)` clears a nullable field.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProjectUpdate {
    pub title: Option<String>,
    pub workspace_root: Option<String>,
    /// `ModelSelection` object, or `Value::Null` to clear.
    pub default_model_selection: Option<Value>,
    pub auto_pull: Option<bool>,
    /// `{kind:"lucide",name,color}`, `{kind:"emoji",emoji}`, `{kind:"monogram",text,color}`, or `Value::Null`.
    pub project_icon: Option<Value>,
    pub favicon_path: Option<Option<String>>,
    /// `local` or `worktree`.
    pub default_thread_env_mode: Option<Option<String>>,
    /// `ProjectScript` objects. Sending them needs `settings:write` too.
    pub scripts: Option<Vec<Value>>,
}
/// `ProjectMutation`, deduplicated by `commandId` (T3 `ProjectService`).
#[derive(Clone, Debug, PartialEq)]
pub enum ProjectMutation {
    Create {
        command_id: String,
        project_id: String,
        title: String,
        workspace_root: String,
        create_workspace_root_if_missing: Option<bool>,
        /// `ModelSelection` object, or `Value::Null`.
        default_model_selection: Option<Value>,
        scripts: Option<Vec<Value>>,
    },
    Update { command_id: String, project_id: String, update: ProjectUpdate },
    Delete { command_id: String, project_id: String, force: Option<bool> },
}
impl ProjectMutation {
    /// Type the output of `projects::command` (rename, settings, remove) with the host's `commandId`.
    pub fn from_command(command_id: &str, built: &Value) -> Result<Self> {
        let mut v = built.clone();
        v["commandId"] = json!(command_id);
        Self::from_json(&v)
    }
    /// The scopes T3 requires for this mutation.
    pub fn required_scopes(&self) -> &'static [&'static str] {
        let scripts = match self {
            Self::Create { scripts, .. } => scripts.is_some(),
            Self::Update { update, .. } => update.scripts.is_some(),
            Self::Delete { .. } => false,
        };
        if scripts {
            &["orchestration:operate", "settings:write"]
        } else {
            &["orchestration:operate"]
        }
    }
    pub fn payload(&self) -> Result<Value> {
        let base = |kind: &str, command_id: &str, project_id: &str| -> Result<Value> {
            Ok(json!({"type":kind,"commandId":ids::id("commandId", command_id)?,"projectId":ids::id("projectId", project_id)?}))
        };
        Ok(match self {
            Self::Create { command_id, project_id, title, workspace_root, create_workspace_root_if_missing, default_model_selection, scripts } => {
                let mut v = base("project.create", command_id, project_id)?;
                v["title"] = json!(ids::text("title", title, TITLE_MAX)?);
                v["workspaceRoot"] = json!(ids::path("workspaceRoot", workspace_root)?.trim());
                put(&mut v, "createWorkspaceRootIfMissing", *create_workspace_root_if_missing);
                put(&mut v, "defaultModelSelection", default_model_selection.as_ref().map(model_selection).transpose()?);
                put(&mut v, "scripts", scripts.as_deref().map(project_scripts).transpose()?);
                v
            }
            Self::Update { command_id, project_id, update: u } => {
                let mut v = base("project.update", command_id, project_id)?;
                put(&mut v, "title", u.title.as_deref().map(|t| ids::text("title", t, TITLE_MAX)).transpose()?);
                put(&mut v, "workspaceRoot", u.workspace_root.as_deref().map(|p| ids::path("workspaceRoot", p).map(str::trim)).transpose()?);
                put(&mut v, "defaultModelSelection", u.default_model_selection.as_ref().map(model_selection).transpose()?);
                put(&mut v, "autoPull", u.auto_pull);
                put(&mut v, "projectIcon", u.project_icon.as_ref().map(project_icon).transpose()?);
                if let Some(path) = &u.favicon_path {
                    v["faviconPath"] = json!(path.as_deref().map(|p| ids::path("faviconPath", p).map(str::trim)).transpose()?);
                }
                if let Some(mode) = &u.default_thread_env_mode {
                    v["defaultThreadEnvMode"] = json!(mode.as_deref().map(|m| one_of("defaultThreadEnvMode", m, THREAD_ENV_MODES)).transpose()?);
                }
                put(&mut v, "scripts", u.scripts.as_deref().map(project_scripts).transpose()?);
                v
            }
            Self::Delete { command_id, project_id, force } => {
                let mut v = base("project.delete", command_id, project_id)?;
                put(&mut v, "force", *force);
                v
            }
        })
    }
    pub fn from_json(v: &Value) -> Result<Self> {
        let mut r = Reader::new(v)?;
        let kind = r.req_str("type").map_err(|_| ids::invalid("type"))?;
        let command_id = r.req_str("commandId")?;
        let project_id = r.req_str("projectId")?;
        let mutation = match kind.as_str() {
            "project.create" => Self::Create {
                command_id,
                project_id,
                title: r.req_str("title")?,
                workspace_root: r.req_str("workspaceRoot")?,
                create_workspace_root_if_missing: r.opt_bool("createWorkspaceRootIfMissing")?,
                default_model_selection: r.nullable_value("defaultModelSelection"),
                scripts: r.opt_array("scripts")?,
            },
            "project.update" => Self::Update {
                command_id,
                project_id,
                update: ProjectUpdate {
                    title: r.opt_str("title")?,
                    workspace_root: r.opt_str("workspaceRoot")?,
                    default_model_selection: r.nullable_value("defaultModelSelection"),
                    auto_pull: r.opt_bool("autoPull")?,
                    project_icon: r.nullable_value("projectIcon"),
                    favicon_path: r.nullable_str("faviconPath")?,
                    default_thread_env_mode: r.nullable_str("defaultThreadEnvMode")?,
                    scripts: r.opt_array("scripts")?,
                },
            },
            "project.delete" => Self::Delete { command_id, project_id, force: r.opt_bool("force")? },
            _ => return Err(ids::invalid("type")),
        };
        r.done()?;
        mutation.payload()?;
        Ok(mutation)
    }
}
fn model_selection(v: &Value) -> Result<Value> {
    if v.is_null() || v.get("model").is_some_and(Value::is_string) {
        Ok(v.clone())
    } else {
        Err(ids::invalid("defaultModelSelection"))
    }
}
/// `ProjectScript[]`: `{id, name, command, icon, runOnWorktreeCreate, runOnSettle?, async?, previewUrl?, autoOpenPreview?}`.
fn project_scripts(scripts: &[Value]) -> Result<Value> {
    let invalid = || ids::invalid("scripts");
    let mut out = Vec::with_capacity(scripts.len());
    for script in scripts.iter().take(256) {
        let mut r = Reader::of("scripts", script)?;
        let mut v = json!({});
        for k in ["id", "name", "command"] {
            let s = r.raw(k).and_then(Value::as_str).ok_or_else(invalid)?;
            v[k] = json!(ids::text("scripts", s, 16_384)?);
        }
        v["icon"] = json!(one_of("scripts", r.raw("icon").and_then(Value::as_str).unwrap_or(""), SCRIPT_ICONS)?);
        v["runOnWorktreeCreate"] = json!(r.req_bool("runOnWorktreeCreate").map_err(|_| invalid())?);
        for k in ["runOnSettle", "async", "autoOpenPreview"] {
            put(&mut v, k, r.opt_bool(k).map_err(|_| invalid())?);
        }
        if let Some(url) = r.opt_str("previewUrl").map_err(|_| invalid())? {
            v["previewUrl"] = json!(ids::text("scripts", &url, 2048)?);
        }
        r.done().map_err(|_| invalid())?;
        out.push(v);
    }
    if out.len() != scripts.len() {
        return Err(invalid());
    }
    Ok(json!(out))
}
/// `ProjectIconOverride` or `null`.
fn project_icon(v: &Value) -> Result<Value> {
    if v.is_null() {
        return Ok(Value::Null);
    }
    let invalid = || ids::invalid("projectIcon");
    let mut r = Reader::of("projectIcon", v)?;
    let mut s = |k: &'static str, max: usize| -> Result<String> {
        let s = r.raw(k).and_then(Value::as_str).ok_or_else(invalid)?;
        ids::text("projectIcon", s, max).map(str::to_string)
    };
    let icon = match s("kind", 16)?.as_str() {
        "lucide" => {
            let name = s("name", 64)?;
            let ok = name.split('-').all(|p| !p.is_empty() && p.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()));
            if !ok {
                return Err(invalid());
            }
            json!({"kind":"lucide","name":name,"color":one_of("projectIcon", &s("color", 16)?, ICON_COLORS)?})
        }
        "emoji" => json!({"kind":"emoji","emoji":s("emoji", 32)?}),
        "monogram" => {
            let text = s("text", 32)?;
            if !text.chars().next().is_some_and(char::is_alphanumeric) {
                return Err(invalid());
            }
            json!({"kind":"monogram","text":text,"color":one_of("projectIcon", &s("color", 16)?, ICON_COLORS)?})
        }
        _ => return Err(invalid()),
    };
    r.done().map_err(|_| invalid())?;
    Ok(icon)
}
/// `projects.mutate`: the resulting `Project`. Deduplicated by `commandId`:
/// after `rpc_outcome_unknown`, resend the same mutation.
pub fn mutate(conn: &Connection, mutation: &ProjectMutation) -> Result<Value> {
    send(conn, &MUTATE, mutation.payload()?)
}
/// `projects.createNew`: `{projectId, workspaceRoot, commitError?}` for a new
/// folder under the server's projects root. Not repeatable.
pub fn create_new(conn: &Connection, name: &str) -> Result<Value> {
    send(conn, &CREATE_NEW, create_new_payload(name)?)
}
fn create_new_payload(name: &str) -> Result<Value> {
    Ok(json!({"name":ids::text("name", name, 200)?}))
}
