# Physical RPC session ownership

`rpc_sessions::Owners` provides opt-in per-environment/read-or-write owners for
the published SDK's persistent unary RPC socket. Its random SDK socket ID is the
cache boundary. Private route/auth admission comparison determines when a socket
must be replaced; it never becomes a public identity or credential-derived epoch.

Catalog rebuilds reconcile only changed or removed admissions. Opening handshakes
serialize within their own environment/access mode and recheck current admission
before publishing. Retiring a pending handshake cannot resurrect its previous
socket. Unrelated environments keep their owners. Generic old stream failures do
not close a newer healthy unary owner: physical transport failures arrive through
the SDK's non-owning `closed()` observer with the exact socket ID.

The watch process forwards `rpc-session-state` events with environment, access
mode, opaque session ID and open/closed state. Consumers must ignore delayed
closed events for an older socket. The owner never retries an uncertain request.
An explicit later read may obtain a replacement socket with a new identity.

The isolated owner test exercises actual bearer WebSockets, route/auth replacement,
retirement during a held handshake, unrelated-environment preservation, physical
failure/replacement, exact close identity and removal from the environment catalog.

This foundation is opt-in. At the exact nightly 4288 source pin, terminal path
activation uses file metadata and native link validation. Terminal and
file-reference/markdown caching and invalidation must follow their actual consumer
lifecycles. Unary ownership does not yet multiplex native subscriptions.

## Native metadata lifecycle oracle

The actual nightly 4288 `fileMetadata.test.ts` and production
`fileMetadata.ts`/query runtime/connection modules pass all 12 native tests in a
private container. The staging tool reads commit
`32a0531748c44b25072d7495ce0b9bd7e9450a87` and records a hash for each of its 737
source/test files; it never copies an older working tree. The original tests use
their own fake RPC/session/Clock boundaries. The cache algorithm is unmodified.

Prepare a dedicated dependency cache from `tests/native-file-metadata-deps` using
`npm ci --ignore-scripts --no-audit --no-fund` inside a private rootless container,
with a private HOME and no host authentication/profile mounts. The lock records
Effect/@effect-vitest 4.1.2, Vitest 5.0.5 and native @noble/hashes 1.8.0. Dependency
acquisition may access the registry; actual tests always have `--network=none`.

Run `python3 tools/test-native-file-metadata-container T3_CHECKOUT DEPENDENCY_CACHE`
through a transient user unit. `O3_NATIVE_ORACLE_ROOT` selects its private evidence
location. Source and dependencies mount read-only; module aliases and Vitest
caches live in container `/tmp`. This verifies the native reference behavior;
it does not by itself establish the O3 cache or pending-validation port.

The isolated O3 `file_metadata_cache` state has 17 targeted cases corresponding
to the native cache scenarios plus retention-reader ownership, global-budget,
partial-seeding and stale-mount guards. It uses actual socket IDs, a 1024-entry
global insertion-order cache, positive mounted-thread retention, 60-second misses
and selective in-flight invalidation. Position-like suffixes remain literal cache
keys. Thread-retained entries are separate from the global budget, as in native
T3. This state module is not yet connected to mounted consumer leases, query
memoization, missing-entry refresh timers or terminal activation validation.

## Shared SDK compatibility

The exact SDK pin `394b4343a28cc8f62f856a7a4c3cb4c8527de4f6` preserves the
shared reader contracts from `7cbad5d` alongside persistent sockets and reset
credit observations. Its Rustdoc ledger preserves all 1,128 legacy public
structural entries; the SDK runtime suite passes 414 tests across 61 targets.
The original Kotlin positional/JVM API and all ABI 3 operations remain intact.

Client dispatch uses the explicit `CommandWithContext` extension so legacy Rust
`Command` and `MessageDispatch` shapes remain unchanged. Optional fresh-context
flags preserve true, false and omission in direct wire dispatch, including repeat
requests and opaque nested context key ordering. Stored accepted commands remain
immutable; this SDK upgrade does not rewrite existing outbox entries.
