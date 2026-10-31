# Shared drafts Desktop integration and Android host contracts

Status: **Desktop editor integration, opt-in**. Preferences → Connections persists
an explicit bridge endpoint ID and sharing switch (off by default). Composer and
project drafts use the durable journal when enabled; local files remain offline
editor caches. No deployment or remote Desktop listener is included.

Reviewed against Desktop base `55c0ce3e81e5aaf348e263e8896cceec6d52a4e6`,
Android `origin/main` `9ef8deb6780557134ec7f3a975399b9fc5d0389a`
(`docs/verification/desktop-parity-status.md`, P14–P17), and OmaBridge
`docs/shared-drafts.md` / published v0.2.4 SHA256SUMS on 2026-10-31.

## Local durable draft journal

`omarchy-t3-client shared-drafts` reads one JSON object from stdin. This is a local
owner CLI, **not** an Android network API. It does not discover/select a bridge or
infer an environment from a display name. Supply an explicitly selected authority
endpoint ID, a stable source environment UUID and project UUID. The authority must
have granted this Desktop endpoint the separate `drafts` service.

Read a document:

```json
{"action":"get","authority":"<endpoint-id>","scope":{"environment":"<uuid>","project":"<uuid>","kind":"thread","id":"<thread-id>"}}
```

`action:snapshot` with the same authority assembles all bounded `list` pages before
returning `{owner,items,cursor}`. It rejects changed snapshots, foreign owners,
non-progressing pagination and oversized responses; it never resumes from a partial
snapshot. Callers must restart on `draft_snapshot_required`. Snapshot does not alter
any pending command, including offline creates against a now-absent key.

Stage a mutation **before** sending it:

```json
{"action":"stage","authority":"<endpoint-id>","journalId":"<new-uuid>","command":{"op":"put","scope":{"environment":"<uuid>","project":"<uuid>","kind":"thread","id":"<thread-id>"},"expected":null,"baseCursor":"<endpoint-id>:12","mutationId":"12.<new-uuid>","payload":{"schema":1,"text":"Private draft","attachments":[]}}}
```

`stage` does no network I/O. It accepts conditional `put`, `delete` and `transfer`
commands, enforces original owner/cursor/mutation anchoring, and rejects overwriting
an existing journal with different content. Payload validation remains authoritative
in OmaBridge. Use `action:retry` or `action:status` with the same authority/journal ID.
Retry sends the persisted command unchanged through:

```json
{"action":"drafts","peer":"<endpoint-id>","confirmed":true,"command":{"op":"put","...":"exact persisted command"}}
```

Reads use `confirmed:false`. Commands travel on stdin; stderr is suppressed by the
existing manage adapter. No command, response, content or attachment is logged.
Journal files are private (directory 0700, files 0600), atomically replaced and
fsynced with a process lock. Symlink and nonregular journal files are refused.
Concurrent access fails boundedly with `shared_drafts_busy`. The CLI's returned
JSON contains private content: consumers must not put it in diagnostics.

A transport failure leaves the command intact for an identical retry after restart.
Applied receipts are retained. Conflict, resync, snapshot-required and mutation-reused
responses are retained and block replay. `conflict:true` is returned alongside the
original command and outcome. Explicit resolution must inspect a completed fresh
snapshot and stage a **new** journal/mutation based on the chosen current revision;
the original branch remains available. There is no automatic rebase or merge.
This CLI does not itself resolve a conflict or delete a journal.

### Desktop composer and migration

`shared_editor` persists private local branches, edit generations, original bases,
completed snapshot cursors and journal identities. Local saves perform no network
I/O. A background worker resolves aliases through authenticated
`/.well-known/t3/environment`, consumes every bounded snapshot page, then stages or
retries exact journal commands. Polling alternates the selected composer and known
threads; the selected project's legacy stash is imported during project refresh.
Full snapshots are used instead of a partial watch cursor. Authority never changes
automatically. A successful authorized list confirms protocol/grant availability;
failed reads preserve all local data and do not send writes.

Exact document scopes (no additional wire fields):

