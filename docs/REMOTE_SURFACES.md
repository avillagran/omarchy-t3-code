# Remote surfaces broker v1 — snapshots and host maintenance

This delivers **owner-published document snapshots and a static HTML render
preview gallery**. It does not deliver a live workspace filesystem, HTML execution,
interactive renders, capture, transcription, browser or device sessions.
The maintenance extension below adds bounded host policy/status and confirmed operations.
P14–P17 remain partial/blocked for those operations. No Android consumer is changed.

## Transport and owner consent

The broker accepts the phone's **existing T3 bearer credential**, validating it with
the explicitly configured local T3 server's `GET /api/auth/session`. Only
`authenticated:true`, `sessionMethod:"bearer-access-token"` and an explicit
`orchestration:read` permission admit a request. An explicit `permissions` array
is authoritative; only an absent field falls back to legacy `scopes`. No Desktop saved credential is read.
The auth descriptor must also advertise a known authenticated policy;
`unsafe-no-auth`, missing and unknown policies fail closed.
The native authorization endpoint is authoritative, including expiry and revocation;
it is called before reading snapshots and again before producing a response.
No cookie, DPoP downgrade, wildcard scope or operate-only fallback is accepted.

Preferences → Connections → **Remote documents on this device** is off by default.
This is a local host setting, independent of the selected remote environment.
Every toggle rotates the consent epoch. Turning off revokes all publications;
turning back on never restores old grants. The owner must publish again.
Enabling consent does not launch the broker, publish a bridge service or grant a peer.

Manual owner launch (not executed by this change):

```sh
omarchy-t3-client remote-surfaces-serve http://127.0.0.1:<T3-port> <broker-port>
```

The auth base must be a literal IPv4 loopback HTTP origin with an explicit port,
no credentials/path/query/fragment. Redirects and proxies are disabled. The listener
binds **only 127.0.0.1** and every application request requires authentication.
There is no unauthenticated content or health route. No CORS or cookie auth.

For remote access, the host owner must explicitly configure a separate OmaBridge
service (suggested name `desktop-snapshots`) targeting that loopback port and grant
it to the phone. Use the phone's authenticated OmaBridge service connection to send
the HTTP below. Service configuration, grant changes and daemon restart are out of
scope here. This is an extra service carried by the existing paired bridge, **not**
a new manage action or T3 RPC. A phone limited to the existing `t3` service cannot
reach it until the owner configures that route. Never publish this port directly.

## Android wire API

HTTP/1.1, one request per connection (GET, or the two maintenance POSTs below), `Authorization: Bearer <phone-T3-token>`.
GETs have no request body. No cookies, Origin, query string or URL escaping. All responses are
JSON with `Cache-Control: no-store`, `nosniff`, restrictive CSP and connection close.

| GET route | Response |
| --- | --- |
| `/v1/session` | `{protocol:1,session:<64 lowercase hex>,capabilities:["documents.read","gallery.read","memory.read","vault.read","host-sync.read","compaction.read","idle-release.read","build-cleanup.read","maintenance.confirm"]}` |
| `/v1/documents` | `{items:[{id,threadId,projectId,title,kind,expiresAt,preview}]}` |
| `/v1/documents/<uuid>` | `{id,threadId,projectId,title,kind,contents,executable:false}` |
| `/v1/gallery` | Same item projection, only HTML snapshots with a PNG preview |
| `/v1/gallery/<uuid>` | `{id,mimeType:"image/png",base64:<PNG>}` |

`session` is SHA-256 of the bearer, **not** a caller-supplied identity or the T3
session database ID. It is an opaque grant recipient. This intentionally binds
publications more narrowly than the logical T3 client: token rotation/reconnect
with a different bearer needs fresh owner publication. Tokens are never persisted.
The same unchanged valid bearer can reconnect without changing resource ownership.
The owner must obtain this fingerprint from the intended phone through a trusted
exchange. A fingerprint alone grants nothing and cannot authenticate a request.

The lists contain only that recipient's current-epoch, unexpired publications.
Foreign, expired, revoked and unknown IDs return the same `404 not_found`.
HTML is returned only inside JSON as non-executable source; never load it into a
privileged WebView. Gallery previews are static, decoded PNG bytes: they carry no
HTML runtime, network/resource grant or input channel. There is no pagination:
the complete host store is capped at 32 publications.

