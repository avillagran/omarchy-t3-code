//! `pullRequests.*`: reads and writes of T3's pull request page. Every call but
//! `list` and `routingIdentity` names one PR with `PrRef`. `crate::pull_requests`
//! (`request`, `payload`) stays as the JSON adapter of ot3's page and builds its
//! payloads with the functions here.
use super::reader::{int, one_of, put, strings, Reader};
use super::{api_fn, ids, send, Args, Function};
use crate::{Connection, Result};
use serde_json::{json, Map, Value};

/// Comment and review bodies (T3 `CommentBody`).
const BODY_MAX: usize = 65_536;
const NAME_MAX: usize = 512;
pub const LIST_STATES: &[&str] = &["all", "open", "closed", "merged"];
pub const INVOLVEMENTS: &[&str] = &["all", "reviewing", "authored"];
pub const VERDICTS: &[&str] = &["comment", "approve", "request-changes"];
pub const ACTIONS: &[&str] = &[
    "merge", "ready", "draft", "close", "reopen", "delete-branch", "update-branch", "enable-auto-merge",
    "disable-auto-merge", "revert", "approve-workflows",
];
pub const MERGE_METHODS: &[&str] = &["merge", "squash", "rebase"];
pub const UPDATE_METHODS: &[&str] = &["merge", "rebase"];
pub const REVIEWER_KINDS: &[&str] = &["user", "team"];

