# Owned native browser audio capture

`ui/voice/browser-capture-owner.mjs` consumes the complete pinned
`NativePcmCapture.js` factory with actual Chromium MediaStream, AudioContext,
AudioWorkletNode, Blob and AbortController objects. The trusted host supplies
the packaged factory/worklet URL and local callbacks. This owner has no server
credentials, model downloader, public capture endpoint or network destination.
The original producer owns device constraints, graph connections, silent output,
chunking, metering, five-minute duration, stop acknowledgement and flush order.

The owner is single-use. It tracks the actual permission promises, tracks and
AudioContext close promises, including a stream returned after cancellation.
It confirms close only after those resources settle. An unsettled system
permission still requires the enclosing process owner's deadline and killable
lifetime; no early successful receipt is manufactured. Cleanup uncertainty is
sticky. Streaming callbacks are fenced immediately on closure or a failed sink.
Buffered output stays in memory, is capped at 19,200,000 bytes, and is returned
once. The owned Blob URL is revoked on consumption or closure. A cancellation
during an asynchronous Blob read refuses the eventual audio result.

The renderer host supplies a sandboxed private origin and permission handler
bound to the exact capture frame, current capture lifetime and current user
decision. A preceding settings read or remembered destination alone cannot
authorize audio transfer. The host must independently recheck current server
authority and the audio consent grant before every SDK feed. The process host
below implements the frame and permission transport. Current server authority, consent store, permission UI and SDK consumers remain
to be connected.
No application preference or public capability selects this owner yet.

## Runtime evidence

`tools/test-native-pcm-browser` verifies the exact admitted Electron 44.10.1
runtime before and after execution. All checks run in a retained rootless
container with read-only sources/runtime, private HOME/tmp/Xauthority/display,
256 MiB shared memory, no network, host audio socket, device or profile.
Chromium's test-only fake media device supplies synthetic audio. Renderer
sandbox, context isolation and Node isolation remain enabled. The fake device
flag and permission handler exist only in the private fixture.

Seven actual media-graph cases cover streaming, buffered capture, cancellation
after preparation, cancellation while getUserMedia is pending, cancellation
during a buffered read, a failed streaming sink, and an unavailable exact device.
The fixture observes nonzero finite Float32 PCM, live graph before recording,
native meter events and stopped/closed tracks/contexts afterward. Late audio is
refused. Raw PCM is neither logged nor included in JSON receipts; only synthetic
buffer size/hash and lifecycle metadata are retained. Sample counts depend on
real audio scheduling and are observations, not fabricated golden values.

The independent renderer numeric comparison matches the entire compiled DSP
resource against the original native modules over 73 cases, all 4,800,000 limit
samples and 24,582 primitive math observations. It also retains the demonstrated
negative comparison with Node 26.13.0. The historical Qt raw-device adapter is
unselected and does not establish browser audio processing parity.

This evidence accepts synthetic device acquisition and the actual renderer
owner. It does not accept a physical microphone, OS permission dialog, acoustic
accuracy, full compositor integration, independent audio/cleanup consent,
cloud transport or public voice activation.

## Private capture process

`electron-capture-main.cjs`, `electron-capture-preload.cjs` and
`electron-capture-renderer.mjs` run that same owner in a separate pinned
Electron process. The process owns one private ephemeral profile, random local
origin and single-use capture. The preload exposes fixed commands and audio
callbacks only to its sandboxed, isolated main frame. Node integration,
navigation, popups, webviews and unrelated requests are denied. The main
process checks the exact frame and nonce on every renderer message.

Before constructing a renderer, the existing native descriptor helper admits
the inherited AF_UNIX/SOCK_STREAM endpoint against its captured parent PID and
UID and marks it close-on-exec. The launch payload carries a random capture
identity and private owned canonical directory; it is removed from the process
environment after parsing. A trusted launcher must still admit/snapshot the
complete runtime, helper and resource graph. No PATH runtime discovery, public
CLI action or application preference selects this process yet.

Audio crosses only that descriptor. Each frame has a 12-byte header:
`OVC1`, little-endian uint32 kind, little-endian uint32 body size. Kind 0 carries
at most 65,536 bytes of control JSON; kinds 1 and 2 carry at most 32,000 binary
PCM bytes for streaming and buffered output respectively. Buffered output is
split without JSON/base64 conversion. Total PCM is capped at 19,200,000 bytes;
application ingress has eight slots and output has an explicit bounded queue.
Header/body writes cannot interleave. Fragmented control frames are admitted
incrementally. No audio is written to stdout, a file, or a network destination.

Control messages carry the exact capture identity and increasing safe-integer
request IDs. `prepare` seals mode and exact microphone choice; `record` and
`stop` are ordered. A real audio permission request creates a separate opaque
decision identity. Only a matching current parent decision can approve it.
Other media/frame requests and stale decisions cannot authorize capture.
`close` bypasses a pending preparation, denies unresolved permission callbacks,
and waits for actual track/context closure before its successful receipt and
socket flush. Audio queued before closure is discarded. Parent loss, malformed
wire data and foreign capture identity exit with failure and manufacture no
successful cleanup receipt. The Rust supervisor below retains its own pidfd/process-group deadline and
uncertainty handling; process exit alone is not a graceful resource receipt.

