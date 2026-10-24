//! `projects.*` RPCs. The existing payload helpers in `crate::projects` stay
//! where they are and are re-exported here.
use super::{api_fn, ids, send, Function};
use crate::{Connection, Result};
use serde_json::{json, Value};

pub use crate::projects::{command, folder_path, folders};

/// T3 `PROJECT_READ_FILE_PATH_MAX_LENGTH`.
const READ_FILE_PATH_MAX: usize = 512;

pub const READ_FILE: Function = api_fn! {
    name: "projects.readFile", module: "projects", tag: Some("projects.readFile"),
    kind: Read, scope: Fixed("filesystem:read"),
    args: [(cwd, Path, required), (relativePath, Path, required), (allowMissing, Bool, optional)],
    payload: |a| read_file_payload(a.req("cwd")?, a.req("relativePath")?, a.bool("allowMissing")?),
};
pub const FUNCTIONS: &[Function] = &[READ_FILE];

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