macro_rules! pr_read {
    ($konst:ident, $name:literal, $doc:literal) => {
        #[doc = $doc]
        pub const $konst: Function = api_fn! {
            name: $name, module: "pullRequests", tag: Some($name),
            kind: Read, scope: Fixed("orchestration:read"),
            args: [
                (projectId, Id, required), (repository, Text, required), (number, Integer, required),
                (host, Text, optional), (expectedAccountId, Text, optional), (allowStale, Bool, optional),
            ],
            payload: |a| PrRef::from_args(a)?.to_json(),
        };
    };
}
pub const LIST: Function = api_fn! {
    name: "pullRequests.list", module: "pullRequests", tag: Some("pullRequests.list"),
    kind: Read, scope: Fixed("orchestration:read"),
    args: [
        (state, Enum(LIST_STATES), required), (involvement, Enum(INVOLVEMENTS), optional), (filters, Object, optional),
        (projectId, Id, optional), (projectIds, Array, optional), (host, Text, optional), (limit, Integer, optional),
        (cursors, Object, optional), (query, Text, optional),
    ],
    payload: |a| ListQuery::from_json(a.all())?.payload(),
};
pr_read!(DETAIL, "pullRequests.detail", "`PullRequestDetail`.");
pr_read!(CHECKS, "pullRequests.checks", "`{state, checks[]}` or `null`.");
pr_read!(ACTIVITY, "pullRequests.activity", "`PullRequestActivity`.");
pr_read!(ROUTING, "pullRequests.routing", "`{accountId, host, provider, viewer, projectTitle, workspaceRoot}`.");
pub const ROUTING_IDENTITY: Function = api_fn! {
    name: "pullRequests.routingIdentity", module: "pullRequests", tag: Some("pullRequests.routingIdentity"),
    kind: Read, scope: Fixed("orchestration:read"),
    args: [(host, Text, required)],
    payload: |a| routing_identity_payload(a.req("host")?),
};
pr_read!(REVIEWER_CANDIDATES, "pullRequests.reviewerCandidates", "`{candidates[], truncated}`.");
pr_read!(LABEL_CANDIDATES, "pullRequests.labelCandidates", "`{candidates[], truncated}`.");
pub const COMMENT: Function = api_fn! {
    name: "pullRequests.comment", module: "pullRequests", tag: Some("pullRequests.comment"),
    kind: UnrepeatableMutation, scope: Fixed("source-control:write"),
    args: [
        (projectId, Id, required), (repository, Text, required), (number, Integer, required), (body, Text, required),
        (host, Text, optional), (expectedAccountId, Text, optional), (allowStale, Bool, optional),
    ],
    payload: |a| comment_payload(&PrRef::from_args(a)?, a.req("body")?),
};
pub const SUBMIT_REVIEW: Function = api_fn! {
    name: "pullRequests.submitReview", module: "pullRequests", tag: Some("pullRequests.submitReview"),
    kind: UnrepeatableMutation, scope: Fixed("source-control:write"),
    args: [
        (projectId, Id, required), (repository, Text, required), (number, Integer, required),
        (verdict, Enum(VERDICTS), required), (body, Text, required), (comments, Array, required),
        (host, Text, optional), (expectedAccountId, Text, optional), (allowStale, Bool, optional),
    ],
    payload: |a| submit_review_payload(&PrRef::from_args(a)?, a.req("verdict")?, a.req("body")?, a.req_array("comments")?),
};
pub const RUN_ACTION: Function = api_fn! {
    name: "pullRequests.runAction", module: "pullRequests", tag: Some("pullRequests.runAction"),
    kind: UnrepeatableMutation, scope: Fixed("source-control:write"),
    args: [
        (projectId, Id, required), (repository, Text, required), (number, Integer, required), (action, Enum(ACTIONS), required),
        (mergeMethod, Enum(MERGE_METHODS), optional), (updateMethod, Enum(UPDATE_METHODS), optional),
        (stackNumber, Integer, optional), (expectedStackHeads, Array, optional),
        (host, Text, optional), (expectedAccountId, Text, optional), (allowStale, Bool, optional),
    ],
    payload: |a| {
        let action = PrAction {
            action: a.req("action")?.into(),
            merge_method: a.str("mergeMethod")?.map(Into::into),
            update_method: a.str("updateMethod")?.map(Into::into),
            stack_number: a.u64("stackNumber")?,
            expected_stack_heads: a.array("expectedStackHeads")?.map(|h| stack_heads(h)).transpose()?,
        };
        run_action_payload(&PrRef::from_args(a)?, &action)
    },
};
pub const REQUEST_REVIEWERS: Function = api_fn! {
    name: "pullRequests.requestReviewers", module: "pullRequests", tag: Some("pullRequests.requestReviewers"),
    kind: IdempotentMutation, scope: Fixed("source-control:write"),
    args: [
        (projectId, Id, required), (repository, Text, required), (number, Integer, required),
        (reviewers, Array, required), (requested, Bool, required),
        (host, Text, optional), (expectedAccountId, Text, optional), (allowStale, Bool, optional),
    ],
    payload: |a| {
        let reviewers = reviewers(a.req_array("reviewers")?)?;
        request_reviewers_payload(&PrRef::from_args(a)?, &reviewers, a.bool("requested")?.unwrap_or_default())
    },
};
pub const SET_LABELS: Function = api_fn! {
    name: "pullRequests.setLabels", module: "pullRequests", tag: Some("pullRequests.setLabels"),
    kind: IdempotentMutation, scope: Fixed("source-control:write"),
    args: [
        (projectId, Id, required), (repository, Text, required), (number, Integer, required),
        (labels, Array, required), (applied, Bool, required),
        (host, Text, optional), (expectedAccountId, Text, optional), (allowStale, Bool, optional),
    ],
    payload: |a| set_labels_payload(&PrRef::from_args(a)?, &strings("labels", a.req_array("labels")?)?, a.bool("applied")?.unwrap_or_default()),
};
pub const CHANGE_BREAKDOWN: Function = api_fn! {
 name: "pullRequests.changeBreakdown", module: "pullRequests", tag: Some("pullRequests.changeBreakdown"),
 kind: Read, scope: Fixed("orchestration:read"),
 args: [(projectId, Id, required),(repository, Text, required),(number, Integer, required),(host, Text, optional),(expectedAccountId, Text, optional),(allowStale, Bool, optional)],
 payload: |a| PrRef::from_args(a)?.to_json(),
};
pub const FILTER_OPTIONS: Function = api_fn! {
 name: "pullRequests.filterOptions", module: "pullRequests", tag: Some("pullRequests.filterOptions"),
 kind: Read, scope: Fixed("orchestration:read"),
 args: [(kind, Enum(&["labels","authors"]), required),(projectId, Id, optional),(projectIds, Array, optional),(host, Text, optional),(query, Text, optional)],
 payload: |a| { if let Some(list)=a.array("projectIds")? {if list.len()>100{return Err("invalid_project_ids".into())} for id in list {ids::id("projectId",id.as_str().ok_or("invalid_project_id")?)?;}} if a.str("query")?.is_some_and(|q|q.encode_utf16().count()>100||q.trim().is_empty()){return Err("invalid_query".into())} Ok(a.all().clone()) },
};
pub const FUNCTIONS: &[Function] = &[
    CHANGE_BREAKDOWN, FILTER_OPTIONS,
    LIST, DETAIL, CHECKS, ACTIVITY, ROUTING, ROUTING_IDENTITY, REVIEWER_CANDIDATES, LABEL_CANDIDATES,
    COMMENT, SUBMIT_REVIEW, RUN_ACTION, REQUEST_REVIEWERS, SET_LABELS,
];

