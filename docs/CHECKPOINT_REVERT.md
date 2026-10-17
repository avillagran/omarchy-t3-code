# Native checkpoint rewind: implementation and parent contract

Reference: `/home/avillagran/Work/t3code-nightly` at `5a4e9980533705774dc3f45b35cacf76c3d78dbc`. This module implements the **user-message “Edit from here”** workflow (the audit calls it “Revert from here”), not Git revert, draft undo, or arbitrary checkpoint rollback. No live revert was executed.

## Owned deliverables and verification

- `src/checkpoint_revert.rs`: pure native-reference resolution, confirmation/availability validation, authenticated-environment outbox envelope, exact command builder, native completion observation and draft-recovery helpers.
- `ui/qml/T3CheckpointRevert.qml`: concrete modal confirmation, destructive file choice, keep-files choice, status, explicit identical retry after an uncertain result, fresh-review flow after terminal failure, scope checking, navigation invalidation and durable-operation resume.
- `tests/checkpoint_revert.rs`: independently imports the module with `#[path]`, so no shared `lib.rs` edit is necessary. Tests cover initial/later checkpoints, exact command/schema, permissions/capability/connectivity/run/attachment gates, no guessed IDs, native read-back, old rollback baseline, draft preservation, environment binding and pinned-source contract fragments.
- `tests/qml/checkpoint-revert.qml`: isolated fixture for the parent's **serial** QML runner. Not executed by this sub-agent, as requested. `qmllint` parsing found no syntax errors in either new QML file. Semantic lint is not clean in the unregistered fixture context (missing `UnderTest` and `qs` imports/type information); this is not a QML runtime pass or a visual comparison.
- Rust verification: `cargo test --release --offline --test checkpoint_revert` — **8 passed**. Native schema verification is pinned source-contract assertions, not execution of the Effect decoder (the reference checkout has no installed node_modules/Bun runtime). No network transport is added here, so no second RPC client or loopback transport fixture is appropriate. The parent must exercise the shared Effect RPC/outbox branch and run the QML fixture before claiming end-to-end parity.

Parent follow-up now integrates shared transport, durable recovery, native visibility and UI; see the integration section below. The original task-owned pure builders remain the contract layer.

## Native evidence and exact semantics

All native paths below are relative to the pinned checkout:

| Contract | Native source |
|---|---|
| Hover user-message action; disabled during work/rewind | `apps/web/src/components/chat/MessagesTimeline.tsx:2619-2620,2716-2743` |
| Only `turn_start` / `queued_turn` user messages with a ready run checkpoint; target is `max(0, checkpointTurnCount - 1)` | `apps/web/src/session-logic.ts:1045-1067` |
| Checkpoint summary maps `appRunOrdinal` to `checkpointTurnCount` | `packages/client-runtime/src/state/threadCheckpoints.ts:29-50` |
| Resolve last initial checkpoint (`ordinalWithinScope == 0 && appRunOrdinal == null`) or last checkpoint at the target app run ordinal, then require ready | `packages/client-runtime/src/operations/commands.ts:889-934` |
| Exact public command fields and RPC method | `packages/contracts/src/orchestrationV2.ts:3084-3091,3263` |
| Capability, reconnect, run/send/connect gates, attachment preflight | `apps/web/src/components/ChatView.tsx:8493-8567` |
| Two explicit confirmation choices | `apps/web/src/components/ChatView.tsx:12208-12243` |
| Dispatch acknowledgement is not rewind completion; wait for run `rolled_back`, match async failure by command ID | `apps/web/src/components/ChatView.logic.ts:1112-1168` |
| Append recalled prompt to current draft, preserve current attachments; focus only original route | `apps/web/src/components/ChatView.tsx:8576-8609` |
| Shared-workspace file-restore safety at both admission and execution | `apps/server/src/orchestration-v2/CheckpointRestoreSafety.ts:9-87`; `Orchestrator.ts:9745-9767`; `CheckpointRollbackService.ts:161-175` |
| Server rollback request and failure correlation | `packages/contracts/src/orchestrationV2.ts:429-438`; `apps/server/src/orchestration-v2/Orchestrator.ts:9790-9804` |

