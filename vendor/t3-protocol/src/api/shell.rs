//! `shell.openInEditor`: open a directory in an editor on the server's host.
use super::reader::{one_of, put};
use super::{api_fn, ids, send, Function};
use crate::{Connection, Result};
use serde_json::{json, Value};

/// T3 `EditorId`.
pub const EDITORS: &[&str] = &[
    "cursor", "trae", "kiro", "vscode", "vscode-insiders", "vscodium", "zed", "antigravity", "idea", "aqua", "clion",
    "datagrip", "dataspell", "goland", "phpstorm", "pycharm", "rider", "rubymine", "rustrover", "webstorm", "file-manager",
];

pub const OPEN_IN_EDITOR: Function = api_fn! {
    name: "shell.openInEditor", module: "shell", tag: Some("shell.openInEditor"),
    kind: UnrepeatableMutation, scope: Fixed("orchestration:operate"),
    args: [(cwd, Path, required), (editor, Enum(EDITORS), required), (reveal, Bool, optional)],
    payload: |a| open_in_editor_payload(a.req("cwd")?, a.req("editor")?, a.bool("reveal")?),
};
pub const FUNCTIONS: &[Function] = &[OPEN_IN_EDITOR];

/// `shell.openInEditor`: `null`. Opens a window on the server's machine; only
/// `file-manager` honors `reveal` (check the config flag `shellRevealInFileManager`).
pub fn open_in_editor(conn: &Connection, cwd: &str, editor: &str, reveal: Option<bool>) -> Result<Value> {
    send(conn, &OPEN_IN_EDITOR, open_in_editor_payload(cwd, editor, reveal)?)
}
fn open_in_editor_payload(cwd: &str, editor: &str, reveal: Option<bool>) -> Result<Value> {
    let mut v = json!({"cwd":ids::path("cwd", cwd)?.trim(),"editor":one_of("editor", editor, EDITORS)?});
    put(&mut v, "reveal", reveal);
    Ok(v)
}