- Composer: `{environment:<source UUID>, project:<project UUID>, kind:"thread", id:<thread ID>}`.
- Project stash: `{environment:<source UUID>, project:<project UUID>, kind:"stash", id:<entry UUID>}`.
- Legacy stash imports reuse the existing entry UUID; new stash entries reuse the
  durable request UUID. Backup entries have fresh UUIDs. There is no `project`
  document kind, path-derived ID, local alias or caller-supplied owner field.

Payloads use `schema:1`, plain `text`, `attachments:[{id,name,sourceEnvironment}]`,
optional `modelSelection:{providerId,modelId,effort?,reasoningEffort?,permissionMode?,speedMode?}`
and `preferences:{interactionMode,deliveryMode,permissionMode?}`. Unsupported local
model options and unfinished uploads fail closed rather than being truncated.
Selection, queue edits, send identities, outbox, approvals, questionnaires and
checkpoint recovery remain local. Imported attachment descriptors are unsendable
until verified by an existing local descriptor or replaced through normal upload;
this UI does not offer cross-host resource copying.

Imports are create-if-absent, retain original files, and persist acknowledgment
state to avoid repeating successful imports. Existing shared content and expired
mutations produce visible conflicts. Composer and legacy-stash conflicts offer
**Keep local** / **Use shared**; explicit resolution archives the original branch
and uses a new mutation anchored to a fresh completed snapshot, including absence.
Remote changes never replace a nonempty active composer. Generation/scope/authority
guards preserve edits and ignore late callbacks.

Stash/restore uses journaled atomic `transfer`; a live destination, even empty,
gets a fresh stash backup. Local before-images are persisted before dispatch and
checked before updating editor caches. Unknown outcomes retain the request for an
identical retry. Acknowledged sends clear with a conditional `delete` against the
submitted branch's base revision; newer remote content survives as a conflict.
Unassociated legacy files (for threads no longer in the native project catalog)
remain intact locally until their source scope can be resolved; no scope is guessed.

## Android wire API and security assessment

Desktop is currently a **client** of T3's authenticated native services. Its local
stdin/QML operations run with the Desktop user's saved credentials. They do not
carry a remote requesting session identity. Wrapping `write::execute`, the QML
handlers or the local CLI in HTTP would create a confused deputy. This pass adds
no such wrapper, grants, remote shell, browser or screen access.

For draft storage Android should call OmaBridge's v0.2.4 mobile ABI directly,
`{"op":"drafts","handle":N,"peer":"<authority-id>","command":C}`, with the phone's
own paired identity and explicit `drafts` grant. That existing bridge API is separate
from the new local Desktop journal CLI. This PR does not modify Android or claim
an authenticated physical-phone round trip.

The existing native terminal contract is already in the vendored library:

| Operation | Native contract | Permission / bounds |
|---|---|---|
| PTY start/attach/read | streaming `terminal.attach`, `{threadId,terminalId,cwd?,worktreePath?,cols?,rows?,providerInstanceId?,restartIfNotRunning?,resume?}` | `terminal:operate`; attach can start a process |
| PTY input | `terminal.write`, `{threadId,terminalId,data}` | `terminal:operate`; UTF-8 data 1–65,536 bytes; never automatically retry unknown writes |
| PTY resize | `terminal.resize`, `{threadId,terminalId,cols,rows}` | `terminal:operate`; cols 1–1000, rows 1–500 |
| Reconnect | resubscribe `terminal.attach` with the same IDs and `restartIfNotRunning:false` | snapshot history + sequence, followed by output; keep exited shell exited |
| Exit | attach events `exited`, `closed`, `error`; snapshots expose status | explicit observation, not inferred from successful command dispatch |

These are RPC names/payloads, not new REST routes. `terminal:operate` is the existing
specific shell permission. Android should call them with its **own** authenticated
native connection, never obtain the Desktop's bearer token. A host broker is still
needed if isolation to a particular requesting auth session, default-off consent,
and audited remote use are required beyond the native server's policy. This PR
neither asserts those server guarantees nor changes native server authorization.
The existing Desktop terminal resolves cwd from thread/project context; a future
broker must do the same, rather than accept arbitrary caller paths/env.

