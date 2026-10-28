# Native keybinding settings and runtime

Preferences contains the editable native keybinding workflow and dispatches
implemented chat actions using the original effective-binding resolver. The
scoped installation is verified; full application/shortcut parity remains
active because native workspace commands still need their corresponding surfaces.

## Reused original logic

`node tools/build-keybindings.cjs [native-checkout]` generates
`ui/js/NativeKeybindings.js`, its provenance manifest, and the Rust command
allowlist `src/keybinding_commands.json`. The build requires Node exposing
`module.stripTypeScriptTypes` and the exact native checkout
`5a4e9980533705774dc3f45b35cacf76c3d78dbc`. All eight input files must match their
committed versions; a dirty input or different revision is rejected.

Original MIT sources:

- `packages/contracts/src/keybindings.ts`: limits and 82 static commands.
- `packages/shared/src/keybindings.ts`: 82 default resolved bindings, shortcut
  parsing and matching, conditions and default merging.
- `apps/web/src/components/settings/KeybindingsSettings.logic.ts`: actual native
  row grouping, labels, filtering, Default/Custom/Project sources, reset target,
  conflicts, condition variables and key capture.
- `apps/web/src/components/usage/usageShortcuts.ts`: usage command order/labels.
- Pure helper slices from `KeybindingsSettings.tsx` (condition-tree operations),
  `apps/web/src/keybindings.ts` (labels/condition evaluation) and
  `packages/contracts/src/editor.ts` (editor metadata/order).
- `apps/web/src/projectScripts.ts`: native script command-ID extraction.
- The keybindings runtime slice also includes original conflict keys, effective
  shortcut selection, and primary-application versus active-script resolution.

There is no replacement shortcut parser. The build erases TypeScript, removes
imports/exports, provides the original dependencies in order and makes explicit Qt
compatibility adaptations: `toSorted` becomes `slice().sort()`, the group-only
`flatMap` becomes map/concat, and the recursive `parseOr` binding is declared
before its closures; native runtime context spread becomes Object.assign. The
pinned script schema predicate is provided without importing Effect, and Qt
uses the native desktop context. The manifest records all adaptations, input
hashes and output SHA256.
The plugin runtime needs neither Node nor npm/network access.

QML: import the generated library and use `Native.nativeKeys`. It exports 46
original helpers/defaults. Keep all rows for conflict checks even when a filter
is active. Native conflicts are conservative labels, not a logical satisfiability
solver. Repeated modifiers are normalized by the original parser; do not add a
separate rejection rule. A literal space is a valid resolved shortcut key.

## Backend and bridge contract

`keybinding_settings::read(&Environment)` reads native `server.getConfig` after
fresh `orchestration:read`. Its bounded projection includes `keybindings`, only
`keybindings.*` issues, the native keybindings path, a revision, environment,
writable (`settings:write`) and opaque origin-bound `viewId`. Raw provider/server
configuration never crosses IPC. Unknown future rule/AST forms are skipped like
native `ForwardCompatibleArray`; a missing or oversized config is an error.

`keybinding_settings::apply(&Environment, input)` accepts:

```json
{"viewId":"opaque","action":"upsert","input":{"command":"composer.sendAndNewThread","key":"mod+shift+enter","when":"composerFocus && !draftThreadRoute","replace":{"command":"composer.sendAndNewThread","key":"mod+alt+enter","when":"composerFocus && !draftThreadRoute"}}}
```

Actions are `upsert` and `remove`. Remove accepts only the original
`{command,key,when?}` target; reset uses upsert with the original helper's default
key/condition and current replacement target. Native mutation RPCs are exactly
`server.upsertKeybinding` / `server.removeKeybinding`. The server owns file parsing,
merging, atomic persistence, cache updates and change emission; O3 never writes
or downloads keybindings.json directly.

Before dispatch, validate fresh read/write permissions and the cached origin and
native config revision. An external edit requires reload. The opaque token is
consumed before RPC; it cannot be reused after acknowledgement loss. The native
RPC has no command-id/CAS parameter. Fresh native read-back must match the
acknowledged binding list before returning `state:complete` with a new read view.
Post-dispatch failure returns `state:uncertain`. There is no automatic write retry;
reload/inspect the native state before a further explicit edit. The fresh check
is not an atomic transaction with other native clients.

