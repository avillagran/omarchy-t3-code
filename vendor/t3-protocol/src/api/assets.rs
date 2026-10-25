//! `assets.createUrl`: short-lived signed same-origin URLs for files, attachments
//! and icons. `Connection::document` and the image readers use the same grant.
//! Also `attachments.delete`. Uploads (`attachments.createUploadUrl`) stay in
//! `Connection::upload`, which streams the file.
use super::{api_fn, ids, send, Function};
use crate::{Connection, Result};
use serde_json::{json, Map, Value};

/// T3 `ASSET_PATH_MAX_LENGTH`.
const PATH_MAX: usize = 1024;

pub const CREATE_URL: Function = api_fn! {
    name: "assets.createUrl", module: "assets", tag: Some("assets.createUrl"),
    kind: Read, scope: Dynamic(&["filesystem:read", "orchestration:read"]),
    args: [(resource, Object, required)],
    payload: |a| create_url_payload(&AssetResource::from_json(a.object("resource")?.ok_or_else(|| ids::invalid("resource"))?)?),
};
pub const DELETE_ATTACHMENT: Function = api_fn! {
    name: "attachments.delete", module: "assets", tag: Some("attachments.delete"),
    kind: IdempotentMutation, scope: Fixed("orchestration:operate"),
    args: [(attachmentId, Token, required)],
    payload: |a| delete_attachment_payload(a.req("attachmentId")?),
};
pub const FUNCTIONS: &[Function] = &[CREATE_URL, DELETE_ATTACHMENT];