Rate windows last 60 seconds: 120 authenticated requests host-wide and 30 per
bearer fingerprint. Missing bearers consume neither budget and never call T3.
Failed authentication probes are limited to eight per window; fingerprints that
already authenticated in that window may still revalidate with T3 after that
limit. Successful authentication never skips either live permission check.
Both fingerprint collections are bounded by the authenticated admission limit
and cleared on window rollover; a failed revalidation removes the known-good
fingerprint. Each T3 authentication call has a 250 ms timeout because the
listener handles connections serially. Audit output is independently capped.

Errors: `401 unauthorized`, `403 consent_required`, `404 not_found`,
`429 rate_limit`; unavailable auth/store or corrupt/oversized store returns 503.
Malformed/oversized requests return 400. The JSON body is `{error:<fixed-code>}`.
Snapshot routes remain read-only. Maintenance mutations use the additional
consent and confirmation protocol below. Unsupported paths never reach local handlers.

## Maintenance wire API (memory slice)

The same loopback service now exposes read-only host policy/status projections.
Reads use the existing explicit `orchestration:read`, bearer-only authentication,
owner read consent, live revocation checks, rate budgets and redacted audit rules.
No Desktop credential is used. `/v1/session` advertises these capabilities.

| GET route | `report` projection |
| --- | --- |
| `/v1/memory` | Local idle-release and compaction policy; no transcript or vault note content |
| `/v1/vault` | Configured/steps/local Git state, enabled flags, configured vault identity, configuration and instruction-file fingerprints |
| `/v1/host-sync` | Bounded persisted host/item outcomes, no backups, file content or secrets |
| `/v1/compaction` | Local settings plus persisted aggregate worker status |
| `/v1/idle-release` | Local settings plus persisted aggregate worker status |
| `/v1/build-cleanup` | Dry-run candidates under owner-selected roots, byte counts, file identities and fresh idle claims |

Each response is `{surface, reportId:<SHA-256>, report:<object>}`. `reportId` binds
the entire projection, including the configured target and relevant settings.
Worker status is explicitly `source:"persisted", live:false`; a claimed or uncertain
operation is `unknown`. This does not claim to inspect another live Desktop process.
Missing state yields an empty persisted status; malformed or oversized state fails
closed. Reads do not refresh instructions, clone/pull/push, sync hosts, compact,
release sessions, clean builds or invoke a generic local command.

Local Preferences → Connections → **Allow remote maintenance on this device** is
an additional consent, default off and independent of the selected environment.
Every toggle and cleanup-root edit rotates `remoteMaintenanceConsent`. Disabling
read consent or changing either epoch revokes pending confirmations. Both consents
must be enabled to mutate. `remoteMaintenanceRoots` is a local-only allowlist of at
most 16 absolute canonical checkout roots, empty by default; remote settings
requests cannot change this allowlist, either consent, vault path or host identity.
The owner enters roots one per line in Preferences. Paths are never accepted in a
remote mutation body. The configured vault is the only vault rollout target.

Only these actions are supported:

| Surface | Action | Extra request fields |
| --- | --- | --- |
| `compaction` | `settings` | `settings` with boolean `autoCompactEnabled`, minutes `autoCompactFallbackMinutes` (1–1440), tokens `autoCompactMinTokens` (1000–1000000) |
| `idle-release` | `settings` | `settings` with boolean `idleReleaseEnabled`, minutes `idleReleaseMinutes` (1–1440) |
| `vault` | `rollout` | None; installs the managed instruction block for this host's already configured/enabled vault, without Git/network operations |
| `build-cleanup` | `run` | None; executes the exact rechecked dry-run report |

Settings are persisted in a separate private `remote-host-memory.json` (16 KiB
read limit), so ordinary Desktop presentation-state writes cannot overwrite them.
They override the global policy only for environment `local`; other environments
managed by the Desktop retain their existing policy. Reports include `hostOverrides`
and merged policy values; null means no saved global value and native defaults apply. Corrupt/unreadable overrides disable the local memory
workers until the owner repairs the file. Settings take effect on the next Desktop
worker poll. No immediate compact/detach
RPC is exposed. Host sync is status-only; remote apply, credential copying and login
are unsupported. Vault rollout never selects another host, remote URL or path.
Instruction-file symlinks and parents resolving outside the local home are refused.
Vault config reads are capped at 64 KiB and instruction files at 64 KiB each.
Vault Git status uses fixed read commands with optional index writes/fsmonitor/hooks
disabled, one-second subprocess deadlines and bounded output; it does not invoke
the broad Desktop vault wizard/read action.

