//! `projectClone.start`: clone a repository into a new project on the server.
use super::reader::{one_of, put, timestamp, Reader};
use super::{api_fn, ids, send, Function};
use crate::{Connection, Result};
use serde_json::{json, Value};

pub const PROTOCOLS: &[&str] = &["auto", "ssh", "https"];

pub const START: Function = api_fn! {
    name: "projectClone.start", module: "projectClone", tag: Some("projectClone.start"),
    kind: UnrepeatableMutation, scope: Fixed("source-control:write"),
    args: [
        (projectId, Id, required), (title, Text, required), (createdAt, Text, required), (destinationPath, Path, required),
        (provider, Slug, optional), (repository, Text, optional), (remoteUrl, Text, optional), (protocol, Enum(PROTOCOLS), optional),
    ],
    payload: |a| CloneStart::from_json(a.all())?.payload(),
};
pub const FUNCTIONS: &[Function] = &[START];

/// `ProjectCloneStartInput`. Name the source with `repository` (`owner/name` on
/// `provider`) or `remote_url`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CloneStart {
    pub project_id: String,
    pub title: String,
    /// ISO 8601.
    pub created_at: String,
    pub destination_path: String,
    /// A `SourceControlProviderKind` such as `github`.
    pub provider: Option<String>,
    pub repository: Option<String>,
    pub remote_url: Option<String>,
    /// `auto`, `ssh` or `https`.
    pub protocol: Option<String>,
}
impl CloneStart {
    pub fn payload(&self) -> Result<Value> {
        let mut v = json!({
            "projectId":ids::id("projectId", &self.project_id)?,
            "title":ids::text("title", &self.title, 255)?,
            "createdAt":timestamp("createdAt", &self.created_at)?,
            "destinationPath":ids::path("destinationPath", &self.destination_path)?.trim(),
        });
        if let Some(p) = &self.provider {
            // T3 `SourceControlProviderKind`: `^[a-z][a-z0-9-]*$`, ≤ 64.
            let ok = p.len() <= 64 && p.bytes().next().is_some_and(|c| c.is_ascii_lowercase()) && p.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-');
            if !ok {
                return Err(ids::invalid("provider"));
            }
            v["provider"] = json!(p);
        }
        put(&mut v, "repository", self.repository.as_deref().map(|r| ids::text("repository", r, 512)).transpose()?);
        put(&mut v, "remoteUrl", self.remote_url.as_deref().map(|u| ids::text("remoteUrl", u, 4096)).transpose()?);
        put(&mut v, "protocol", self.protocol.as_deref().map(|p| one_of("protocol", p, PROTOCOLS)).transpose()?);
        Ok(v)
    }
    pub fn from_json(v: &Value) -> Result<Self> {
        let mut r = Reader::new(v)?;
        let start = Self {
            project_id: r.req_str("projectId")?,
            title: r.req_str("title")?,
            created_at: r.req_str("createdAt")?,
            destination_path: r.req_str("destinationPath")?,
            provider: r.opt_str("provider")?,
            repository: r.opt_str("repository")?,
            remote_url: r.opt_str("remoteUrl")?,
            protocol: r.opt_str("protocol")?,
        };
        r.done()?;
        Ok(start)
    }
}
/// `projectClone.start`: `{projectId, cwd, remoteUrl, repository}`. Not repeatable:
/// reconcile with the orchestration shell by project id.
pub fn start(conn: &Connection, clone: &CloneStart) -> Result<Value> {
    send(conn, &START, clone.payload()?)
}
