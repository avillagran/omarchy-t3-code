# Remote surfaces broker v1 — scoped snapshot slice

This delivers **owner-published document snapshots and a static HTML render
preview gallery**. It does not deliver a live workspace filesystem, HTML execution,
interactive renders, maintenance, capture, transcription, browser or device sessions.
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

HTTP/1.1, one GET per connection, `Authorization: Bearer <phone-T3-token>`.
No request body, cookies, Origin, query string or URL escaping. All responses are
JSON with `Cache-Control: no-store`, `nosniff`, restrictive CSP and connection close.

| GET route | Response |
| --- | --- |
| `/v1/session` | `{protocol:1,session:<64 lowercase hex>,capabilities:["documents.read","gallery.read"]}` |
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

Errors: `401 unauthorized`, `403 consent_required`, `404 not_found`,
`429 rate_limit`; unavailable auth/store or corrupt/oversized store returns 503.
Malformed/oversized requests return 400. The JSON body is `{error:<fixed-code>}`.
There are no remote writes/input routes, so `orchestration:operate` cannot turn
this API into a mutation capability. Unsupported paths never reach local handlers.

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
- 30 parsed requests/session/minute, 120 connections/host/minute, including
  malformed requests and invalid credentials. Rate state has at most 120 keys and resets every minute.
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
2. Memory/vault/host-sync/compaction/idle release/build cleanup: require scoped
   policy projections; explicit configured vault/target allowlists; fresh idle
   claims; dry-run plans and confirmation receipts; `orchestration:operate` or
   `environment:maintain` at ingress. No generic shell/SSH wrapper is exposed.
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
