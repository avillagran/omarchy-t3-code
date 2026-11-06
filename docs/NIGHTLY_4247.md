# Nightly 4247 parity work

Target: [v0.1.8-nightly.20261106.4247](https://github.com/pingdotgg/t3code/releases/tag/v0.1.8-nightly.20261106.4247),
commit `e136e8e3d4cb85a56463d449f1d3835313737e43` (2026-11-06).
The overall target has advanced to [Nightly 4262](https://github.com/pingdotgg/t3code/releases/tag/v0.1.8-nightly.20261106.4262),
commit `c55db3e9c72fdf584f4b54192fb39beb88667b59`. The feature behavior documented here
was introduced in 4247; the server source comparison confirms the fresh-context
routes are unchanged in 4262.

The validated reference in `upstream/t3code.json` stays at 3726 until the remaining
integration work passes. This document does not certify full parity.

## Scheduled task conversation modes

The task editor offers **New thread each run**, **Continue conversation**, and
**Fresh session each run** when the selected server advertises
`environment.capabilities.scheduledTaskThreadModes`. A fresh-session task keeps
one thread while the server starts a new provider session for each later run.

Existing explicit modes survive edits, including fresh-session tasks that have
not run and therefore have no bound thread yet. Legacy tasks infer their display
mode from their thread binding and omit `threadMode` on unchanged saves. Servers
without the capability receive the previous payload. An explicit unsupported mode
is rejected before mutation. Selecting new-thread mode detaches the binding only
in the submitted payload; changing the selection back before saving preserves it.

Task snapshots load through `scheduledTasks.subscribe`, with a fallback to
`scheduledTasks.list` when a legacy server rejects the subscription endpoint.
This also supports intermediate Nightlies without task conversation modes.

Protocol implementation: lib-t3-rust branch `feat/nightly-4247-task-modes`,
commit `5c236f49d0f7106b488184b2879a8a4a86c859ba`, vendored without local changes.
Reference contract: `packages/contracts/src/scheduledTask.ts`,
`scheduledTaskThreadMode` and `ScheduledTaskUpsertInput`; reference UI:
`apps/web/src/components/settings/ScheduledTasksSettings.tsx`.

Validation: 14 protocol API/stream tests, 123 client unit tests, three
WebSocket integration tests, the scheduled-tasks QML fixture, and all 20
translation catalogs. No running client/server or user task was modified.

## Fresh sessions in the desktop composer

`/clear` prepares the next message to start a new provider session in the same
thread. It preserves visible history, offers an undo banner, and appears in slash
completion when the server advertises `environment.capabilities.freshContext`
and the current thread has messages. Selecting the completion arms directly;
the whole-message command is case-insensitive. Questions and approvals retain
ownership of the composer. Attachments prevent interpreting text as a command.

Preparation is ephemeral and scoped by environment/thread. Once submitted,
`freshContext` and the arm identity survive draft/outbox persistence and retries.
A fresh-context message queues instead of steering an active run. Acceptance
consumes only the captured arm, preserving an arm made while a send was in flight.
Failures preserve the draft and intent. The native adapter checks the capability
again before dispatch. The transcript marker comes from the server's
`user_message.freshContext` turn item, without removing earlier messages.

Protocol revision: `eeed1780c07aa4e92a4ac5376878457edd38be53`, on the same library
branch as task modes. Reference behavior: `packages/shared/src/composerTrigger.ts`,
`ChatView.logic.ts::planComposerTurn`, `chat/useComposerSend.ts`,
`chat/freshSessionArms.ts`, and `chat/ChatComposer.tsx` at the target revision.

Validation: five protocol command tests; two client integration tests covering
capability rejection, queued wire payloads, exact retries, persisted attempts,
and transcript marker updates; QML fresh-session, plan-flow and completion
fixtures; 20 translation catalogs and vendor hashes. The new fixtures use
synthetic servers and isolated state. No real provider session was restarted.

O3 Server has a candidate implementation on branch
`feat/nightly-4247-fresh-context`, commit `4e5fb01`. It now advertises the capability after ten supervisor tests,
60 cases executed from exact T3 source functions, eight real Claude/Codex turns,
and validation of a disposable shared-schema copy (81 records and 80 events,
zero rejections with the reference's Effect 4.1.2). It preserves multi-thread
sessions and canonical currency-qualified context costs. A native T3 runtime
comparison, migration-80 acceptance and a live desktop-composer check remain;
this does not establish full-platform parity or a deployed feature. Server work
is isolated from the unrelated goal-lifecycle changes in the owner's checkout.

## Remaining work

The lexical inventory from 3726 to 4247 discovered 65 additional entries across
RPCs, orchestration, HTTP, settings, and keybindings. Entries are unreviewed;
referencing a name does not establish an implemented UI or equivalent behavior.
Refreshing to 4262 adds `client-setting:toolCallVisibility` as missing and
unreviewed. This scanner is a discovery aid; its counts must still be reconciled
with the broader headless inventory in O3 Server.
Regenerate the detailed local report with:

```sh
node tools/upstream-drift.cjs --source /path/to/t3code --rev v0.1.8-nightly.20261106.4262 --write
```

Next integration areas include fresh-context execution in O3 Server, scheduled task
session execution in O3 Server, pending questions outside the loaded transcript,
secret-provider requests, dictation environment selection, and refreshed audits
of historical UI differences. Shared-state schema compatibility must be checked
before changing the server reference or deploying a newer migration head.
