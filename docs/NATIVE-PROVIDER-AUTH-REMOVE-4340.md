# Remote ChatGPT connection removal, nightly 4340

The client consumes actual native `ProviderAuthState.methods` and their optional boolean `canRemove` flags. The source reference is upstream `a9e709f15f825cacac1b6d665fa1920c328d68b7`, native change `1b952135a1`; the whole picker, setup section and contract hashes are recorded beside `NativeProviderAuth.js`.

For an actual native-managed Codex state (`credentialOwner: t3`), provider settings offer the account picker. Selection follows the original order: keep an explicitly chosen different-account option, otherwise retain a still-present profile, otherwise select the first profile, otherwise use a different account. Closing the popup retains selection; changing the environment or instance clears it. Profile names and errors render as plain text.

Removal is visible only for the selected method's actual `canRemove: true`, and requires current provider management permission with no pending auth operation. It sends that exact instance and method once, keeps the picker open, renders failures inline, and uses returned method metadata after success. It never signs in, cancels a flow, signs out, deletes a provider instance, or infers removability from labels or authentication status. Native unsupported CLI controllers do not acquire fake saved connections.

The Rust consumer decodes through the public SDK helper, refreshes `/api/auth/session` immediately before mutation, requires authenticated `providers:manage`, and then sends the unrepeatable RPC. The server remains the final fresh authorization and eligibility boundary.

Reconnect preserves an actually selected method using JavaScript trim and the 128 UTF-16-unit bound, including internal controls. A present invalid value fails before opening a socket; only an absent method permits the provider default. Existing flow/interaction validators are unchanged. Isolated UTF-16 surrogates remain a separate generic Rust JSON-ingress limitation, recorded by the original-schema oracle; the scalar input cases do not claim full JavaScript string equivalence.

## Published SDK and exact vendor snapshot

