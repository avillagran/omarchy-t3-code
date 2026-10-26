//! Git and pull requests: the row badge and the pull request page, the branch
//! picker and the Git & changes sheet.
use super::{domain_fn, DomainFn};
use crate::api::Args;
use crate::{branch_picker, git_controls, pull_requests, Result};
use serde_json::{json, Value};

const PR_READS: &[&str] = &["list", "detail", "checks", "activity", "diff"];
const PR_WRITES: &[&str] = &["comment", "review", "action", "reviewer-candidates", "label-candidates", "reviewers", "labels"];
const PR_LINKS: &[&str] = &["link", "unlink", "watch", "unwatch"];

/// A `(method, payload)` pair as `{method, payload}`.
fn method((method, payload): (impl Into<String>, Value)) -> Value {
    json!({"method":method.into(),"payload":payload})
}

/// The checkout a Git sheet reads, as `git_controls::scope` takes it.
fn git_scope(a: &Args) -> Result<Value> {
    let input = json!({"environment":a.req("environment")?,"thread":a.req("thread")?,"requestId":a.req("requestId")?,
        "projectId":a.req("projectId")?,"cwd":a.req("cwd")?});
    git_controls::scope(a.req_object("shell")?, &input)
}

pub const FUNCTIONS: &[DomainFn] = &[
    domain_fn! {
        name: "pullRequests.sidebarBadge", module: "pullRequests",
        args: [(thread, Object, required)],
        returns: "object {kind: single, multiple or stack, text, state: open, draft, merged, closed or unknown, current, links} of a thread's linked pull requests, or null",
        run: |a| Ok(pull_requests::sidebar_badge(a.req_object("thread")?)),
    },
    domain_fn! {
        name: "pullRequests.request", module: "pullRequests",
        args: [
            (action, Enum(PR_READS), required), (state, Text, optional), (involvement, Text, optional), (query, Text, optional),
            (cursors, Object, optional), (reference, Object, optional), (cursor, Text, optional),
        ],
        returns: "object {method, payload}: the typed pullRequests.list, detail, checks or activity call and its args (list: 50 per page, state open and involvement all by default), or method diff with the /api/pull-requests/diff query; reference is {projectId, repository, host, number}",
        run: |a| Ok(method(pull_requests::request(a.all())?)),
    },
    domain_fn! {
        name: "pullRequests.payload", module: "pullRequests",
        args: [
            (kind, Enum(PR_WRITES), required), (reference, Object, required), (body, Text, optional), (verdict, Text, optional),
            (action, Text, optional), (mergeMethod, Text, optional), (reviewers, Array, optional), (requested, Bool, optional),
            (labels, Array, optional), (applied, Bool, optional),
        ],
        returns: "object {method, payload}: the typed pullRequests.comment, submitReview, runAction, reviewerCandidates, labelCandidates, requestReviewers or setLabels call and its args, after the page's rules (a comment needs words; only the page's actions)",
        run: |a| Ok(method(pull_requests::payload(a.all())?)),
    },
    domain_fn! {
        name: "pullRequests.threadCommand", module: "pullRequests",
        args: [(thread, Id, required), (link, Enum(PR_LINKS), required), (reference, Object, required), (commandId, Id, optional)],
        returns: "command thread.pull-request.link, unlink or watch for a reference {host, repository, number, url}; commandId only when given",
        run: |a| {
            let mut command = pull_requests::thread_command(a.all())?;
            match a.str("commandId")? {
                Some(id) => command["commandId"] = json!(id),
                None => {
                    command.as_object_mut().unwrap().remove("commandId");
                }
            }
            Ok(command)
        },
    },
    domain_fn! {
        name: "branchPicker.dedupe", module: "branchPicker",
        args: [(refs, Array, required)],
        returns: "array: vcs.listRefs refs without the origin refs whose local branch is listed",
        run: |a| Ok(json!(branch_picker::dedupe(a.req_array("refs")?))),
    },
    domain_fn! {
        name: "branchPicker.deriveLocal", module: "branchPicker",
        args: [(name, Text, required)],
        returns: "string: the local branch name of a remote ref (origin/feature/x → feature/x)",
        run: |a| Ok(json!(branch_picker::derive_local(a.req("name")?))),
    },
    domain_fn! {
        name: "branchPicker.sanitize", module: "branchPicker",
        args: [(name, Text, required)],
        returns: "string: a new branch name with whitespace runs as single dashes",
        run: |a| Ok(json!(branch_picker::sanitize(a.req("name")?))),
    },
    domain_fn! {
        name: "branchPicker.selectionTarget", module: "branchPicker",
        args: [(projectCwd, Path, required), (worktreePath, Path, optional), (ref, Object, required)],
        returns: "object {checkoutCwd, nextWorktreePath, reuseExistingWorktree}: where to switch to the picked ref and the thread's next worktree",
        run: |a| {
            let (cwd, next, reuse) = branch_picker::selection_target(a.req("projectCwd")?, a.str("worktreePath")?, a.req_object("ref")?);
            Ok(json!({"checkoutCwd":cwd,"nextWorktreePath":next,"reuseExistingWorktree":reuse}))
        },
    },
    domain_fn! {
        name: "branchPicker.mismatch", module: "branchPicker",
        args: [(worktreePath, Path, optional), (threadBranch, Text, optional), (currentBranch, Text, optional)],
        returns: "object {threadBranch, currentBranch, canRestore} when the checkout is not on the thread's branch, else null",
        run: |a| Ok(branch_picker::mismatch(a.str("worktreePath")?, a.str("threadBranch")?, a.str("currentBranch")?)),
    },
    domain_fn! {
        name: "branchPicker.temporaryBranch", module: "branchPicker",
        args: [(name, Text, required)],
        returns: "bool: a temporary worktree branch (t3-/t3/ or t3code-/t3code/ plus 8 hex, or t3code/ plus a UUID v4)",
        run: |a| Ok(json!(branch_picker::temporary_branch(a.req("name")?))),
    },
    domain_fn! {
        name: "gitControls.scope", module: "gitControls",
        args: [
            (shell, Object, required), (environment, Text, required), (thread, Id, required), (requestId, Text, required),
            (projectId, Id, required), (cwd, Path, required),
        ],
        returns: "object {environment, thread, requestId, projectId, cwd, projectRoot, worktreePath}: the thread's checkout resolved against the shell snapshot (project threads only; git_scope_changed when projectId or cwd moved)",
        run: git_scope,
    },
    domain_fn! {
        name: "gitControls.resolveScripts", module: "gitControls",
        args: [(settings, Object, required), (project, Object, required)],
        returns: "array of the project's scripts {id, name, command, icon, runOnWorktreeCreate, …} with T3's precedence (project override, defaults, legacy override, project scripts)",
        run: |a| git_controls::resolve_scripts(a.req_object("settings")?, a.req_object("project")?),
    },
    domain_fn! {
        name: "gitControls.actions", module: "gitControls",
        args: [(status, Object, required)],
        returns: "object {commit, push, pull, create_pr, commit_push, commit_push_pr} of bools: what the Git menu offers for a vcs.refreshStatus result",
        run: |a| Ok(git_controls::actions(a.req_object("status")?)),
    },
    domain_fn! {
        name: "gitControls.present", module: "gitControls",
        args: [(scope, Object, required), (status, Object, required), (scripts, Array, required)],
        returns: "object {scope, status, scripts, actions, revision}: the Git & changes sheet; revision fingerprints checkout, status and scripts (not the requestId)",
        run: |a| {
            a.req_array("scripts")?;
            git_controls::present(a.req_object("scope")?, a.req_object("status")?, &a.all()["scripts"])
        },
    },
    domain_fn! {
        name: "gitControls.scopedResult", module: "gitControls",
        args: [(expected, Object, required), (receipt, Object, required)],
        returns: "the receipt's result for the sheet's still-open scope and request, null for a late or other checkout; the receipt's error otherwise",
        run: |a| git_controls::scoped_result(a.req_object("expected")?, a.req_object("receipt")?).unwrap_or(Ok(Value::Null)),
    },
    domain_fn! {
        name: "gitControls.buildAction", module: "gitControls",
        args: [(current, Object, required), (intent, Object, required)],
        returns: "object {scope, revision, actionId, operation, steps [{tag, payload}], retryAutomatically: false}: a confirmed action (intent {scope, revision, confirmed, actionId (UUID), operation, defaultBranchConfirmed?, commitMessage?, filePaths?, scriptId?}) as git.runStackedAction, vcs.pull or terminal.open + terminal.write steps",
        run: |a| git_controls::build_action(a.req_object("current")?, a.req_object("intent")?),
    },
    domain_fn! {
        name: "gitControls.buildVerifiedAction", module: "gitControls",
        args: [(current, Object, required), (fresh, Object, required), (intent, Object, required)],
        returns: "gitControls.buildAction's plan, only when a fresh read right before dispatch has the same revision (git_stale_result otherwise)",
        run: |a| git_controls::build_verified_action(a.req_object("current")?, a.req_object("fresh")?, a.req_object("intent")?),
    },
];
