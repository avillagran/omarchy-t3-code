# Global browser profile reporting

Source-only implementation based on client commit `546f939` and T3 nightly 4314
`83a3674ffd84ffb33de768210c806cb1ac53fd80`. No build, runtime, transport, or
integration acceptance is claimed by this checkpoint.

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

## Pending validation and qualifications

No new test was executed. Required coordinated checks are:

1. Whole-native/helper differential identity traces, including equal snapshots,
   provider-only pushes, equal versus changed settings, target drop/reappearance,
   failed report, and explicit equal-content profile-array replacement.
2. Actual singleton QML hydration and hidden/multi-environment reporting, unrelated
   preference snapshots, worker loss, stale socket-close events, and one initial
   attempt rather than a second attempt caused by socket-open notification.
3. Private real HTTP/WS authority and persistent reporter lifetime: at least two
   environments, one denied scope, one absent capability, changed config/settings,
   report rejection without retry, server disconnect and fresh socket admission,
   route/auth replacement, stale queued input refusal, and retirement on removal.
4. Strict compilation, shutdown/cancellation, and bounded mailbox/coalescing checks.

The Rust projection mirrors native JSON config event identity but has not yet
been compared with the whole native Effect projection, especially decoded
Duration equality. Generic lone-surrogate JSON ingress remains a separately
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
Only the Python source transformation and static diff checks were executed; no
JavaScript or Qt runtime result is claimed by this correction.