The native confirmation is **“Edit from here?”** / **“Rewind chat to before this message. Your prompt and attachments return to the composer.”** Choices: Cancel, **Revert files too** (`restoreFiles:true`), **Revert and keep changes** (`restoreFiles:false`). The server defaults omitted `restoreFiles` to restoring files; this implementation therefore requires an explicit boolean and never silently takes that destructive default. The warning about preserving the existing draft describes a required parent recovery guarantee, not an operation done by this QML component.

File restore snapshots the **whole checkout**, not just files listed on the message. A non-null worktree is necessary but **not sufficient**: native checks realpaths, overlapping active/archived thread workspaces, historical checkpoint scopes and provider sessions. The QML file choice is disabled when no worktree exists; even when enabled, native admission/execution can still reject it. Do not substitute `git reset`, local file writes or a provider-specific `thread/revert` request. Keeping changes bypasses the native file-restore branch, not conversation rollback policy.

The separate arbitrary-checkpoint action (`ChatView.tsx:8645-8705`) has a different generic confirmation and no user-message composer recovery. It is deliberately not exposed by this user-message module.

## Rust API and preservation before compaction

1. On each raw native projection, call `reference(projection, message_id)` **before** `normalize_messages_cached` drops `runId` / `inputIntent`. Attach the returned value as a message/timeline `revertReference` (parent chooses the field name). It retains `threadId`, `messageId`, `messageRunId`, `messageCheckpointId`, `checkpointId`, `checkpointRunId`, `scopeId`, `providerThreadId`, `providerInstanceId`, target `turnCount` and `priorRollbackRequestId`. Do not substitute displayed row indices, thread keys or provider-native IDs.
2. Failure means do not show/enable the action, or expose refresh/unavailable. The bounded snapshot may omit the initial checkpoint, scopes or run. Do not guess ordinal 0 or use the clicked run's **completed** checkpoint (that would rewind to after the prompt, not before it). If necessary, explicitly fetch the existing native `orchestration.getThreadProjection` with `{threadId}`; see `commands.ts:273-277`. Do not invent a checkpoint-context RPC. Merge neither environments nor incompatible projection revisions.
3. On first confirmed dispatch, capture `recovery_message(projection, reference)`. This preserves the original text, attachment metadata and structured context without Qt/HTML transformation. Persist it in the parent's existing operation/draft journal, not the ordinary compact transcript. Do not log private prompts or attachment URLs.
4. Populate `Safety` from real authenticated environment/provider and current thread/composer state. All permission/capability/connectivity booleans default false in the pure builder. Parent source verification corrected the optional capability semantics: native `ChatView.tsx:3297-3303` requires a present provider snapshot, then uses `supportsConversationRollback !== false`; an omitted optional flag on a present provider is supported. A missing provider is never inferred from its driver name. Native exposes `supportsConversationRollback` in provider status (`packages/contracts/src/server.ts:266`); parent must preserve the actual selected provider's value. `rewinding` is the parent's global **environment + thread** operation guard, not merely popup visibility.
5. Call `outbox_envelope(authenticated_environment.id, input, fresh_projection, safety)` for first admission. It binds environment, requires explicit confirmation/file choice, validates request UUID and reference identities, re-derives the current reference and rejects changed/unavailable checkpoints/providers. It returns `{environment,command}` without touching disk or dispatching. `command` is the lower-level builder and alone cannot bind an environment.

Input from the QML `requested(request)` signal:

```json
{
  "requestId": "b3e86a43-744c-46ec-b5f8-65ae79316907",
  "environment": "local",
  "thread": "thread:one",
  "confirmed": true,
  "restoreFiles": false,
  "reference": "the complete object returned by reference(), not this placeholder"
}
```

The **only native mutation** to dispatch, using the existing shared `rpc`, is:

```text
rpc(environment, "orchestration.dispatchCommand", {
  "type": "checkpoint.rollback",
  "commandId": "omarchy-b3e86a43-744c-46ec-b5f8-65ae79316907",
  "threadId": "thread:one",
  "scopeId": "scope:one",
  "checkpointId": "checkpoint:1",
  "restoreFiles": false
})
```

