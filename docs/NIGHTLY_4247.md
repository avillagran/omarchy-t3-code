# Nightly 4247 parity work

Target: [v0.1.8-nightly.20261106.4247](https://github.com/pingdotgg/t3code/releases/tag/v0.1.8-nightly.20261106.4247),
commit `e136e8e3d4cb85a56463d449f1d3835313737e43` (2026-11-06).
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

## Remaining work

The lexical inventory from 3726 to 4247 discovers 65 additional entries across
RPCs, orchestration, HTTP, settings, and keybindings. Entries are unreviewed;
referencing a name does not establish an implemented UI or equivalent behavior.
Regenerate the detailed local report with:

```sh
node tools/upstream-drift.cjs --source /path/to/t3code --rev v0.1.8-nightly.20261106.4247 --write
```

Next integration areas include fresh-context messages (`/clear`), scheduled task
session execution in O3 Server, pending questions outside the loaded transcript,
secret-provider requests, dictation environment selection, and refreshed audits
of historical UI differences. Shared-state schema compatibility must be checked
before changing the server reference or deploying a newer migration head.