/// `PullRequestRef`: one PR of a project. `host` defaults to the project's provider host.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrRef {
    pub project_id: String,
    /// `owner/name`.
    pub repository: String,
    pub number: u64,
    pub host: Option<String>,
    pub expected_account_id: Option<String>,
    pub allow_stale: Option<bool>,
}
impl PrRef {
    pub fn new(project_id: &str, repository: &str, number: u64) -> Self {
        Self { project_id: project_id.into(), repository: repository.into(), number, host: None, expected_account_id: None, allow_stale: None }
    }
    pub fn to_json(&self) -> Result<Value> {
        let mut v = json!({
            "projectId":ids::id("projectId", &self.project_id)?,
            "repository":ids::text("repository", &self.repository, NAME_MAX)?,
            "number":int("number", self.number, 1, u32::MAX as u64)?,
        });
        put(&mut v, "host", self.host.as_deref().map(|h| ids::text("host", h, NAME_MAX)).transpose()?);
        put(&mut v, "expectedAccountId", self.expected_account_id.as_deref().map(|i| ids::text("expectedAccountId", i, NAME_MAX)).transpose()?);
        put(&mut v, "allowStale", self.allow_stale);
        Ok(v)
    }
    /// The six `PullRequestRef` members of a call's arguments.
    fn from_args(a: &Args) -> Result<Self> {
        Ok(Self {
            project_id: a.req("projectId")?.into(),
            repository: a.req("repository")?.into(),
            number: a.req_u64("number")?,
            host: a.str("host")?.map(Into::into),
            expected_account_id: a.str("expectedAccountId")?.map(Into::into),
            allow_stale: a.bool("allowStale")?,
        })
    }
}
fn pr_payload(f: &Function, conn: &Connection, pr: &PrRef) -> Result<Value> {
    send(conn, f, pr.to_json()?)
}
/// `pullRequests.detail`: `PullRequestDetail`.
pub fn detail(conn: &Connection, pr: &PrRef) -> Result<Value> {
    pr_payload(&DETAIL, conn, pr)
}
/// `pullRequests.checks`: `{state, checks[]}` or `null`.
pub fn checks(conn: &Connection, pr: &PrRef) -> Result<Value> {
    pr_payload(&CHECKS, conn, pr)
}
/// `pullRequests.activity`: `{comments[], commentCount, reviewThreads[], commits[], …}`.
pub fn activity(conn: &Connection, pr: &PrRef) -> Result<Value> {
    pr_payload(&ACTIVITY, conn, pr)
}
/// `pullRequests.routing`: `{accountId, host, provider, viewer, projectTitle, workspaceRoot}`.
pub fn routing(conn: &Connection, pr: &PrRef) -> Result<Value> {
    pr_payload(&ROUTING, conn, pr)
}
/// `pullRequests.reviewerCandidates`: `{candidates[], truncated}`.
pub fn reviewer_candidates(conn: &Connection, pr: &PrRef) -> Result<Value> {
    pr_payload(&REVIEWER_CANDIDATES, conn, pr)
}
/// `pullRequests.labelCandidates`: `{candidates[], truncated}`.
pub fn label_candidates(conn: &Connection, pr: &PrRef) -> Result<Value> {
    pr_payload(&LABEL_CANDIDATES, conn, pr)
}
/// `pullRequests.routingIdentity`: `{accountId, host, provider, viewer}` for a host.
pub fn routing_identity(conn: &Connection, host: &str) -> Result<Value> {
    send(conn, &ROUTING_IDENTITY, routing_identity_payload(host)?)
}
fn routing_identity_payload(host: &str) -> Result<Value> {
    Ok(json!({"host":ids::text("host", host, NAME_MAX)?}))
}