These IDs are illustrative fixture values. `turnCount`, `messageId`, `runId`, `confirmed`, environment and recovery data **are not fields in this native command**. `omarchy-<requestId>` matches the existing outbox command-ID convention; the UUID is never regenerated on uncertain retry. Identifiers are preserved verbatim; local validation rejects whitespace/control characters and oversized IDs rather than repairing them. This is deliberately stricter than native branded nonempty-string schemas.

## Parent outbox/IPC integration: mandatory ordering

- Add an explicit checkpoint branch **before** the existing `write::dispatch` generic message branch. Otherwise a new `op` can accidentally fall through to `message.dispatch`. No independent persistence or RPC implementation is provided in this module.
- Serialize admission with send/rewind for the same environment/thread; verify the original selected environment and thread again, preflight current uploads and restored attachment capacity, then capture recovery data and obtain the validated envelope. Save the existing draft without clearing it. Persist the exact envelope/recovery operation **before** calling native RPC. Disk failure means no mutation.
- For an existing durable request, require explicit retry, environment equality and exact persisted native command equality against `confirmed_command(input)`; also compare the persisted full input/reference/recovery operation. Reject a changed checkpoint, file choice, environment or message even though the RPC payload does not contain message IDs. Do not run first-admission `reference()` again for an identical durable retry: completion can remove/stale its source. Check the operation is still unresolved and authenticated; never replay a known terminal/superseded operation. Native deduplicates by command ID.
- Keep this operation outside send-new-thread/draft-clear logic: native receipt is not permission to clear composer text. Transport disconnect/timeout/lost response means **uncertain**, not failed or complete. There is no automatic retry.
- After receipt, subscribe/refetch the original native projection with a monotonically scoped subscription/revision. Call `observation(input, projection)`. It returns `pending`, `complete`, `failed` with native error, or `superseded`. Completion requires both this command's `rollbackRequestId` and the original message run being `rolled_back` beyond the target ordinal. Missing messages or missing runs in a bounded projection are **not** success. An old rollback ID captured in `priorRollbackRequestId` is baseline, not supersession.
- Watch native async `rollbackFailure.requestId`, not just the dispatch result. Native accepted rollback effects can fail later. A terminal native failure cannot be restarted by replaying its deduplicated command; **Review and retry** must refresh the reference and ask for a new explicit choice/new UUID. A superseding operation must be manually reconciled, never replayed as an uncertain retry.
- On timeout (native UI uses 120 seconds), retain pending operation/recovery data and global lock; refresh and/or explicitly replay the exact ID only if the outcome remains unknown. On close/navigation/restart, retain it in the parent. QML only detaches; it does not cancel an already admitted native rewind or release that guard.

## Draft recovery and transcript refresh

Recovery belongs in the shared parent draft store, including after navigation or restart. After **verified complete**, append once to the **latest** draft for the original environment/thread, not the currently selected thread. `append_prompt(current, restored)` preserves existing text with two newlines and retains it when restored text is empty. Do not overwrite or replace a draft that changed while waiting. Journal the recovery completion/idempotency marker atomically with the draft update so late results/restarts cannot append the prompt or attachments twice. Do not rely solely on the QML `completed` signal: a closed or different-scope popup intentionally rejects late results.

For exact native text semantics, pass the original message through the existing equivalent of `recallableComposerPrompt` (`apps/web/src/components/chat/composerPromptHistory.ts:110-145`) before `append_prompt`: native strips send-time terminal/element/review/preview context blocks and the Claude ultrathink prefix. The helper here intentionally preserves raw source text; it is **not** a port of that rich-context parser. Keep structured context metadata separate rather than blindly reinserting stale serialized context as text. Parent must complete/reuse that parser for rich-context prompt-recall parity.

Prepare restored attachments **before** dispatch as native does (`ChatView.tsx:8552-8567`). Resolve attachment assets through the existing authenticated asset/upload pipeline, preserve current attachments and check combined capacity. Stored native metadata is not a local `File` or a fresh upload token. Attachment preparation failure must block admission; a later recovery failure must retain the recovery journal and offer recovery without reverting again. Do not fabricate browser object URLs or overwrite existing attachments.

