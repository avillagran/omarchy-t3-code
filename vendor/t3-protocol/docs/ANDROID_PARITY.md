# Android/Desktop shared contracts

Checked 2026-10-31 against lib `origin/main` bf245ee, Android request/status
sources at 9ef8deb, Desktop sources at 55c0ce3. ABI remains **3**; all operations
and domain functions below are additive. C symbols and existing request shapes
are unchanged. `describe` and generated `T3Api.kt` are updated together.

| Request | Baseline and result |
| --- | --- |
| U18 | `history.path/envelope` existed. Added authenticated `historyPage` and `history.mergeView`. |
| U26 | Sanitizers existed. Added `quotaSources.present`, preserving bounded unavailable messages. |
| U29 | Catalog/runtime mutation existed. Added `models.permissionView` with Desktop's empty-advertisement fallback. |
| U31 | List/detail/checks/activity existed. Added `pullRequests.listView` and authenticated bounded `pullRequestDiff`. |
| U32 | Typed launch/workspace strategies and `nativeSettings.threadDefaults` existed. Added `composer.newThread` for search, defaults, missing choices and explicit launch recipes. **Automatic routing remains blocked** as described below. |
| U33 | Path/catalog/reference inserts existed. Added `composer.transform` for UTF-16 completion, checked replacement, recall and list continuation. |
| U34 | Validated catalog URL existed. Added cancellable credential-free `registryIconBytes` over the existing binary ABI. |
| U35 | Scoped `documents::resource`/document read existed in Rust. Added `reader.reference`, `reader.threadReference` and bounded text-only `document` JSON operation/Kotlin binding. Media uses `assetBytes`. |
| U36 | Typed `orchestration.subscribeCommandOutput` already existed, including replace frames. Added `commandOutput.apply`; use the existing cancellable subscription, without polling invented endpoints. |
| Project metadata | `projects::command` and typed `projects.mutate` existed. Added `projects.mutation` to expose validated metadata commands with caller-owned command IDs. |
| Pin order | Added `sidebar.pinPlan`, ported from Desktop `ui/js/PinOrder.js`. Use `sidebar.pinCommands` with persisted command IDs to obtain validated `thread.pin.reorder` commands. |
| Provider administration | Safe projection, atomic mutation and read-back confirmation already existed in Rust. Added domain bindings `providerInstances.projection/mutation/confirmTarget`. |
| Default inheritance | `nativeSettings.threadDefaults` and scoped settings builders already existed and were already exposed. Reused unchanged. |
| Reader actions | Added `commentary`/`showMessageActions` to timeline rows with Desktop terminal-assistant semantics. |
| P19 | Refreshed all 20 locales from Desktop; retained old source keys for compatibility: 2,027 sources per locale (2,004 Desktop plus 23 retained sources). |

## Consumers

`T3Client.historyPage(threadId, cursor)` fetches and expands the opaque page.
Convert it using `history.envelope`, then render that projection using
`timeline.view`. Pass its `timeline` as `page` to `history.mergeView`, together
with `older` and current `recent` presentation rows. Live thread state, approvals,
queue, runs, permissions and stream sequence remain exclusively in the live
projection. `owner` is an opaque host/thread/connection-generation token; echo
it as `pageOwner` together with the requested `pageCursor`. Mismatches return
`applied:false` without advancing the cursor. Page rows replace matching older
IDs, recent rows always win, and rows sort by creation time then ID. Pass
`nextCursor`/`hasMoreHistory` from the page; exhaustion clears the cursor.
Reading anchors and cancellation belong to the host.

`pullRequests.listView({pages:[{items:[...]}]})` uses the supplied ordered page
sequence. A refresh supplies a fresh sequence, not a concatenation of stale
cursor chains. Later occurrences replace earlier rows by environment, normalized
host/repository and number; output sorts by updatedAt descending (invalid dates
are zero, ties deterministic). `T3Client.pullRequestDiff(reference,cursor)` uses
Desktop's existing authenticated `POST /api/pull-requests/diff`, capped at 8 MiB.
No RPC tag for diff is invented. Preserve the server's opaque response/cursor.

`quotaSources.present({environments:[{id,name,connected,error?,refreshFailed?,state}],now})`
accepts `quotaSources.apply` state. It sanitizes observations, deduplicates by
provider/email or credential fingerprint, disambiguates known workspaces,
retains isolated identities when ambiguous, and chooses the freshest reading.
Windows provide available percent, elapsed expectation, pace, aligned nullable
segments and reset translation `{key,args}`. Session pools account for exhausted
weekly limits. Hosts render notices and external links from this safe projection.
This is read-only; credit redemption is not exposed.