The consumer now pins public SDK commit `64e322be2a2796fd8175ab13102098a34a636bd3`, reviewed in [SDK draft PR #40](https://github.com/avillagran/lib-t3-rust/pull/40). Its same-head PR native CI passed Linux x86-64 and ARM tests, C ABI/JNI and Kotlin checks; the Android PR workflow built all four ABIs, verified the AAR checksum and uploaded a workflow artifact without publishing a release. The duplicate push Kotlin job failed toolchain acquisition with HTTP 504 before executing tests; the same-head PR Kotlin job passed. No rerun, version bump, release tag or release publication was needed for this Git source pin.

All 134 vendor files match the public committed Git blobs. A reviewed owner applied four individually admitted vendor leaves plus `Cargo.toml` and `UPSTREAM.json`, with exact preimages, directory-relative `O_NOFOLLOW` access and atomic leaf replacement. It deleted no paths, extracted no archive, copied no runtime or cache, and preserved the 14 pre-existing client WIP files and `Cargo.lock`. The historical recursive `tools/sync-vendor` was not used. This checkpoint is stacked on public client commit `ba6324bbcb1f4a5505ef344f5d0eb3338f4022a3`.

## Focused proof and its limits

Exact retained receipt paths, hashes and scope are recorded in [provider-auth-remove-4340-proof.json](provider-auth-remove-4340-proof.json).

The strict consumer container passed offline metadata resolution and `cargo clippy --locked --offline --all-targets -- -D warnings`. Its 1,695-path snapshot predates the later QML fixture, stale-owner handler and documentation updates; executable Rust sources have not changed since that check. It did not run Rust tests or link a new final client binary. Existing target state was reused with networking disabled, two Cargo jobs, incremental compilation disabled, read-only source/dependency mounts and private RAM bookkeeping. The CLI, GUI, watch fixture, library test and classifier probe executables retained their admitted identities and hashes.

The actual Qt settings and picker were exercised through QtTest mouse clicks in Quickshell on an owned private headless Sway compositor, using the real project controls, style and imports with a synthetic metadata peer. The sequence retained all assertions for plain remote labels/errors, explicit `canRemove`, selection retention, pending/error/success, suppressed clicks after permission loss or during an active flow, exact UTF-16-valid reconnect, instance/environment replacement and delayed stale metadata. The inherited backend singleton had no runnable client/watch/dictation binaries or credentials in the container; it established no real backend session.

The retained attempts show the boundary of this proof:

- UI1 failed because the fixture searched popup descendants outside `contentItem`; the scope was corrected without changing product validation.
- UI2 exposed a product stale-owner bug: replacing an instance did not close and clear the previous picker.
- UI3 reached every control assertion's PASS marker but failed the unchanged binding-loop rejection rule after a synchronous derived-property reset.
- UI4 passed cleanly after deferring owner clearing with `Qt.callLater` and checking that the captured owner is still current and absent. It emitted all five required compositor, metadata, controls and terminal markers with no rejected JavaScript or binding-loop errors. The container was confirmed removed and all source/dependency/reserved executable fences matched afterward.

No real provider RPC, account removal, credential, managed registration/session store or server controller ran. The two Rust consumer permission/WebSocket tests subsequently passed in a separate private container as described below. Generic Rust JSON still cannot represent isolated UTF-16 surrogate strings. These results establish neither whole-nightly parity nor primary-client integration.

## Actual Rust consumer transport proof

RPC3 executed the unchanged `tests/provider_auth_remove.rs` source against the actual compiled consumer library and public SDK vendor. Both cases passed: `removal_refreshes_authorization_and_sends_the_exact_selected_method_once` and `reconnect_forwards_the_present_native_method_and_never_defaults_invalid_input` (two passed, zero failed or ignored). The tests use private loopback HTTP permission snapshots and WebSocket replies. They check a fresh management permission before one exact removal, suppress the denied request, preserve valid selected reconnect IDs, and reject present invalid values without silently choosing the default.

Cargo built only the library with `--lib --locked --offline`; the owner selected the exact root, serde_json and tungstenite rlibs from its emitted artifact graph. Direct `rustc --test` linked a distinct probe in the existing target, avoiding Cargo's implicit integration-test binary build. The admitted probe SHA256 is `f39bc5c1fc2a6b79bd03dda9a3038ec7a7285e812be5814c6ed79f1c8dfd7085` (158,510,040 bytes). The five reserved CLI, GUI, watch, library-test and classifier-probe executables retained their identities and hashes; all 1,696 source paths and admitted dependencies were unchanged during this proof. The container was confirmed removed after successful completion.

The two earlier attempts remain failures: RPC1 completed the library build but rejected the root rlib location before compiling the test; its original static intended-stage fields are explicitly corrected by a separate `actual-reached-stages.json` receipt. RPC2 selected the graph correctly but failed linking because the image lacked an unversioned SQLite development symlink; no test ran and its empty reserved output remains retained. RPC3 admitted the already packaged image SQLite file by hash, created only a private RAM symlink and added its native search directory. No download, host library mount, manifest change, source workaround or persistent dependency-cache mutation was used.

The accepted RPC3 receipt is `/var/tmp/o3-build/auth-method-4340-client-rpc3-plan-lgk4jzkn/rpc-result.json`, SHA256 `9aa673f41e3d8697dcd702ea5a406c828c6a7fb7a0adf34438c3f4452ba677ca`. All attempt, runtime and terminal receipt hashes remain in the scoped proof JSON. These synthetic peers do not establish native controller eligibility, real-account removal, persisted managed-store behavior or end-to-end server acceptance.

## Remaining acceptance boundary

Validate real server authorization/controller eligibility and persistent managed-store behavior separately. Public consumer CI and primary checkout integration remain subsequent work; whole-nightly compatibility is not claimed. Keep all prior receipts and source fences intact; a failed run must not be relabeled as acceptance or retried unchanged.
