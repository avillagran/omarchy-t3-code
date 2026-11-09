# Lossless JavaScript UTF-16 RPC

Nightly 4380 (`ade473018d32003658b7d38693e083ee2e314bd8`) normalizes a
server search in `apps/web/src/state/queries.ts:112` with
`query.trim().slice(0, THREAD_SEARCH_QUERY_MAX_LENGTH).trim()`. The maximum is
200 JavaScript UTF-16 code units. Slicing can leave an isolated high surrogate;
a supplied string can also contain an isolated low surrogate. Native JSON
preserves those units with escapes. Search result snippets can likewise end at
an isolated unit after their own slice.

Rust `String` and `serde_json::Value::String` are Unicode scalar strings. They
cannot express that complete native domain. Converting through them, inserting
U+FFFD, dropping a unit, or clipping at a different boundary changes the query.

The additive public boundary provides:

- `Wtf16String`: owned code units, exact JSON escape decode/encode, explicit
  fallible UTF-8 conversion, and ECMAScript trim. It never repairs surrogates.
- `RawJson`: the public `serde_json::value::RawValue` type with validated JSON
  syntax. It retains surrogate escapes instead of decoding them into `String`.
- `Connection::rpc_raw` and `rpc_raw_with_timeout`: one physical request,
  existing authentication/WebSocket/heartbeat/deadline behavior, no retry.
- `RawRpcExit::Success(Some(value))` or `Failure(cause)`: raw result and cause.
  `Success(None)` preserves a native void success whose `value` is omitted by
  JSON.stringify, distinct from an explicit JSON null value.
- `search_threads_utf16` and its offline payload builder: the ordinary typed
  search options plus a lossless query. They enforce native ECMAScript trim and
  2..200 **code units**; callers retain responsibility for the original UI's
  clipping. The SDK does not clip or substitute units.

The existing Unicode-string search method also uses code-unit bounds and
ECMAScript trim. For example, U+FEFF trims, U+0085 does not, and 101 supplementary
characters exceed the 200-unit limit. Unrelated typed APIs are unchanged.

```rust
use t3_protocol::{RawJson, RawRpcExit, wtf16::Wtf16String};
let raw = RawJson::from_string(r#""ab\ud800""#.to_owned())?;
let query = Wtf16String::from_json(&raw)?;
let payload = t3_protocol::api::orchestration::search_threads_utf16_payload(
    &query, &serde_json::json!({"limit": 10}),
)?;
// Keep the returned RawJson raw when framing a host response as well.
```

C hosts use `t3_client_call_raw` with the same ownership/freeing contract and
8 MiB bound as the existing call. It accepts `rpc` and `searchThreadsUtf16`
requests. Success data and server rejection causes remain raw JSON; the latter
uses `rpc_rejected` plus the exact `cause`. Transport failures retain their
ordinary stable error envelope. The existing Value-based symbol is unchanged.

JNI marshals bytes through `callRaw`; it never converts query code units through
modified UTF-8. Kotlin exposes `executeRaw`, `utf16JsonString` and
`searchThreadsUtf16`. The string helper escapes every Java `Char` before an
ASCII byte conversion, including isolated surrogates. Raw responses remain byte
arrays. Hosts should not feed them through a Unicode-scalar JSON decoder before
forwarding a value to their native JavaScript/UTF-16 UI.

## Validation boundary

New source fixtures cover high/low/reversed/pair units, clipping at 200,
ECMAScript whitespace differences, malformed JSON/UTF-8, actual WebSocket
requests/results/rejection causes, stale request IDs and pings, and the exported
raw ABI. An admitted Node fixture uses actual JSON.parse/JSON.stringify and the
original trim/slice expression, and produces a 240-unit snippet ending at a high
surrogate for C/JNI transport assertions. It is a synthetic transport peer,
not a search scoring or full server acceptance oracle.

Private canonical android-sandbox validation passed strict all-target metadata
and a normal library build, followed by all seven genuine Rust tests, the
exported C ABI caller, and JNI/Kotlin wire assertions. The native JavaScript
peer handled nine requests and retired all nine connections normally with zero
remaining connections. Executables were linked only in container RAM against
the actual emitted library graph; the existing target remained read-only.
The complete seven-test typed command compatibility fixture also passed after
its negative visitedAt case was corrected to reject a non-string value; arbitrary
strings belong to the native server date-validation domain. All Android/JNI/Kotlin compilation and execution must use the
canonical rootless android-sandbox. No host tests, new Cargo target, private
vendor patch, default merge, release or installation is part of this change.

The same source contract audit found an independent typed visit restriction:
`Command::ThreadVisit` previously applied a client RFC 3339 parser to
`visitedAt`. The original IsoDateTime declaration is Schema.String; the native
server's DateTime.make owns validity and canonicalization. The typed command
now passes the exact string unchanged, including date-only, zone-free, expanded
TimeClip dates, surrounding whitespace and invalid/empty strings. The latter
reach the native dispatch error instead of an invented client validation error.
Other typed timestamp validators are unchanged. A real WebSocket fixture checks
those exact payload strings; it does not pretend to be the server date engine.