`composer.transform({input})` actions: `completion` (`text,cursor`), `replace`
(`text,start,cursor,expectedRange,insert`), `list` (`text,cursor`), and `recall`
(`messages`). All offsets use UTF-16 units and reject split surrogate pairs.
`expectedText` optionally checks the full draft; replacements always check the
range. Hosts own IME, selection locks, undo grouping and scratch history state.
`composing:true` returns `changed:false`. The helper does not send messages.

`reader.reference` receives `href,environmentId,threadId,environments`, with
explicit `{id,connected,threads}` environment entries. Thread references use
`t3-thread://v1/<encoded id>`, prefer the selected connected environment and
return unavailable when no connected matching thread is supplied. File paths
are percent-decoded, numeric line/column suffixes removed, and scoped to the
caller-supplied remote thread. HOME is never expanded on the client. Web schemes
are case-insensitive; unsafe schemes are unsupported and malformed escapes fail.
`T3Client.document(resource)` returns UTF-8 contents/mimeType capped at 4 MiB;
images/audio/video must use the existing binary asset operation. Consumers reject
stale results when owner/connection changes.

`registryIconBytes(url,requestId)` accepts only the catalog's official HTTPS CDN
namespace, disables redirects and proxies, sends no bearer/cookies, validates SVG
MIME/UTF-8 and caps the response at 256 KiB with a 5 s deadline. Use an opaque,
unique in-flight request ID; `cancelRegistryIcon(requestId)` signals cancellation.
Cancellation is checked around network waits and every body chunk; a blocking
network wait can take up to the deadline. Render SVG as an untrusted image with
scripting/external resources disabled. The bundled mark remains the fallback.

`commandOutput.apply({reference,state?,frames,closed?,error?})` consumes existing
`{kind:append|replace,text,running?,truncated?}` frames. Include environment and
connection generation in `reference` beside required `threadId,itemId`. Different
references reset state; frames explicitly naming another reference are ignored.
Pass returned opaque `state` on subsequent calls. ANSI CSI/OSC parsing and CR/LF
state survive chunk boundaries; replace resets them. Text is a UTF-8-safe 64 KiB
tail, with running/truncation/error presentation. Cancel the existing subscription
on Stop, inspector dismissal or owner change. No additional stdout/stderr/progress
wire families exist in the pinned T3 contract; richer channels require a host
contract change rather than a Kotlin parser.

## U32 host gap

Desktop `T3Backend.beginNewThread` performs automatic host selection through the
Desktop host operation `connection-route`; this is not a T3 RPC in the pinned
contract. `composer.newThread({input})` therefore reports `missing:["host_route"]`
for `automatic:true` and never fabricates a route. The required host change is an
authenticated route-admission operation that returns an authorized environment,
project and route receipt from current capabilities/preferences/load, or a host
adapter that supplies that verified decision before calling the helper.

The helper filters authorized projects by title/path and resolves source,
project/settings, sibling, available catalog and draft model defaults. It uses
existing runtime inheritance, honors explicit workspace strategies, and exposes
`workspace_confirmation` or `base_ref` when choices are missing. With `build:true`
and caller-persisted `commandId,threadId,title`, it returns validated
`orchestration.launchThread` arguments (including optional `initialMessage`).
The same IDs and exact payload must be retained after an unknown outcome.
UI confirmation, authorization of input snapshots and durable storage stay in the
consumer. Automatic routing is not claimed complete by this release candidate.

## Validation and release

Fixtures are in `tests/android_parity.rs`, `tests/fixtures/android-quota.json`,
`tests/ffi_transport.rs`, registry HTTP unit fixtures and Kotlin JNI fixtures.
Run the full commands documented in README/ANDROID: cargo tests, C ABI, JNI,
Android Kotlin compilation and Kotlin JNI. `cargo clippy --locked --all-targets`
is also run; the repository has existing warnings. Global rustfmt currently has
pre-existing differences; new modules are rustfmt-clean.

No tag/release or consumer pin changes are part of this PR. After review/merge,
choose and bump the next version, regenerate describe/Kotlin, validate native
x86_64/aarch64 and four Android ABIs, then create the matching version tag under
separate release authorization. The Android release workflow builds/packages
`arm64-v8a x86_64 armeabi-v7a x86`, publishes AAR plus SHA-256, and uses Kotlin
2.3.21/NDK 28.2.13676358/android-36. Android must update its pinned AAR/checksum;
Desktop must update its pinned Rust revision. PR CI can produce an unpublished
AAR artifact for integration testing before release.