After native completion, refresh/reset the original thread's transcript, history cursor, queue, task/approval state, checkpoint/diff summary and render cache from native read-back; do not splice visible rows by guessed indices. Recompute available revert references. Only refocus/update cursor in the composer if the original environment/thread is still selected. Sending into a thread with an unresolved rewind remains blocked by the parent guard.

## QML API and scoped receipt envelope

Register `T3CheckpointRevert` in the shared `qmldir`, instantiate once with `bridge` (existing `uuid()`), `popupHost`, selected `environment`/`threadId` and all safety properties. Add the parent's user-message action with the existing `undo-2` icon and translated **Edit from here** label. `begin(reference)` opens confirmation without allocating an ID or sending. The two choices call `choose(bool)` and emit `requested(request)` once. Parent handles durable admission/dispatch above.

Route both acknowledgements and native observations back through `accept(receipt)`:

```text
{
  requestId, environment, thread,
  reference: originalFullReference,
  restoreFiles: originalBoolean,
  state: "accepted" | "pending" | "complete" | "failed" |
         "rejected" | "uncertain" | "superseded",
  error: "" // or bounded, plain-text failure
}
```

This is a **local integration envelope**, not a native RPC schema. The popup rejects foreign ID/environment/thread/reference/file-choice and closed/late responses. `accepted` displays waiting; only `complete` displays success/emits `completed(request)`. `rejected` is a definitive pre-admission rejection, never a network error. Late acknowledgements cannot regress terminal success/failure; once superseded, late receipts are ignored. `retry()` emits the unchanged saved request only for uncertain outcomes, never for native failure or supersession.

`refreshRequested(request)` is read-only reconciliation of the durable original operation. `reviewRequested(messageId)` asks the parent to fetch the current projection and call `begin(newReference)`; it does not automatically retry. `resume(savedRequest, "waiting" | "uncertain" | "superseded")` restores an unresolved persisted operation without sending. `dismiss()` and `closed()` detach only; the parent still owns pending state. On environment/thread changes, popup state clears and late UI receipts are ignored; keep handling results in the parent's durable operation map.

Add `checkpoint-revert` to the shared serial test-runner allowlist and run with the parent's existing Wayland/Omarchy environment. The authored fixture uses an isolated floating window and no `T3Backend`; no live account/thread/draft/Git writes occur. UI rendering/pixel parity remains unverified until that serial run and visual inspection.

## Translation keys for parent catalogs

All component-visible English strings use `T3I18n.tr`; untrusted IDs/errors use plain text. Add/reuse these keys (plus the parent trigger `Edit from here`) in all catalogs; no shared catalogs were edited:

- `You do not have permission to rewind this thread.`
- `Reconnect this environment before rewinding.`
- `This provider does not support reverting conversation history. Start a new thread instead.`
- `Interrupt the current turn before reverting checkpoints.`
- `Wait for attachments to finish preparing before rewinding.`
- `File restore requires an isolated worktree. Rewind the conversation without restoring files instead.`
- `Another rewind replaced this operation. Refresh the thread before continuing.`
- `The checkpoint is no longer available. Refresh the thread and try again.`
- `Invalid rewind request. Refresh the thread and try again.`
- `A rewind is already pending for this thread.`
- `Edit from here?`
- `Close`
- `Rewind chat to before this message. Your prompt and attachments return to the composer.`
- `This action cannot be undone. Existing draft text and attachments are preserved.`
- `Sending rewind request…`
- `Waiting for the thread to rewind…`
- `Outcome unknown. Refresh to check before retrying the same action.`
- `Thread rewound.`
- `The thread could not be rewound.`
- `Cancel`
- `Revert files too`
- `Revert and keep changes`
- `Refresh thread`
- `Retry same action`
- `Review and retry`
- `Done`

Map remaining local validation errors via the parent's normal `T3I18n.error` catalog rather than showing raw codes; preserve native server failure details as selectable/plain text. Do not mark full checkpoint parity complete until parent outbox, preflight, draft recovery, rich-context recall, serial QML and transcript read-back are exercised.

## Parent shared integration (installed 2026-10-17)

