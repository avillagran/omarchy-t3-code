# Raw shell, HTTP and stream transport candidate

This additive source starts at public SDK
`5867a17ef461cff94490362ecfbcea9d553ff3e5`. Private strict all-target metadata and
normal library compilation passed, followed by eight real WebSocket cases,
four real HTTP cases and one original shared HTTP/DPOP factory regression.
These observations cover this additive Rust transport, not whole-client parity.

The first private compile failed with E0433 at the generic Serialize bound:
serde was only transitive. The retained correction adds a direct edge to the
already locked/cached serde 1.0.229 and its root lock dependency edge, without a
version or checksum change. The first runtime remains FAIL overall: its eight
WebSocket and four HTTP cases passed, then the separate DPOP fixture failed to
link because the RAM harness omitted reqwest. A second runtime added that
already-admitted extern and ran only the original DPOP case successfully; the
twelve passing cases were not repeated.

`Connection::subscribe_raw` and `subscribe_raw_probed` retain native item JSON,
including isolated UTF-16 units in shell titles, patches, messages and causes.
`RawStreamTick` separates Item, ChunkEnd and Idle. A null item remains an actual
Item. `RawStreamExit` separates local cancellation, native Success (with omitted
value distinct from null) and native Failure with its complete raw cause.

The authenticated WebSocket factory, 100 ms read ticks, existing SDK ping/probe/
idle constants, matching request IDs, ACK after the complete callback sequence,
and one physical subscription remain unchanged. Callback refusal ends that
socket without ACK. The host owns retry, resumption, cursor application and
compact-wire transformations. This API returns actual wire items and does not
silently promise the existing Value API's compact-ID/body convenience decoding.
The original Value subscription and wire decoder are byte-identical.

The typed `subscribe_shell_raw` and `subscribe_thread_raw` helpers reuse the
original ShellSub and ThreadSub payload validators. Generic payloads retain the original 8 MiB frame bound; only a thread
subscription that actually requests acceptBoundedSnapshot gets the existing
64 MiB factory limit. Raw callbacks do not extend those limits.

`request_json_raw` and `request_json_raw_with_limit` reuse the existing
same-origin credentialed request construction. The original send implementation
is moved intact behind a generic Serialize wrapper; Value callers retain the
same code and serialization type. Raw bodies implement that existing serializer.
Status, JSON media type, encoding, response limits and bounded-thread GET rules
remain the original rules. The raw response is not scalar-decoded or ID-expanded.
Raw HTTP is needed for the shell fallback as well as its live subscription.

## Private validation

Eight new real WebSocket cases cover shell snapshots and title deltas, lone high
and low units, escaped control strings, stale request IDs, JSON/native ping/pong,
null items, ordered ACK, omitted/null native exits, raw causes, cancellation,
callback refusal, malformed input, the actual three-second wake probe, and the
existing bounded-thread opt-in and typed pre-socket admission. Four real HTTP cases cover native strings and
POST bodies, exact protocol/auth headers, original status/media/encoding/JSON/
size errors, no socket on invalid routes/limits, and the original 8/64 MiB rule.
All thirteen component observations ran in canonical rootless android-sandbox,
using the actual normal Cargo graph, read-only source and target, and fresh RAM
fixture links. No Cargo command ran during runtime validation, no old executable
slot was released, and no executable was copied. The actual container and client
were observed terminal before postflight fences were accepted.

Validation receipts:

- Compile2: strict 235 artifacts and normal 162 artifacts, with changed SDK and
  affected integration targets genuinely fresh=false; result SHA256
  `c00e0e85a41a3894d5720542684250950cdaf23ae5320fff083bfcf34ca53f5e`.
- Runtime1b: 8 WebSocket and 4 HTTP passes, overall harness-link FAIL retained;
  result SHA256
  `f372e42e1d0155cdf6154429db0c346015a5ebf0ea533f5b8657da309d65f3c3`.
- Runtime2: the original `relay_dpop_http_and_rpc_contract` passed alone;
  result SHA256
  `82e7f20bc5251c3edbacca515c5f407517ec5c75ba72bf1f1bf72c021e09f327`.

All 334 graph files, the ring native directory, 70 SDK executables and 24 server
reservations remained unchanged across the runtime checks.

## Client integration remains separate

A raw transport alone does not fix a scalar Value reducer. The next client phase
must retain original text through shell snapshot/delta reduction and backend
text-dependent decisions. The original nightly 4380 source is authoritative:
`packages/client-runtime/src/state/shellReducer.ts` uses Effect Schema equivalence
and the actual contracts. A complete original declaration/module closure in the
admitted V8 runtime is preferred to copying an approximate reducer or replacing
unrepresentable text with empty strings. The search-only metadata/raw-row split
is not a general backend-state strategy.

The existing SDK's heartbeat timing is also not asserted as complete original
Effect timing parity: native protocol.ts supplies 15 seconds, while the original
Effect pinger checks lastSeen on five-second ticks. The additive API currently
inherits the existing SDK constants; an exact differential timing audit is a
separate requirement. No silent timeout change is made to either API here.
