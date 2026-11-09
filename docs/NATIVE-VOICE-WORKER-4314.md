# Persistent recording and stream owners

`voice_worker` joins the admitted capture process, original consent realm and
public SDK below the original browser platform/controller. A native
`CaptureFactory` owns the runtime, immutable helper graph, directory and display.
The platform's explicit mode selects buffered capture or the exact matching
stream; no cloud/local or streaming/buffered fallback is inferred here.

`PreparedStream` first obtains a consent-bound ticket. Its one-shot connection
starts on an explicitly owned network thread/runtime while capture prepares on
its own thread. No credential, ticket, executable path or PCM is serialized to
QML. The stream feeder checks identical transport/options/owner and admits only
one recorder. PCM queues retain the original five-minute byte bound and coalesce
adjacent packets without delaying delivery for a full chunk. SDK reads complete
their own bounded poll; receiving a command never drops an entered read future.

Capture stop drains the actual final PCM/result and produces an opaque buffered
recording when applicable. It does **not** send stream finish. The original
controller stops its recorder, awaits `releaseAudioSession`, rechecks its current
operation, then calls `streaming.finish`; the separate worker finish command
preserves that order. Confirmed microphone release leaves the network owner
alive until explicit finish/abort. Buffered recordings retain original options,
transport and owner across explicit retries, using a fresh cancellation token.
Their bytes are private zeroizing memory, never paths or ordinary UI payloads.

Abort retires network before microphone close can wait. Launch/capture failure,
owner loss and abandoned command channels retire the coupled stream. Separate
startup and running RAII guards also retire network before microphone cleanup
on unwind. Startup cancellation is permanent and uses the exact launched
capture identity, including before renderer readiness. Physical microphone
retirement remains distinct from a transcript or logical controller state.

One process-wide reservation admits a single physical capture. Unknown retirement
is sticky until application restart; a later successful close cannot clear it.
A proven not-started operation reports Confirmed rather than inventing physical
uncertainty. This host guard preserves the existing O3 cleanup boundary even
when the original logical controller releases its session after an error.
Permission denial now retains a bounded `capture_permission_denied` discriminator
through the renderer and capture protocol. The Qt adapter must reconstruct the
native `NotAllowedError` identity before public activation.

## Physical acceptance

`voice-worker-process-53paxlex/result.json` passes ten scenarios and combined
all-target strict compilation in private rootless Podman. The real peer withholds
stream ready until the actual microphone permission prompt exists, proving
parallel preparation. Actual binary audio arrives while capture is still active;
34,080 bytes reach the private stream and match the capture's terminal count.
The peer refuses an early finish until Confirmed microphone release. Cases also
verify duplicate/foreign feeder refusal, permission-denied identity, cancellation
with permission pending, retained buffered failure/retry (25,724 bytes with equal
digest), owner-loss refusal before HTTP, pre-spawn cancellation, no-child callback
panic classification, and a real killed-child Unknown blocking its successor.
Only the exact newly owned child is signalled, through pidfd in that last case.

The expected no-child callback panic is a caught worker failure, not a failed
test or a compositor crash. Rejected helper artifacts are retained inside the
container. No tree is recursively removed. The legacy capture and transport
regressions pass separately on the same source: eight capture scenarios,
including failed startup, and sixteen consent/HTTP/WS/cleanup scenarios.

Failed intermediate artifacts remain: `wd_hi5wu` and `3jve9gln` failed only strict
fixture style checks, before runtime. `w6q5o8ck` failed a fixture assertion that
incorrectly treated the native 64 KiB maximum frame size as a minimum; the actual
worklet normally emits a 32 KiB packet. Production chunking was unchanged.
`cal4eh5s` passes the preceding nine-case worker checkpoint; it does not cover
the final no-child callback-panic case. Evidence resides under `/var/tmp/o3-build`.

These results use real processes, consent, kernel identities and sockets with a
synthetic microphone/private inference peers. They do not accept acoustic
accuracy, a physical microphone, complete UTF-16 transport, or the persistent
watch-process/Qt/settings adapter. Public UI selection remains pending that
adapter and the original full status/options/cleanup consumers. Runtime safety
collection remains restricted to reviewed targets.
