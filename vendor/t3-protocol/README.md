# lib-t3-rust

Portable Rust core for [Omarchy T3 Code](https://github.com/avillagran/omarchy-t3-code)
and Omi. Extracted with source history from ot3; the Cargo package remains
**`t3-protocol`**, preserving Rust imports and the C ABI.

```toml
[dependencies]
t3-protocol = { git = "https://github.com/avillagran/lib-t3-rust", tag = "v0.3.0" }
```

For reproducible application builds, pin a full `rev` and commit Cargo.lock.
The project is not published on crates.io.

Shared Rust library for Omarchy T3 Code and Omi. The desktop plugin now uses this
library for T3 HTTP reads, Effect RPC writes, signed attachment streaming and
T3 Connect authorization. The library has no Quickshell, Electron, filesystem,
SSH, keyring, Python or JavaScript runtime dependency.

## Rust API

```rust,no_run
use std::sync::Arc;
use t3_protocol::{connect_relay, Connection, Credential, DpopKey};
use serde_json::json;

# fn example() -> t3_protocol::Result<()> {
let direct = Connection::new(
    "https://environment.example", None, Credential::bearer("host-provided-token")?
)?;
let shell = direct.request_json("GET", "/api/orchestration/shell", None, true)?;
let relay = connect_relay(
    "https://relay.example", "host-provided-cloud-jwt", "t3-mobile",
    "00000000-0000-4000-8000-000000000001", Arc::new(DpopKey::generate())
)?;
let config = t3_protocol::api::server::config(&relay)?;
let vcs = relay.rpc("server.discoverSourceControl", json!({}))?; // a tag `api` does not wrap yet
# Ok(()) }
```

The host supplies the current Clerk JWT issued for T3's relay template. A native
OAuth CLI token and a browser's cached DPoP access token are not interchangeable
with that JWT. A `RelayClient` can reuse relay authorization across environments.
`Connection::upload_attachment(name, mime, size, reader)` streams host-owned file
or Android content data through the same RPC grant and signed POST protocol.

The core signs ES256 DPoP proofs with fresh jti, htm, normalized htu, iat and ath;
checks the environment descriptor before exchanging its bootstrap credential;
and obtains a one-use WebSocket ticket for DPoP RPC. Credentials never go into
ordinary URLs. The ticket is used only for the WebSocket upgrade, as T3 requires.
Mutation requests are never automatically retried. A post-send failure returns
`rpc_outcome_unknown`; the host owns reconciliation and stable command IDs.

Keep keys and tokens in the platform's secure store if persisting them. Types
containing secrets do not implement Debug/Serialize. Expired DPoP credentials
fail with `session_expired`; reauthorize using the host's current account session.
Run blocking calls on a worker thread. Native TLS certificate validation stays on.
Direct bearer endpoints may use HTTP when inherited from T3; relay URLs require
HTTPS (loopback HTTP is allowed for isolated tests). Redirects are disabled.

## Omi / Kotlin

See the [Android integration guide](docs/ANDROID.md) for NDK builds, JNI ownership,
threading and the current C ABI limitations. Linux packages are not Android binaries.

The crate builds as `rlib` and `staticlib`. `include/t3_protocol.h` exposes an opaque
handle and UTF-8 JSON C ABI for a thin Kotlin/JNI adapter. The canonical [Kotlin/JNI adapter](bindings/android) marshals UTF-8 byte arrays
and file descriptors; HTTP/RPC/authentication stay in this library. Android
releases include a four-ABI AAR so applications do not need to compile Rust.

1. Build `t3-protocol` for each Android ABI with the Android NDK/toolchain.
2. Link `libt3_protocol.a` into Omi's JNI library and include `t3_protocol.h`.
3. Create a handle with `t3_client_new`, call from a worker thread, release each
   response with `t3_string_free`, then release the handle with `t3_client_free`.
4. Supply Omi's current signed-in cloud session to `connectRelay`; do not read
   Omarchy's files or duplicate the protocol in Kotlin. The Rust streaming API
   accepts a reader supplied by the Android content adapter.

C ABI commands:

```json
{"op":"connectBearer","httpBaseUrl":"https://environment.example","token":"host-provided-token"}
{"op":"connectRelay","relayUrl":"https://relay.example","cloudJwt":"host-provided-jwt","clientId":"t3-mobile","environmentId":"00000000-0000-4000-8000-000000000001"}
{"op":"request","method":"GET","path":"/api/orchestration/shell","protocol":true}
{"op":"rpc","tag":"server.getConfig","payload":{}}
{"op":"rpcWithTimeout","tag":"device.configure","payload":{},"timeoutMs":120000}
{"op":"describe"}
{"op":"call","fn":"vcs.refreshStatus","args":{"cwd":"/home/me/repo"}}
{"op":"subscribe","tag":"orchestration.subscribeThread","payload":{"threadId":"selected"}}
{"op":"pollSubscription","subscriptionId":"returned-id","timeoutMs":1000}
{"op":"cancelSubscription","subscriptionId":"returned-id"}
{"op":"wakeSubscriptions"}
{"op":"disconnect"}
{"op":"connectBearer","httpBaseUrl":"https://environment.example","token":"host-provided-token","options":{"requestTimeoutMs":8000,"connectTimeoutMs":3000,"poolIdleTimeoutMs":90000}}
{"op":"request","method":"GET","path":"/api/orchestration/shell","maxBytes":1048576}
{"op":"requestOk","method":"POST","path":"/api/auth/clients/revoke","body":{"sessionId":"s-1"},"protocol":false}
{"op":"fetchDescriptor","httpBaseUrl":"https://environment.example","environmentId":"00000000-0000-4000-8000-000000000001"}
{"op":"pairBearer","httpBaseUrl":"https://environment.example","pairingCode":"from-the-pairing-link","client":{"label":"Omi","deviceType":"mobile","os":"android"},"connect":true}
{"op":"boundedThread","threadId":"selected"}
```

Signed assets return bytes through `t3_client_image`'s callback, never as
Base64 JSON: metadata `{"op":"assetBytes","resource":{...},"maxBytes":N}` or
`{"op":"projectFavicon","cwd":"/repo","path":"favicon.svg"}`. Metadata without
`op` is the original attachment image read.

Responses are `{ "ok": true, "data": ... }` or
`{ "ok": false, "error": "stable_code" }`; a rejected RPC may add
`"message"` with the server's one-line reason, so `error` stays comparable. Inputs remain owned by the caller;
the caller clears its credential buffers. Never free a handle during a call.
Connection changes are atomic: a failed connect leaves the prior connection
intact, so the host must treat the returned error as a failed switch.

The library supplies native Android JNI binaries and a Kotlin adapter. It does
not modify or publish the Omi application. Host tests and Android cross-builds
are separate from Android device/UI validation; see the integration guide.

## Typed API

`t3_protocol::api` wraps T3 RPC tags in typed functions,
`fn(conn: &Connection, …) -> Result<Value>`, and lists them in one registry.
Arguments are typed and validated before a socket opens; responses stay
`serde_json::Value`, because T3 declares them forward compatible. Each function
sends exactly one request and never retries. A function also states its kind and
the T3 scope it needs. The library does not pick a token; the host chooses the
token from the published scope.

```rust,no_run
use t3_protocol::api::{self, orchestration::Command};
# fn example(conn: &t3_protocol::Connection) -> t3_protocol::Result<()> {
let status = api::vcs::refresh_status(conn, "/home/me/repo")?;
// commandId is minted and kept by the host, so a resend after
// rpc_outcome_unknown is deduplicated by the server.
let archive = Command::ThreadArchive { command_id: "omarchy-1".into(), thread_id: "t-1".into() };
api::orchestration::dispatch(conn, &archive)?;
// The same through the registry, with JSON arguments (what the C ABI does):
api::call(conn, "vcs.refreshStatus", &serde_json::json!({"cwd":"/home/me/repo"}), None)?;
# Ok(()) }
```

Over the C ABI (ABI 3, additive) the same registry is two ops:

```json
{"op":"describe"}
{"op":"call","fn":"vcs.refreshStatus","args":{"cwd":"/home/me/repo"}}
{"op":"call","fn":"orchestration.dispatchCommand","timeoutMs":20000,"args":{"type":"thread.archive","commandId":"omarchy-1","threadId":"t-1"}}
```

`describe` needs no connection and returns, per function, `fn`, `module`, `tag`,
`kind` (`read`/`mutation`), `replay`, `scope` (or `"dynamic"` plus `scopes`),
`timeoutMs`, `stream` and typed `args`. `replay` is what a host may do after
`rpc_outcome_unknown`: `free` (repeat the read), `id` (resend the same payload
and `commandId`), `set` (repeating is harmless) or `never` (read the state back
first). `call` checks the function name (`unknown_function`), `timeoutMs`
(1..300 000, `invalid_rpc_timeout`) and `args` (an unknown member is
`invalid_input`; a missing or malformed one is `invalid_<field>`, such as
`invalid_cwd` or `invalid_command_id`) before it needs a connection.
`rpc`/`rpcWithTimeout` remain for tags the registry does not wrap yet.

`Command` types the 34 `dispatchCommand` variants ot3 sends. The existing builders
remain the source of truth for the commands they build: `Command::queue`, `answer`,
`approve`, `runtime_mode`, `sidebar` and `thread_action` wrap `queue`, `questions`,
`approvals`, `models` and `sidebar`. The other contract types travel as
`Command::Raw`.

Authorization failures are classified by the error's `_tag`
(`EnvironmentAuthorizationError`), not by its text: a git failure saying
"Author identity unknown" or "Authentication failed" stays `rpc_rejected`. A
missing scope reads `write_not_authorized: [settings:write] <message>`;
`errors::required_scope` and `errors::message` split it, and the C ABI returns
`{"ok":false,"error":"write_not_authorized","message":"…","requiredScope":"settings:write"}`.

Tanda 1 (the ten most used tags in ot3):

| Function (`fn`) | Rust | Kind · replay | Scope | Args |
| --- | --- | --- | --- | --- |
| `server.getConfig` | `api::server::config(conn)` | read · free | `orchestration:read` | — |
| `server.getSettings` | `api::server::settings(conn)` | read · free | `orchestration:read` | — |
| `server.updateSettings` | `api::server::update_settings(conn, patch, mutation?)` | mutation · set | dynamic: `settings:write`, `providers:manage` | `patch`, `providerInstanceMutation?` |
| `orchestration.dispatchCommand` | `api::orchestration::dispatch(conn, &Command)` | mutation · id | `orchestration:operate` | the command: `type`, `commandId`, … |
| `orchestration.launchThread` | `api::orchestration::launch_thread(conn, &ThreadLaunch)` | mutation · id | `orchestration:operate` | `commandId`, `projectId`, `title`, `modelSelection`, `runtimeMode`, `interactionMode`, `workspaceStrategy`, … |
| `preview.list` | `api::preview::list(conn, thread_id)` | read · free | `orchestration:read` | `threadId` |
| `assets.createUrl` | `api::assets::create_url(conn, &AssetResource)` | read · free | dynamic: `filesystem:read`, `orchestration:read` | `resource` |
| `vcs.refreshStatus` | `api::vcs::refresh_status(conn, cwd)` | read · free | `orchestration:read` | `cwd` |
| `review.getDiffPreview` | `api::review::diff_preview(conn, cwd, base_ref?, ignore_whitespace?, file?)` | read · free | `filesystem:read` | `cwd`, `baseRef?`, `ignoreWhitespace?`, `file?` |
| `projects.readFile` | `api::projects::read_file(conn, cwd, relative_path, allow_missing?)` | read · free | `filesystem:read` | `cwd`, `relativePath`, `allowMissing?` |

`tests/fixtures/t3-contract.json` pins the tag, stream and scope of every T3 RPC
at the commit in `api::CONTRACT`. Regenerate it with
`tools/extract-t3-contract <t3code checkout>`. `tests/api_contract.rs` fails if
the registry drifts from it. With `T3_CONTRACTS_DIR=<checkout>/packages/contracts/src`
set, the same test also re-reads the live sources.

## Validation

```sh
cargo test --locked
tools/test-c-abi
```

Fixtures verify real ES256 signatures and request/token binding, fresh proof IDs,
relay/environment exchanges, identity mismatch before credential release,
HTTP protocol headers, WebSocket tickets, RPC envelopes, C ABI ownership/errors
and attachment streaming via the desktop integration test. No paid agent runs
or real conversations are created by these tests.

## Shared modules

| Area | Modules |
| --- | --- |
| Native state | `projection`, `history`, `lifecycle`, `stream` |
| Transport | `ConnectionOptions`, `Connection::rpc`, `Connection::rpc_with_timeout`, `stream` (`subscribe`, `subscribe_probed`), `assets`, `pairing`, `bounded_snapshot` |
| Chat operations | `composer`, `queue`, `questions`, `approvals`, `tasks` |
| Presentation | `activity`, `changes`, `documents`, `sidebar`, `models`, `i18n` |
| Devices | `device_hub` |
| Configuration | `connection_catalog`, `projects`, `settings` |
| Cache policy | `compaction` |
| Helpers | `util` (`text`, `truth`, `valid_id`, `epoch`) |
| Checkpoints and plans | `checkpoint_revert`, `plan_workflow`, `prompt_recall`, `message_text` |
| Rich text | `latex` (Qt rich text, see below) |
| Settings contracts | `storage_settings`, `native_settings`, `scoped_settings`, `provider_instances` |
| Review and tools | `diff_review`, `tool_inspector`, `failure_context` |
| Git and threads | `git_controls`, `branch_picker`, `thread_details`, `pull_requests` |
| Usage and quotas | `quota_sources`, `usage_summary` |
| Server lifecycle | `server_updates`, `reconnect` |

Modules derive portable models and validate native commands; hosts own dispatch,
persistence, deduplication, the reconnection loop, wakeup detection and UI. Preventive compaction
requires a durable host-owned one-attempt claim before dispatch. Transport tests
use local fixture servers, never real conversations or paid provider runs.

### Added in 0.3.0

- **Long RPCs.** `Connection::rpc_with_timeout(tag, payload, timeout)` runs the
  same Effect request/exit as `rpc` with an explicit deadline of up to 300 s and
  heartbeats, for lifecycle calls that outlast `rpc`'s 20 s. It never retries:
  a deadline sends `Interrupt` and returns `rpc_outcome_unknown`.
- **Failure reasons.** A rejected RPC reads `rpc_rejected: <message>` (or
  `write_not_authorized: <tag>`), taking only the cause's first error message or
  tag, bounded to one line. (Authorization is now decided by `_tag`; see Typed API.) `error_code()` strips the reason before comparing.
- **Stream liveness.** Subscriptions ping every 5 s and drop after 20 s without
  any frame (`STREAM_PING_INTERVAL`, `STREAM_IDLE_TIMEOUT`). `subscribe_probed`
  lets the host ask for an immediate check after a resume or network change; a
  socket that sends nothing within 3 s fails with `stream_timeout`. C hosts get
  the same check from `{"op":"wakeSubscriptions"}`; see [docs/ANDROID.md](docs/ANDROID.md)
  for the poll contract (`boundaries`, `idle`, `completionMarker`).
- **Bounded snapshots.** Protocol GETs of `/api/orchestration/threads/<id>/bounded`
  and `orchestration.subscribeThread` with `acceptBoundedSnapshot` accept up to
  64 MiB; everything else stays at 8 MiB. Through the C ABI that stream's queue
  takes one 64 MiB item (72 MiB in total), so the snapshot reaches the host whole.
- **Images.** `media_image(thread, path)` reads host-local media through a signed
  grant scoped to that thread and path. Attachments also accept HEIC, HEIF and
  AVIF, and the served type must match the attachment.
- **`device_hub`.** Builds the native device hub proxy paths for iOS
  (serve-sim MJPEG and input) and Android (serve-emu H.264 socket with `SEMU`
  headers), screenshots for both, with a session `wsTicket` and `hostId`.
  `MjpegFrames` splits JPEG streams, `parse_semu` reads access units and
  `input_message` encodes normalized touch, button, key and text input.
  Decoding and display stay with the host.
- **T3 0.1 / 0.1.2 models.** `projection` applies partial-payload events
  (pull request link updates, detached provider sessions, cancelled background
  work, rollback requests), mirrors context handoffs and transfers, keeps token
  usage and html_render / MCP app references. `documents::mcp_app` reads MCP
  apps, `activity` marks failed tool rows, `sidebar::sidebar_status` follows
  T3's sidebar status and snooze rules, `compaction` leaves settled threads
  asleep, and `queue` keeps a message promoted to steer visible.

### Domain modules shared with ot3

The pure halves of ot3's domain modules live here so Omi's Kotlin client reuses
them through JNI instead of reimplementing them. Each one is JSON in, JSON out,
with the same function names and output bytes ot3 had; ot3 re-exports them (see
[OT3_REEXPORTS.md](docs/OT3_REEXPORTS.md)) and keeps the parts that need its
environment, an RPC, the filesystem or a process.

- **Checkpoints and plans.** `checkpoint_revert` builds the rewind reference,
  command and read-back observation. `plan_workflow` projects proposed plans and
  their follow-up commands; the caller passes its Markdown renderer.
  `prompt_recall` recovers a sent prompt as plain text.
- **Settings.** `storage_settings`, `native_settings` and `scoped_settings`
  project server settings into descriptors and validate patches, including
  project overrides and `t3.json`. `provider_instances` validates provider
  instance mutations and confirms them against the read-back.
- **Review, Git and threads.** `diff_review` parses checkpoint diffs,
  `tool_inspector` scopes and bounds turn-item reads, `failure_context` picks a
  run's failed tools, `git_controls` builds confirmed VCS action plans,
  `branch_picker` holds the ref and branch rules, `thread_details` resolves the
  workspace and editors, and `pull_requests` now also builds comment, review and
  action payloads and the `thread.pull-request.*` commands.
- **Usage and server lifecycle.** `quota_sources` keeps usage limit observations
  free of credentials, `usage_summary` sums `server.getUsageSummary` (the host
  passes its IANA time zone), `server_updates` compares versions and decides when
  an automatic update may run, and `reconnect` holds the backoff and the
  transient-error rule. The host reports resume or network changes with
  `reconnect::notify()`; `Waiter::default()` follows that process-wide counter.

### Rich text is Qt's for now

`latex::to_rich_text` and the Markdown that ot3 renders around it emit Qt's rich
text subset (`<sup>`, `<sub>`, inline styles, `<p align="center">`), not general
HTML. `latex::render(tex, display, Surface::Qt)` is the same output behind an
explicit surface. An Android surface gets its own variant once its renderer is
designed; until then, Kotlin should treat this output as Qt-specific.
`plan_workflow` takes the renderer as a parameter, so it is not tied to Qt.

### Unreleased (transport, lote 2)

All additive; `Connection::new`, `with_http_client` and C ABI v2 behave as before.

- **`ConnectionOptions`.** Request timeout (12 s), connect timeout (none), pool
  idle timeout (90 s) and `request_json` limit (8 MiB), defaults in one place.
  `Connection::with_options`, `connect_relay_with_options` and
  `RelayClient::with_options` take them. `build_http_client()` gives a pooled
  `reqwest` client (no proxy, no redirects) to share. `with_credential` puts a
  read and a write token on one pool. They apply to HTTP only: WebSocket RPC
  keeps 8 s connect, 12 s read/write and the 20 s `rpc` deadline
  (`rpc_with_timeout` for longer).
- **HTTP details.** `Content-Type` is matched without case or parameters
  (`Application/JSON; charset=utf-8`), `Content-Encoding: identity` without case.
  `request_json_with_limit` sets one call's limit (1 byte to 64 MiB).
  `request_ok` accepts any 2xx without reading the body.
- **Signed assets.** `Connection::asset_bytes(resource, limit)` /
  `asset_bytes_with(resource, &AssetOptions)` read any `assets.createUrl`
  resource: same origin, under `/api/assets/` after dot segments, no credential,
  no redirects, exact limit (up to 64 MiB), 30 s by default. Errors:
  `invalid_asset_url`, `asset_unreachable`, `asset_unavailable`,
  `asset_too_large`. `assets::attachment_resource(id, name, mime, inline)` builds
  the attachment resource. `project_favicon(cwd, path)` reads 2 MiB within 3 s
  and returns `None` for T3's missing-favicon URL without fetching it.
- **Discovery and pairing.** `fetch_descriptor(base)` and
  `verify_environment(base, id, &options)` read `/.well-known/t3/environment`
  without credentials. `pair_bearer(base, secret, expected, &PairingClient,
  &options)` checks the descriptor (UUID, expected environment) before sending
  the pairing secret to `POST /oauth/token`. It returns a `PairedBearer` with the
  token, descriptor, scope and `connection()`. Errors: `pairing_rejected` (any
  non-2xx), `unsupported_pairing_token`, `t3_environment_mismatch`,
  `invalid_t3_environment_id`, `invalid_pairing_code`, `invalid_pairing_client`.
- **Session pruning.** `Connection::revoke_stale_sessions(select)` lists
  `/api/auth/clients`, applies the host's policy and revokes each chosen session.
- **Bounded threads.** `bounded_snapshot::{path, restore, valid_thread_id}` and
  `Connection::bounded_thread(id)` read `bounded?compactTurnItems=1` and restore
  compacted local turn items.

Behaviour hosts take over from their own HTTP code, and what stays theirs:

| Topic | Library behaviour |
| --- | --- |
| Proxy, redirects | Always disabled, including descriptor, pairing and pruning. Paths that used to follow `HTTP(S)_PROXY` or redirects no longer do. |
| 401 / 403, other non-2xx, network | `unauthorized`, `http_<code>`, `unreachable`. Descriptor reads keep these codes: map them if the UI wants another one. |
| Error bodies | Discarded, as before. Only RPC failures carry a reason. |
| Extra headers, absolute URLs to other hosts | Not supported. Same-origin T3 traffic only; third-party HTTP stays in the host. |
| Asset error codes | One family (`asset_*`). A host that shows `attachment_*` or `revert_attachment_*` maps them; an exact-size check is `limit = sizeBytes` plus `bytes.len() == sizeBytes`. |
| Image error codes | Unchanged (`image_*`, `invalid_image`, `invalid_asset_url`). |

## Development

Run `cargo test --locked` and `tools/test-c-abi` before changes are consumed by
either app. Native CI runs on Linux x86_64 and aarch64. Keep platform integrations
outside this crate. Fix shared behavior here, tag a version, then update each
consumer's pinned revision instead of copying source trees between applications.

The initial extraction preserves the source from ot3 commit
`fbe3dc94ba4d4f84dddea9424eb0913b27f34e64`. The [upstream contract reference](docs/UPSTREAM.md) records the original T3 code
used for the Android/event extension. Device/UI validation remains consumer work.

[MIT License](LICENSE). Independent community project; not an official T3 SDK.