/// What to sign (`AssetResource`, a union by `_tag`).
#[derive(Clone, Debug, PartialEq)]
pub enum AssetResource {
    WorkspaceFile { thread_id: String, path: String },
    MediaFile { thread_id: String, path: String },
    DraftWorkspaceFile { cwd: String, path: String },
    Attachment {
        attachment_id: String,
        file_name: Option<String>,
        mime_type: Option<String>,
        /// `inline` or `attachment`.
        disposition: Option<String>,
        /// Only `mcp-app`.
        render_intent: Option<String>,
    },
    ToolOutputImage { thread_id: String, item_id: String, index: u64 },
    ProjectFavicon { cwd: String, path: Option<String> },
    /// `app` is a `ToolActivityNativeAppReference` object, passed as is.
    NativeAppIcon { app: Value },
    GithubMedia { cwd: String, url: String },
}
fn bounded_path<'a>(field: &str, value: &'a str) -> Result<&'a str> {
    let path = ids::path(field, value)?;
    if path.chars().count() > PATH_MAX {
        return Err(ids::invalid(field));
    }
    Ok(path)
}
impl AssetResource {
    /// The scope T3 requires for this resource (`RpcAuthorization.ts`, assets.createUrl).
    pub fn required_scope(&self) -> &'static str {
        match self {
            Self::WorkspaceFile { .. } | Self::MediaFile { .. } | Self::DraftWorkspaceFile { .. } => "filesystem:read",
            _ => "orchestration:read",
        }
    }
    pub fn to_json(&self) -> Result<Value> {
        Ok(match self {
            Self::WorkspaceFile { thread_id, path } => {
                json!({"_tag":"workspace-file","threadId":ids::id("threadId", thread_id)?,"path":bounded_path("path", path)?})
            }
            Self::MediaFile { thread_id, path } => {
                json!({"_tag":"media-file","threadId":ids::id("threadId", thread_id)?,"path":bounded_path("path", path)?})
            }
            Self::DraftWorkspaceFile { cwd, path } => {
                json!({"_tag":"draft-workspace-file","cwd":bounded_path("cwd", cwd)?,"path":bounded_path("path", path)?})
            }
            Self::Attachment { attachment_id, file_name, mime_type, disposition, render_intent } => {
                let mut v = json!({"_tag":"attachment","attachmentId":ids::token("attachmentId", attachment_id, 256)?});
                if let Some(name) = file_name {
                    v["fileName"] = json!(ids::text("fileName", name, 255)?);
                }
                if let Some(mime) = mime_type {
                    v["mimeType"] = json!(ids::text("mimeType", mime, 100)?);
                }
                if let Some(d) = disposition {
                    if !matches!(d.as_str(), "inline" | "attachment") {
                        return Err(ids::invalid("disposition"));
                    }
                    v["disposition"] = json!(d);
                }
                if let Some(intent) = render_intent {
                    if intent != "mcp-app" {
                        return Err(ids::invalid("renderIntent"));
                    }
                    v["renderIntent"] = json!(intent);
                }
                v
            }
            Self::ToolOutputImage { thread_id, item_id, index } => {
                json!({"_tag":"tool-output-image","threadId":ids::id("threadId", thread_id)?,"itemId":ids::id("itemId", item_id)?,"index":index})
            }
            Self::ProjectFavicon { cwd, path } => {
                let mut v = json!({"_tag":"project-favicon","cwd":bounded_path("cwd", cwd)?});
                if let Some(p) = path {
                    v["path"] = json!(bounded_path("path", p)?);
                }
                v
            }
            Self::NativeAppIcon { app } => {
                if !app.get("_tag").is_some_and(Value::is_string) {
                    return Err(ids::invalid("app"));
                }
                json!({"_tag":"native-app-icon","app":app})
            }
            Self::GithubMedia { cwd, url } => {
                json!({"_tag":"github-media","cwd":bounded_path("cwd", cwd)?,"url":ids::text("url", url, 2048)?})
            }
        })
    }
    pub fn from_json(v: &Value) -> Result<Self> {
        let o = v.as_object().ok_or_else(|| ids::invalid("resource"))?;
        let keys = |allowed: &[&str]| -> Result<()> {
            if o.keys().all(|k| k == "_tag" || allowed.contains(&k.as_str())) {
                Ok(())
            } else {
                Err("invalid_input".into())
            }
        };
        let s = |k: &str| -> Result<String> { o.get(k).and_then(Value::as_str).map(str::to_string).ok_or_else(|| ids::invalid(k)) };
        let opt = |k: &str| -> Result<Option<String>> { optional(o, k) };
        let resource = match o.get("_tag").and_then(Value::as_str) {
            Some("workspace-file") => {
                keys(&["threadId", "path"])?;
                Self::WorkspaceFile { thread_id: s("threadId")?, path: s("path")? }
            }
            Some("media-file") => {
                keys(&["threadId", "path"])?;
                Self::MediaFile { thread_id: s("threadId")?, path: s("path")? }
            }
            Some("draft-workspace-file") => {
                keys(&["cwd", "path"])?;
                Self::DraftWorkspaceFile { cwd: s("cwd")?, path: s("path")? }
            }
            Some("attachment") => {
                keys(&["attachmentId", "fileName", "mimeType", "disposition", "renderIntent"])?;
                Self::Attachment {
                    attachment_id: s("attachmentId")?,
                    file_name: opt("fileName")?,
                    mime_type: opt("mimeType")?,
                    disposition: opt("disposition")?,
                    render_intent: opt("renderIntent")?,
                }
            }
            Some("tool-output-image") => {
                keys(&["threadId", "itemId", "index"])?;
                Self::ToolOutputImage {
                    thread_id: s("threadId")?,
                    item_id: s("itemId")?,
                    index: o.get("index").and_then(Value::as_u64).ok_or_else(|| ids::invalid("index"))?,
                }
            }
            Some("project-favicon") => {
                keys(&["cwd", "path"])?;
                Self::ProjectFavicon { cwd: s("cwd")?, path: opt("path")? }
            }
            Some("native-app-icon") => {
                keys(&["app"])?;
                Self::NativeAppIcon { app: o.get("app").filter(|a| a.is_object()).cloned().ok_or_else(|| ids::invalid("app"))? }
            }
            Some("github-media") => {
                keys(&["cwd", "url"])?;
                Self::GithubMedia { cwd: s("cwd")?, url: s("url")? }
            }
            _ => return Err(ids::invalid("resource")),
        };
        resource.to_json()?;
        Ok(resource)
    }
}
fn optional(o: &Map<String, Value>, k: &str) -> Result<Option<String>> {
    match o.get(k) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(ids::invalid(k)),
    }
}

/// `assets.createUrl`: `{relativeUrl, expiresAt, sourcePath?, imageDimensions?}`.
/// The URL is relative to the connection's origin and needs no credential.
pub fn create_url(conn: &Connection, resource: &AssetResource) -> Result<Value> {
    send(conn, &CREATE_URL, create_url_payload(resource)?)
}
fn create_url_payload(resource: &AssetResource) -> Result<Value> {
    Ok(json!({"resource":resource.to_json()?}))
}
/// The grant behind `Connection::document` and the image readers, which keep
/// their own (older, looser) resource checks.
pub(crate) fn grant(conn: &Connection, resource: Value) -> Result<Value> {
    send(conn, &CREATE_URL, json!({"resource":resource}))
}
/// `attachments.delete`: `null`. Deleting a deleted attachment changes nothing.
pub fn delete_attachment(conn: &Connection, attachment_id: &str) -> Result<Value> {
    send(conn, &DELETE_ATTACHMENT, delete_attachment_payload(attachment_id)?)
}
fn delete_attachment_payload(attachment_id: &str) -> Result<Value> {
    Ok(json!({"attachmentId":ids::token("attachmentId", attachment_id, 256)?}))
}