/// `PullRequestListInput`.
#[derive(Clone, Debug, PartialEq)]
pub struct ListQuery {
    /// `all`, `open`, `closed` or `merged`.
    pub state: String,
    pub involvement: Option<String>,
    /// `{draft?, review?, checks?, labels?, excludedLabels?, author?}`.
    pub filters: Option<Value>,
    pub project_id: Option<String>,
    pub project_ids: Option<Vec<String>>,
    pub host: Option<String>,
    /// 1..=500.
    pub limit: Option<u64>,
    /// Opaque `nextCursors` from the previous page.
    pub cursors: Option<Map<String, Value>>,
    pub query: Option<String>,
}
impl ListQuery {
    pub fn new(state: &str) -> Self {
        Self { state: state.into(), involvement: None, filters: None, project_id: None, project_ids: None, host: None, limit: None, cursors: None, query: None }
    }
    pub fn payload(&self) -> Result<Value> {
        let mut v = json!({"state":one_of("state", &self.state, LIST_STATES)?});
        put(&mut v, "involvement", self.involvement.as_deref().map(|i| one_of("involvement", i, INVOLVEMENTS)).transpose()?);
        put(&mut v, "filters", self.filters.as_ref().map(list_filters).transpose()?);
        put(&mut v, "projectId", self.project_id.as_deref().map(|p| ids::id("projectId", p)).transpose()?);
        if let Some(ids_) = &self.project_ids {
            if ids_.len() > 100 {
                return Err(ids::invalid("projectIds"));
            }
            v["projectIds"] = json!(ids_.iter().map(|p| ids::id("projectIds", p)).collect::<Result<Vec<_>>>()?);
        }
        put(&mut v, "host", self.host.as_deref().map(|h| ids::text("host", h, NAME_MAX)).transpose()?);
        put(&mut v, "limit", self.limit.map(|l| int("limit", l, 1, 500)).transpose()?);
        if let Some(cursors) = &self.cursors {
            let ok = cursors.len() <= 64
                && cursors.iter().all(|(k, c)| !k.trim().is_empty() && k.len() <= NAME_MAX && c.as_str().is_some_and(|c| ids::text("cursors", c, 4096).is_ok()));
            if !ok {
                return Err(ids::invalid("cursors"));
            }
            v["cursors"] = json!(cursors);
        }
        put(&mut v, "query", self.query.as_deref().map(|q| ids::text("query", q, 200)).transpose()?);
        Ok(v)
    }
    pub fn from_json(v: &Value) -> Result<Self> {
        let mut r = Reader::new(v)?;
        let query = Self {
            state: r.req_str("state")?,
            involvement: r.opt_str("involvement")?,
            filters: r.opt_object("filters")?,
            project_id: r.opt_str("projectId")?,
            project_ids: r.opt_array("projectIds")?.map(|p| strings("projectIds", &p)).transpose()?,
            host: r.opt_str("host")?,
            limit: r.opt_u64("limit")?,
            cursors: r.opt_object("cursors")?.and_then(|c| c.as_object().cloned()),
            query: r.opt_str("query")?,
        };
        r.done()?;
        Ok(query)
    }
}
/// `PullRequestListFilters` (T3 `pullRequest.ts`).
fn list_filters(v: &Value) -> Result<Value> {
    let invalid = || ids::invalid("filters");
    let mut r = Reader::of("filters", v)?;
    let mut out = json!({});
    for (k, allowed) in [
        ("draft", &["only", "hide"][..]),
        ("review", &["approved", "changes-requested", "review-required", "none"][..]),
        ("checks", &["passing", "failing"][..]),
    ] {
        if let Some(s) = r.opt_str(k).map_err(|_| invalid())? {
            out[k] = json!(one_of("filters", &s, allowed)?);
        }
    }
    let label = |l: &Value| -> Result<String> { l.as_str().ok_or_else(invalid).and_then(|l| ids::text("filters", l, 200).map(str::to_string)) };
    if let Some(groups) = r.opt_array("labels").map_err(|_| invalid())? {
        if groups.len() > 10 {
            return Err(invalid());
        }
        let groups = groups
            .iter()
            .map(|g| {
                let g = g.as_array().filter(|g| g.len() <= 10).ok_or_else(invalid)?;
                g.iter().map(label).collect::<Result<Vec<_>>>()
            })
            .collect::<Result<Vec<_>>>()?;
        out["labels"] = json!(groups);
    }
    if let Some(excluded) = r.opt_array("excludedLabels").map_err(|_| invalid())? {
        if excluded.len() > 10 {
            return Err(invalid());
        }
        out["excludedLabels"] = json!(excluded.iter().map(label).collect::<Result<Vec<_>>>()?);
    }
    if let Some(author) = r.opt_str("author").map_err(|_| invalid())? {
        out["author"] = json!(ids::text("filters", &author, 200)?);
    }
    r.done().map_err(|_| invalid())?;
    Ok(out)
}
/// `pullRequests.list`: `{viewers, providers, entries, errors, truncated, nextCursors}`.
pub fn list(conn: &Connection, query: &ListQuery) -> Result<Value> {
    send(conn, &LIST, query.payload()?)
}

