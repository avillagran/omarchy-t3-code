# Native storage settings parity

Reference: T3 Code `5a4e9980533705774dc3f45b35cacf76c3d78dbc`, `apps/web/src/components/settings/StorageSettings.tsx`, `scopedSettings.ts`, `packages/contracts/src/settings.ts`, and `packages/shared/src/projectSettings.ts`.

Status: backend contracts and the shared native retention control are implemented. The Storage preferences page, cleanup report subscription/run controller, translations and original visual comparison are still missing. This does not establish screen parity or enable a local cleanup implementation.

`src/storage_settings.rs` projects only worktree/artifact/log retention settings and four capability flags. It validates nullable integer retention from1 through3650 days, exact keep-when options and boolean rules, and trims the worktree directory. Partial environment patches keep sibling settings. The shared QML days control accepts a per-field initial value: storage enables at8 days, while existing settlement remains3 days.

The original resolver is ported after scope folding. Inherit uses environment rules, Off disables the four deletion rules while keeping the environment's preservation policy, and Custom uses its complete rule set. Project edits complete partial rules from each target's effective policy and preserve unrelated overrides. Switching Custom to Off removes the custom-rules shape; Inherit removes the project override. Mixed mode and mixed effective rules remain distinct across targets. Environment worktree edits also update a pre-existing root worktree policy, matching the original scoped write planner; artifact/log retention remains environment-only.

Writes require the advertised native capability, with project overrides also requiring project cleanup support. Source discovery and cleanup are not performed locally. Scoped responses expose safe storage capability flags for the future native scope notices. New fields are filtered against native capabilities and participate in the existing exact-target planner and acknowledgement/readback flow.

`tools/build-storage-cases.cjs` checks the pinned original file and executes its unchanged `resolveWorktreeCleanup` body after TypeScript erasure. Its dependency adapter supplies already-folded settings; it does not claim to test the complete original project resolver. Nine generated cases cover all preservation choices with Inherit, Off and Custom. Rust tests also cover retention bounds, safe projection, partial-policy completion, mode changes, mixed rules and an actual isolated native WebSocket update/readback. QML checks exercise the shared native control and existing source-control preferences.

Next: mount the complete native Storage page with the original section/order/conditional visibility, project policy chooser and scope notices. Implement native `server.getStorageCleanupReport` and `server.runStorageCleanup` via the existing worker administration queues, using original permissions and per-environment single-flight behavior. The original report includes trigger/timestamps, removed/kept/failed counts, byte totals, omitted count and worktree/browser-artifact/log entries. Preserve partial/unknown outcomes and do not retry uncertain deletion. Then complete locale coverage and inspect native rendering against the original.

## Low disk space notice (ot3 only)

T3 does not warn about low free space, and its servers do not report it (neither
0.0.46 nor 0.1.2 exposes a free-space reading). ot3 therefore checks **this
computer only**: every 5 minutes `src/disk_space.rs` runs one `statvfs` on
`$T3CODE_HOME/worktrees` (or the T3 home). Below the threshold (15 GB by default;
Storage → This computer, stored as `lowDiskThresholdGb` in the panel state, Off
disables it) a dismissible card appears above the chat with:

- **Clean up now**: `server.runStorageCleanup` on the local environment, then a
  new reading;
- **Settings**: this Storage page, with the cleanup rules (all off by default);
- **Worktrees**: measured on demand, each worktree of a local thread or under the
  worktree folder with the size of its `target/`, whether its PR is merged or its
  HEAD is in the default branch, and whether it has uncommitted or untracked files.

`omarchy-t3-client disk-report` prints the same report. Remote environments are
not checked.