Mutations require both explicit read permission and `orchestration:operate` or
`environment:maintain`. Authoritative `permissions` still overrides legacy
`scopes`; wildcard, cookie, missing read and unsafe auth remain denied. The server
revalidates these permissions before preparing/confirming and after report reads.
Android must implement this two-step user-confirmed flow:

1. GET the surface report with the phone's bearer.
2. POST `/v1/maintenance/prepare` with that `reportId` and the exact proposed action:
   `{"surface":"compaction","action":"settings","settings":{"autoCompactEnabled":false},"reportId":"<GET reportId>"}`.
3. Show the returned `request` and full `report` to the user. Response:
   `{confirmationToken:<UUID>, report:{surface,reportId,report}, request, expiresIn:60, retry:false}`.
4. Only after confirmation, POST `/v1/maintenance/confirm` with
   `{"confirmationToken":"<token>","reportId":"<same reportId>"}`.

POSTs require a single `Content-Length` of 1–4096 bytes and a JSON body. GETs still
reject bodies. Only the two POST paths above exist; header/path/bearer bounds,
no cookie/Origin/query/escaping, no redirects/proxies and connection-close rules
are unchanged. Both steps share the existing 30/session and 120/host minute budget,
failed-auth gate and audit cap with all reads. Tokens are random, session-bound,
action-bound, epoch-bound, expire after 60 seconds and are consumed when authenticated confirmation validation begins, including
session/epoch/report/expiry failures. Authentication and rate rejection happen
before token validation and never dispatch a mutation. At most 32 tokens are pending.
Any changed/expired report gives `409 stale_report`; get a new report and ask the
user again. A token from another bearer or consent epoch cannot be used.

A nonblocking cross-process operation lock covers fresh-report validation,
authorization, claim and apply; concurrent broker mutations fail closed without
queuing. Before any side effect the broker persists and fsyncs an irreversible claim for
that exact session/epochs/request/report in its private state. Up to 128 claims
are retained without automatic eviction; saturation fails closed. A duplicate
report/action after disconnect or broker restart gives `409 already_claimed`.
The adapter rechecks consent and the report again immediately before acting.
The claim store has a 64 KiB read limit, 0700 directory/0600 atomic files,
nonblocking lock and no-follow checks. Owner removal of claims is not a retry
mechanism and can discard protection against replay; no remote reset exists.

Success is `{outcome:"completed",result:<bounded result>,retry:false}`. Backend
failure or oversized reply after claiming is `{outcome:"unknown",retry:false}`:
work may have partially completed, and neither broker nor Android may retry the
mutation. Lost HTTP responses are also unknown. Refresh read status and ask the
owner to reconcile; do not automatically prepare the same action again. No
outcome is inferred from a timeout. Claims intentionally survive unknown outcomes.
The fixed audit outcome is `unknown` for these responses, never a completed action.
Additional fixed errors include `confirmation_required`, `invalid_command`,
`report_limit`, `surface_unavailable`, and `429 confirmation_limit`.

Cleanup uses only the caller's bearer against the fixed local T3 origin for shell
and bounded-thread reads (1 MiB response limit, two-second HTTP timeout), never
saved Desktop credentials. Each relevant thread must have a complete idle
projection: no active/queued runs, pending requests/plans, subagents or background
nodes/items. Missing claims fail closed, and at most 16 thread claims are read.
Only configured, canonical, ignored, recognized regenerable build directories
qualify. Custom cache-tag directories and `target-orphan/` are excluded. The dry run
is capped at 32 candidates and 10000 filesystem entries, with a two-second walk
budget. Fixed Git reads have one-second subprocess deadlines and 64 KiB file-list /
4 KiB git-directory output bounds. Other checkouts are never discovered by this path. Recent nested-file changes postpone cleanup. Exact filesystem identity,
settings and native idle claims are checked again before removal. A final legacy
read/delete race with another client or external build remains possible; no atomic
server lease is claimed. Never use this API to clean an actively used checkout.