Local IPC `keybindings-read` / `keybindings-write` are routed on the existing
separate preferences worker and echo transport `requestId` / environment.
Read-only CLI: `target/release/omarchy-t3-client keybindings-read <environment>`.
Do not add a CLI write shortcut for validation.

## Current source UI and bridge

`T3Keybindings.qml` replaces the fixed Preferences list with the original group
hierarchy, filtering, key pills/capture, Add command selector, per-row save/reset/
remove and conflict badges. `T3WhenEditor.qml` and `T3WhenNode.qml` port the original
recursive condition tree, add/remove/group/not controls, raw expression field,
last-valid-tree preservation during invalid text and unknown-variable warnings.
Their transformations and evaluator use the original generated helpers. Recursive
Qt Loader children derive their membership directly from the input node, and set
depth before the node, to avoid stale intermediate bindings during construction.
The popup editor loads only when open. `QtKeybindings.js` adapts Qt/Linux XKB scan
codes to the original recorder's physical punctuation/digit events and exposes
native formatting/condition matching.

The bridge captures the exact selected environments; every target must have a
fresh writable view before fan-out. Each native result is retained separately.
Partial failures keep the Add draft and disable writes until an explicit reload.
IPC loss remains uncertain with no write replay. Same-revision reads preserve row
drafts; scope changes close capture. Existing global Qt shortcuts are suppressed
while recording, so they cannot steal the captured combination. Native file-open
uses the representative environment's view/path and actual native editor list.
It checks fresh `orchestration:operate`, native configured path and availability,
then calls `shell.openInEditor`; no local xdg fallback for a remote path. Default
editor selection follows native definition order. Existing Electron browser-local
favorite-editor selection has not been imported into O3.

## Runtime routing and native updates

Original `state/primaryEnvironment.ts`, `state/server.ts` and `ChatView.tsx`
distinguish primary-server application bindings from active-thread-server project
scripts. The snapshot explicitly identifies the native primary as `local`; it is
not inferred from the selected chat. `ShortcutRouting.js` uses the original
`effectiveShortcutsForCommand` across every command. Later conditional claimers
win, including commands whose surfaces are not yet implemented; an unsupported
claimer must not accidentally invoke an earlier action or a legacy Qt alias.

`T3ShortcutRouter.qml` registers effective Qt WindowShortcuts and rebuilds them
when context/configuration changes. It coordinates the older local aliases,
recording, Preferences, model/permission pickers and modal workflows. The
requested Enter-send and Shift+Enter-newline behavior is reserved while the
composer is focused. Ctrl+Enter remains a send alias, and Ctrl+Shift+Enter starts
a new thread. The expanded editor keeps plain Enter for multiline editing. Native DOM Enter maps to both Qt Return and keypad Enter.

Implemented dispatch: sidebar, conversation find, new/new-local, thread previous/
next/jumps, existing external native-terminal frontend, working-tree diff,
model/provider/model-number controls, permission/effort pickers, latest-user undo
preview, stop, PR-link/thread-ID copy, project draft stash, send/new-thread and
first queued steer/latest queued edit. New Local selects root explicitly without
rewriting the saved worktree default. Permission mode is not Plan/Build.

Script shortcuts first read a fresh native Git/script/permission scope. Removed
scripts, read-only roles, changed selection and pending/unknown native operations
cannot dispatch. Success feeds the existing journaled terminal open/write
pipeline once; it never sends the composer draft or automatically retries a
script. Its existing progress and terminal-attachment UI remains visible.

Native `subscribeServerConfig {}` supplies invalidations for version1 snapshot/
keybindingsUpdated events only. Raw config/provider payloads never cross IPC.
Visible primary/active/editor-scope environments are watched; hiding stops the
streams. Nonces reject late events; capture and writes defer fresh bounded reads,
including an event arriving during an older read. A read-only 30-second refresh
also covers reconnection/older servers; it does not replay edits.

