# Cancellable voice HTTP

The blocking speech calls retain their existing API. `Connection::cancellable_speech`
adds async same-credential session preflight, status, preparation, cloud-key presence, buffered transcription and
postprocessing for hosts that must interrupt an operation when its recording or
draft owner disappears. Constructing this client performs no request. The caller
supplies its async runtime and one `RequestCancellation` per operation lifetime.

`RequestCancellation` is permanent and cloneable. Cancelling wakes every entered
request; later requests with that token fail before connecting. Dropping a
request future drops the actual reqwest transaction and response and retires its
body producer without cancelling sibling operations that share a public token. The caller's
runtime owns the connection driver and must continue running through retirement
or retire its own runtime scope. An operation's Cancelled result is not a remote
inference-completion or capture-cleanup receipt.

Each operation checks the supplied current-consent/authority predicate before
starting and before returning its result. The host must also cancel its token
when that owner or consent changes during an await. Cancellation has priority
over a predicate that finishes after cancellation. This preserves the browser
caller's abort-signal contract without creating a hidden polling thread or
restoring grants from settings. Audio capture and the separate cleanup grant
remain responsibilities of the host.

The async client copies the same admitted origin and credential, supports DPoP,
disables proxies and redirects, and uses the existing native per-route deadlines:
30 seconds for ordinary voice control, 30 minutes for local inference and
postprocessing, and 130 seconds for cloud transcription. No retry policy can
replay a speech request; the blocking speech client also explicitly disables
reqwest's low-level retry classifier. This does not roll back audio already
transmitted before cancellation.

The PCM envelope, options, model query and postprocessing payload use the existing
codecs. Cloud errors and native invalid-request reasons/trace IDs share the same
pure decoders as the blocking API. Successful preparation accepts the original
empty 2xx response. Responses remain bounded with the existing media-type and
encoding rules. Neither the client nor its token exposes credentials or audio
through Debug.

Cancellation's registered wakers use the standard library. The actual HTTP body
also checks an operation-local lifetime between bounded 32 KiB frames, so the connection driver
cannot continue producing a recording merely because it retained the body after
the response future was dropped. Buffered socket bytes already transmitted are
not rolled back. `bytes` and `http-body` become explicit dependencies already
present in the production graph. Tokio, socket2 and futures-util are now explicit
production dependencies for the shared async stream adapter; its sole new
locked package is tokio-tungstenite 0.28.0. All compilation and runtime validation uses
the official rootless `tools/android-sandbox` and its existing isolated cache.
This Rust API does not add C/JNI voice entrypoints or activate a desktop client.

Physical loopback checks cover cancellation before connection, revoked consent,
the same-credential session, actual PCM envelope, OpenRouter query, original cloud/control errors, empty
preparation, shared-token wakeup, a blocked inference response, a partial response,
a backpressured maximum recording upload, dropping its future without cancelling
a shared token, discarded late text and refused audio
redirects. The server independently observes EOF/reset while the caller runtime
continues driving its transport. The initial paused-runtime fixture failed its
socket observation; that retained result is not used as acceptance.

The final local checkpoint passes nine physical request cases, six existing
speech unit cases and eleven existing HTTP/control/stream cases, followed by
actual C static linking and JNI runtime checks. All-target Clippy succeeds with
the same 26 warning fingerprints as its published baseline and no additions.
The retained `sdk-cancellable-request-20261107/proof.json` fences the final
production/test bytes and `check-v6.log`. Earlier paused-runtime and unbounded
upload failures remain retained. Full remote x86/ARM/Kotlin checks are separate
from this local result; no Android artifact release is implied.
