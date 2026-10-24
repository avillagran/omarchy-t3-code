# Android / Omi integration

`lib-t3-rust` keeps the Cargo package/import names `t3-protocol` / `t3_protocol`
and the static archive `libt3_protocol.a`. ABI v2 is additive: the original four
C symbols and synchronous JSON operations remain available to existing adapters.

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
try {
    while (isForeground) {
        val batch = stream.poll() // waits up to 1s; wakes immediately on new data
        applyNativeItems(batch.items)
        if (batch.closed) break // inspect batch.error, reconcile/reconnect explicitly
    }
} finally {
    stream.close()
    client.close()
}
```

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

## Events through the existing JSON ABI

```json
{"op":"capabilities"}
{"op":"subscribe","tag":"orchestration.subscribeThread","payload":{"threadId":"selected","afterSequence":10,"requestCompletionMarker":true,"acceptBoundedSnapshot":true,"acceptCompactTurnItems":true}}
{"op":"pollSubscription","subscriptionId":"returned-id","timeoutMs":1000}
{"op":"cancelSubscription","subscriptionId":"returned-id"}
```

`capabilities` reports ABI/library versions and limits without connecting.
Subscribe returns `subscriptionId`; poll returns `{items, closed, error,
errorMessage}` inside the usual `{ok,data}` envelope. `error` is a stable code;
`errorMessage` carries a reason when the failure had one. Items preserve T3's native snapshot/event/
synchronized shapes. One consumer polls each stream. Poll waits on a condition
variable rather than repeatedly calling Java; it does not hold the connection
lock and cannot stall sends or reads. At most four streams exist per handle;
each queue holds at most 64 items and 8 MiB of serialized payload. Overflow ends
the stream with `stream_backpressure`, preserving already queued events. Resume
from the last applied cursor or obtain a fresh snapshot; never silently drop gaps.
Close a finished subscription to release its slot. Disconnect or a successful
connection switch cancels all streams; a failed switch preserves the old session.

The worker handles Effect Chunk/Ack, Ping/Pong and Interrupt framing in Rust.
It pings every 5 s and closes a stream with `stream_timeout` after 20 s without
any frame, so a suspended device or a changed network surfaces quickly.
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
- `tools/test-kotlin-jni`: canonical Kotlin adapter, typed errors and concurrent
  close against a real native request. Supply the cached compiler variables above
  and `JSON_JAR` for the host's org.json implementation.
- Android cross-build/link checks cover all four ABIs. These are not Android
  device/UI/lifecycle E2E checks, which remain consumer work.

The upstream reference and inspected source paths are recorded in
[UPSTREAM.md](UPSTREAM.md). Provider calls and real conversations are not used by
these fixture tests.
