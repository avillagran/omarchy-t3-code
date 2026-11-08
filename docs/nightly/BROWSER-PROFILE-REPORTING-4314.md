# Global browser profile reporting

Implementation based on client commit `546f939`, merged onto public client
`d899fee3b9916bdcc4a74ca443fe5f2831022e7c`, and T3 nightly 4314
`83a3674ffd84ffb33de768210c806cb1ac53fd80`. Focused checks below validate native
identity behavior, the actual singleton, and actual client transport separately.
A composed GUI-to-real-worker-to-server-catalog test remains pending.

## Native source boundary

- `apps/web/src/browser/BrowserProfileReporter.ts`: whole `takeDueProfileReports`
  and `sameReportInput` helpers copied with TypeScript annotations removed;
  upstream MIT notice retained in `NativeBrowserProfileReporting.js`.
- `packages/client-runtime/src/state/serverConfigProjection.ts`:
  `applyServerConfigProjection` and `reuseUnchanged` define config identity.
- `packages/client-runtime/src/state/presentation.ts`: `withoutProviders` excludes
  provider pushes from report identity.
- `apps/web/src/hooks/useSettings.ts`: hydrated settings and shallow patch
  replacement preserve unchanged profile-array identity.
- `packages/contracts/src/preview.ts`: `PreviewReportProfilesInput` sends the raw
  user profile list, default ID, and recording frame rate, without resolved
  built-ins or filesystem discovery.

The singleton reports all configured, authenticated `preview:operate` environments
whose actual server config advertises `environment.capabilities.serverBrowser`.
It does not depend on a selected thread, browser panel, or visible window. Initial
placeholder settings do not hydrate reporting. Successful decoded settings reads
and successful settings updates do. Repeated serialized sidebar snapshots retain
unchanged profile-array identity; an explicit successful profile-array replacement
uses a fresh reference, including equal content.

## Socket and request ownership

The config watcher admits the existing shared **write** `RpcSession`. Its opaque
physical socket ID accompanies the safe config identity. Report transport uses
`rpc_sessions::existing`, never reconnects an obsolete request, rechecks the
current private admission and fresh authenticated scope, and sends
`preview.reportProfiles` on that exact persistent socket. Its server-side reporter
therefore survives the unary result and retires with that connection. No credential
or raw server config is sent to QML.

The native helper records due inputs before submission. Report errors are
silently retained as attempted; they do not schedule retries. Actual config
changes, target removal/reappearance, settings changes, or physical session
replacement can produce a new attempt. An equal config snapshot or provider-only
push does not. Failed config subscriptions retain the last identity while its
actual write socket remains current. Reconnection uses the existing bounded
stream backoff, independently of report outcomes.

Each environment has one latest-report mailbox and one in-flight unary request;
queued intermediate updates coalesce. Safe target notices also coalesce per
environment with one pending wakeup. Obsolete watcher generations cannot publish
into a replacement. Removed environments cancel both workers; already sent unary
requests retain their existing request lifetime. The unary wait currently follows
the SDK's existing 20-second one-shot response budget, without replay.

## Focused validation and remaining boundary

- `browser-profile-report-producer-check-1/helper/result.json`: private pinned
  Node whole-native helper/consumer PASS, 11 identity trace steps and 10 consumer
  assertions, source unchanged.
- `browser-profile-report-producer-check-1/projection-native/result.json`:
  private actual whole original projection/presentation PASS, 20 reference steps
  with Effect 4.1.2, source unchanged.
- `browser-profile-report-producer-check-2/result.json`: corrected all-target
  strict Clippy and library/integration no-run links PASS, source unchanged.
- `browser-profile-report-producer-check-2/runtime-result.json`: three library
  tests PASS, including all 20 original reference steps; one actual-client
  HTTP/WS transport test PASS; zero ignored. The global build lock remained held
  while original binaries were mounted read-only, with unchanged hashes before
  and after runtime and no executable copies.
- `/var/tmp/o3-device-panel-njvpuqf4/result.json` and
  `profile-source-fence.json`: actual production singleton QML PASS, nine reports
  across two hidden/unselected environments. Covers hydration, raw user list,
  cloned snapshots, actual settings responses, stale close, changed config,
  fresh socket, scope withdrawal/reappearance, and equal-content explicit array
  replacement. Its peer is metadata-only.

The old first strict/no-run receipt predates the own-undefined correction and
is retained as build-only evidence of that earlier source. The second receipt
covers the corrected production code. No broad suite was run. These checks do
not claim full ServerConfig schema validation or every Effect value type.

A full composed actual singleton/worker/server-catalog lifetime proof remains
required before claiming the complete route. Server catalog tests, browser
execution, and public selector activation are independent boundaries. The
transport fixture's reporter counter proves actual client socket lifetime;
it does not execute the server catalog implementation.

Generic lone-surrogate JSON ingress remains a separately
qualified client/server limitation. Existing browser preference decoding accepts
more than the native report's 24-profile limit; the producer preserves that actual
list and lets the native report schema reject it rather than silently truncating
or fabricating profiles. No public selector, SDK, or service configuration changes
are included.

## Source correction before runtime

Review found a retained TypeScript local annotation (`const due: EnvironmentId[]`)
in the first source-only helper extraction. It was invalid JavaScript and the
earlier source freeze is not syntax or runtime acceptance. The corrected helper
is generated by `tools/generate-browser-profile-report-helper.py`, which pins the
complete original source hash and requires all three known annotation erasures,
including the local array declaration. The prepared private Node comparison also
asserts that this annotation is absent before evaluating the imported helper.
At that initial correction, only the Python source transformation and static
diff checks had been executed. The later helper and QML receipts listed above
validate the corrected import and consumer separately.

## Config identity correction and original reference

Review found a second source-only mismatch: an empty themes/sources update adds
an own property whose value is `undefined` in JavaScript. That differs from an
absent property, and the next wire snapshot removes it again. The initial Rust
projection removed the property immediately and missed both identity changes.
Internal carry bits now preserve that distinction without adding fields to the
wire or GUI. JSON number equality also follows JavaScript Number semantics, so
equivalent integer/float encodings retain identity.

`tools/test-browser-profile-config-projection.mjs` ran privately against the whole
original `serverConfigProjection.ts` and complete `withoutProviders` function,
with actual Effect 4.1.2 Option/Equal and `Schema.DurationFromMillis` decoding.
All 20 reference steps passed, including the four original own-undefined
counterexamples, repeated empty updates, simultaneous undefined fields, provider
exclusion, fresh equal Duration objects, equivalent integer/float wire values,
changed Duration, and repeated keybinding arrays. The JSON reference is at
`tests/fixtures/browser-profile-config-projection-4314.json`. These are pure
projection-value cases, not full ServerConfig schema validation. The registered Rust comparison has
subsequently passed in the focused second-check runtime receipt above. The old strict/no-run receipt is retained separately and does not
cover this correction.

The passing actual QML singleton coverage uses a fixed metadata-only peer, never
a real browser/account. The HTTP/WS integration case executes the real client
authority checks, Control, and persistent RpcSession against a fixed synthetic
peer, including a token-exact header assertion; its reporter counter proves
client socket lifetime, not execution of the independent server catalog model.
No broad runtime suite or public activation follows from either fixture.
