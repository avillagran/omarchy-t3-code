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

The registry has 99 unary functions in 20 modules: every unary T3 RPC that ot3
calls, plus `pullRequests.list` and `terminal.resize`, which the library already
built payloads for. The 13 streams and 4 composed reads are in
[Typed streams](#typed-streams). A function's `fn` is its wire tag, so
`provider.auth.start` keeps its three parts; the generated Kotlin method is
`provider.authStart`. `module` is the Rust module in camelCase
(`api::pull_requests` → `pullRequests`); a few tags live in another module
(`server.getHostResources` in `diagnostics`, `attachments.delete` in `assets`).
A deadline other than 20 s is ot3's own (`rpc_with_timeout`), and a caller's
`timeoutMs` overrides it.

Four functions take a union through `openArgs`: `orchestration.dispatchCommand`
(`type`), `projects.mutate` (`project.create`/`update`/`delete`), `device.action`
(14 `type`s) and, as a struct, `scheduledTasks.upsert`. Where the scope depends on
the payload, a pure helper tells the host which token it needs before the call:
`AssetResource::required_scope`, `ProjectMutation::required_scopes` (scripts add
`settings:write`), `vcs::prepare_pull_request_thread_scopes` (a worktree for a
thread adds `orchestration:operate`) and `device::list_scope` (`updateTool` or
`retryHostId` need `orchestration:operate`). Inputs that carry secrets
(`provider::AuthResponse::Credentials`, `secrets::SecretAnswer`, a webhook
`TaskUpsert`) redact them in `Debug`, and no error string contains a value.

`scheduledTasks.upsert` is `replay: id`: T3 derives the task id from
`commandId` (`scheduled-task:<commandId>`) and upserts it, so the library requires
`commandId` or an existing `id` and a resend never creates a second task.

**`server`** (9)

| Function (`fn`) | Tag | Rust (`api::…`) | Kind · replay | Scope | Deadline | Args |
| --- | --- | --- | --- | --- | --- | --- |
| `server.getConfig` | same | `server::config(conn)` | read · free | `orchestration:read` | 20 s | — |
| `server.getSettings` | same | `server::settings(conn)` | read · free | `orchestration:read` | 20 s | — |
| `server.updateSettings` | same | `server::update_settings(conn, patch, mutation?)` | mutation · set | dynamic: `settings:write`, `providers:manage` | 20 s | `patch`, `providerInstanceMutation`? |
| `server.discoverSourceControl` | same | `server::discover_source_control(conn)` | read · free | `orchestration:read` | 20 s | — |
| `server.reportClientActivity` | same | `server::report_client_activity(conn, &ClientActivity)` | mutation · set | `orchestration:read` | 20 s | `clientId`, `clientKind`, `visible`, `focused`, `recentlyInteracted`, `scopes`, `observedAt`, `environmentId`?, `appState`?, `lowPowerMode`?, `batteryState`?, `networkType`?, `ttlMs`? |
| `server.upsertKeybinding` | same | `server::upsert_keybinding(conn, &Keybinding, replace?)` | mutation · set | `settings:write` | 20 s | `key`, `command`, `when`?, `replace`? |
| `server.removeKeybinding` | same | `server::remove_keybinding(conn, &Keybinding)` | mutation · set | `settings:write` | 20 s | `key`, `command`, `when`? |
| `server.searchAcpRegistry` | same | `server::search_acp_registry(conn, query, limit?)` | read · free | `orchestration:read` | 20 s | `query`, `limit`? |
| `server.prepareAcpRegistryAgent` | same | `server::prepare_acp_registry_agent(conn, agent_id)` | mutation · set | `providers:manage` | 20 s | `agentId` |

**`orchestration`** (10)

| Function (`fn`) | Tag | Rust (`api::…`) | Kind · replay | Scope | Deadline | Args |
| --- | --- | --- | --- | --- | --- | --- |
| `orchestration.dispatchCommand` | same | `orchestration::dispatch(conn, &Command)` | mutation · id | `orchestration:operate` | 20 s | the variant: `type`, `commandId`, … |
| `orchestration.launchThread` | same | `orchestration::launch_thread(conn, &ThreadLaunch)` | mutation · id | `orchestration:operate` | 20 s | `commandId`, `projectId`, `title`, `modelSelection`, `runtimeMode`, `interactionMode`, `workspaceStrategy`, `creationSource`?, `threadId`?, `reuseExistingThread`?, `generateTitle`?, `initialMessage`? |
| `orchestration.getThreadProjection` | same | `orchestration::thread_projection(conn, thread_id)` | read · free | `orchestration:read` | 20 s | `threadId` |
| `orchestration.getTurnItem` | same | `orchestration::turn_item(conn, thread_id, item_id, revision?)` | read · free | `orchestration:read` | 20 s | `threadId`, `itemId`, `revision`? |
| `orchestration.getTurnDiff` | same | `orchestration::turn_diff(conn, thread_id, from, to, ignore_whitespace?)` | read · free | `orchestration:read` | 20 s | `threadId`, `fromTurnCount`, `toTurnCount`, `ignoreWhitespace`? |
| `orchestration.getFullThreadDiff` | same | `orchestration::full_thread_diff(conn, thread_id, to, ignore_whitespace?)` | read · free | `orchestration:read` | 20 s | `threadId`, `toTurnCount`, `ignoreWhitespace`? |
| `orchestration.getWorkflowScript` | same | `orchestration::workflow_script(conn, thread_id, script_path)` | read · free | `orchestration:read` | 20 s | `threadId`, `scriptPath` |
| `orchestration.stopWorkflow` | same | `orchestration::stop_workflow(conn, thread_id, subagent_id)` | mutation · set | `orchestration:operate` | 20 s | `threadId`, `subagentId` |
| `orchestration.searchThreads` | same | `orchestration::search_threads(conn, query, limit?)` | read · free | `orchestration:read` | 20 s | `query`, `limit`? |
| `orchestration.getArchivedShellSnapshot` | same | `orchestration::archived_shell(conn)` | read · free | `orchestration:read` | 20 s | — |

**`preview`** (9)

| Function (`fn`) | Tag | Rust (`api::…`) | Kind · replay | Scope | Deadline | Args |
| --- | --- | --- | --- | --- | --- | --- |
| `preview.list` | same | `preview::list(conn, thread_id)` | read · free | `orchestration:read` | 20 s | `threadId` |
| `preview.open` | same | `preview::open(conn, thread_id, &PreviewOpen)` | mutation · never | `preview:operate` | 20 s | `threadId`, `url`?, `viewport`?, `profileId`?, `runtime`?, `reveal`? |
| `preview.navigate` | same | `preview::navigate(conn, thread_id, tab_id, url, resolved_title?)` | mutation · set | `preview:operate` | 20 s | `threadId`, `tabId`, `url`, `resolvedTitle`? |
| `preview.resize` | same | `preview::resize(conn, thread_id, tab_id, viewport)` | mutation · set | `preview:operate` | 20 s | `threadId`, `tabId`, `viewport` |
| `preview.adjust` | same | `preview::adjust(conn, thread_id, tab_id, &PreviewAdjust)` | mutation · set | `preview:operate` | 20 s | `threadId`, `tabId`, `colorScheme`?, `zoomFactor`?, `hardReload`?, `clear`? |
| `preview.refresh` | same | `preview::refresh(conn, thread_id, tab_id)` | mutation · set | `preview:operate` | 20 s | `threadId`, `tabId` |
| `preview.close` | same | `preview::close(conn, thread_id, tab_id?)` | mutation · set | `preview:operate` | 20 s | `threadId`, `tabId`? |
| `preview.clearProfile` | same | `preview::clear_profile(conn, profile_id)` | mutation · set | `preview:operate` | 60 s | `profileId` |
| `preview.reportStatus` | same | `preview::report_status(conn, thread_id, tab_id, nav, back, forward)` | mutation · set | `preview:operate` | 20 s | `threadId`, `tabId`, `navStatus`, `canGoBack`, `canGoForward` |

**`assets`** (2)

| Function (`fn`) | Tag | Rust (`api::…`) | Kind · replay | Scope | Deadline | Args |
| --- | --- | --- | --- | --- | --- | --- |
| `assets.createUrl` | same | `assets::create_url(conn, &AssetResource)` | read · free | dynamic: `filesystem:read`, `orchestration:read` | 20 s | `resource` |
| `attachments.delete` | same | `assets::delete_attachment(conn, attachment_id)` | mutation · set | `orchestration:operate` | 20 s | `attachmentId` |

**`vcs`** (7)

| Function (`fn`) | Tag | Rust (`api::…`) | Kind · replay | Scope | Deadline | Args |
| --- | --- | --- | --- | --- | --- | --- |
| `vcs.refreshStatus` | same | `vcs::refresh_status(conn, cwd)` | read · free | `orchestration:read` | 20 s | `cwd` |
| `vcs.listRefs` | same | `vcs::list_refs(conn, cwd, &RefQuery)` | read · free | `orchestration:read` | 20 s | `cwd`, `query`?, `cursor`?, `includeMatchingRemoteRefs`?, `refKind`?, `refresh`?, `limit`? |
| `vcs.createRef` | same | `vcs::create_ref(conn, cwd, ref_name, switch_ref?)` | mutation · never | `source-control:write` | 20 s | `cwd`, `refName`, `switchRef`? |
| `vcs.switchRef` | same | `vcs::switch_ref(conn, cwd, ref_name)` | mutation · set | `source-control:write` | 20 s | `cwd`, `refName` |
| `vcs.pull` | same | `vcs::pull(conn, cwd)` | mutation · set | `source-control:write` | 20 s | `cwd` |
| `vcs.init` | same | `vcs::init(conn, cwd, kind?)` | mutation · set | `source-control:write` | 20 s | `cwd`, `kind`? |
| `git.preparePullRequestThread` | same | `vcs::prepare_pull_request_thread(conn, cwd, reference, mode, thread_id?)` | mutation · never | dynamic: `source-control:write`, `orchestration:operate` | 20 s | `cwd`, `reference`, `mode`, `threadId`? |

**`review`** (2)

| Function (`fn`) | Tag | Rust (`api::…`) | Kind · replay | Scope | Deadline | Args |
| --- | --- | --- | --- | --- | --- | --- |
| `review.getDiffPreview` | same | `review::diff_preview(conn, cwd, base_ref?, ignore_whitespace?, file?)` | read · free | `filesystem:read` | 20 s | `cwd`, `baseRef`?, `ignoreWhitespace`?, `file`? |
| `review.getDiffFileContents` | same | `review::diff_file_contents(conn, &FileContentsQuery)` | read · free | `filesystem:read` | 20 s | `cwd`, `sourceKind`, `changeType`, `oldPath`, `newPath`, `baseRef`?, `headRef`? |

**`projects`** (4)

| Function (`fn`) | Tag | Rust (`api::…`) | Kind · replay | Scope | Deadline | Args |
| --- | --- | --- | --- | --- | --- | --- |
| `projects.readFile` | same | `projects::read_file(conn, cwd, relative_path, allow_missing?)` | read · free | `filesystem:read` | 20 s | `cwd`, `relativePath`, `allowMissing`? |
| `projects.searchEntries` | same | `projects::search_entries(conn, cwd, query, limit, kind?, image_only?)` | read · free | `filesystem:read` | 20 s | `cwd`, `query`, `limit`, `kind`?, `imageOnly`? |
| `projects.mutate` | same | `projects::mutate(conn, &ProjectMutation)` | mutation · id | dynamic: `orchestration:operate`, `settings:write` | 20 s | the variant: `type`, `commandId`, `projectId`, … |
| `projects.createNew` | same | `projects::create_new(conn, name)` | mutation · never | `orchestration:operate` | 20 s | `name` |

**`diagnostics`** (8)

| Function (`fn`) | Tag | Rust (`api::…`) | Kind · replay | Scope | Deadline | Args |
| --- | --- | --- | --- | --- | --- | --- |
| `server.getHostResources` | same | `diagnostics::host_resources(conn)` | read · free | `orchestration:read` | 20 s | — |
| `server.getProcessDiagnostics` | same | `diagnostics::processes(conn)` | read · free | `diagnostics:read` | 20 s | — |
| `server.getResourceTelemetryHistory` | same | `diagnostics::telemetry_history(conn, window_ms, bucket_ms)` | read · free | `diagnostics:read` | 20 s | `windowMs`, `bucketMs` |
| `server.signalProcess` | same | `diagnostics::signal_process(conn, pid, start_time_ms, signal)` | mutation · set | `environment:maintain` | 20 s | `pid`, `startTimeMs`, `signal` |
| `server.checkOtlpEndpoint` | same | `diagnostics::check_otlp_endpoint(conn, signal, url)` | read · free | `settings:write` | 20 s | `signal`, `url` |
| `server.getUsageSummary` | same | `diagnostics::usage_summary(conn, &UsageQuery)` | read · free | `diagnostics:read` | 20 s | `sinceDay`, `untilDay`, `timeZone`, `resolution`?, `sinceTime`?, `untilTime`?, `awaitRefresh`? |
| `server.refreshUsageRates` | same | `diagnostics::refresh_usage_rates(conn)` | read · free | `diagnostics:read` | 20 s | — |
| `server.runStorageCleanup` | same | `diagnostics::run_storage_cleanup(conn)` | mutation · never | `settings:write` | 20 s | — |

**`provider`** (9)

| Function (`fn`) | Tag | Rust (`api::…`) | Kind · replay | Scope | Deadline | Args |
| --- | --- | --- | --- | --- | --- | --- |
| `server.refreshProviders` | same | `provider::refresh(conn, instance_id?, cwd?, fresh?, refresh_models?)` | read · free | `orchestration:read` | 20 s | `instanceId`?, `cwd`?, `fresh`?, `refreshModels`? |
| `server.updateProvider` | same | `provider::update(conn, provider, target_version?, instance_id?)` | mutation · never | `providers:manage` | 300 s | `provider`, `targetVersion`?, `instanceId`? |
| `provider.auth.start` | same | `provider::auth_start(conn, instance_id, method_id?, return_url?, callback_mode?)` | mutation · never | `providers:manage` | 20 s | `instanceId`, `methodId`?, `returnUrl`?, `callbackMode`? |
| `provider.auth.respond` | same | `provider::auth_respond(conn, instance_id, flow_id, interaction_id, &AuthResponse)` | mutation · never | `providers:manage` | 20 s | `instanceId`, `flowId`, `interactionId`, `response` |
| `provider.auth.cancel` | same | `provider::auth_cancel(conn, instance_id, flow_id)` | mutation · set | `providers:manage` | 20 s | `instanceId`, `flowId` |
| `provider.auth.logout` | same | `provider::auth_logout(conn, instance_id)` | mutation · set | `providers:manage` | 20 s | `instanceId` |
| `provider.install.start` | same | `provider::install_start(conn, instance_id)` | mutation · never | `providers:manage` | 20 s | `instanceId` |
| `provider.install.cancel` | same | `provider::install_cancel(conn, instance_id, operation_id)` | mutation · set | `providers:manage` | 20 s | `instanceId`, `operationId` |
| `provider.install.remove` | same | `provider::install_remove(conn, instance_id)` | mutation · set | `providers:manage` | 20 s | `instanceId` |

**`projectClone`** (1)

| Function (`fn`) | Tag | Rust (`api::…`) | Kind · replay | Scope | Deadline | Args |
| --- | --- | --- | --- | --- | --- | --- |
| `projectClone.start` | same | `project_clone::start(conn, &CloneStart)` | mutation · never | `source-control:write` | 20 s | `projectId`, `title`, `createdAt`, `destinationPath`, `provider`?, `repository`?, `remoteUrl`?, `protocol`? |

**`filesystem`** (2)

| Function (`fn`) | Tag | Rust (`api::…`) | Kind · replay | Scope | Deadline | Args |
| --- | --- | --- | --- | --- | --- | --- |
| `filesystem.browse` | same | `filesystem::browse(conn, partial_path, cwd?)` | read · free | `filesystem:read` | 20 s | `partialPath`, `cwd`? |
| `filesystem.getMetadata` | same | `filesystem::metadata(conn, paths)` | read · free | `filesystem:read` | 20 s | `paths` |

**`pullRequests`** (13)

| Function (`fn`) | Tag | Rust (`api::…`) | Kind · replay | Scope | Deadline | Args |
| --- | --- | --- | --- | --- | --- | --- |
| `pullRequests.list` | same | `pull_requests::list(conn, &ListQuery)` | read · free | `orchestration:read` | 20 s | `state`, `involvement`?, `filters`?, `projectId`?, `projectIds`?, `host`?, `limit`?, `cursors`?, `query`? |
| `pullRequests.detail` | same | `pull_requests::detail(conn, &PrRef)` | read · free | `orchestration:read` | 20 s | `projectId`, `repository`, `number`, `host`?, `expectedAccountId`?, `allowStale`? |
| `pullRequests.checks` | same | `pull_requests::checks(conn, &PrRef)` | read · free | `orchestration:read` | 20 s | `projectId`, `repository`, `number`, `host`?, `expectedAccountId`?, `allowStale`? |
| `pullRequests.activity` | same | `pull_requests::activity(conn, &PrRef)` | read · free | `orchestration:read` | 20 s | `projectId`, `repository`, `number`, `host`?, `expectedAccountId`?, `allowStale`? |
| `pullRequests.routing` | same | `pull_requests::routing(conn, &PrRef)` | read · free | `orchestration:read` | 20 s | `projectId`, `repository`, `number`, `host`?, `expectedAccountId`?, `allowStale`? |
| `pullRequests.routingIdentity` | same | `pull_requests::routing_identity(conn, host)` | read · free | `orchestration:read` | 20 s | `host` |
| `pullRequests.reviewerCandidates` | same | `pull_requests::reviewer_candidates(conn, &PrRef)` | read · free | `orchestration:read` | 20 s | `projectId`, `repository`, `number`, `host`?, `expectedAccountId`?, `allowStale`? |
| `pullRequests.labelCandidates` | same | `pull_requests::label_candidates(conn, &PrRef)` | read · free | `orchestration:read` | 20 s | `projectId`, `repository`, `number`, `host`?, `expectedAccountId`?, `allowStale`? |
| `pullRequests.comment` | same | `pull_requests::comment(conn, &PrRef, body)` | mutation · never | `source-control:write` | 20 s | `projectId`, `repository`, `number`, `body`, `host`?, `expectedAccountId`?, `allowStale`? |
| `pullRequests.submitReview` | same | `pull_requests::submit_review(conn, &PrRef, verdict, body, comments)` | mutation · never | `source-control:write` | 20 s | `projectId`, `repository`, `number`, `verdict`, `body`, `comments`, `host`?, `expectedAccountId`?, `allowStale`? |
| `pullRequests.runAction` | same | `pull_requests::run_action(conn, &PrRef, &PrAction)` | mutation · never | `source-control:write` | 20 s | `projectId`, `repository`, `number`, `action`, `mergeMethod`?, `updateMethod`?, `stackNumber`?, `expectedStackHeads`?, `host`?, `expectedAccountId`?, `allowStale`? |
| `pullRequests.requestReviewers` | same | `pull_requests::request_reviewers(conn, &PrRef, reviewers, requested)` | mutation · set | `source-control:write` | 20 s | `projectId`, `repository`, `number`, `reviewers`, `requested`, `host`?, `expectedAccountId`?, `allowStale`? |
| `pullRequests.setLabels` | same | `pull_requests::set_labels(conn, &PrRef, labels, applied)` | mutation · set | `source-control:write` | 20 s | `projectId`, `repository`, `number`, `labels`, `applied`, `host`?, `expectedAccountId`?, `allowStale`? |

**`device`** (7)

| Function (`fn`) | Tag | Rust (`api::…`) | Kind · replay | Scope | Deadline | Args |
| --- | --- | --- | --- | --- | --- | --- |
| `device.list` | same | `device::list(conn, update_tool?, inspect_only?, retry_host_id?)` | read · free | dynamic: `orchestration:read`, `orchestration:operate` | 300 s | `updateTool`?, `inspectOnly`?, `retryHostId`? |
| `device.configure` | same | `device::configure(conn, enabled?, agent_access_enabled?, onboarding_completed?)` | mutation · set | `settings:write` | 300 s | `enabled`?, `agentAccessEnabled`?, `onboardingCompleted`? |
| `device.testHost` | same | `device::test_host(conn, &SshHost)` | read · free | `settings:write` | 300 s | `id`, `label`, `target`, `kind`?, `identityFile`?, `port`? |
| `device.open` | same | `device::open(conn, thread_id, device_id, platform, host_id?, boot?)` | mutation · never | `orchestration:operate` | 180 s | `threadId`, `deviceId`, `platform`, `hostId`?, `boot`? |
| `device.close` | same | `device::close(conn, thread_id, host_id?, device_id?, shutdown?)` | mutation · set | `orchestration:operate` | 60 s | `threadId`, `hostId`?, `deviceId`?, `shutdown`? |
| `device.detail` | same | `device::detail(conn, device_id, host_id?)` | read · free | `orchestration:read` | 20 s | `deviceId`, `hostId`? |
| `device.action` | same | `device::action(conn, &DeviceAction)` | mutation · never | `orchestration:operate` | 30 s | the variant: `type`, `deviceId`, `hostId`, … |

**`scheduledTasks`** (5)

| Function (`fn`) | Tag | Rust (`api::…`) | Kind · replay | Scope | Deadline | Args |
| --- | --- | --- | --- | --- | --- | --- |
| `scheduledTasks.list` | same | `scheduled_tasks::list(conn)` | read · free | `orchestration:read` | 20 s | — |
| `scheduledTasks.upsert` | same | `scheduled_tasks::upsert(conn, &TaskUpsert)` | mutation · id | `orchestration:operate` | 20 s | `title`, `prompt`, `enabled`, `schedule`, `projectId`, `workspaceStrategy`, `modelSelection`, `runtimeMode`, `interactionMode`, `commandId`?, `id`?, `requireExisting`?, `threadId`?, `createdBy`?, `creationSource`? |
| `scheduledTasks.setEnabled` | same | `scheduled_tasks::set_enabled(conn, id, enabled)` | mutation · set | `orchestration:operate` | 20 s | `id`, `enabled` |
| `scheduledTasks.delete` | same | `scheduled_tasks::delete(conn, id)` | mutation · set | `orchestration:operate` | 20 s | `id` |
| `scheduledTasks.runNow` | same | `scheduled_tasks::run_now(conn, id)` | mutation · never | `orchestration:operate` | 20 s | `id` |

**`agentSessions`** (2)

| Function (`fn`) | Tag | Rust (`api::…`) | Kind · replay | Scope | Deadline | Args |
| --- | --- | --- | --- | --- | --- | --- |
| `agentSessions.scan` | same | `agent_sessions::scan(conn)` | read · free | `orchestration:read` | 20 s | — |
| `agentSessions.import` | same | `agent_sessions::import(conn, project_id, expected_workspace_root?)` | mutation · set | `orchestration:operate` | 20 s | `projectId`, `expectedWorkspaceRoot`? |

**`mcpApps`** (3)

| Function (`fn`) | Tag | Rust (`api::…`) | Kind · replay | Scope | Deadline | Args |
| --- | --- | --- | --- | --- | --- | --- |
| `mcpApps.callTool` | same | `mcp_apps::call_tool(conn, thread_id, item_id, name, arguments)` | mutation · never | `orchestration:operate` | 20 s | `threadId`, `itemId`, `name`, `arguments` |
| `mcpApps.readResource` | same | `mcp_apps::read_resource(conn, thread_id, item_id, uri)` | read · free | `orchestration:read` | 20 s | `threadId`, `itemId`, `uri` |
| `mcpApps.updateModelContext` | same | `mcp_apps::update_model_context(conn, thread_id, item_id, conversation_thread_id, content?, structured_content?)` | mutation · set | `orchestration:operate` | 20 s | `threadId`, `itemId`, `conversationThreadId`, `content`?, `structuredContent`? |

**`shell`** (1)

| Function (`fn`) | Tag | Rust (`api::…`) | Kind · replay | Scope | Deadline | Args |
| --- | --- | --- | --- | --- | --- | --- |
| `shell.openInEditor` | same | `shell::open_in_editor(conn, cwd, editor, reveal?)` | mutation · never | `orchestration:operate` | 20 s | `cwd`, `editor`, `reveal`? |

**`secrets`** (1)

| Function (`fn`) | Tag | Rust (`api::…`) | Kind · replay | Scope | Deadline | Args |
| --- | --- | --- | --- | --- | --- | --- |
| `secrets.answerRequest` | same | `secrets::answer(conn, thread_id, turn_item_id, &SecretAnswer)` | mutation · never | `orchestration:operate` | 20 s | `threadId`, `turnItemId`, `answer` |

**`worktreeSetup`** (1)

| Function (`fn`) | Tag | Rust (`api::…`) | Kind · replay | Scope | Deadline | Args |
| --- | --- | --- | --- | --- | --- | --- |
| `worktreeSetup.cancel` | same | `worktree_setup::cancel(conn, thread_id)` | mutation · set | `orchestration:operate` | 20 s | `threadId` |

**`terminal`** (3)

| Function (`fn`) | Tag | Rust (`api::…`) | Kind · replay | Scope | Deadline | Args |
| --- | --- | --- | --- | --- | --- | --- |
| `terminal.open` | same | `terminal::open(conn, &TerminalOpen)` | mutation · set | `terminal:operate` | 20 s | `threadId`, `terminalId`, `cwd`, `worktreePath`?, `cols`?, `rows`?, `env`?, `providerInstanceId`? |
| `terminal.write` | same | `terminal::write(conn, thread_id, terminal_id, data)` | mutation · never | `terminal:operate` | 20 s | `threadId`, `terminalId`, `data` |
| `terminal.resize` | same | `terminal::resize(conn, thread_id, terminal_id, cols, rows)` | mutation · set | `terminal:operate` | 20 s | `threadId`, `terminalId`, `cols`, `rows` |

Merged with what the library already had, so each payload has one builder:

- `pull_requests::request` and `pull_requests::payload` keep their signatures and
  error codes (`invalid_filter`, `invalid_pull_request`, `invalid_command`,
  `invalid_comment`, `invalid_review`) and the page's own rules (a comment needs
  words; only the page's nine actions), but build through `api::pull_requests`.
  Changes: a `list` query over 200 characters once trimmed and malformed
  `cursors` are now `invalid_filter` (before, the first reached the server and
  the second was dropped), and `projectId` must pass `ids::id`.
- `terminal::{session, dimensions, write, resize}` keep their signatures and codes
  and build through `api::terminal`; `Connection::terminal_write`/`terminal_resize`
  send through the registry. Changes: the thread id must pass `ids::id`, and an
  empty terminal id is `invalid_terminal` (was `invalid_id`).
- `usage_summary::request` builds its payload with `api::diagnostics::UsageQuery`;
  a blank time zone is now `invalid_time_zone`.
- `projects::command` is unchanged; `ProjectMutation::from_command(command_id, &built)`
  types its output for `projects.mutate`.
- `models::model_catalog`/`set_model_favorite` and `Connection::document` already
  went through `api::server` and `api::assets` (tanda 1). `Connection::upload`
  keeps `attachments.createUploadUrl`, which it streams.
- Removed as redundant: the strict reader and small checks private to
  `api::orchestration` (now `api::reader`, shared by every module), the private
  payload code in `pull_requests` and `terminal`, and `pull_requests::ACTIONS`
  (private; `api::pull_requests::ACTIONS` lists all eleven contract actions).

`tests/fixtures/t3-contract.json` pins the tag, stream and scope of every T3 RPC
at the commit in `api::CONTRACT`, and the list of T3 scopes. Regenerate it with
`tools/extract-t3-contract <t3code checkout>`. `tests/api_contract.rs` fails if
the registry drifts from it: a tag T3 does not serve, another stream flag, a
fixed scope that differs, a dynamic scope T3 does not have (or the reverse), or a
scope string T3 does not define. One scope is decided inside a handler rather
than in T3's tables: `device.list` calls `requiredScopeForDeviceList` in `ws.ts`.
The extractor records it as an exception and stops if the handler no longer
calls it. With `T3_CONTRACTS_DIR=<checkout>/packages/contracts/src`
set, the same test also re-reads the live sources.

### Typed streams

The 13 stream tags ot3 uses are registry rows with `"stream": true` in
`api::streams`. Each has a `*_spec` that validates the payload with no network
and returns a `StreamSpec` (registry row plus exact wire payload), and a
`subscribe_*` that runs the existing `Connection::subscribe_probed` loop with
the caller's `cancel`, `probe` and `receive`. Neither reconnects nor backs off:
that stays with the host, as does keeping the cursor.

```rust,no_run
use t3_protocol::api::streams::{self, ThreadSub};
# fn example(conn: &t3_protocol::Connection, cancelled: impl Fn() -> bool) -> t3_protocol::Result<()> {
let sub = ThreadSub { after_sequence: Some(41), accept_bounded_snapshot: Some(true), ..ThreadSub::new("t-1") };
let mut cursor = sub.after_sequence;
streams::subscribe_thread(conn, &sub, cancelled, || false, |item| {
    if let Some(item) = item {
        // A snapshot sets the cursor; events, known or not, only move it on.
        cursor = t3_protocol::stream::advance("orchestration.subscribeThread", cursor, &item);
    }
    Ok(())
})?;
// After a cut: subscribe again with `after_sequence: cursor`, with the host's backoff.
# Ok(()) }
```

| `fn` | Wire tag | Rust | Items · cursor |
| --- | --- | --- | --- |
| `orchestration.subscribeShell` | same | `subscribe_shell(conn, &ShellSub, …)` | `kind`: `snapshot`, `synchronized`, `project.*`, `thread.*` · `afterSequence` |
| `orchestration.subscribeThread` | same | `subscribe_thread(conn, &ThreadSub, …)` | `kind`: `snapshot`, `synchronized`, `event` · `afterSequence` |
| `orchestration.subscribeCommandOutput` | same | `subscribe_command_output(conn, thread_id, item_id, …)` | `kind`: `replace`, `append` |
| `terminal.attach` | same | `subscribe_terminal_attach(conn, &TerminalAttach, …)` | `type`, first `snapshot` · `sequence` (ordering only) |
| `device.subscribeDeviceState` | `subscribeDeviceState` | `subscribe_device_state(conn, …)` | whole state per item |
| `preview.subscribeDiscoveredLocalServers` | `subscribeDiscoveredLocalServers` | `subscribe_local_servers(conn, urls?, …)` | whole list per item |
| `preview.subscribePreviewEvents` | `subscribePreviewEvents` | `subscribe_preview_events(conn, …)` | `type` events |
| `server.subscribeServerConfig` | `subscribeServerConfig` | `subscribe_server_config(conn, &ConfigSub, …)` | `type`, first `snapshot` |
| `worktreeSetup.subscribeWorktreeSetup` | `subscribeWorktreeSetup` | `subscribe_worktree_setup(conn, thread_id, …)` | whole state or `null` · `sequence` (ordering only) |
| `server.getStorageCleanupReport` | same | `subscribe_storage_cleanup_report(conn, …)` | report or `null` |
| `provider.install.subscribe` | same | `subscribe_provider_install(conn, instance_id, …)` | whole state per item |
| `provider.auth.subscribe` | same | `subscribe_provider_auth(conn, instance_id, …)` | whole state per item |
| `git.runStackedAction` | same | `subscribe_stacked_action(conn, &StackedAction, …)` | `kind` events, ends with `action_finished`/`action_failed`; replay `never` |

`stream::cursor_of(tag, &item)` is an item's place in its stream:
`snapshotSequence` of a shell or thread snapshot, otherwise `sequence`, also for
thread events this build cannot decode. `stream::advance` applies ot3's rule (a
snapshot sets the cursor; any other item only moves it forward), and
`stream::start_of` tells whether a first item replaces the held state
(`Snapshot`), resumes it from `afterSequence` (`Resume`) or only carries events
(`Events`).

`stream::first_item(conn, &spec, timeout, accept)` reads a stream like a request
with a deadline. Four composed functions without a wire tag use it, callable
through `call` like any unary function: `provider.installState`,
`provider.authState` (10 s), `diagnostics.storageCleanupReport` (10 s, `null`
before the first run) and `preview.discoveredServers` (4 s, waits for a list
with servers). Nothing within the deadline is `stream_timeout`.

Over the C ABI, `subscribe` takes `fn` and `args` next to the raw `tag` and
`payload`. With `fn` the arguments are validated before a connection is looked
up (`unknown_function`, `not_a_stream`, `invalid_<field>`); the raw form is
unchanged. `call` on a stream function is `stream_function`. `describe` adds
`streams`, one entry per stream function: `itemKey`, `items`, `starts`, `cursor`
(`afterSequence` or `sequence`), `ends` and `firstItem`. `pollSubscription` adds
`cursor` (the payload's `afterSequence` advanced over every polled item) and
`start`. All of this is additive; `abiVersion` stays 3.

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
