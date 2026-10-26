//! Review screens: the diff, the tool inspector, plans and checkpoint rewind.
use super::{domain_fn, DomainFn, NULL};
use crate::api::Args;
use crate::checkpoint_revert::{self, Safety};
use crate::{diff_review, plan_workflow, timeline, tool_inspector, Result};
use serde_json::{json, Value};

/// A timeline row's `inspector` reference scoped to the viewer's environment and thread.
fn inspector_input(a: &Args) -> Result<Value> {
    let mut input = a.req_object("reference")?.clone();
    input["environment"] = json!(a.req("environment")?);
    input["threadId"] = json!(a.req("threadId")?);
    Ok(input)
}

const INTERACTION_MODES: &[&str] = &["default", "plan"];

/// ot3's plan view with each plan's Markdown as `markdown.blocks` instead of
/// Qt rich text: the same planView `timeline.view` returns.
fn plan_view(a: &Args) -> Result<Value> {
    let math = a.bool("mathRenderingEnabled")?.unwrap_or(false);
    let render = timeline::Render::neutral(math);
    let mut view = plan_workflow::view(a.req_object("projection")?, render.plan);
    for plan in view["plans"].as_array_mut().into_iter().flatten() {
        timeline::plan_blocks(plan, math);
    }
    timeline::plan_blocks(&mut view["ready"], math);
    Ok(view)
}

