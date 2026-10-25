//! `filesystem.*`: directory browsing and metadata on the server's host.
//! `projects::folders` post-processes `browse`.
use super::reader::put;
use super::{api_fn, ids, send, Function};
use crate::{Connection, Result};
use serde_json::{json, Value};

/// T3 `FILESYSTEM_PATH_MAX_LENGTH` and `FILESYSTEM_METADATA_BATCH_LIMIT`.
const PATH_MAX: usize = 512;
const METADATA_BATCH_MAX: usize = 64;

pub const BROWSE: Function = api_fn! {
    name: "filesystem.browse", module: "filesystem", tag: Some("filesystem.browse"),
    kind: Read, scope: Fixed("filesystem:read"),
    args: [(partialPath, Path, required), (cwd, Path, optional)],
    payload: |a| browse_payload(a.req("partialPath")?, a.str("cwd")?),
};
pub const GET_METADATA: Function = api_fn! {
    name: "filesystem.getMetadata", module: "filesystem", tag: Some("filesystem.getMetadata"),
    kind: Read, scope: Fixed("filesystem:read"),
    args: [(paths, Array, required)],
    payload: |a| metadata_payload(&super::reader::strings("paths", a.req_array("paths")?)?),
};
pub const FUNCTIONS: &[Function] = &[BROWSE, GET_METADATA];

fn bounded<'a>(field: &str, value: &'a str) -> Result<&'a str> {
    let path = ids::path(field, value)?.trim();
    if path.chars().count() > PATH_MAX {
        return Err(ids::invalid(field));
    }
    Ok(path)
}
/// `filesystem.browse`: `{parentPath, entries[{name, fullPath}]}` completing `partial_path`.
pub fn browse(conn: &Connection, partial_path: &str, cwd: Option<&str>) -> Result<Value> {
    send(conn, &BROWSE, browse_payload(partial_path, cwd)?)
}
fn browse_payload(partial_path: &str, cwd: Option<&str>) -> Result<Value> {
    let mut v = json!({"partialPath":bounded("partialPath", partial_path)?});
    put(&mut v, "cwd", cwd.map(|c| bounded("cwd", c)).transpose()?);
    Ok(v)
}
/// `filesystem.getMetadata`: `{entries: (Metadata | null)[]}` in request order, for
/// 1..=64 absolute host paths (`~/` allowed).
pub fn metadata(conn: &Connection, paths: &[String]) -> Result<Value> {
    send(conn, &GET_METADATA, metadata_payload(paths)?)
}
fn metadata_payload(paths: &[String]) -> Result<Value> {
    if paths.is_empty() || paths.len() > METADATA_BATCH_MAX {
        return Err(ids::invalid("paths"));
    }
    let paths = paths.iter().map(|p| bounded("paths", p)).collect::<Result<Vec<_>>>()?;
    Ok(json!({"paths":paths}))
}
