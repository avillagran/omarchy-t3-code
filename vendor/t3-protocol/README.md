# lib-t3-rust

Portable Rust core for [Omarchy T3 Code](https://github.com/avillagran/omarchy-t3-code)
and Omi. Extracted with source history from ot3; the Cargo package remains
**`t3-protocol`**, preserving Rust imports and the C ABI.

```toml
[dependencies]
t3-protocol = { git = "https://github.com/avillagran/lib-t3-rust", tag = "v0.2.0" }
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
let config = relay.rpc("server.getConfig", json!({}))?;
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
{"op":"subscribe","tag":"orchestration.subscribeThread","payload":{"threadId":"selected"}}
{"op":"pollSubscription","subscriptionId":"returned-id","timeoutMs":1000}
{"op":"cancelSubscription","subscriptionId":"returned-id"}
{"op":"disconnect"}
```

Responses are `{ "ok": true, "data": ... }` or
`{ "ok": false, "error": "stable_code" }`. Inputs remain owned by the caller;
the caller clears its credential buffers. Never free a handle during a call.
Connection changes are atomic: a failed connect leaves the prior connection
intact, so the host must treat the returned error as a failed switch.

The library supplies native Android JNI binaries and a Kotlin adapter. It does
not modify or publish the Omi application. Host tests and Android cross-builds
are separate from Android device/UI validation; see the integration guide.

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
| Chat operations | `composer`, `queue`, `questions`, `approvals`, `tasks` |
| Presentation | `activity`, `changes`, `documents`, `sidebar`, `models`, `i18n` |
| Configuration | `connection_catalog`, `projects`, `settings` |
| Cache policy | `compaction` |

Modules derive portable models and validate native commands; hosts own dispatch,
persistence, deduplication, reconnection, backoff and UI. Preventive compaction
requires a durable host-owned one-attempt claim before dispatch. Transport tests
use local fixture servers, never real conversations or paid provider runs.

## Development

Run `cargo test --locked` and `tools/test-c-abi` before changes are consumed by
either app. Native CI runs on Linux x86_64 and aarch64. Keep platform integrations
outside this crate. Fix shared behavior here, tag a version, then update each
consumer's pinned revision instead of copying source trees between applications.

The initial extraction preserves the source from ot3 commit
`fbe3dc94ba4d4f84dddea9424eb0913b27f34e64`. The [upstream contract reference](docs/UPSTREAM.md) records the original T3 code
used for the Android/event extension. Device/UI validation remains consumer work.

[MIT License](LICENSE). Independent community project; not an official T3 SDK.