`tools/test-native-pcm-host` builds only the unchanged descriptor addon on the
host under the compile lock; all executable checks use the same private
rootless isolation as the graph tests. Its Python fixture is an explicit test
parent, not a replacement for the future Rust consumer. Seven real-process
cases cover streaming, buffered output, denial, cancellation while permission
is pending, parent loss, oversized input and foreign capture identity. They
also reject stale permission decisions and split every control command into
seven-byte writes. Real PCM is finite and nonzero, stop byte counts match the
binary frames, private FD close-on-exec is physically observed before use, and
stdout remains empty. A captured pidfd confirms each actual process exit.
Negative exits retain their uncertainty classification. Sources/runtime are
verified again after execution; fixtures are retained without cleanup.

## Rust artifact admission

`src/voice_capture/resources.rs` implements the supervisor's local artifact
admission. Its compiled manifest pins all 72 files of the complete original
Electron 44.10.1 Linux x64 archive. `tools/pin-voice-electron-runtime` derives
that manifest directly from the checksum-pinned retained ZIP inside a private
container, without extraction or network access. Mutable acquisition-sidecar
hashes are not an authority. The three numeric/graph/process runners now use
the same source-controlled complete file manifest.

Runtime admission checks every original file's SHA-256, size, executable role,
canonical path, owner, read-only mode and single-link identity. It retains open
descriptors and checks device/inode, size, mtime, ctime and mode again before
reuse. The complete directory graph must match; unexpected files, directories
or links cannot extend the runtime's library search graph. It reuses the exact
admitted runtime rather than copying another large executable cache.

The six-file helper graph is captured into a fresh private directory from the
compiled original application resources and an explicitly admitted native
descriptor-addon receipt. Each small file is created exclusively, flushed and
made read-only before admission. No existing destination is adopted, and no
recursive cleanup is performed. Test fixtures retain their owned drift, link
and rejected-artifact cases.

`tools/test-voice-capture-resources` compiles only the opt-in example and passes
strict Clippy. Runtime checks run in a private rootless container against the
actual original runtime and actual helper snapshots. They cover all 72 runtime
files, six helper files, repeat-directory refusal, modified-snapshot refusal,
an owned-directory symlink, incorrect digest, writable addon and an owned
hardlink. This example opens no microphone or child process. It accepts artifact
admission only. The process supervisor has separate physical evidence below;
current server authority, consent and SDK integration remain to be implemented.


## Rust process supervisor

`src/voice_capture/mod.rs` consumes the admitted Runtime/Assets and a retained
native `Display` with sealed Xauthority. It creates a fresh private capture
directory and one private socket pair, starts a new process session, captures a
pidfd and keeps the leader unreaped until its terminal group fence. The child
receives a minimal environment, its captured peer PID/descriptor and one random
capture identity. No server credential, model path or transport destination is
forwarded. The ordinary factory enables no fake device or sandbox bypass. The
separate synthetic factory is restricted to an explicit container fixture.

Preparation, recording, stop and permission decisions are typed operations with
bounded slots and a single ordered writer. The incremental reader retains partial
frames across polling deadlines, validates identity/kind/length/phase, finite
Float32 PCM and cumulative size, and correlates stop byte counts. Actual runtime
and helper identities are rechecked before audio reaches the caller. The PCM
event implements neither Serialize nor Debug; it is not a status payload.

A cloneable cancellation handle submits one fixed urgent close with a reserved
request ID. It cannot change mode, device, capture or server scope. Fresh
commands stop immediately, queued audio is discarded, and the owner drains the
real close acknowledgement. Cancellation during a buffered stop also discards
its already queued ordinary result, preserving the close receipt. A successful
close requires the exact native acknowledgement, observed pidfd exit, terminal
group fence before reap, and successful leader status. Other outcomes remain
sticky Unknown. Startup errors after creating a process carry typed
`StartupFailure.cleanup`; unresolved startup is not silently treated as closed.
The parent application must preserve that uncertainty and refuse resource reuse.

The process is killable; blocking network inference is not performed on its
reader or in this module. Drop attempts the same bounded close. Constructor
failure has an independent process guard and never signals a numeric group after
reaping can release its identity. An exceptional pending reap uses a separate
resource-only reaper, not an unkillable microphone/inference thread.

`tools/test-voice-capture-process` runs the actual Rust owner, original native
producer and real Chromium graph inside the private container. Eight cases cover
streaming, buffered capture, denied permission, cross-thread cancellation during
permission and recording, cancellation during buffered stop, Drop and invalid
native-addon startup. The normal cases observe real leader exit and confirmed
closure; the invalid startup explicitly remains Unknown. No hardware, host
profile, server or credentials are used. Selected Clippy passes, and the
separate dispatcher/notices checkpoint also passes strict all-target compilation
and four reviewed runtime tests. Its dual-credential fixture verifies status,
models and explicit preparation use the same write identity as preflight, while
current revocation prevents those operations. The SDK's boolean is a binary/JSON
content flag, not a credential selector.

No GUI action or public capability creates this supervisor yet. The next real
consumer must bind the existing Backend/Composer voice controller, independent
audio/cleanup consent, current request authority and the exact SDK session;
every audio send still requires its own fresh consent/authority check.
