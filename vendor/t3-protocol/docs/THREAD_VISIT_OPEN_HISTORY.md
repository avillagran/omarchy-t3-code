# Thread visit and open history

The typed `Command::ThreadVisit` carries optional `opened: Option<bool>` and
`search_query: Option<String>` fields. JSON uses `opened` and `searchQuery`.
Omitting both produces the unchanged legacy watermark-only command. Explicit
null is invalid; an empty search query is valid. Query strings are preserved
and bounded to 200 UTF-16 units, so 100 supplementary-plane emoji fit and 101
do not. The server owns query folding and open-history timestamps.

The source contract originated in nightly 4367. The complete thread.visit
contract, server dispatch, visit tracker, palette selection and original open
history/search source inputs are byte-identical through nightly 4380
(`ade473018d32003658b7d38693e083ee2e314bd8`). The source comparison covers
13 files; its retained receipt SHA-256 is
`828c75c0c29b0a8090fa6f497bc2c12398aa2712b3090c8d3d2a41db93a68799`.

The generic C ABI and generated Kotlin `orchestration.dispatchCommand` wrapper
keep their signatures. No generated describe/Kotlin file changed: the actual
saved-registry test verifies both, and the real JNI fixture exercises legacy,
optional false/true, exact UTF-16 boundary and invalid-null cases. Rust callers
constructing `Command::ThreadVisit` directly supply `None` for both optional
fields when they want the legacy shape.

## Private validation

The validated implementation checkpoint is
`a22c5a5c206bcb0dc4d2a676f030936a810dca66`.

A canonical rootless `android-sandbox` compile passed strict all-target Clippy,
normal Cargo test compilation without execution, a release static library,
and actual C/JNI/Kotlin compilation. Its actual Cargo graphs recorded 71
strict root artifacts, 71 normal root artifacts and one release library.
Unchanged library metadata reuse was admitted against exact earlier emitted
artifacts, source, toolchain and profile hashes; changed integration tests and
normal/release library generation were fresh. No new persistent Cargo target
or executable/runtime copy was created. The compile receipt SHA-256 is
`0471787aa5fcb9be1d7290c3d6b521d1e66c5ba6bc127cdb55a85ae1180415eb`.

A separately reviewed runtime used those exact artifacts in read-only mounts:

- 105 library unit tests, seven typed command tests, one saved binding test,
  one pinned contract test, eight FFI transport tests, four protocol tests and
  three long-RPC tests: 129 passed, none failed or ignored.
- The actual C client exited successfully with its assertions enabled.
- The canonical Kotlin/JNI fixture exited successfully with both its complete
  fixture marker and the new thread.visit marker. The original long-RPC
  deadline and authentication callback assertions were retained.

The runtime completed in 30.294 seconds; its actual client and retained
container identity were terminal, and all source, artifact and dependency
fences passed. Its receipt SHA-256 is
`6c652389929aa46846fae20a32145dbd93fbf3d753e7edd6149fd74545592374`.
Receipts remain in the private build evidence; compiled binaries are not
committed to this repository.

Earlier admissions and strict failures remain recorded. Pre-existing compiler
style findings were corrected without suppressions or changed expectations:
independent guards gained line breaks, equivalent standard expressions were
used, unchanged test modules moved after production items, and WebSocket
fixtures implement the existing fixed callback trait with identical assertion
bodies. These are compatibility fixes rather than open-history behavior.

This validates the SDK boundary. Server persistence, client navigation and
complete nightly 4380 acceptance remain separate. Consumer pins require green
public SDK CI; this work does not merge the default branch or publish a release.
