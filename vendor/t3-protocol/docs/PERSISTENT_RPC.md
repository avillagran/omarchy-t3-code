# Persistent unary RPC sessions

`Connection::rpc_session(SessionOptions::default())` is an opt-in owner of one
physical authenticated WebSocket. The existing one-shot RPC, HTTP, streaming and
foreign-function APIs retain their behavior. A session never reconnects or
replays a request. Hosts must replace it explicitly after transport failure,
route changes or authentication changes.

Clones share the same socket and opaque random `id()`. A new physical connection
has a new ID even when it uses the same route and account. Cache entries may use
that ID as their session boundary; environment IDs, thread IDs, credentials and
snapshot counters are not substitutes. `is_open()` becomes false when the socket
fails or its owner closes. `failure()` exposes only a transport error code.

Use `start(tag, payload, timeout)` to submit concurrent requests, then wait on each
`PendingRpc`. Replies are correlated by request ID and may arrive out of order.
`rpc` is the blocking convenience form. `start_function` and `call_function` use
the existing typed registry's validation, payload builder and default deadline.
They reject streaming and composed functions before sending a unary request.

Dropping or cancelling a call sends `Interrupt` if it was transmitted. Its
cancellation cannot undo a server mutation. Transmitted timeout or transport loss
returns `rpc_outcome_unknown`; transmitted cancellation returns
`rpc_cancelled_outcome_unknown`. Unsent cancellation, timeout and session closure
have distinct codes. No error causes an automatic retry. Application rejections
finish only their own request and leave a healthy session usable. Late replies to
cancelled or expired requests cannot complete a subsequent request.

The default bounds are 16 in-flight calls, 8 MiB per frame and 16 MiB of queued
serialized requests. Serialization stops at the frame bound. Completed unread
results retain their in-flight permits, limiting retained response frames to
128 MiB at default settings, in addition to parser/object overhead. Hosts may set
1–128 calls, 1 byte–64 MiB frames and 1 byte–64 MiB queued requests. They remain
responsible for choosing an appropriate aggregate memory budget. Call deadlines
include queueing and must be between 1 nanosecond and 300 seconds. Socket reads
poll at 25 ms, writes time out at 100 ms; a failed/partial write is uncertain and
closes the session. Native 5-second ping and 20-second idle budgets apply.

Dropping the last session owner or calling `close()` ends its worker and completes
pending calls. Pending call handles and `closed()` lifecycle observers do not
keep that owner alive. A lifecycle observer can notify a host cache when its
physical socket finishes, without polling or retaining the owner. Session IDs,
errors and public handles contain no authorization material. The initial socket
uses the existing bearer/DPoP handshake and native TLS policy.

This API currently multiplexes unary requests only. Persistent subscriptions and
host environment lifecycle ownership require explicit integration; creating a
session alone does not establish full native client cache lifecycle parity.