The Android consumer is not changed in this PR. No bridge route, deployment,
restart or phone acceptance is performed.

## Publishing and revoking (local owner only)

`remote-surfaces-owner` reads a JSON object on stdin (2 MiB limit). It bypasses no
remote authorization: it is available only as the local owner CLI, **never** an
HTTP endpoint. Content must already be selected/exported by the owner. The broker
does not read a phone-supplied path, invoke `xdg-open`, select a thread, fetch using
the Desktop's T3 token or execute a renderer. Scope IDs are owner assertions about
the exported immutable snapshot, not a native filesystem authority.

Publish one snapshot after enabling Preferences:

```json
{"action":"publish","confirmed":true,"session":"<fingerprint-from-phone>","projectId":"<project-id>","threadId":"<thread-id>","title":"Review","kind":"text","contents":"Owner-selected document text","expiresAt":<Unix-seconds-within-next-hour>}
```

Returns `{id:<new-uuid>}`. `kind:"html"` may include `previewPng:<base64>` from an
**already owner-rendered** PNG. The broker validates PNG decoding, dimensions and
allocation limits but does not render the HTML or assert that source and PNG match.
Automatic export from Desktop's document viewer/render gallery remains future work.
Explicit local publication is the extra resource grant required beyond T3 read scope.

Local revocation commands:

```json
{"action":"revoke","id":"<publication-uuid>"}
{"action":"clear","confirmed":true}
```

Removing one publication or clearing the store takes effect on the next read;
a second atomic store read after auth also catches revocation during slow auth.
A response already handed to the socket cannot be recalled. The phone must delete
cached copies when leaving a session/revoking access; downloaded content cannot
be retroactively erased by the host. Expired/old-epoch records are pruned on the
next owner operation. Turning off prevents reading them immediately.

## Bounds, persistence and audit

- One synchronous request in flight, no application work queue or renderer process.
- Header limit 8 KiB; path 128 bytes; token 4096 bytes; two-second read/write/auth
  timeouts. Header assembly also has an elapsed-time bound. No streaming input.
- 30 authenticated requests/session/minute, 120 authenticated requests/host/minute.
  Eight failed auth probes/window; known-good fingerprints still revalidate.
  Missing bearers never call T3. Each fingerprint collection has at most 120 keys
  and resets every minute.
- 1 MiB UTF-8 content, 256-byte title, 512 KiB decoded PNG, max 1920×1080 pixels,
  max decoder allocation 16 MiB, TTL at most one hour, 32 publications/host,
  aggregate JSON store 32 MiB. Auth response limit 16 KiB.
- Private owner store under the app's XDG state directory: directory 0700, files
  0600, atomic rename/fsync, nonblocking process lock. Symlink/nonregular store
  files are refused. Reads and writes are bounded. No remote file path crosses
  the boundary. Local owner CLI output contains no snapshot content.
- Each admitted HTTP attempt emits a fixed `surfaceAudit:1` JSON record on stderr with only
  operation, one-way session fingerprint (empty before parsing), fixed outcome.
  No title, resource ID, URL, token, text, HTML, image, audio or keystroke is logged.
  Flood rejection is audited once per window after saturation (at most 121 audit
  records/minute). The launcher controls stderr retention/rotation; the broker creates no log file.

## Deliberately remaining

1. Automatic authorized-thread document export and live HTML render sessions:
   require native resource resolution using the requester's authorization, isolated
   renderer/network grants, bounded frames and an input owner. No arbitrary QML.
2. Cross-host vault rollout and host-sync apply; manual compact/detach actions;
   live worker control and automatic retry/reconciliation remain unsupported.
3. Capture/transcription: need display/region/model allowlists and bounded upload,
   cancel and result ownership. No microphone or model installer is invoked.
4. Browser/device sessions: need isolated profiles, cookie/resource grants,
   session-owned decoder/input queues, frame limits and revocation. No desktop
   browser profile or implicit physical device selection is exposed.

No deployment, service restart, phone access, Android acceptance or live bridge
round trip is claimed. The broker must be configured and integrated by consumers
before this snapshot slice is reachable from a production phone.

## Validation (2026-10-31)

