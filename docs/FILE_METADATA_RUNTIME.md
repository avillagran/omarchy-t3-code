# File metadata runtime integration

The source boundary is T3 Code nightly 4288, commit `32a0531748c44b25072d7495ce0b9bd7e9450a87`.

The metadata cache is scoped to the actual admitted SDK RPC socket. Credentials, route names, display modes and UI counters are not socket identities. Mounted thread leases keep positive entries through virtualized-row unmounts and popup visibility changes; misses retain the native 60-second lifetime. Requests collect at most 64 logical queries per RPC batch and deduplicate their wire paths within that batch.

Generic metadata requires `filesystem:read`. The terminal hover and click consumer additionally requires `orchestration:operate`; opening an editor checks its separate operate authorization. Admission uses the native query/path composition before any metadata request, including the UNC/network-path refusal and the 512 UTF-16-unit limit.

A mounted conversation observes actual checkpoint IDs, statuses and file arrays. Changed or removed checkpoints invalidate the union of their old and new resolved paths. Initial observation, missing data, thread changes and cwd changes establish a baseline. Checkpoint status never seeds a positive metadata result. Actual workspace file reads and path searches seed only their admitted socket, guarded against intervening invalidation.

The terminal uses its actual surface request ID as a cancellation owner and the mounted conversation's lease ID as its retention owner. Destroying a surface cancels its pending logical waiters. A shared batch is interrupted only after all its waiters have gone. Invalidation revisions, actual socket closure and surface/thread ownership fence stale click receipts. A failed query is not a cached negative result.

## Evidence and activation boundary

`generate-file-metadata-admission.mjs` executes the complete pinned native query helper and path functions: 720 query, terminal-composition and workspace observations, including newline suffixes, UTF-16 limits and large numeric positions. `generate-file-metadata-provider.mjs` executes the whole native provider with injected React/Atom/command services: 19 checkpoint traces. Its Qt wrapper retains the original effect callback. `generate-file-metadata-wire.mjs` executes the complete native filesystem/base schemas with Effect 4.1.2: 30 decoding observations, including safe-integer bounds, JavaScript whitespace/BOM trimming and optional null rejection.

The focused Rust admission target compares the native fixtures and verifies the real normalized conversation output. The private Qt fixture executes the generated helpers, applies the actual structural delta helper, compiles the central panel type, and exercises real Loader mount, checkpoint update, cwd reset, destruction and remount. The terminal fixture uses the real pane and viewport for pending-click, invalidation, scope, destroyed-surface and replacement-socket fences.

The active transport uses published shared SDK `88411c6ef894ae8463f1ee1423b06f8d0e81b2f7`, including the unchanged scoped `RpcCancellation` API. The focused metadata evidence below was recorded on published2979a94; its RPC/cache interfaces are unchanged by the later device-frame/settings delta. Reviewed private HTTP/persistent-RPC tests exercise 130 logical queries, the 64-query wire bound, actual socket reuse, retained positives and expiring misses, full unmount/remount, shared-waiter and shared-batch cancellation, selective invalidation retry, and read-seeding races. They also call the actual workspace document, composer search and terminal/editor dispatch consumers, checking that seeded reads avoid redundant stats and that denied metadata or editor actions do not fall back to another credential.

The focused transport partition contains seven Rust tests; the terminal consumer test additionally verifies actual document/search seeding and release dispatch. Actual socket-owner regressions remain part of the partition. Synthetic peers require the native protocol-negotiation header for shell reads. Qt tests also fence a completed old-socket receipt and a previous mounted lease on the same still-live surface.

These are source, codec and physical consumer proofs for the wired metadata paths. They do not certify every native UI caller, arbitrary platform TLS stacks, or the entire nightly feature set.

All runtime evidence uses rootless Podman with private HOME/tmp, read-only source/dependencies and no external network. Fixtures retain their directories until container disposal; they do not delete trees or traverse fixture symlinks. Qt import aliases mount only the already documented read-only module/dependency paths. The client-wide runtime audit marker remains in force.

JavaScript lone-surrogate strings cannot cross Rust's UTF-8 JSON boundary. Valid surrogate pairs are included in the native admission corpus. This boundary is not treated as evidence that Rust can represent isolated UTF-16 surrogates.

The seeding adapter also validates the complete native `ProjectReadFileResult`, `ProjectSearchEntriesResult` and `ProjectListEntriesResult` before accepting any positive seed. A second whole-module oracle imports `project.ts` and eight transitive contract modules, producing 40 result-codec observations with real Effect 4.1.2. This preserves trimmed paths, required truncation flags, optional existence/ignored fields and safe integer sizes; invalid successful replies cannot create cache entries.
