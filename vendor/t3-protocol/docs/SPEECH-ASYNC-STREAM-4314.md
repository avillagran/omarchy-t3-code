# Cancellable async local speech stream

`CancellableSpeech::speech_stream` opens the original media-purpose ticket and
local voice WebSocket on the caller's Tokio runtime. `AsyncSpeechStream` uses
the existing complete `Protocol` directly: options/framing, 64,000-byte chunks,
two acknowledged frames in flight, the 19,200,000-byte outstanding bound, equal
revisions, nullable updates, finish ordering and the native response watchdog
all come from one implementation. The blocking API remains available.

`prepare_speech_stream` separates ticket acquisition from its one-shot
`SpeechStreamOpening::connect`. The host can store that opaque capability and
drive connection concurrently with microphone preparation, matching the original
browser platform's order. The UI never receives the ticket URL. Connect checks
current consent and cancellation again before touching the network, consumes
the capability once and starts the socket-opening watchdog at that point.

The media ticket uses the same admitted origin and credential, with no proxy,
redirect, retry or fallback. The WebSocket carries the private media ticket and
no Authorization header. DNS/TCP/TLS/WebSocket establishment is an owned future;
cancellation drops it and never opens a replacement. System resolver work may
outlive its dropped future on the caller runtime, with no audio or credential.
The original 120-second stream watchdog begins after ticket acquisition and
includes socket opening. The SDK's existing 8 MiB inbound WebSocket bound stays
separate from native application chunk and recording limits.

The host supplies current consent/authority and recording/draft ownership. Its
per-operation token cancels opening, reads and writes. Once connected, an
operation-local registration also shuts down the physical socket when the token
is cancelled while the caller runtime is idle. Registrations use weak callbacks
and are removed when their stream drops; a retired registration cannot keep a
socket or grant alive. The existing `SpeechCancellation` handle also interrupts
the actual socket directly from another thread.

Dropping an entered operation future retires the stream. There is no resumable
write or replay after abandoning a future. `poll_event` completes after at most
100 ms without data, allowing a capture actor to alternate owned reads and
feeds without abandoning the read future. Closing is bounded to the blocking
adapter's existing 100 ms best-effort close followed by physical shutdown.
Cancellation cannot undo audio already transmitted and is not a remote
inference-completion or microphone-cleanup receipt.

Tokio, futures-util and socket2 become explicit production dependencies already
present in the graph. The sole new locked package is tokio-tungstenite 0.28.0;
existing package versions remain unchanged. It uses the same tungstenite 0.28.0
protocol and Rustls/WebPKI roots as the blocking implementation. No runtime,
background polling thread or renderer network client is created by this API.

## Physical verification

Fifteen private loopback cases execute the actual SDK and sockets:

- Media ticket authority, upgrade path, missing WebSocket Authorization header,
  two-frame acknowledgment window, equal revisions and final text.
- Cancellation during an actual ticket wait or incomplete WebSocket handshake.
- Dropped opening and active-reader futures, with independent EOF/reset checks.
- Idle connected-stream cancellation while its runtime is not being polled.
- Multiple idle streams sharing a token after a registration slot is reused.
- Refused initial consent and revocation before a real audio feed.
- Refused cross-origin WebSocket redirect without ticket replay.
- Invalid binary server events and physical retirement.
- A maximum recording held behind the original acknowledgment window, cancelled
  before its remaining queued audio is transmitted.
- Deferred one-shot connection after ticket preparation, with no premature
  WebSocket connection; cancellation and revocation before connect refuse it.

The final checkpoint also passes nine buffered-request cases, four existing
blocking-stream cases, seven existing HTTP/control cases and six speech unit
cases, including the unchanged complete original 66-sequence stream oracle.
C static linking and JNI checks pass. All-target Clippy retains the same 26
baseline warning fingerprints with no additions. Every build/runtime check uses
official rootless android-sandbox with private state and network disabled.
Only dependency acquisition enables private container networking.

The retained `sdk-async-stream-20261107/check-v5.log` and opening proof fence the final
sources. These are synthetic loopback peers; no provider, model, microphone or
physical TLS endpoint is accepted by this result. Full x86/ARM/Kotlin CI and
the desktop capture/controller consumer are separate acceptance steps. There
are no new C/JNI speech entrypoints, release or automatic capability activation.
