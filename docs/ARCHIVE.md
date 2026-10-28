# Archived threads

`Preferences > Archive` ports native `ArchivedThreadsPanel` from pinned T3 Code
`5a4e9980533705774dc3f45b35cacf76c3d78dbc`. Source:
`apps/web/src/components/settings/SettingsPanels.tsx:3443–3655`,
`lib/archivedThreadsState.ts`, `hooks/useThreadActions.ts`, and
`packages/contracts/src/orchestrationV2.ts`.

The native archive query is **`orchestration.getArchivedShellSnapshot {}`**; its
archived rows are in `threads`, not `archivedThreads`. Project order and descending
archive timestamp / thread ID order follow T3. Environment, project and checkout
selection use exact native project IDs; a removed or empty selection does not
expand to all projects. Environments can return independent loading/error states.
The list is virtualized and reconciled by identity to retain scroll position.
Titles are plain text. Groups use the shared project icon component and dates
follow the native minute/hour/day relative format.

Unarchive and the right-click Unarchive/Delete menu dispatch exactly
`{type:"thread.unarchive"|"thread.delete",threadId,commandId}` through native
`orchestration.dispatchCommand`. Unarchive does not navigate. Archived deletion
performs no worktree cleanup and never removes independent project drafts.
The native canonical `confirmThreadDelete` preference is read with its default
`true`; the explicit Delete click skips the dialog only if that preference is
`false`. O3 does not change that preference.

## Admission and completion

`archive_management.rs` caches bounded, origin-bound read snapshots. Before a new
mutation it verifies fresh native `orchestration:operate` and `orchestration:read`
permissions, selected row membership, and the current thread/project/title/archive
fingerprint. A changed or expired row needs refresh. All commands retain their
captured environment even if the user changes scope or closes Preferences.

A private local journal persists the exact command before the RPC. Per-thread
admission uses the shared checkpoint lock; an unresolved archive operation blocks
a new archive action for the same environment/origin/thread. A valid sequence
acknowledgement means **accepted**, not completed. Read-back checks the archive
and active shell snapshots, including `archivedThreads`, and requires acknowledged
sequences to be visible before reporting the desired state. This observes the
desired state; it does not claim a correlated native completion event.

Transport loss remains uncertain and discoverable after reopening/restarting.
Check status only reads. Explicit Retry reuses the exact saved command ID after a
fresh permission/fingerprint check. There is no automatic mutation retry; visible
accepted operations receive read-only status refreshes. Late observations of an
older action cannot overwrite a newer thread guard. Read-only accounts can view
the archive but cannot mutate it. Dedicated IPC work avoids blocking draft/send
workers during archive queries.

## Verification and remaining scope

- `cargo test --release --test archive_management --test connection_preferences`:
  scoped grouping/order/projection, malformed inputs, production HTTP/Effect WS,
  journal-before-send, permission changes, confirmation, stale row rejection,
  ACK versus completion, lost ACK, identical explicit retry, restart discovery,
  superseded IDs, origin mismatch and native delete-confirmation defaults.
- Serial QML `archive`, `archive-preferences`, `preferences-scope`,
  `preferences-design`: virtual rows, stable scroll, immutable origin capture,
  synchronous locks, uncertainty, confirmation preference, scope/dialog changes,
  Preferences navigation and no unrelated settings permission requirement.
- Read-only live native archive queries on local and node succeeded, with zero
  archived rows in each. No live unarchive/delete mutation was performed.
- `audit/screenshots/ot3-archive-preferences-fixture.png` is a synthetic Qt capture,
  not a pixel comparison with the original application.

Full settings search/scroll-to-target is still partial across Preferences; this
page has no new independent search implementation. Native live mutation testing,
full original-app visual comparison and the remaining administration pages stay
pending in the screen audit.

Installation also exposed a long-thread reload failure (`too_large`) in the pre-existing noncompact chat transport. The client now requests and restores the original native compact bounded snapshot for HTTP and live socket snapshots, with a specific 64 MiB cap for bounded thread HTTP/live snapshots; generic responses retain the 8 MiB limit. See the development guide and `tests/bounded_snapshot.rs`; this is a transport correction rather than an archive mutation.