`src/checkpoint_journal.rs` owns private atomic per-action/per-thread outbox records and a shared per-thread flock. `write::dispatch` takes that same lock and refuses new message/queue/request mutations while a rewind is unresolved. Late recovery/results for an old ID update only its archive, preserving the newer thread guard. `src/checkpoint_operations.rs` implements fresh native auth/config/full-projection preview and confirmation admission, native asset download + fresh attachment upload before rollback, exact durable command replay on explicit uncertain retry, correlated native observation, local once-only recovery and explicit resolved-operation reconciliation. There is no Git/reset/provider-specific rollback fallback and no automatic mutation retry.

`drafts` moved into the library for one shared store. Recovery uses its per-draft flock and a single atomic draft document containing text, attachment descriptors and optional recoveryIds. Ordinary stale autosaves missing a committed recovery ID are rejected; the bridge then rebases recovery onto its latest in-memory draft. The composer is frozen only for that local recovery transaction; typing during native rollback remains possible while sends are blocked. Original environment/thread ownership survives navigation/restart. If recovery fails after rollback, the UI retries only local recovery, never another native rollback. Native attach/file limits and the adapter's120k-character draft bound are checked before admission and again against the latest draft.

`src/prompt_recall.rs` ports native recallableComposerPrompt: trailing review comments, terminal/element/preview blocks and canonical context chips are stripped, typed content is preserved and app-composed plan/attachment-only prompts are omitted. The same projected recall text feeds ordinary prompt history. Native run status rolled_back hides its user/assistant messages, activity, plan cards and checkpoint file rows after read-back; old paginated history is reset on successful recovery.

The shared `T3CheckpointRevert` confirmation is used from the user-message action. Native projection/context is re-read before opening and again at admission. Backend operations live in a dedicated queue independent of selected-thread detail cancellation, and restart discovery/observation is read-only. A compact composer status lets the user reopen an unresolved operation after navigation. Existing translated confirmation strings and shared buttons are reused; the popup mirrors the native title, description and file choices; internal checkpoint IDs are not displayed. Confirmation closes after a choice, as native does, while progress remains in the composer banner. Reopening progress or closing it only detaches that view. Explicit user attestation after native inspection can release an uncertain/superseded guard without replay; if native read-back proves completion, recovery is performed instead.

Final verification: Rust checkpoint_revert9, checkpoint_operations2 (including production HTTP/Effect RPC, ACK != completion, exact replay ID, late old recovery/new guard, permission failure, stale autosave rejection, original/latest draft merge, correlated failure and signed asset/new upload), prompt_recall2 and existing drafts5/writes1/plan transport+workflow regressions. QML checkpoint-revert and actual-panel checkpoint-flow pass; twelve serial QML regressions pass (checkpoint-revert, checkpoint-flow, project-drafts, draft-undo, composer, send-new-thread, plan-flow, git-flow, preferences-defaults, work-details, copy-actions, queue). The full focused Rust group passed26tests. Catalog20locales ×700keys, no warnings/no missing literal UI keys; plugin validation and diff checks pass. Read-only preview succeeded on a real settled thread in this project, returning the exact native checkpoint reference; no live rollback/file restore/message was sent. Final scoped backup `/tmp/ot3-before-checkpoint-revert-xe73733v`;13draft hashes, original selection/open panel and ready local/node environments preserved. [Spanish confirmation fixture](audit/screenshots/ot3-checkpoint-confirm-fixture.png) inspected; this is not a native pixel comparison. A real native rollback/provider round-trip remains unverified.

Native data-shape correction found by live verification: `OrchestrationV2ConversationMessage` has no inputIntent (`packages/contracts/src/orchestrationV2.ts:1204-1229`). Native `session-logic.ts:651-683` joins intent from projected user_message items. The adapter and fixtures now do that join and verify message/run identity; raw conversation-row inputIntent cannot override it. A bounded snapshot can offer a read-only full-context preview if the item is omitted and a ready checkpoint exists; mutation still requires the exact native item/reference. Updated9pure tests and2transport/projection tests cover this actual shape, typed-while-waiting text/files, capacity rejection before rollback and hiding all reverted-run rows.
