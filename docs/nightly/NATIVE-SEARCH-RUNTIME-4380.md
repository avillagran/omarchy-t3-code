# Native search runtime and consumers

The current target is Nightly 4380 at
`ade473018d32003658b7d38693e083ee2e314bd8`. Search and visit source inputs are
byte identical to 4367. This candidate links the original search algorithms to
the existing admitted Electron runtime and wires Rust and QML consumers. The
application executable and combined UI acceptance remain pending; this document
does not claim full search, server-history or global Nightly parity.

## Original algorithms and local process

`tools/build-native-search-runtime.mjs` verifies nine complete upstream source
files and the existing compiler lock, erases TypeScript, and checks every free
runtime identifier. The emitted module contains complete palette scoring, PR
search and thread sorting modules, with original pure declaration closures for
thread subjects, local opens, stable late ordering, palette filtering and
matched-message find navigation. No Unicode, scoring, cache, decay, ordering,
native limit or find-query approximation replaces those original functions.

Current generated module: `ui/search/NativeSearchRuntime.cjs`, SHA256
`de144c42fe1ebaea9f08d9e30a378bbbec54cb34660349193333e3329b66b290`.
The adjacent source receipt records each input and declaration. An earlier
complete module without find navigation (`08a9d39d`) remains preserved in its
generation output and the separate server-history branch.

The metadata worker runs Electron 44.10.1 in Node mode. Its embedded Node is
24.22.1; the build compiler is Node 26.13.0. The complete official 72-file
runtime graph is admitted by the existing application capability. The worker
receives JSON data over owned pipes and does not receive account credentials,
user-supplied code, executable paths or filesystem actions.

`src/native_search.rs` embeds the program, owns the exact child through reaping,
uses bounded packets and a serialized queue, coalesces obsolete pending ranking
requests and preserves open-event order. Native algorithm state persists across
requests. Qt uses request generations to reject stale results. Inner JSON
strings preserve native UTF16 values across Rust's UTF8 JSON representation.
Both pipe directions are nonblocking and share the request deadline, including
large requests when the child's input pipe is full. The pure worker has no
writable profile or temporary directory and creates no per-process files.

The factory selects an owner-configured admitted Electron runtime or the
existing adjacent native-voice capsule. Normal package discovery, installation
and platform coverage still need combined application verification. No new
runtime binary was copied or installed for these component gates.

## Consumer source

The snapshot projects actual thread search inputs, retaining field presence
for older-server fallbacks and compact PR search facts. QML palette ranking
uses the native group algorithm, 20-thread/10-message/3-secondary limits,
native match hints and safe highlighted titles. Recent threads use local open
recency. Message rows preserve their positions when later matches arrive and
carry the original matched-message find query and entry target. Existing
Obsidian results remain a separate final extension group.

Visit-source integration distinguishes local recency, server open deduplication,
explicit palette picks and read watermarks. The typed SDK and atomic server
open-history implementation remain in separate candidates awaiting their own
runtime, publication and exact-pin checks. The primary SDK is not privately
patched to make those fields appear supported.

## Actual component evidence

All paths are under `/var/tmp/o3-build/`. Owners retained the same Podman client,
actual ID and name through terminal state; admitted inputs remained unchanged.
No host account, display or profile was used.

| Gate | Actual result | Qualification |
| --- | --- | --- |
| Source generation 3 | Pass; 6 modules, 6,124 inputs unchanged | Generation only, no application compilation |
| Electron core 2 | Pass; all 34 original callbacks and 192 assertions | Complete original expectations with explicit assertion adapters, not the upstream runner |
| Worker protocol 2 | Pass; 9 scenarios and 16 requests | Real child, ranking, actions-only filter, local open/deduplication, pick persistence, stable late results, accent-preserving find, malformed request recovery and normal reaping |
| Packed UTF16 protocol 3 | Pass; 10 scenarios and 17 requests, actual child reaped | Includes the packed Rust-facing JSON envelope and a lone UTF16 surrogate without data loss |
| Strict Rust and normal library | Pass; all-target metadata and normal library | Actual primary root and SDK source, both freshly compiled; no application executable or test link |
| Compiled Rust owner | Pass; 8 private cases against actual admitted Electron | 1.4 MB request, Unicode and packed UTF16, error recovery, 50 terminal ranking waiters, two ordered opens, normal closure and a stopped child's actual timeout/reaping |
| Qt consumers | Pending combined acceptance | The current executable was not rebuilt or installed |

