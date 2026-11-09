# Consent-bound asynchronous speech consumer

The client consumes public SDK revision
`ecfdfa8b6e92baa5def50b6b73d782bf014d971f` exactly across 133 crate files.
There are no private SDK patches. Its final x86/ARM/Kotlin CI passes 465 Rust
tests per native runner, one opt-in ignored case and 69 result reports, plus
C ABI/JNI and Kotlin checks. The sole new locked package is tokio-tungstenite
0.28.0; all prior versions remain unchanged. The existing native license
generator ran in a private offline container and added its exact MIT notice,
bringing the catalog to 274 entries without changing any existing entry.

`BoundSpeech::prepare_stream` first checks the same captured audio grant and
current recording owner, then rereads session authority with the same frozen
write credential used for the media ticket. It requires orchestration:operate.
A cloud destination refuses local streaming without making a request. The
returned SDK opening is opaque and one-shot: the original platform can start
connection alongside capture preparation without exposing a ticket to QML.
Both the grant and owner are checked again at connect and by the connected
stream. Same-destination reselection invalidates an old grant. Cancellation
physically closes idle streams even when their Tokio runtime is not polling.

The existing real consent/capture/cleanup consumer now passes 16 physical
scenarios and combined all-target strict compilation. The private Chromium
synthetic microphone produced 23,124 bytes; both buffered HTTP and actual SDK
WebSocket transport delivered the same length and digest to private peers.
The stream verifies actual start/ready, binary PCM, acknowledgments, finish and
final transcript. Additional cases fence cloud-to-local substitution, denied
session before ticket, grant revocation after ticket and cancellation before
connect. Original cleanup continues to use the draft's independent environment,
credential and grant. Three compiled license/dispatcher cases also pass.

Evidence is `/var/tmp/o3-build/voice-transport-process-o3h44b0a/result.json`
and `/var/tmp/o3-build/client-async-stream-20261107/license-tests-result.json`.
The first new build passed compilation but strict Clippy rejected a test
handshake closure's mandated large error type. Replacing that closure with the
existing handshake callback trait passed strict checks; no lint was disabled.
Its failed build is retained at `voice-transport-process-gescq6x9`. No runtime
test ran in that failed build.

These tests use real consent/capture processes and sockets, with synthetic
devices, server replies and transcripts in private rootless containers. They
do not accept a physical microphone, acoustic accuracy, a real cleanup provider,
simultaneous live capture, or the persistent worker/controller UI connection.
The whole native stream protocol, 15 SDK physical cases and original controller
have separate evidence. The global runtime-audit marker remains active.