Command names, conflict labels, command choices and translated search reuse the
original English command labels plus localized fixed segments. Numbers and script
names remain data. F1 help now lists actual effective implemented routes and
unclaimed local aliases, with bounded scrolling.

## Limits and remaining parity

The editor preserves all82native static commands and project script commands.
Commands for absent workspace/browser/file/search/palette/usage/theme and other
native surfaces remain unimplemented; they are not redirected to unrelated chat
actions. The complete-screen goal is not complete. Embedded terminal/split/close
behavior is still an external-frontend adaptation. The fixed user send keys are
an intentional difference from the original defaults.

Recording adapts native physical punctuation/digit codes. Runtime registration
uses Qt QKeySequence; non-Latin layouts, alternate physical punctuation mappings,
AltGr, dead keys, IME and compositor-reserved combinations have not been verified
as equivalent to native browser event matching. Actual Qt Latin shortcuts/Enter,
context changes and conflict precedence were tested. No real native keybinding
edit, editor-open, script run or message send was used for verification.

Existing native browser-local favorite editor selection is not imported. A fresh
native revision check before an edit is not atomic CAS with other native clients;
unknown outcomes remain explicit and are never automatically replayed.

## Verification

`cargo test --release --test keybinding_settings` covers safe projection, native
space/script commands, schema bounds, unknown rows, exact replace/remove payloads,
production HTTP/Effect WebSocket roles, fresh-config rejection, successful
read-back, consumed-token protection after lost ACK, and origin mismatch.

Serial QML checks `keybindings-editor`, `keybinding-row`, `keybindings-flow` cover
the recursive tree/invalid drafts, actual key capture/Escape/replace/reset/remove,
real Preferences editor, all-environment permissions, fan-out partial results,
draft preservation, native file-open intent, stale/wrong receipts, explicit reload
and IPC-loss uncertainty. `preferences-scope`, `archive-preferences`,
`preferences-design`, `composer`, `send-new-thread` regressions passed.

`tools/test-qml keybindings-logic` runs the real generated logic in Qt: defaults,
group order, custom overrides/scripts, command filtering, plus/repeated modifiers,
condition parsing/unknown variables, physical punctuation, recorder and conflicts.
Live local/node reads are read-only; no real shortcut file is modified by tests.

Live read-only source-binary checks: local/node each returned82bindings and0issues.
Both can write/open by role; native available-editor counts were local0/node2, so
the file-open control is correctly unavailable locally with that native config.
Catalog20x804 warning-free. Latest live reads use current native IDs: `local` and
`8b55eaad-3149-443c-953f-afd6dd002666` (node); both return82bindings/0issues.
Both config streams emit valid native notifications and close cleanly. The old
`node` alias now returns unknown_environment; it is not the current environment ID.
No real shortcut edit or real file-open operation was performed.

Additional Rust config-stream test covers exact RPC/payload, ignored provider/
settings/future-version events, permission denial and cancellation Interrupt.
`shortcut-routing` sends actual Qt keys only after focusing its own fixture PID;
its runner requires an explicit or unique Hyprland compositor socket.
`keybindings-runtime` covers primary/active ownership, draft preservation, modal/
capture guards, event nonce/scope and hidden cancellation. `script-shortcuts`
covers fresh roles/membership/scope, selection cancellation, unknown/pending
suppression and exactly one journaled intent. Git controls/operations regressions
and native model-provider wrap/disabled/search behavior also pass.

The existing native light-mode screenshot and updated Spanish dark fixture were
visually reviewed for groups/rows/key pills/search/condition controls. Different
DPR/theme/font/fixtures mean this is structural evidence, not pixel equality.

Scoped install verified 2026-10-17: backup
`/tmp/ot3-before-keybindings-runtime-i_zbjay_`, installer
`/tmp/ot3-install-keybindings-runtime.py`. All23installed files match source.
All13draft hashes, native client-settings, selected thread and panel-open state
were preserved. Local and native-ID node are ready; messages load without error.
The restarted desktop log has no TypeError, ReferenceError or binding loop from
this integration. Global plugin locations were not modified.
