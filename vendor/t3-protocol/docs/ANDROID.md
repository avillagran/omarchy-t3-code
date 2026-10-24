# Android / Omi integration

`lib-t3-rust` keeps the Cargo package/import names `t3-protocol` / `t3_protocol`
and the static archive `libt3_protocol.a`. ABI v2 and v3 are additive: the original
four C symbols and synchronous JSON operations remain available to existing adapters.
ABI v3 adds the typed API ops `call` and `describe` (see [Typed API](#typed-api)).

## Using the Android package

The Android release provides an AAR with the canonical Kotlin adapter and JNI
libraries for **arm64-v8a, armeabi-v7a, x86 and x86_64**, targeting API 24+.
It has no UI, account configuration or background service. Add the verified AAR
to the Android application's dependencies:

```kotlin
implementation(files("libs/lib-t3-rust-android-v0.3.0.aar"))
```

The application supplies its existing INTERNET permission and Kotlin standard
library. The adapter is built with Kotlin 2.3.21 / JVM 17; use a compatible Kotlin
toolchain, or build the provided adapter sources with the application's compiler.
Its JNI class name is kept by the AAR's consumer ProGuard rules.

```kotlin
// All of this runs on a worker / Dispatchers.IO, never the UI thread.
val client = dev.avillagran.t3.T3Client()
client.connectBearer(baseUrl, hostOwnedToken)
val shell = client.request("/api/orchestration/shell")
val stream = client.subscribe("orchestration.subscribeThread", JSONObject()
    .put("threadId", selectedThreadId)
    .put("requestCompletionMarker", true)
    .put("acceptBoundedSnapshot", true)
    .put("acceptCompactTurnItems", true))
// A resume can also set afterSequence from the last applied native event.
val chunks = dev.avillagran.t3.T3Chunks()
try {
    while (isForeground) {
        val batch = stream.poll() // waits up to 1s; wakes at once on items, chunk ends, idle
        for (chunk in chunks.accept(batch)) applyNativeChunk(chunk) // whole server chunks
        if (batch.completionMarker || batch.idle) paintIfChanged() // initial state complete
        if (batch.closed) break // inspect batch.error/errorMessage, reconnect explicitly
    }
} finally {
    stream.close()
    client.close()
}
```

On resume or a `ConnectivityManager` network change, call
`client.wakeSubscriptions()` (or `stream.wake()`): a dead socket then closes the
batch with `stream_timeout` in about 3 s instead of 20 s, and the host subscribes
again from its cursor. Besides `errorMessage`, `T3Batch` carries `boundaries`,
`idle` and `completionMarker`; `T3Chunks` regroups items into whole server chunks.

Omi owns endpoint discovery, login, Android secure storage, lifecycle, cursor
persistence and UI. It connects directly to T3. Omarchy-Link is not a T3 message
relay. A current T3 cloud JWT can instead be supplied to `connectRelay`; a browser
access token bound to a different DPoP key cannot be reused. The C-handle proof
key currently lasts for that handle; Rust hosts can persist keys through secure
platform storage.

Stop subscriptions when the foreground view is released. `close()` cancels native
subscriptions before waiting for in-flight calls and releasing the handle; it is
idempotent and may block, so close from a worker. A screen/environment generation
must fence late results from an older request. No foreground service, background
permission, automatic reconnection or mutation retry is introduced by this adapter.

## Typed API

Prefer the typed API to hand-built `rpc` payloads. The Rust registry validates
arguments before any socket opens, applies each function's deadline, and sends
once. Kotlin sees one generic call and the registry's own description:

```kotlin
// Worker thread / Dispatchers.IO, as for every other call.
val status = client.call("vcs.refreshStatus", JSONObject().put("cwd", repoPath)) as JSONObject
client.call("orchestration.dispatchCommand", JSONObject()
    .put("type", "thread.archive")
    .put("commandId", outbox.commandIdFor(action)) // minted and persisted by the app
    .put("threadId", threadId))
val functions = client.describe().getJSONArray("functions") // offline
```

```json
{"op":"describe"}
{"op":"call","fn":"vcs.refreshStatus","args":{"cwd":"/home/me/repo"},"timeoutMs":20000}
```

`timeoutMs` is optional (1..300000) and overrides the function's deadline. Errors
arrive as `T3Exception(code, reason, requiredScope)`: `code` stays stable
(`unknown_function`, `invalid_input`, `invalid_<field>` such as `invalid_cwd`,
`not_connected`, `rpc_rejected`, `rpc_outcome_unknown`, …), `reason` is the
server's one-line message, and `requiredScope` names the scope a
`write_not_authorized` token lacked. Authorization is decided from T3's
`EnvironmentAuthorizationError` tag, never from the text of a message. The app
decides whether another token is available; the library never chooses one.

Each `describe` entry gives `fn`, `module`, `tag`, `kind`, `replay`, `scope` (or
`"dynamic"` plus `scopes`), `timeoutMs` and typed `args`. After
`rpc_outcome_unknown`, `replay` tells the app what is safe: `free` repeat a read,
`id` resend the identical payload with the same `commandId`, `set` repeating is
harmless, `never` read the state back first. The library never retries.

Typed Kotlin wrappers are generated from the saved `describe`
([`bindings/android/describe.json`](../bindings/android/describe.json)). Today the
generated [`T3Api.kt`](../bindings/android/tests/dev/avillagran/t3/T3Api.kt) is a test
fixture (`T3Api(client).vcs.refreshStatus(path)`); `cargo test --test api_describe`
fails when the registry, the saved `describe` and the wrappers disagree.

Functions in this release:

| `fn` | Kind · replay | Scope | Args |
| --- | --- | --- | --- |
| `server.getConfig` | read · free | `orchestration:read` | — |
| `server.getSettings` | read · free | `orchestration:read` | — |
| `server.updateSettings` | mutation · set | dynamic: `settings:write`, `providers:manage` | `patch`, `providerInstanceMutation?` |
| `orchestration.dispatchCommand` | mutation · id | `orchestration:operate` | the command object (`type`, `commandId`, …) |
| `orchestration.launchThread` | mutation · id | `orchestration:operate` | `commandId`, `projectId`, `title`, `modelSelection`, `runtimeMode`, `interactionMode`, `workspaceStrategy`, `creationSource?`, `threadId?`, `reuseExistingThread?`, `generateTitle?`, `initialMessage?` |
| `preview.list` | read · free | `orchestration:read` | `threadId` |
| `assets.createUrl` | read · free | dynamic: `filesystem:read`, `orchestration:read` | `resource` |
| `vcs.refreshStatus` | read · free | `orchestration:read` | `cwd` |
| `review.getDiffPreview` | read · free | `filesystem:read` | `cwd`, `baseRef?`, `ignoreWhitespace?`, `file?` |
| `projects.readFile` | read · free | `filesystem:read` | `cwd`, `relativePath`, `allowMissing?` |

## Events through the existing JSON ABI

```json
{"op":"capabilities"}
{"op":"subscribe","tag":"orchestration.subscribeThread","payload":{"threadId":"selected","afterSequence":10,"requestCompletionMarker":true,"acceptBoundedSnapshot":true,"acceptCompactTurnItems":true}}
{"op":"pollSubscription","subscriptionId":"returned-id","timeoutMs":1000}
{"op":"cancelSubscription","subscriptionId":"returned-id"}
{"op":"wakeSubscriptions"}
```

`capabilities` reports ABI/library versions and limits without connecting:
`maxSubscriptions` (8), `maxLargeSubscriptions` (2), `queueMaxItems` (64),
`queueMaxBytes`/`itemMaxBytes` (8 MiB), `largeItemMaxBytes` (64 MiB),
`largeQueueMaxBytes` (72 MiB), `maxPollMs` (1000), `probeTimeoutMs` (3000),
`idleTimeoutMs` (20000) and `pollFields`. Boundaries, idle, the completion marker
and `wakeSubscriptions` are additive, so `abiVersion` stays 3; a host can test
for `"wakeSubscriptions"` in `operations` or `"boundaries"` in `pollFields`.
Subscribe returns `subscriptionId`; poll returns `{items, boundaries, idle,
completionMarker, closed, error, errorMessage}` inside the usual `{ok,data}`
envelope. `error` is a stable code; `errorMessage` carries a reason when the
failure had one. Items preserve T3's native snapshot/event/synchronized shapes.

Poll returns as soon as something is new (items, a chunk end, `idle` turning
true, or the end of the stream), otherwise after `timeoutMs`:

| Field | Meaning |
|---|---|
| `items` | Native items in server order, never dropped or reordered. |
| `boundaries` | Strictly ascending end offsets into this batch's `items`: `n` means a server chunk ended after `items[0..n)`. Apply items and repaint at boundaries, so a replay burst never shows half-applied state. Items after the last boundary belong to a chunk that continues in the next poll; `0` means the chunk that ended the previous batch ended right there. |
| `idle` | The last chunk is complete and no new frame completed during one 100 ms read tick: caught up for now. A large frame may still be in transit, and it can be `true` before the first item. Stays `true` until the next item; turning `true` wakes one waiting poll. Without `requestCompletionMarker` it is the only catch-up signal (the same rule ot3 uses); with it, wait for `completionMarker`. |
| `completionMarker` | Only with `"requestCompletionMarker":true` in the payload: `true` from the batch that carries the server's `{"kind":"synchronized"}` item (left in `items` at its position) onward. The initial snapshot or replay is complete; paint it. |
| `closed`, `error`, `errorMessage` | Once `closed` nothing more arrives. `error` is a stable code (`null` after cancel); `errorMessage` an optional one-line reason. |

```json
{"items":[{"kind":"snapshot","snapshotSequence":41,"projection":{}},{"kind":"synchronized"},{"kind":"event","sequence":42,"event":{}}],
 "boundaries":[2],"idle":false,"completionMarker":true,"closed":false,"error":null,"errorMessage":null}
```

Here the first chunk (snapshot and marker) is complete and can be painted; the
event belongs to a chunk whose end arrives in a later poll. One consumer polls each stream. Poll waits on a condition
variable rather than repeatedly calling Java; it does not hold the connection
lock and cannot stall sends or reads. At most eight streams exist per handle
(shell, open thread, command output, server config, preview, terminal, devices…),
each on its own WebSocket because T3 serves one request per socket. Each queue
holds at most 64 items and 8 MiB of serialized payload. The one exception is
`orchestration.subscribeThread` with `acceptBoundedSnapshot:true`: its socket
accepts 64 MiB frames, so one item may be a 64 MiB snapshot and its queue holds
72 MiB (that snapshot plus 8 MiB of events). At most two of the eight streams may
use that mode, so a handle never queues more than 2 × 72 + 6 × 8 = 192 MiB of
serialized JSON (in practice far less: each poll drains its queue). A ninth
stream, or a third bounded one, fails with `subscription_limit`. Without the
flag a larger frame is refused by the socket and the stream ends with `disconnected`.
Overflow ends the stream with `stream_backpressure`, preserving already queued events. Resume
from the last applied cursor or obtain a fresh snapshot; never silently drop gaps.
Close a finished subscription to release its slot. Disconnect or a successful
connection switch cancels all streams; a failed switch preserves the old session.

The worker handles Effect Chunk/Ack, Ping/Pong and Interrupt framing in Rust.
It pings every 5 s and closes a stream with `stream_timeout` after 20 s without
any frame, so a suspended device or a changed network surfaces quickly.

To check sooner, call wake when the app returns to the foreground or
`ConnectivityManager` reports a network change:

```json
{"op":"wakeSubscriptions"}
{"op":"wakeSubscriptions","subscriptionId":"returned-id"}
```

It returns `{"probed":n}` (open streams probed; an unknown id fails with
`unknown_subscription`). Each probed stream pings at once, on its worker's next
100 ms read tick, and ends with `stream_timeout` unless a frame arrives within 3 s
of the call: a dead socket surfaces in about 3 s (3.0 to 3.1 s measured), a live
one keeps streaming. The library never reconnects; on `stream_timeout` the host
cancels the subscription and subscribes again from its cursor, with its own backoff.
Cancellation wakes a blocked poll immediately; the network worker observes it on
its next read tick. DNS/connect/TLS negotiation already in progress can take its
normal timeout before the worker exits. No callbacks into the JVM occur from the
stream worker. Server cursors, projections and retry/backoff remain host-owned.

## Long-running RPCs

```json
{"op":"rpcWithTimeout","tag":"device.configure","payload":{"enabled":true},"timeoutMs":120000}
```

Same request/exit as `rpc`, with an explicit deadline of 1 ms to 300 s
(`timeoutMs`, validated before connecting; otherwise `invalid_rpc_timeout`).
It is never retried: a deadline interrupts the request and returns
`rpc_outcome_unknown`, which the host reconciles. Failures keep `error` a stable
code and may add `message` with the server's one-line reason. Use it through
`T3Client.execute(JSONObject)` from a worker.

## Attachments

`T3Client.upload(fd, name, mimeType, sizeBytes)` accepts a borrowed readable file
descriptor. Open Android content with the resolver on a worker and keep its
ParcelFileDescriptor valid until JNI has entered the call. JNI owns a CLOEXEC
duplicate; Rust streams through T3's `attachments.createUploadUrl` grant and
signed same-origin POST. The duplicate is released on success and every error
path. There is no full-file byte-array copy. A nonresponsive provider read has a
bounded native deadline. Limits follow the core: 50 MiB/file, 10 MiB/image; the
host must also honor any stricter current provider/server policy.

`T3Client.image(id, name, mimeType)` returns validated image bytes through
`assets.createUrl` and its signed same-origin GET, with a 10 MiB bound. Kotlin
owns the returned byte array and supplies decoding/cache/UI. Images do not pass
through Base64 JSON. No bearer credential is sent to signed asset requests.

For other native hosts, `include/t3_protocol.h` exposes reader/release and image
callbacks. Upload transfers context ownership only when both callbacks exist;
release runs exactly once when Rust drops the reader, including invalid input.
Reader callbacks must be thread-safe and bounded. Image bytes are borrowed only
for the synchronous callback duration. Callbacks must not throw/unwind. Existing
JNI adapters can keep their four original C symbols and use the new JSON stream
operations; attachment callbacks require adapting their JNI marshaling.

## Transport options, pairing and assets

All of this is additive to ABI v2; `capabilities` lists it in `operations`,
`binaryOperations`, `connectionOptions` and `maxAssetBytes`.

```json
{"op":"connectBearer","httpBaseUrl":"https://environment.example","token":"stored-token","options":{"requestTimeoutMs":8000,"connectTimeoutMs":3000,"poolIdleTimeoutMs":90000,"maxBytes":8388608}}
{"op":"fetchDescriptor","httpBaseUrl":"https://environment.example"}
{"op":"pairBearer","httpBaseUrl":"https://environment.example","pairingCode":"from-the-link","environmentId":"optional-expected-id","client":{"label":"Omi","deviceType":"mobile","os":"android"},"connect":true}
{"op":"boundedThread","threadId":"selected"}
{"op":"requestOk","method":"POST","path":"/api/auth/clients/revoke","body":{"sessionId":"s-1"},"protocol":false}
```

`options` is optional on `connectBearer` and `connectRelay`. Unknown keys,
non-numeric values, a zero timeout, a timeout over 300 s or `maxBytes` outside
1 byte to 64 MiB fail with `invalid_connection_options`; `null` clears the
connect or pool-idle timeout. Options govern HTTP. Streams and RPC keep their
own deadlines (see above).

`fetchDescriptor` and `pairBearer` need no connection and send no credential
until the descriptor is checked. `pairBearer` returns `{accessToken,
tokenType, environmentId, descriptor, scope, expiresIn, connected}`. Store
`accessToken` in the Android Keystore-backed store and clear the response buffer.
With `"connect":true` the handle switches to the new session after the exchange
succeeds, cancelling its streams; a failed pairing leaves the old one in place.
`boundedThread` returns the restored bounded snapshot (up to 64 MiB).

Signed assets use the binary path, never Base64 JSON. Pass the metadata to the
JNI `image` native (`t3_client_image`), which forwards it unchanged:

```json
{"op":"assetBytes","resource":{"_tag":"attachment","attachmentId":"a","fileName":"clip.mp4","mimeType":"video/mp4","disposition":"inline"},"maxBytes":52428800,"timeoutMs":120000}
{"op":"projectFavicon","cwd":"/repo","path":"favicon.svg"}
```

The status is `{sizeBytes, mimeType}`, or `{found:false, sizeBytes:0}` with no
bytes for a project without a favicon. Errors are `asset_*`,
`invalid_asset_url`, `invalid_asset_limit` and `invalid_asset_timeout`; through
`t3_client_call` these two operations fail with `binary_operation`. The Kotlin
`T3Client` has no typed wrapper for them yet. A wrapper is a one-line
`T3Native.image` call with this metadata; the C++ adapter needs no change.

## Building from source

On an x86_64 Linux build host, explicitly install Rust's four Android targets and
the application's Android NDK. The build helper does not install or download SDKs
or targets. Omi's checked build uses NDK 28.2.13676358 and API 24:

```sh
rustup target add aarch64-linux-android x86_64-linux-android armv7-linux-androideabi i686-linux-android
ANDROID_NDK_HOME="$ANDROID_HOME/ndk/28.2.13676358" tools/build-android arm64-v8a x86_64 armeabi-v7a x86
```

Set `CARGO_NET_OFFLINE=true` with an already populated Cargo cache or vendor
configuration for offline builds. ELF libraries use 16 KiB maximum page alignment.
Outputs live under `dist/android/<abi>/libt3_jni.so`. Desktop Linux libraries are
not Android libraries, even when the CPU matches.

`tools/package-android` compiles the Kotlin wrapper and assembles an AAR. It needs
`JAVA_HOME`, `ANDROID_JAR`, `KOTLIN_STDLIB` and `KOTLIN_COMPILER_CLASSPATH` pointing
to explicitly installed/cached tools. Run it in the clean, committed release
checkout after building all four ABIs. It produces the versioned AAR and SHA-256
checksum in `dist/`; it does not sign or publish an Android application.

## Validation

- `cargo test --locked`: local HTTP/WebSocket fixtures cover exact event delivery,
  long-poll concurrency, cancellation/Interrupt, queue bounds, attachment grants,
  transport options and proxy bypass, signed asset bytes, descriptor/pairing and
  bounded snapshots, through the Rust API and the JSON/binary ABI.
- `tools/test-c-abi`: a real C caller links the static Rust archive.
- `JAVA_HOME=... tools/test-jni`: real JNI/JVM calls, Unicode, error paths and ownership.
- `tools/test-kotlin-jni`: canonical Kotlin adapter, typed errors, `call`/`describe`
  through the generated wrappers, and concurrent close against a real native
  request; `StreamFixture` drives streams through a JDK-only fake WebSocket (chunk
  boundaries, completion marker, idle, wake probe and error reasons). Supply the cached compiler variables above
  and `JSON_JAR` for the host's org.json implementation.
- Android cross-build/link checks cover all four ABIs. These are not Android
  device/UI/lifecycle E2E checks, which remain consumer work.

The upstream reference and inspected source paths are recorded in
[UPSTREAM.md](UPSTREAM.md). Provider calls and real conversations are not used by
these fixture tests.
