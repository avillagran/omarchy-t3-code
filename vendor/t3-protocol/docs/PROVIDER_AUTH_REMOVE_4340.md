# Provider auth-method removal, nightly 4340

This targeted extension implements `provider.auth.removeMethod` from upstream revision `a9e709f15f825cacac1b6d665fa1920c328d68b7`, introduced by `1b952135a1`. The existing `t3code@3f9448747` contract fixture retains its 203 methods. The additional method has separate whole-source provenance in `tests/fixtures/provider-auth-remove-method-4340-source.json`; it does not claim that the whole SDK is verified against 4340.

The operation requires `providers:manage`, returns `ProviderAuthState`, and preserves typed `ProviderSetupError` / `EnvironmentAuthorizationError` responses. It is an unrepeatable mutation: the library never replays it after a rejection or uncertain transport outcome. Actual eligibility and storage belong to the remote controller.

For Rust-representable strings, `AuthRemoveMethodInput` implements JavaScript trim, the 64-character ASCII provider slug, and a nonblank method ID of at most 128 UTF-16 code units. Internal NUL and newline are accepted. Unknown members are stripped. The public typed helper, registry payload, JSON ABI and Kotlin wrappers all reach this decoder. Registry argument types are descriptive metadata; `Args::checked` does not apply a separate Token validator.

Reconnect/start uses the same auth-method decoder. An absent `methodId` keeps the provider default; a present null, non-string, blank or excessive ID returns an error instead of silently selecting that default. A selected `chatgpt-profile:` plus 50 emoji is valid despite exceeding 128 UTF-8 bytes. Other flow and interaction Token validators remain unchanged.

JavaScript strings can also contain isolated UTF-16 surrogates. Generic Rust `serde_json::Value` ingress rejects those before the method decoder, and Rust `str` cannot represent them. The whole original 23-case result retains both isolated-surrogate inputs and outcomes unchanged. A separate 21-case scalar bridge names exactly those two exclusions and the original result SHA256; it does not insert invalid scalar strings into the Rust fixture. They document a compatibility gap, not whole-string equivalence or a private transport workaround.

The describe generator emits a raw `JSONObject` overload and a typed `(instanceId, methodId)` Kotlin overload for this operation. Both wrappers delegate to the same Rust decoder. No generated Kotlin or describe file has been privately edited.

The provider-instance projection also preserves only an actually present Codex `setupMode` value of `managed` or `existing`. This hidden form field is nonsensitive controller metadata. Missing or invalid values stay absent; no other hidden fields or inferred managed defaults are introduced. The client can therefore request native auth metadata only for an explicitly configured managed instance.

## Completed focused validation (2026-11-08)

The native input oracle passed in a private rootless container with networking disabled. It decoded all 23 original removal cases and seven start cases without loading a controller or account store. The scalar bridge includes 21 removal cases and explicitly excludes the two native-accepted isolated-surrogate strings that generic Rust JSON cannot represent. The original output SHA256 is `8830abbdaaaaa498aae510082c098e512e9e45864bebdc69b5a7d7951e1de540`; the adopted bridge SHA256 is `0f428533e870e11c221daf8c554076ab8be668c134a2157da2d4bf8676041426`. Its retained receipt is `/var/tmp/o3-build/auth-method-4340-oracle-1l5osfaq/result.json`.

The canonical repository `tools/android-sandbox` sequence passed using existing image `sha256:61e9757cf3a806b48805abfd21b740019248b0d0a30138d34cf55905207ad749`, networking disabled, two Cargo jobs, incremental compilation disabled and the existing private target. Nine Rust tests passed: one canonical generator test, five removal/reconnect tests, one JSON ABI test, one metadata projection test and one contract test. The current release static library, current C++ JNI adapter and actual Kotlin fixture also completed; the fixture emitted `KOTLIN_JNI_PASS`. This is a focused check, not a full repository suite or four-ABI Android release build.

The durable unit `o3-auth-method-sdk-validation-20261108b.service` exited successfully after 111 seconds. The owner retained the exact container ID and confirmed `podman container exists` returned 1 after automatic removal, with a successful final query finding no surviving matching container. Receipts remain under `/var/tmp/o3-build/auth-method-4340-sdk-plan2-864bf4x3`:

| Receipt | SHA256 |
| --- | --- |
| `result.json` | `0d69366ea38ab3ff1efb61311badab832c8223d19a1eaea22330638859fdaf68` |
| `container-terminal.json` | `0d092a2b9d8866015bbf7b2ce1ca338a5adc522c1b6187b93eb39446529ac7ab` |
| `runtime.log` | `4ad4afc98d50eb872d7df140cb078702942f16739bb83fa35f9dbd28b37e64c0` |

The generator changed exactly `bindings/android/describe.json` (SHA256 `69e5255546412d362459d935eeff8cd5cec96a1630b62be285528545b14d5c24`) and `bindings/android/kotlin/dev/avillagran/t3/T3Api.kt` (SHA256 `99a454b11d7eb0ab01cacd8194d6bc0a4474dea88bac055b735cbc973d927807`). All 260 admitted SDK file paths remained present; all other source and dependency hashes were unchanged during validation. This documentation update follows that proof and does not alter executable sources.

The first retained attempt, `/var/tmp/o3-build/auth-method-4340-sdk-plan-ckc72xz7`, failed offline dependency resolution before compiling or generating anything: its private registry lacked `tokio-tungstenite` and `unicode-normalization`. The corrected reviewed attempt mounted only the existing complete registry read-only, admitted all 191 locked package archives and extracted sources, and kept Cargo home bookkeeping in private RAM. It made no registry copies, downloads or new persistent caches. Neither attempt ran host tests or a host JVM.

The generated describe field `methodId: token` is descriptive metadata, not a generic Token validation rule or a byte limit. Consumers must defer auth method validation to the canonical decoder: JavaScript trim, nonblank and at most 128 UTF-16 code units, including allowed internal NUL/newline. Both Kotlin overloads, registry dispatch and JSON ABI were checked against this distinction. Flow and interaction IDs retain their existing stricter validators.

No live provider, credential, managed registration/session store, server or client acceptance, whole-nightly compatibility, public release, dependency pin or vendoring is established by these receipts.

## Checkpoint and publication gate

This feature is stacked directly on public SDK commit `ecfdfa8b6e92baa5def50b6b73d782bf014d971f`. Keep its source checkpoint separate from a version release. Before consumer pinning or vendoring, publish the reviewed SDK commit and require the existing native Linux x86-64/ARM, C ABI/JNI and Kotlin CI jobs to pass through their canonical sandbox. The Android release workflow must also produce and verify its four-ABI AAR as a nonpublishing workflow artifact before a release tag.

The repository distributes Git tags and Android GitHub releases, not crates.io packages. `Cargo.toml` currently declares 0.3.7, while remote tags already include `v0.3.8`; do not reuse either version. A subsequent reviewed version change must choose an unused version (currently `0.3.9` is the next candidate), update all version-bearing release inputs consistently, and require the `v*` tag to match `Cargo.toml`. No version bump, tag or publication is part of this checkpoint. Only after successful SDK CI and publication may consumers update their pin or vendor snapshot.

## Native lifecycle boundary

The original managed ChatGPT controller permits removal only when the saved registration exists, its subject property is absent, and no saved session uses that client ID. Under its locks, it rewrites only the registration catalog; it neither deletes saved sessions nor stops active sessions. Removing the last-selected registration selects the first remaining registration or null. The auth flow must be idle, reloads actual methods, and publishes an idle state after success. None of these remote controller internals are reimplemented by this transport helper.