fn comment_body<'a>(field: &str, body: &'a str) -> Result<&'a str> {
    if body.is_empty() || body.chars().count() > BODY_MAX {
        return Err(ids::invalid(field));
    }
    Ok(body)
}
/// `pullRequests.comment`: `null`. A repeat posts the comment twice.
pub fn comment(conn: &Connection, pr: &PrRef, body: &str) -> Result<Value> {
    send(conn, &COMMENT, comment_payload(pr, body)?)
}
pub(crate) fn comment_payload(pr: &PrRef, body: &str) -> Result<Value> {
    let mut v = pr.to_json()?;
    v["body"] = json!(comment_body("body", body)?);
    Ok(v)
}
/// `pullRequests.submitReview`: `null`. `body` may be empty (≤ 65 536); each
/// comment is `{path, oldPath?, body, position:{kind:"added",newLine}|{kind:"deleted",oldLine}|{kind:"context",oldLine,newLine,side}}`.
pub fn submit_review(conn: &Connection, pr: &PrRef, verdict: &str, body: &str, comments: &[Value]) -> Result<Value> {
    send(conn, &SUBMIT_REVIEW, submit_review_payload(pr, verdict, body, comments)?)
}
pub(crate) fn submit_review_payload(pr: &PrRef, verdict: &str, body: &str, comments: &[Value]) -> Result<Value> {
    if body.chars().count() > BODY_MAX {
        return Err(ids::invalid("body"));
    }
    let mut v = pr.to_json()?;
    v["verdict"] = json!(one_of("verdict", verdict, VERDICTS)?);
    v["body"] = json!(body);
    v["comments"] = json!(comments.iter().map(review_comment).collect::<Result<Vec<_>>>()?);
    Ok(v)
}
fn review_comment(c: &Value) -> Result<Value> {
    let invalid = || ids::invalid("comments");
    let mut r = Reader::of("comments", c)?;
    let path = r.raw("path").and_then(Value::as_str).ok_or_else(invalid)?;
    let mut v = json!({"path":ids::path("comments", path)?.trim()});
    if let Some(old) = r.opt_str("oldPath").map_err(|_| invalid())? {
        v["oldPath"] = json!(ids::path("comments", &old)?.trim());
    }
    v["body"] = json!(comment_body("comments", r.raw("body").and_then(Value::as_str).ok_or_else(invalid)?)?);
    let mut p = Reader::of("comments", r.raw("position").ok_or_else(invalid)?)?;
    let line = |p: &mut Reader, k: &'static str| p.req_u64(k).ok().filter(|l| *l >= 1).ok_or_else(invalid);
    v["position"] = match p.req_str("kind").map_err(|_| invalid())?.as_str() {
        "added" => json!({"kind":"added","newLine":line(&mut p, "newLine")?}),
        "deleted" => json!({"kind":"deleted","oldLine":line(&mut p, "oldLine")?}),
        "context" => {
            let (old, new) = (line(&mut p, "oldLine")?, line(&mut p, "newLine")?);
            let side = p.req_str("side").map_err(|_| invalid())?;
            json!({"kind":"context","oldLine":old,"newLine":new,"side":one_of("comments", &side, &["left", "right"])?})
        }
        _ => return Err(invalid()),
    };
    p.done().map_err(|_| invalid())?;
    r.done().map_err(|_| invalid())?;
    Ok(v)
}

