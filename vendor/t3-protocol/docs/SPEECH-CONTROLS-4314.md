# Local speech controls and transcript cleanup

The buffered speech API now covers every native 4314 HTTP voice endpoint, including
local model status, catalog/benchmarks, explicit model preparation, download,
selection, cancellation, removal, acceleration, unload timeout and transcript
postprocessing. The existing buffered local/cloud PCM and cloud key/catalog APIs
remain unchanged. No call records audio, downloads a model or starts a worker
automatically. The host must explicitly invoke mutations and obtain consent for
transcript processing separately from recognition consent.

`Connection::speech_status` and `speech_models` return bounded, forward-compatible
JSON. Unsupported native platforms remain ordinary `supported:false` status
responses. `LocalModelAction` closes the mutation path set; the model identifier
travels only in JSON and follows the native 1..160 UTF-16-unit string codec without
invented trimming. Acceleration and unload options expose validated wire values.

`PostProcessRequest::decode` validates and shapes the native options, dictionary,
prompt, transcript and draft selection. Unknown fields are discarded. Dictionary
normalization uses the original speech codec and retains its documented Unicode 17,
en-US/default-locale and isolated-surrogate boundaries. Selection is a JavaScript
number, not a bounded integer: negative/fractional selections are accepted, large
JSON integers have double semantics, and signed zero canonicalizes on encoding.
The request's Debug representation does not expose transcript, draft or instructions.

Every call uses the admitted origin/credential and the existing no-proxy,
no-redirect speech HTTP transport. It follows the native client deadline: 30 seconds
for queries/control, 130 seconds for cloud PCM, and 30 minutes for model download,
local PCM transcription and transcript postprocessing. The cloud server may independently end execution after 2 minutes. There is no
retry, provider fallback
or account fallback. Void model preparation accepts any successful status without
requiring a JSON body. Success JSON is bounded to 8 MiB. Native invalid-request 400
errors are bounded to 64 KiB and preserve the recognized reason and trace identity
through the additive `ControlFailure` type; unknown errors retain generic protocol
failure rather than exposing arbitrary response fields. Existing cloud/PCM error
types are unchanged.

The reference generator executes actual pinned Effect schemas from nightly4314,
verifies their source hash, and retains 79 boundary/encoding outcomes. Tests include
raw JSON inputs beyond 2^53, UTF-16 limits, newline-sensitive GPU identifiers,
custom vocabulary duplicates and prompt/draft shaping. Private loopback HTTP
fixtures exercise all eleven new call shapes, authority headers, body omission,
tagged busy errors, refusal before transport, no replay, error bounds and a
cross-origin redirect that receives neither transcript nor credential.

The native HTTP authority is orchestration:operate for status, catalogs, preparation,
transcription and postprocessing. Model/configuration changes and cloud key writes
require settings:write. The server checks the current credential; the library does
not silently switch to another credential. Sixteen whole native client request
traces additionally verify the per-operation deadline and redirect policy.

All Rust/native/JNI tests use official android-sandbox exclusively. No live model
download, microphone, audio upload, provider request or fixture tree deletion is
used. These methods are currently Rust-only. The [local live stream](SPEECH-STREAM-4314.md)
provides a separate owned WebSocket API; new speech C/JNI entry points and the
actual host microphone/voice interface remain pending.