| Requested surface | Existing reusable implementation | Safe decision / remaining boundary |
|---|---|---|
| PTY (priority 1) | `src/terminal.rs`, `api::terminal`, `api::streams::TERMINAL_ATTACH` | Reuse native RPC above. Defer a Desktop broker until auth-session ownership, consent, revocation and audit exist. Never forward arbitrary IDs into the owner's connection. |
| Scoped documents | `src/documents.rs`, `src/document_session.rs`, native document/files API | Can expose bounded text/resource reads after host resolves caller-authorized thread resources. Defer remote wrapper: current local reader can invoke `xdg-open` and takes local cwd. No filesystem path or temporary file URL may cross the boundary. |
| Interactive HTML/QML | separate document renderer and preview processes | Defer: isolate untrusted HTML, navigation, network/resource grants and input to a new session. No arbitrary QML execution in a remote owner's process. |
| Memory / idle release / compaction | `src/session_memory.rs`, `src/idle_release.rs`, `src/auto_compact.rs` | Policy/status projection could be safe; mutations need `orchestration:operate` plus fresh idle checks/claims and session scoping. Workers are process-owned today, so do not expose their local operations directly. |
| Vault | `src/journal_vault.rs` | Defer remote sync/rollout/conflict resolution. Host paths, git credentials, hooks and cross-host SSH require an explicit configured-vault allowlist and stronger maintenance permission. No arbitrary remote/path/ref supplied by phone. |
| Maintenance / host sync / cleanup | `src/host_sync.rs`, `src/build_cleanup.rs`, `src/disk_space.rs` | Bounded status projections are feasible. Defer apply/cleanup until target allowlists, dry-run plans, confirmation receipts and `environment:maintain` are enforced at ingress. Never expose generic shell or scripts. |
| Browser sessions / frames / input | `src/browser_sessions.rs`, preview managers | Defer remote wrapper: isolate profile, cookies, grants, frame transport and owner session; default off. Input needs `orchestration:operate` or stricter native preview permission. No shared desktop profile or frame-file paths. |
| Device sessions | `src/device_screen.rs`, `src/device_watch.rs`, native device API | Defer remote wrapper: session/device ownership and revocation must cover frame decoder and input queue; default off, no implicit physical device selection. |
| Transcription / model lifecycle | `native/dictate`, local dictation helper | Feasible with bounded uploaded audio and explicit model allowlist. Defer: local microphone/model installer must not be indirectly activated. Stronger permission for lifecycle writes; no audio in logs. |
| Host screen capture | existing screenshot flow | Defer: explicit host consent, region/display allowlist, bounded image bytes and per-session ownership; default off. Phone-local capture is an Android responsibility. |

Any future broker must authenticate before work, scope opaque resource IDs to the
requesting session, recheck authorization after reconnect/revocation, enforce bounded
queues/bytes/time and emit content-free audit records (operation, opaque session,
outcome). Writes/input require `orchestration:operate` or a documented stronger
service-specific scope. Never log text, keystrokes, frames, audio, tokens, full URLs
with credentials, or draft payloads. No remote capability is advertised in this pass.

## Validation (2026-10-31)

- `cargo test --no-fail-fast`: **453 passed, 4 failed, 0 ignored**.
- The same four failures were reproduced from an archived clean
  `origin/feat/o3-brand` source: `approvals`, `history`, `plan_transport`, and
  `third_party_licenses`. These are pre-existing fixture/compiled-notice failures.
  The renderer fixture that failed in PR 21 passes in this environment now.
- Journal/migration/transfer tests: **14/14**; local draft tests: **5/5**.
  Covers anchored retry, import acknowledgment/deduplication, collision resolution,
  absent keys after compaction, stale send clear, payload privacy, and live-empty
  target backup. Preferences has a new default-off/authority sanitizer test.
- QML: `shared-drafts`, `composer`, `project-drafts`, `preferences` pass. The new
  fixture covers composer conflicts, retained editor text, project migration
  conflict branches, and explicit/default-off Preferences controls.
- Translation flow: **20 locales × 2011 keys**, seven new strings, no warnings.
- `cargo clippy --lib --test shared_drafts`: passes with 24 existing library warnings.
- `rustfmt --check` for new Rust files and `git diff --check`: pass.

No deployment, service restart, phone access, or physical Android/Desktop round
trip was performed. `target-orphan/` was left untouched. The Android wire API and
host broker security boundaries described above remain unchanged.