- Final focused Rust run: **24/24** (23 broker tests + one Preferences sanitizer).
  Includes denied/foreign sessions, token revoke during read, owner revoke during
  the second auth call, epoch disable/re-enable, authoritative empty permissions,
  PNG/text/store/header/rate limits and absent mutation routes.
- Isolated `tools/test-remote-surfaces`: passes against the built CLI/HTTP process,
  synthetic local auth and private temporary state. Documents and gallery both
  exercise denied, revoked and over-limit behavior; audit redaction and private
  file permissions are checked. It uses no live credentials, bridge or phone.
- `tools/test-qml shared-drafts`: passes, including the new default-off control,
  explicit enable, disable and epoch rotation assertions.
- Translation flow: **20 locales × 2014 keys**, three new strings, no warnings.
- Full Rust suite run: **470 passed, 4 failed**. Final ingress hardening and seven
  additional regression tests were then checked with the focused run and HTTP
  fixture above. The four failures are the same ones documented and reproduced
  on the clean base in `SHARED_DRAFTS_HOST.md`: `approvals`, `history`,
  `plan_transport`, `third_party_licenses`. Those files, dependencies and compiled
  notices are unchanged in this branch; they are reported separately from this work.
- `cargo build -j 6`, `cargo clippy -j 6 --lib`, new-module rustfmt and
  `git diff --check`: pass. Clippy reports 24 existing warnings, none in the broker.

`target-orphan/` and the user's local `feat/o3-brand` commit were left untouched.

### Authentication-budget regression (2026-10-31)

- Focused broker tests: **27 passed** (four new regressions); isolated HTTP fixture
  passes 120 missing-bearer requests without T3 calls, bounded random-bearer
  probes, known-good live revalidation, revocation and per-session limits.
- Clippy `--lib -j 6`: no broker warnings; 24 existing warnings elsewhere.
  Module rustfmt, build `-j 6` and `git diff --check` pass.
- Complete `cargo test -j 6 --no-fail-fast -- --test-threads=1`:
  **475 passed, 10 failed**. Four documented baseline failures: `approvals`,
  `history`, `plan_transport`, `third_party_licenses`. Six additional failures in
  unchanged browser/Qt tests: `browser_guest` (2, runtime unavailable),
  `preview_transport` (1, no successful navigation status), `qml_preview` (1,
  no sandboxed frame), `qml_preview_renderer` (2, missing frames).
- Initial parallel full run stopped at two broker test failures (synthetic auth
  timeout and `store_busy`); both pass in the complete serial run (98 library
  tests green). No deployment, phone access or merge.

### Memory and maintenance validation (2026-10-31)

- Focused final Rust checks: **49 passed** (41 library, 8 binary), including
  scope/consent denial, token/owner revocation, stale and expired reports,
  over-limit confirmations/claims/reports/filesystem walks, cross-process
  exclusion, unknown outcomes and durable replay refusal. Local memory overrides
  cannot change another environment's policy. Restored mtime and unchanged size
  do not bypass cleanup identity checks (ctime/inode are also included).
- `tools/test-remote-memory`: **44 assertions passed**, synthetic auth and private
  `/var/tmp` HOME/state only. Includes actual confirmed cleanup and vault rollout,
  late revocation returning/auditing `unknown`, no replay after unknown outcomes
  or broker restart, stale/denied/rate/body limits and unchanged global settings.
- Existing document/gallery HTTP fixture passes. QML `shared-drafts` fixture
  passes, including the maintenance default-off control and epoch rotation, using
  the observed Wayland test display and private `/var/tmp` state. No compositor
  or product service was started or restarted.
- **20 locales × 2018 keys**, four new strings, translation validator clean.
- Product clippy (`--lib --bin omarchy-t3-client -j 6`) passes with 24 existing
  library and 16 existing binary warnings, none in the added modules. The broader
  all-targets check encountered the existing unchecked-read lint in unchanged
  `tests/quota_sources.rs`. Rustfmt checks on the changed broker/policy modules
  and new cleanup/state sections, build `-j 6`, and `git diff --check` pass.
- Final full `cargo test -j 6 --no-fail-fast -- --test-threads=1`:
  **502 passed, 4 failed**. The four documented baseline failures are `approvals`,
  `history`, `plan_transport`, and `third_party_licenses`; those files are unchanged.
  All 112 library and 39 binary tests pass. No browser/Qt failures occurred.