/// A `pullRequests.runAction` action and its options.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PrAction {
    pub action: String,
    pub merge_method: Option<String>,
    pub update_method: Option<String>,
    /// Only for environments that advertise `pullRequestStackActions`.
    pub stack_number: Option<u64>,
    /// `(number, headSha)` of each layer the action expects.
    pub expected_stack_heads: Option<Vec<(u64, String)>>,
}
fn stack_heads(items: &[Value]) -> Result<Vec<(u64, String)>> {
    let invalid = || ids::invalid("expectedStackHeads");
    items
        .iter()
        .map(|h| {
            let mut r = Reader::of("expectedStackHeads", h)?;
            let head = (r.req_u64("number").map_err(|_| invalid())?, r.req_str("headSha").map_err(|_| invalid())?);
            r.done().map_err(|_| invalid())?;
            Ok(head)
        })
        .collect()
}
/// `pullRequests.runAction`: `null`. Not repeatable; reconcile with `detail`.
pub fn run_action(conn: &Connection, pr: &PrRef, action: &PrAction) -> Result<Value> {
    send(conn, &RUN_ACTION, run_action_payload(pr, action)?)
}
pub(crate) fn run_action_payload(pr: &PrRef, a: &PrAction) -> Result<Value> {
    let mut v = pr.to_json()?;
    v["action"] = json!(one_of("action", &a.action, ACTIONS)?);
    put(&mut v, "mergeMethod", a.merge_method.as_deref().map(|m| one_of("mergeMethod", m, MERGE_METHODS)).transpose()?);
    put(&mut v, "updateMethod", a.update_method.as_deref().map(|m| one_of("updateMethod", m, UPDATE_METHODS)).transpose()?);
    put(&mut v, "stackNumber", a.stack_number.map(|n| int("stackNumber", n, 1, u32::MAX as u64)).transpose()?);
    if let Some(heads) = &a.expected_stack_heads {
        let heads = heads
            .iter()
            .map(|(n, sha)| Ok(json!({"number":int("expectedStackHeads", *n, 1, u32::MAX as u64)?,"headSha":ids::text("expectedStackHeads", sha, 256)?})))
            .collect::<Result<Vec<_>>>()?;
        v["expectedStackHeads"] = json!(heads);
    }
    Ok(v)
}

/// A reviewer to request or remove: `(id, kind)` with `kind` `user` or `team`.
pub type Reviewer = (String, String);
fn reviewers(items: &[Value]) -> Result<Vec<Reviewer>> {
    let invalid = || ids::invalid("reviewers");
    items
        .iter()
        .map(|r| {
            let mut r = Reader::of("reviewers", r)?;
            let reviewer = (r.req_str("id").map_err(|_| invalid())?, r.req_str("kind").map_err(|_| invalid())?);
            r.done().map_err(|_| invalid())?;
            Ok(reviewer)
        })
        .collect()
}
/// `pullRequests.requestReviewers`: `null`. `requested: false` removes them.
pub fn request_reviewers(conn: &Connection, pr: &PrRef, reviewers: &[Reviewer], requested: bool) -> Result<Value> {
    send(conn, &REQUEST_REVIEWERS, request_reviewers_payload(pr, reviewers, requested)?)
}
pub(crate) fn request_reviewers_payload(pr: &PrRef, reviewers: &[Reviewer], requested: bool) -> Result<Value> {
    if reviewers.is_empty() || reviewers.len() > 25 {
        return Err(ids::invalid("reviewers"));
    }
    let list = reviewers
        .iter()
        .map(|(id, kind)| Ok(json!({"id":ids::text("reviewers", id, NAME_MAX)?,"kind":one_of("reviewers", kind, REVIEWER_KINDS)?})))
        .collect::<Result<Vec<_>>>()?;
    let mut v = pr.to_json()?;
    v["reviewers"] = json!(list);
    v["requested"] = json!(requested);
    Ok(v)
}
/// `pullRequests.setLabels`: `null`. `applied: false` removes them.
pub fn set_labels(conn: &Connection, pr: &PrRef, labels: &[String], applied: bool) -> Result<Value> {
    send(conn, &SET_LABELS, set_labels_payload(pr, labels, applied)?)
}
pub(crate) fn set_labels_payload(pr: &PrRef, labels: &[String], applied: bool) -> Result<Value> {
    if labels.is_empty() || labels.len() > 25 {
        return Err(ids::invalid("labels"));
    }
    let labels = labels.iter().map(|l| ids::text("labels", l, NAME_MAX)).collect::<Result<Vec<_>>>()?;
    let mut v = pr.to_json()?;
    v["labels"] = json!(labels);
    v["applied"] = json!(applied);
    Ok(v)
}
pub fn change_breakdown(conn: &Connection, pr: &PrRef) -> Result<Value> {
    CHANGE_BREAKDOWN.call(conn,&pr.to_json()?,None)
}
pub fn filter_options(conn: &Connection, kind: &str, project: Option<&str>, host: Option<&str>, query: Option<&str>) -> Result<Value> {
    let mut args=json!({"kind":kind});put(&mut args,"projectId",project);put(&mut args,"host",host);put(&mut args,"query",query);
    FILTER_OPTIONS.call(conn,&args,None)
}