Receipts:

- `native-search-runtime-build-4367-3-amha21ac/result.json` records the actual emitted module and source receipt hashes.
- `native-search-electron-core-4367-2-n146lx1f/result.json`, SHA256 `2fabf7a2927cda9c622dcf1c11dd873a2acf0f6e43de4d665e47d20f60d4db01`.
- `native-search-protocol-4367-2-slrbg7fc/result.json`, SHA256 `303ec43c44289fe4351732bcbb948a078cdb6b99055b3580f448c96d91d6d4aa`.
- `native-search-protocol-4380-3-ec6j49tw/result.json`, SHA256 `0495ba53477f578b75a10e24156b6f507cf08b86fb3a61571d797c251e1ccd97`.
- `thread-open-4380-source-equivalence-o2j6ptbn/review.json`, SHA256 `828c75c0c29b0a8090fa6f497bc2c12398aa2712b3090c8d3d2a41db93a68799`, compares all search/visit inputs with 4380.
- `primary-native-search-4367-compile-d3_81jq6/output/result.json`, SHA256 `9a0e426496bbf2e1c7b0af1b97eaf147afaa389aa054d87aa7ba5a03c795fca5`. Complete Cargo JSON records the actual 221-row normal graph and 405 admitted outputs, including the main all-target metadata check and freshly compiled primary SDK.
- `native-search-engine-runtime-4380-6kpy7xaw/result.json`, SHA256 `d02b143f613008677ddf813f0850cc74dcdbf7170f90c7f572d7f1d336ded48c`. The private Rust fixture links the actual emitted application rlib and dependency graph, without manually compiling the SDK. All 2,334 inputs remain unchanged; the same actual container and client terminate normally. The deliberately stopped child belongs to that isolated fixture and is killed/reaped only after its real pipe deadline.

`tests/fixtures/native-search-engine.rs` preserves that exact accepted fixture.
It runs only in an isolated container with the admitted `/electron` capsule and
the current complete application graph; it is not a host test command.

## Sidebar candidate, separate from the compiled application

The next generated candidate adds the complete original `searchSidebarThreads`
closure and match-key helper. It uses native title/branch/PR ranking without
project labels or palette picks, followed by message-only rows in stable order.
This candidate is not yet applied to the Qt sidebar or current application rlib.

- `native-sidebar-search-build-4380-2-vjq8g8eq/result.json`, SHA256 `1051ce8b652330594be4aee9112662452f73c44f59a4398e03b39e525e11ed2a`: seven-module generation pass; output SHA256 `3b801ebcec6eba56fdad0a3d2d954447835edd1dcc8976df985ae2a470f7b1f4`.
- `native-sidebar-search-runtime-4380-zxfuvbmg/result.json`, SHA256 `88ef70a0be13c6ea558e7ec2dc240a3ac852b1292ceae2466ef0e08eb85ec526`: 34 original core callbacks / 192 assertions, the complete original sidebar describe block, and 15 worker scenarios / 27 requests pass in actual Electron. Assertion and opaque-ID branding adapters are explicit; the original search expectations remain unchanged.

Generation 1 rejected unrelated re-exports before producing output; generation 2
omits only exports outside the selected declaration closure, retaining the free
identifier audit. The original selected function body is unchanged. Qt sidebar
wiring, partial environment replies, older-settled results and combined UI
acceptance remain required.

Earlier failures are retained: source generation 1 rejected two missing standard
V8 globals in the linkage audit before producing output. Direct QML execution
of the older candidate fails Unicode property expressions and lookbehind; that
candidate remains inactive. The source-pinned V8 implementation replaces its
execution path rather than narrowing queries or skipping original cases.
