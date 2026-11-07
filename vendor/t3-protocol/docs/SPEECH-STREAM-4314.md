# Owned local speech stream

`Connection::speech_stream` opens the native `/ws/voice` endpoint using a fresh
media-purpose ticket from the admitted origin and current credential. The ticket
is not an orchestration ticket, public connection URL, log field or error detail.
The HTTP ticket request and WebSocket handshake do not follow redirects. There
is no reconnection, mutation replay, endpoint fallback or cloud upload.

The host supplies its current consent/authority callback, explicitly feeds
Float32 samples, requests finish and consumes `LiveEvent` values. Bounded `poll_event` lets an actor alternate capture
input with responses; `next_event` waits for the next event. This API does
not record a microphone or infer consent. It converts the supplied samples to
little-endian PCM, copies only the supplied view and preserves the original
producer's splitting/coalescing behavior: 64,000-byte frames, two acknowledged
frames in flight and at most 19,200,000 queued plus unacknowledged bytes.
Acknowledgments release queue capacity; this is not a cumulative recording-time
limit. The host must enforce the native five-minute recording limit separately.
As in the original client, sample validation belongs to the receiving service;
the buffered PCM API provides its separate validation helper.

Ready/update/finished/error processing follows the complete original client
producer, including repeated equal revisions, safe-integer revision validation,
nullable text updates, finish after every pending acknowledgment, timeout reset
rules and cancellation without an error notification. The native response
watchdog is 120 seconds, including socket opening; an idle ready stream with no
pending work has no response timer. The SDK bounds individual incoming messages
to 8 MiB and polls physical reads at 100 ms so current consent and cancellation
can be observed. DNS lookup remains subject to the system resolver's behavior.

`SpeechCancellation` can interrupt a blocked read from another thread. Dropping
the stream shuts down its own physical socket; cancellation handles share one
descriptor and cannot reconnect it. Audio queues are cleared on failure and
their owned buffers are zeroized on drop. Transcript-bearing event/error Debug
representations omit their content. Raw transcript access is explicit.

The native oracle runs the entire pinned `openSpeechStream` implementation with
actual Effect event schemas and an inert socket, clock and abort signal. Its
66 sequences contain 1,137 observations, including chunk hashes from offset
Float32 views, queue limits, revision boundaries, consent, cancellation and
watchdog transitions. A separate 18-case whole PCM-envelope oracle confirms
fatal UTF-8, exactly one initial BOM, metadata bounds and borrowed view limits;
the existing decoder passes without production changes.

Physical private HTTP/WebSocket tests verify media-ticket authority, exact
upgrade path, binary frames, the two-frame acknowledgment window, final text,
cross-thread cancellation, consent revocation and redirect refusal. Every
runtime/build/JNI check uses official android-sandbox with private state and no
network access outside synthetic loopback. No model, provider, microphone or
audio upload is used. Server engines, host capture/controller integration and
new speech C/JNI entry points remain separate work.