pub const FUNCTIONS: &[DomainFn] = &[
    domain_fn! {
        name: "diffReview.present", module: "diffReview",
        args: [(result, Object, required), (summary, Object, optional)],
        returns: "object {files [{path, oldPath, kind, binary, missing?, additions, deletions, summaryAdditions?, summaryDeletions?, rows [{kind: hunk, addition, deletion, context or meta, text, oldLine, newLine}], splitRows [{kind, left, right}]}], additions, deletions, fromTurnCount, toTurnCount} from a diff RPC result {diff}",
        run: |a| diff_review::present(a.req_object("result")?, a.get("summary").unwrap_or(&NULL)),
    },
    domain_fn! {
        name: "diffReview.scopedResult", module: "diffReview",
        args: [(expected, Object, required), (receipt, Object, required), (summary, Object, required)],
        returns: "diffReview.present's object for a receipt of the still-open request, null for a late or other request; the receipt's error otherwise",
        run: |a| diff_review::scoped_result(a.req_object("expected")?, a.req_object("receipt")?, a.req_object("summary")?).unwrap_or(Ok(Value::Null)),
    },
    domain_fn! {
        name: "toolInspector.compactReference", module: "toolInspector",
        args: [(row, Object, required)],
        returns: "object {sourceThreadId, sourceItemId, type, runId, providerTurnId, revision} of a turn item or projected row (revision live while it runs); timeline activity entries already carry it as inspector",
        run: |a| tool_inspector::compact_reference(a.req_object("row")?),
    },
    domain_fn! {
        name: "toolInspector.scope", module: "toolInspector",
        args: [(reference, Object, required), (environment, Text, required), (threadId, Id, required)],
        returns: "object {environment, threadId, sourceThreadId, sourceItemId, type, runId, providerTurnId, revision}: the inspector's local scope, to match the read's answer against",
        run: |a| tool_inspector::scope(&inspector_input(a)?),
    },
    domain_fn! {
        name: "toolInspector.request", module: "toolInspector",
        args: [(reference, Object, required), (environment, Text, required), (threadId, Id, required)],
        returns: "object {method: orchestration.getTurnItem, payload {threadId, itemId, revision}}: the typed call that reads the item",
        run: |a| {
            let (method, payload) = tool_inspector::request(&inspector_input(a)?)?;
            Ok(json!({"method":method,"payload":payload}))
        },
    },
    domain_fn! {
        name: "toolInspector.parse", module: "toolInspector",
        args: [(reference, Object, required), (environment, Text, required), (threadId, Id, required), (response, Object, required)],
        returns: "object {scope, available, title, status, failed, exitCode, sections [{key, label, text, truncated}], metadata, metadataTruncated, raw, rawTruncated, outputOmitted, emptyOutput} from orchestration.getTurnItem's {item}; {scope, available: false, sections: [], raw: \"\", rawTruncated: false} when the item is gone",
        run: |a| tool_inspector::parse(&inspector_input(a)?, a.req_object("response")?),
    },
    domain_fn! {
        name: "planWorkflow.view", module: "planWorkflow",
        args: [(projection, Object, required), (mathRenderingEnabled, Bool, optional)],
        returns: "object {plans [{id, threadId, runId, status, markdown, title, displayMarkdown, blocks, updatedAt}], ready (the plan the banner offers to implement, or null), error?}: the projection's proposed plans with markdown.blocks of their display Markdown",
        run: plan_view,
    },
    domain_fn! {
        name: "planWorkflow.followUp", module: "planWorkflow",
        args: [(plan, Object, required), (threadId, Id, required), (draft, Text, optional)],
        returns: "object {text, interactionMode, sourcePlanRef?}: an empty draft implements the plan (interactionMode default, sourcePlanRef {threadId, planId}); a non-empty one is feedback in plan mode",
        run: |a| Ok(plan_workflow::follow_up(a.req_object("plan")?, a.req("threadId")?, a.str("draft")?.unwrap_or(""))?),
    },
    domain_fn! {
        name: "planWorkflow.attachSourceRef", module: "planWorkflow",
        args: [(command, Object, required), (followUp, Object, required)],
        returns: "command: the message.dispatch command with the follow-up's sourcePlanRef, also when it targets a new thread",
        run: |a| {
            let mut command = a.req_object("command")?.clone();
            plan_workflow::attach_source_ref(&mut command, a.req_object("followUp")?)?;
            Ok(command)
        },
    },
    domain_fn! {
        name: "planWorkflow.implementationTitle", module: "planWorkflow",
        args: [(markdown, Text, required)],
        returns: "string: the new thread's title for \"Implement in new thread\" (Implement <plan title>, else Implement plan; at most 255 characters)",
        run: |a| Ok(json!(plan_workflow::implementation_title(a.req("markdown")?))),
    },
    domain_fn! {
        name: "planWorkflow.mode", module: "planWorkflow",
        args: [(planModeEnabled, Bool, required), (provider, Object, optional), (requested, Enum(INTERACTION_MODES), required)],
        returns: "object {enabled, interactionMode}: Plan / Build as the composer offers it for the thread's provider",
        run: |a| Ok(plan_workflow::mode(a.bool("planModeEnabled")?.unwrap_or(false), a.get("provider"), a.req("requested")?)),
    },
    domain_fn! {
        name: "checkpointRevert.canPreview", module: "checkpointRevert",
        args: [(projection, Object, required), (message, Object, required)],
        returns: "bool: offer \"Edit from here\" on this user message (a ready checkpoint for its run and a turn-start intent, when known)",
        run: |a| Ok(json!(checkpoint_revert::can_preview(a.req_object("projection")?, a.req_object("message")?))),
    },
    domain_fn! {
        name: "checkpointRevert.reference", module: "checkpointRevert",
        args: [(projection, Object, required), (messageId, Id, required)],
        returns: "object {threadId, messageId, messageRunId, messageCheckpointId, checkpointId, checkpointRunId, scopeId, providerThreadId, providerInstanceId, turnCount, priorRollbackRequestId}: the checkpoint just before the message, on the raw projection (revert_* when unknown or stale)",
        run: |a| checkpoint_revert::reference(a.req_object("projection")?, a.req("messageId")?),
    },
    domain_fn! {
        name: "checkpointRevert.safety", module: "checkpointRevert",
        args: [(safety, Object, required)],
        returns: "object {safety, allowed, error}: the live gate {canOperate, connected, supportsConversationRollback, working, sendBusy, connecting, pendingAttachments, rewinding} (anything but true is false) and the first reason a rewind is refused, or null",
        run: |a| {
            let safety = Safety::from_json(a.req_object("safety")?);
            let error = safety.check().err();
            Ok(json!({"safety":safety.to_json(),"allowed":error.is_none(),"error":error}))
        },
    },
    domain_fn! {
        name: "checkpointRevert.command", module: "checkpointRevert",
        args: [(input, Object, required), (projection, Object, required), (safety, Object, required)],
        returns: "command checkpoint.rollback {commandId omarchy-<requestId>, threadId, scopeId, checkpointId, restoreFiles} for first admission: input {confirmed, restoreFiles, requestId (UUID), environment, thread, reference} with the reference still fresh and the safety gate open",
        run: |a| checkpoint_revert::command(a.req_object("input")?, a.req_object("projection")?, &Safety::from_json(a.req_object("safety")?)),
    },
    domain_fn! {
        name: "checkpointRevert.confirmedCommand", module: "checkpointRevert",
        args: [(input, Object, required)],
        returns: "command checkpoint.rollback from the input alone, to compare with an already persisted outbox command on an explicit retry",
        run: |a| checkpoint_revert::confirmed_command(a.req_object("input")?),
    },
    domain_fn! {
        name: "checkpointRevert.observation", module: "checkpointRevert",
        args: [(input, Object, required), (projection, Object, required)],
        returns: "object {state: pending, complete, failed or superseded, error?}: the rewind as the thread's read-back shows it",
        run: |a| checkpoint_revert::observation(a.req_object("input")?, a.req_object("projection")?),
    },
    domain_fn! {
        name: "checkpointRevert.recoveryMessage", module: "checkpointRevert",
        args: [(projection, Object, required), (reference, Object, required)],
        returns: "object {id, runId, text, attachments, context}: the message to restore to the composer, captured before dispatch",
        run: |a| checkpoint_revert::recovery_message(a.req_object("projection")?, a.req_object("reference")?),
    },
    domain_fn! {
        name: "checkpointRevert.appendPrompt", module: "checkpointRevert",
        args: [(current, Text, required), (restored, Text, required)],
        returns: "string: the composer draft with the restored prompt after it (a blank line between)",
        run: |a| Ok(json!(checkpoint_revert::append_prompt(a.req("current")?, a.req("restored")?))),
    },
];
