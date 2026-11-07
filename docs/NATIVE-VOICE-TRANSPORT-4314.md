# Consent-bound voice transport

`BoundSpeech` selects an environment, cloud provider and exact OpenRouter model
only from `AudioGrantGuard::destination_json()`. The original consent realm
decoded that destination and bound its identity. A null, stale or unrepresentable
destination is unavailable. The trusted environment resolver must return the
same environment ID; it cannot select a primary or draft server as a fallback.

Binding freezes one write credential and origin into the public SDK's
`CancellableSpeech`. Its blocking `Connection` is created and dropped by the
synchronous host factory before entering the caller's async runtime. Each
operation makes a fresh session request with that same credential and requires
`orchestration:operate`. The original audio guard and current recording/draft
owner predicate are checked before and after each request. A read credential
cannot authorize a different write identity. Authority revoked by the server
denies before PCM; the server also remains responsible for current admission
when it receives a request.

The caller owns cancellation and must cancel its operation token when its owner,
connection or consent changes during an await. The SDK retires the actual body
producer on cancellation or future Drop. It preserves native route deadlines,
PCM/error codecs, no replay and same-origin credential handling. Audio stays in
binary Rust buffers and the private capture descriptor. QML, generic worker
stdout, settings and logs receive no PCM or credential. Transcript delivery
still needs the original controller's owner/revision checks.

## Accepted consumer checkpoint

The SDK is public at `7ce6d441641a65de88ebe194ae66753bc2f6ce21` in
[PR38](https://github.com/avillagran/lib-t3-rust/pull/38). PR CI42417233413 and
push CI42417210783 pass on x86, ARM and Kotlin/JNI. Each native runner reports
450 passing Rust tests, one opt-in ignored case and 68 result reports including
doctests. The client vendors all 131 tracked crate files byte-for-byte, with no
private patch or package-version changes. Tokio is an explicit existing test
dependency for the example's caller-owned runtime.

`tools/test-voice-transport-process` compiles the actual Rust consumer and runs
only its reviewed example in a private rootless container. Sources/runtime are
read-only; HOME/tmp/display and HTTP loopback are private. The renderer has an
explicit synthetic audio device. Eight scenarios verify:

- Actual original consent helper, SDK session preflight and binary PCM HTTP,
  with distinct read/write fixture tokens and the write identity on both calls.
- Same-destination reselection denies the old guard before any connection.
- A resolver returning a foreign environment is refused.
- Fresh server authority revocation permits no audio request.
- A retired owner permits no request.
- Acknowledged cloud consent selects its exact OpenRouter provider/model and
  queries actual key-presence metadata using the same write identity.
- Cancellation during real HTTP inference waiting closes its actual socket.
- Actual Chromium capture produces 22,564 PCM bytes, retires with Confirmed
  cleanup and passes those identical bytes through the SDK. The server decodes
  the real envelope and independently compares the recording length and hash.

Receipt `voice-transport-process-tlg4viyn/result.json` fences the consumer,
capture/consent resources, public vendor manifest and Cargo inputs. Build,
strict all-target Clippy and runtime all pass. The earlier compile-only attempt
failed because its new dependency revision/lock was not cached; its retained
result is excluded from runtime acceptance. Acquisition populated that exact
public Git revision and changed zero package versions before the accepted run.

The destination/status interfaces here supply the original browser platform's
network dependency. This checkpoint does not replace its language/locale/stream
selection, cleanup-grant comparison or voice controller. Production streaming,
cleanup on the draft environment, persistent worker/controller wiring, consent
settings controls and physical microphone acceptance remain separate work.
The private HTTP server supplies deterministic transcript responses; this result
does not certify provider inference or acoustic accuracy. Public voice controls
are not activated by this owner-library checkpoint.
