//! `review.*`: working-tree and branch-range diffs and the two sides of one file.
use super::reader::{one_of, Reader};
use super::{api_fn, ids, send, Function};
use crate::{Connection, Result};
use serde_json::{json, Value};

/// Bound for refs; ot3 uses the same for branch names.
const REF_MAX: usize = 1024;

pub const GET_DIFF_PREVIEW: Function = api_fn! {
    name: "review.getDiffPreview", module: "review", tag: Some("review.getDiffPreview"),
    kind: Read, scope: Fixed("filesystem:read"),
    args: [(cwd, Path, required), (baseRef, Text, optional), (ignoreWhitespace, Bool, optional), (file, Object, optional)],
    payload: |a| {
        let file = a.object("file")?.map(DiffFile::from_json).transpose()?;
        diff_preview_payload(a.req("cwd")?, a.str("baseRef")?, a.bool("ignoreWhitespace")?, file.as_ref())
    },
};
pub const GET_DIFF_FILE_CONTENTS: Function = api_fn! {
    name: "review.getDiffFileContents", module: "review", tag: Some("review.getDiffFileContents"),
    kind: Read, scope: Fixed("filesystem:read"),
    args: [
        (cwd, Path, required), (sourceKind, Enum(SOURCE_KINDS), required), (changeType, Enum(CHANGE_TYPES), required),
        (oldPath, Path, required), (newPath, Path, required), (baseRef, Text, optional), (headRef, Text, optional),
    ],
    payload: |a| FileContentsQuery::from_json(a.all())?.payload(),
};
pub const FUNCTIONS: &[Function] = &[GET_DIFF_PREVIEW, GET_DIFF_FILE_CONTENTS];

pub const SOURCE_KINDS: &[&str] = &["working-tree", "branch-range"];
pub const CHANGE_TYPES: &[&str] = &["change", "rename-pure", "rename-changed", "new", "deleted"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceKind {
    WorkingTree,
    BranchRange,
}
impl SourceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::WorkingTree => "working-tree",
            Self::BranchRange => "branch-range",
        }
    }
}
/// One file of a diff preview (`ReviewDiffPreviewInput.file`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffFile {
    pub path: String,
    pub previous_path: Option<String>,
    pub source_kind: SourceKind,
}
impl DiffFile {
    pub fn to_json(&self) -> Result<Value> {
        let invalid = |_| ids::invalid("file");
        let previous = self.previous_path.as_deref().map(|p| ids::path("file", p)).transpose().map_err(invalid)?;
        Ok(json!({"path":ids::path("file", &self.path).map_err(invalid)?,"previousPath":previous,"sourceKind":self.source_kind.as_str()}))
    }
    pub fn from_json(v: &Value) -> Result<Self> {
        let invalid = || ids::invalid("file");
        let o = v.as_object().ok_or_else(invalid)?;
        if o.len() != 3 || !o.contains_key("previousPath") {
            return Err(invalid());
        }
        let file = Self {
            path: v["path"].as_str().ok_or_else(invalid)?.into(),
            previous_path: match &v["previousPath"] {
                Value::Null => None,
                p => Some(p.as_str().ok_or_else(invalid)?.into()),
            },
            source_kind: match v["sourceKind"].as_str() {
                Some("working-tree") => SourceKind::WorkingTree,
                Some("branch-range") => SourceKind::BranchRange,
                _ => return Err(invalid()),
            },
        };
        file.to_json()?;
        Ok(file)
    }
}

/// `review.getDiffPreview`: `{cwd, generatedAt, sources[]}` for the working tree,
/// or for `base_ref..HEAD`, optionally narrowed to one file.
pub fn diff_preview(
    conn: &Connection,
    cwd: &str,
    base_ref: Option<&str>,
    ignore_whitespace: Option<bool>,
    file: Option<&DiffFile>,
) -> Result<Value> {
    send(conn, &GET_DIFF_PREVIEW, diff_preview_payload(cwd, base_ref, ignore_whitespace, file)?)
}
fn diff_preview_payload(
    cwd: &str,
    base_ref: Option<&str>,
    ignore_whitespace: Option<bool>,
    file: Option<&DiffFile>,
) -> Result<Value> {
    let mut payload = json!({"cwd":ids::path("cwd", cwd)?});
    if let Some(base) = base_ref {
        payload["baseRef"] = json!(ids::text("baseRef", base, REF_MAX)?);
    }
    if let Some(ignore) = ignore_whitespace {
        payload["ignoreWhitespace"] = json!(ignore);
    }
    if let Some(file) = file {
        payload["file"] = file.to_json()?;
    }
    Ok(payload)
}

/// `ReviewDiffFileContentsInput`. `base_ref`/`head_ref` are sent even when `None`
/// (as `null`): T3 requires both keys.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileContentsQuery {
    pub cwd: String,
    pub source_kind: SourceKind,
    /// `change`, `rename-pure`, `rename-changed`, `new` or `deleted`.
    pub change_type: String,
    pub base_ref: Option<String>,
    pub head_ref: Option<String>,
    pub old_path: String,
    pub new_path: String,
}
impl FileContentsQuery {
    pub fn payload(&self) -> Result<Value> {
        let reference = |field: &str, r: &Option<String>| -> Result<Option<String>> {
            r.as_deref().map(|r| ids::text(field, r, REF_MAX).map(str::to_string)).transpose()
        };
        Ok(json!({
            "cwd":ids::path("cwd", &self.cwd)?,
            "sourceKind":self.source_kind.as_str(),
            "changeType":one_of("changeType", &self.change_type, CHANGE_TYPES)?,
            "baseRef":reference("baseRef", &self.base_ref)?,
            "headRef":reference("headRef", &self.head_ref)?,
            "oldPath":ids::path("oldPath", &self.old_path)?,
            "newPath":ids::path("newPath", &self.new_path)?,
        }))
    }
    /// An absent `baseRef`/`headRef` is sent as `null`.
    pub fn from_json(v: &Value) -> Result<Self> {
        let mut r = Reader::new(v)?;
        let nullable = |r: &mut Reader, k: &'static str| r.opt_str(k);
        let query = Self {
            cwd: r.req_str("cwd")?,
            source_kind: match r.req_str("sourceKind")?.as_str() {
                "working-tree" => SourceKind::WorkingTree,
                "branch-range" => SourceKind::BranchRange,
                _ => return Err(ids::invalid("sourceKind")),
            },
            change_type: r.req_str("changeType")?,
            base_ref: nullable(&mut r, "baseRef")?,
            head_ref: nullable(&mut r, "headRef")?,
            old_path: r.req_str("oldPath")?,
            new_path: r.req_str("newPath")?,
        };
        r.done()?;
        Ok(query)
    }
}
/// `review.getDiffFileContents`: `{oldContents, newContents}`.
pub fn diff_file_contents(conn: &Connection, query: &FileContentsQuery) -> Result<Value> {
    send(conn, &GET_DIFF_FILE_CONTENTS, query.payload()?)
}
