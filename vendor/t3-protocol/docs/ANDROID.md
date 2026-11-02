# Android / Omi integration

`lib-t3-rust` keeps the Cargo package/import names `t3-protocol` / `t3_protocol`
and the static archive `libt3_protocol.a`. ABI v2 and v3 are additive: the original
four C symbols and synchronous JSON operations remain available to existing adapters.
ABI v3 adds the typed API ops `call` and `describe` (see [Typed API](#typed-api)) and
`domain`, the library's pure functions (see [Domain operations](#domain-operations)).

## Using the Android package

The Android release provides an AAR with the canonical Kotlin adapter and JNI
libraries for **arm64-v8a, armeabi-v7a, x86 and x86_64**, targeting API 24+,
and ot3's string catalog as the asset `lib-t3-rust/i18n.json` (see Domain
operations, Strings). It has no UI, account configuration or background service. Add the verified AAR
to the Android application's dependencies:

```kotlin
implementation(files("libs/lib-t3-rust-android-v0.3.5.aar"))
```

The application supplies its existing INTERNET permission and Kotlin standard
library. The adapter is built with Kotlin 2.3.21 / JVM 17; use a compatible Kotlin
toolchain, or build the provided adapter sources with the application's compiler.
Its JNI class name is kept by the AAR's consumer ProGuard rules.

```kotlin
// All of this runs on a worker / Dispatchers.IO, never the UI thread.
val client = dev.avillagran.t3.T3Client()
client.connectBearer(baseUrl, hostOwnedToken)
val shell = client.request("/api/orchestration/shell")
val stream = client.subscribe("orchestration.subscribeThread", JSONObject()
    .put("threadId", selectedThreadId)
    .put("requestCompletionMarker", true)
    .put("acceptBoundedSnapshot", true)
    .put("acceptCompactTurnItems", true))
// A resume can also set afterSequence from the last applied native event.
val chunks = dev.avillagran.t3.T3Chunks()
try {
    while (isForeground) {
        val batch = stream.poll() // waits up to 1s; wakes at once on items, chunk ends, idle
        for (chunk in chunks.accept(batch)) applyNativeChunk(chunk) // whole server chunks
        if (batch.completionMarker || batch.idle) paintIfChanged() // initial state complete
        if (batch.closed) break // inspect batch.error/errorMessage, reconnect explicitly
    }
} finally {
    stream.close()
    client.close()
}
```

On resume or a `ConnectivityManager` network change, call
`client.wakeSubscriptions()` (or `stream.wake()`): a dead socket then closes the
batch with `stream_timeout` in about 3 s instead of 20 s, and the host subscribes
again from its cursor. Besides `errorMessage`, `T3Batch` carries `boundaries`,
`idle` and `completionMarker`; `T3Chunks` regroups items into whole server chunks.

Omi owns endpoint discovery, login, Android secure storage, lifecycle, cursor
persistence and UI. It connects directly to T3. Omarchy-Link is not a T3 message
relay. A current T3 cloud JWT can instead be supplied to `connectRelay`; a browser
access token bound to a different DPoP key cannot be reused. The C-handle proof
key currently lasts for that handle; Rust hosts can persist keys through secure
platform storage.

Stop subscriptions when the foreground view is released. `close()` cancels native
subscriptions before waiting for in-flight calls and releasing the handle; it is
idempotent and may block, so close from a worker. A screen/environment generation
must fence late results from an older request. No foreground service, background
permission, automatic reconnection or mutation retry is introduced by this adapter.

## Typed API

Prefer the typed API to hand-built `rpc` payloads. The Rust registry validates
arguments before any socket opens, applies each function's deadline, and sends
once. Kotlin sees one generic call and the registry's own description:

```kotlin
// Worker thread / Dispatchers.IO, as for every other call.
val status = client.call("vcs.refreshStatus", JSONObject().put("cwd", repoPath)) as JSONObject
client.call("orchestration.dispatchCommand", JSONObject()
    .put("type", "thread.archive")
    .put("commandId", outbox.commandIdFor(action)) // minted and persisted by the app
    .put("threadId", threadId))
val functions = client.describe().getJSONArray("functions") // offline
```

```json
{"op":"describe"}
{"op":"call","fn":"vcs.refreshStatus","args":{"cwd":"/home/me/repo"},"timeoutMs":20000}
```

`timeoutMs` is optional (1..300000) and overrides the function's deadline. Errors
arrive as `T3Exception(code, reason, requiredScope)`: `code` stays stable
(`unknown_function`, `invalid_input`, `invalid_<field>` such as `invalid_cwd`,
`not_connected`, `rpc_rejected`, `rpc_outcome_unknown`, …), `reason` is the
server's one-line message, and `requiredScope` names the scope a
`write_not_authorized` token lacked. Authorization is decided from T3's
`EnvironmentAuthorizationError` tag, never from the text of a message. The app
decides whether another token is available; the library never chooses one.

Each `describe` entry gives `fn`, `module`, `tag`, `kind`, `replay`, `scope` (or
`"dynamic"` plus `scopes`), `timeoutMs`, `stream` and typed `args`; stream
functions also appear under `streams` with their item shapes (see Typed streams). After
`rpc_outcome_unknown`, `replay` tells the app what is safe: `free` repeat a read,
`id` resend the identical payload with the same `commandId`, `set` repeating is
harmless, `never` read the state back first. The library never retries.

Typed Kotlin wrappers are generated from the saved `describe`
([`bindings/android/describe.json`](../bindings/android/describe.json)). The
generated [`T3Api.kt`](../bindings/android/kotlin/dev/avillagran/t3/T3Api.kt) sits
next to `T3Client.kt` and ships in the AAR (`T3Api(client).vcs.refreshStatus(path)`);
`cargo test --test api_describe` fails when the registry, the saved `describe` and
the wrappers disagree.

The generator maps `integer` to `Long`, `number` to `Double`, `object` and
`command` to `JSONObject`, `array` to `JSONArray` and the rest to `String`; a Kotlin
keyword used as an argument is backquoted (`` `when` `` in the keybinding calls).
Functions with `openArgs` (`dispatchCommand`, `projects.mutate`, `device.action`)
take the whole variant as one `JSONObject`. `cargo test --test api_describe` checks
`T3Api.kt` against `describe.json` byte for byte; `tools/compile-android-kotlin`
compiles it with `T3Client.kt` against `android.jar` (as `tools/package-android`
does) and `tools/test-kotlin-jni` runs it through JNI, which needs a JDK and `kotlinc`.

Unary functions in this release (99; the README's Typed API section has each
one's Rust signature, kind, scope, deadline and arguments; streams follow in
[Typed streams](#typed-streams)). Kotlin method names camelCase the part after
the module, so `provider.auth.start` is `T3Api(client).provider.authStart(…)`:

| Kotlin (`T3Api(client).…`) | Functions |
| --- | --- |
| `server` (9) | `getConfig`, `getSettings`, `updateSettings`, `discoverSourceControl`, `reportClientActivity`, `upsertKeybinding`, `removeKeybinding`, `searchAcpRegistry`, `prepareAcpRegistryAgent` |
| `orchestration` (10) | `dispatchCommand`, `launchThread`, `getThreadProjection`, `getTurnItem`, `getTurnDiff`, `getFullThreadDiff`, `getWorkflowScript`, `stopWorkflow`, `searchThreads`, `getArchivedShellSnapshot` |
| `preview` (9) | `list`, `open`, `navigate`, `resize`, `adjust`, `refresh`, `close`, `clearProfile`, `reportStatus` |
| `assets` (2) | `createUrl`, `delete` |
| `vcs` (7) | `refreshStatus`, `listRefs`, `createRef`, `switchRef`, `pull`, `init`, `preparePullRequestThread` |
| `review` (2) | `getDiffPreview`, `getDiffFileContents` |
| `projects` (4) | `readFile`, `searchEntries`, `mutate`, `createNew` |
| `diagnostics` (8) | `getHostResources`, `getProcessDiagnostics`, `getResourceTelemetryHistory`, `signalProcess`, `checkOtlpEndpoint`, `getUsageSummary`, `refreshUsageRates`, `runStorageCleanup` |
| `provider` (9) | `refreshProviders`, `updateProvider`, `authStart`, `authRespond`, `authCancel`, `authLogout`, `installStart`, `installCancel`, `installRemove` |
| `projectClone` (1) | `start` |
| `filesystem` (2) | `browse`, `getMetadata` |
| `pullRequests` (13) | `list`, `detail`, `checks`, `activity`, `routing`, `routingIdentity`, `reviewerCandidates`, `labelCandidates`, `comment`, `submitReview`, `runAction`, `requestReviewers`, `setLabels` |
| `device` (7) | `list`, `configure`, `testHost`, `open`, `close`, `detail`, `action` |
| `scheduledTasks` (5) | `list`, `upsert`, `setEnabled`, `delete`, `runNow` |
| `agentSessions` (2) | `scan`, `import` |
| `mcpApps` (3) | `callTool`, `readResource`, `updateModelContext` |
| `shell` (1) | `openInEditor` |
| `secrets` (1) | `answerRequest` |
| `worktreeSetup` (1) | `cancel` |
| `terminal` (3) | `open`, `write`, `resize` |

A scope listed as `dynamic` depends on the payload. The rules, so the app can
pick a token before calling: `assets.createUrl` needs `filesystem:read` for
workspace and media files and `orchestration:read` for the rest;
`server.updateSettings` needs `providers:manage` for provider keys or a
`providerInstanceMutation`; `projects.mutate` adds `settings:write` when it sends
`scripts`; `git.preparePullRequestThread` adds `orchestration:operate` for
`mode: "worktree"` with a `threadId`; `device.list` needs `orchestration:operate`
with `updateTool` or `retryHostId` (unless it is `inspectOnly` without
`updateTool`).

## Domain operations

What the app derives from T3's JSON comes from the library too, so Kotlin holds
no protocol rules: `T3Client.domain(fn, args)` runs one pure function by name,
with no connection and no network (it works before `connectBearer` and while
offline), and `T3Api(client).domain.<module>.<function>(…)` wraps each one. Call
them on a worker like everything else: they are CPU work over JSON that can be
a few MiB.

```kotlin
val api = T3Api(client)
// Home: every saved environment's shell in one sidebar.
val environments = JSONArray().apply {
    for (e in saved) put(JSONObject().put("id", e.id).put("name", e.name)
        .put("shell", e.shell /* last shell snapshot or stream state, or null */)
        .put("state", e.connectionState /* null when connected */))
}
var returns: JSONObject? = null
val sidebar = api.domain.sidebar.view(environments, System.currentTimeMillis(),
    filter = "all", query = search, grouped = groupShelves, returns = returns,
    selectedKey = selected, colors = theme.colors) as JSONObject
returns = sidebar.getJSONObject("returns") // keep it for the next call
val active = sidebar.getJSONObject("sections").getJSONArray("active")

// Thread: keep the projection current, then derive the timeline.
projection = (api.domain.projection.apply(projection, events = batch) as JSONObject).getJSONObject("projection")
val view = api.domain.timeline.view(projection, mathRenderingEnabled = settings.math) as JSONObject
val rows = view.getJSONArray("timeline") // each row has "rev": redraw only when it changes
```

```json
{"op":"domain","fn":"markdown.blocks","args":{"markdown":"See [docs](./README.md)"}}
```

Arguments are validated before the function runs, as for `call`:
`T3Exception("unknown_function")`, `"invalid_input"` (an unknown member) or
`"invalid_<field>"` (missing or of the wrong JSON type), then the function's own
codes. `describe().getJSONArray("domain")` lists each function's `args` and a
one-line `returns`; the README's Domain operations table has them all. A
`domain` request may be up to 72 MiB (a bounded thread snapshot and its
envelope); other operations keep 8 MiB.

- **Sidebar.** `sidebar.view` returns the sections (`active`, `working`,
  `snoozed`, `settled`), `sectionCounts`, the header `counts`, the
  `filterCounts`, `projectGroups`, `projectIdentities` (monogram, label, colour
  slot and, given the theme's `colors`, the colour) and `returns`. Rows carry
  `detail`, `state`, `statusLabel` (English source string for the catalog),
  `statusRole` (working, done, failed, warning, input, waiting or muted) and
  `time` (`kind` elapsed, countdown or age, `at` and the `label` for `now`;
  recompute between views with the same rule). Shelf expansion stays app state;
  the lists are always complete.
- **Thread.** `timeline.view` returns `timeline`, `threadState` (working,
  run clock, sort anchors, context window, subagents, merge-back target),
  `activity` (the working pill), `approvals`, `questions`, `tasks`, `queue`,
  `planView`, `historyCursor` and `hasMoreHistory`. Message rows are the user and
  assistant rows of `timeline`; assistant rows and plans carry `blocks`.
- **Markdown blocks.** Each block has a `type`: `paragraph` and `heading`
  (`level`) with `spans`; `list` (`ordered`, `start`, `items[].blocks`); `quote`
  (`blocks`); `code` (`index`, `language`, `code`, `runnable`, `command`);
  `table` (`alignments`, `header`, `rows` of span arrays, `markdown`, `csv`);
  `image` (`id` matching the row's `markdownImages`, `url`, `alt`, `title`);
  `math` (`tex`, `display`); `rule`. Spans are flat: `text` with optional
  `strong`, `emphasis`, `strikethrough` and `link`; `code` (a path in inline code
  links to the file); `break` (`hard`); `image`; `math`. A `link` is `{href, kind}`
  with kind `github`, `web`, `thread`, `file` (plus `fileType`) or `anchor`;
  links ot3 would not follow have no `link` at all.
- **Diff and Git.** Parse `orchestration.getTurnDiff` (and `review.getDiffPreview`)
  results with `diffReview.present`, using `changes.summaries` for the ranges and
  `diffReview.scopedResult` to drop a late answer. The Git & changes sheet is
  `gitControls.scope` (from the shell snapshot), `vcs.refreshStatus`,
  `gitControls.present` and, after the user confirms, `gitControls.buildVerifiedAction`
  with a fresh read: run its `steps` in order (`git.runStackedAction` is a stream,
  `vcs.pull` and `terminal.*` are calls) and never retry them automatically.
- **Branches and pull requests.** `branchPicker.dedupe` over `vcs.listRefs`,
  `selectionTarget` for where to switch and `mismatch` for the composer's notice.
  `pullRequests.request` / `payload` give `{method, payload}` for `call`;
  `pullRequests.sidebarBadge` is the row badge (also in `sidebar.metadata`).
- **Plans, rewind and the inspector.** `planWorkflow.view` (the `planView` of
  `timeline.view`, plans with `blocks`), `followUp` for the banner's actions and
  `attachSourceRef` on the `message.dispatch`. Rewind: `checkpointRevert.canPreview`
  on a user row, `reference` on the raw projection, `safety` for the gate,
  `command` to persist in the outbox before dispatch, `observation` on each
  read-back. The inspector: `toolInspector.request(row.inspector, environment,
  threadId)` → `call(method, payload)` → `toolInspector.parse(…, response)`.
- **Settings and usage.** `nativeSettings.snapshot` / `patch` for an environment;
  for a project scope over several environments, per target
  `scopedSettings.targetView` (after the reads) and `writePatch` (before
  `server.updateSettings`, then `writeWasDropped` on the read-back), and
  `scopedSettings.page` over all of them. Usage: `usageSummary.request(period,
  today, timeZone)` with the device's local date and IANA zone, then `aggregate`;
  the quota strip folds `server.subscribeServerConfig` events with
  `quotaSources.apply`.
- **Minimap and subagents.** `timeline.minimap(rows, visibleRowIds, atEnd)` on
  scroll (debounced) gives the markers and the current one;
  `timeline.minimapIndex` maps a touch on the rail. `subagents.cards(row.subagents,
  now, threadState.subagents)` per subagents row (refresh `now` each second while
  `anyLive`).
- **Theme.** `theme.tokens(colorsToml, id)` returns every ot3 role as an opaque
  colour (`roles`, `statusColors`, `projectPalette`/`projectFills`, `sky`,
  `document`), the same values ot3 draws, so the app needs no blending. The
  `appearance` rule is the sky's (Rec.709 luminance of the background > 0.5).
- **Strings.** `i18n.t(locale, key, args)` is ot3's lookup with its placeholders;
  the key is the English source. Two ways to use the catalog: at run time,
  `i18n.catalog(locale)` once per locale change (pass the app language or the
  device locales joined by `:`) and look strings up in memory, so the app needs
  no generated `strings_ot3.xml`; or at build time, a generator reads
  `assets/lib-t3-rust/i18n.json` from the AAR (the same file, pinned to the lib
  version) instead of an ot3 checkout and keeps writing `strings_ot3.xml`. The
  first one also translates the English sources other functions return
  (`statusLabel`, subagent labels, summary parts) without a name table.

- **Notifications.** `awareness.view` turns the followed environments' shells
  into the ongoing card, its chip, alerts and summaries, with T3's phases and
  lifetimes and ot3's alert key; see [Following threads](#following-threads-notifications-and-live-updates).

Kotlin wrappers under `T3Api(client).domain` (109):

| Module | Functions |
| --- | --- |
| `projection` (1) | `apply` |
| `sidebar` (8) | `status`, `metadata`, `sortTimestamps`, `command`, `threadAction`, `snapshot`, `view`, `detail` |
| `awareness` (1) | `view` |
| `projects` (2) | `identity`, `monogram` |
| `timeline` (4) | `view`, `liveActivity`, `minimap`, `minimapIndex` |
| `activity` (1) | `describe` |
| `changes` (1) | `summaries` |
| `markdown` (2) | `blocks`, `link` |
| `queue` (2) | `derive`, `command` |
| `approvals` (2) | `pending`, `command` |
| `questions` (2) | `pending`, `command` |
| `models` (4) | `catalog`, `preferences`, `favoritePayload`, `runtimeModeCommand` |
| `composer` (4) | `paths`, `catalog`, `fileLink`, `attachmentReference` |
| `history` (2) | `path`, `envelope` |
| `connectionCatalog` (1) | `pairingTarget` |
| `codeBlocks` (1) | `parse` |
| `i18n` (6) | `translate`, `normalize`, `t`, `catalog`, `resolve`, `languages` |
| `diffReview` (2) | `present`, `scopedResult` |
| `toolInspector` (4) | `compactReference`, `scope`, `request`, `parse` |
| `planWorkflow` (5) | `view`, `followUp`, `attachSourceRef`, `implementationTitle`, `mode` |
| `checkpointRevert` (8) | `canPreview`, `reference`, `safety`, `command`, `confirmedCommand`, `observation`, `recoveryMessage`, `appendPrompt` |
| `usageSummary` (3) | `window`, `request`, `aggregate` |
| `quotaSources` (4) | `limits`, `providers`, `sources`, `apply` |
| `pullRequests` (4) | `sidebarBadge`, `request`, `payload`, `threadCommand` |
| `branchPicker` (6) | `dedupe`, `deriveLocal`, `sanitize`, `selectionTarget`, `mismatch`, `temporaryBranch` |
| `gitControls` (7) | `scope`, `resolveScripts`, `actions`, `present`, `scopedResult`, `buildAction`, `buildVerifiedAction` |
| `nativeSettings` (7) | `snapshot`, `patch`, `description`, `modelSelection`, `idleCompaction`, `idleCompactionPatch`, `threadDefaults` |
| `scopedSettings` (10) | `effective`, `projectPatch`, `projectFile`, `validatedTargets`, `writePatch`, `writeWasDropped`, `targetView`, `aggregate`, `page`, `projectCatalogConfig` |
| `subagents` (4) | `buckets`, `describe`, `cards`, `fromThread` |
| `theme` (1) | `tokens` |

### Following threads: notifications and Live Updates

The follow service (a foreground service whose notification is the ongoing
card) keeps no rule of its own: one `awareness.view` call per change gives it
everything it posts. Shapes are in the README's Domain operations; `card` and
`alerts` are what ot3-Android's `NotificationContract` parses.

```kotlin
// On a worker, after every shell change, at nextDeadline and when the screen changes.
fun refresh(now: Long = System.currentTimeMillis()) {
    val environments = JSONArray().apply {
        for (e in followed) put(JSONObject().put("id", e.id).put("name", e.name)
            .put("shell", e.shell /* the supervisor's shell state */)
            .put("state", e.connectionState /* null when connected */))
    }
    val view = api.domain.awareness.view(environments, now,
        baseline = baselineStore.load(),               // null the first time
        onScreen = visibleThread?.let { JSONObject().put("environmentId", it.environmentId).put("threadId", it.threadId) },
        preferences = JSONObject().put("approval", true).put("input", true).put("completion", true).put("failure", true),
        lifecycle = lifecycle, locale = appLocales /* "es-CL:en" */) as JSONObject
    lifecycle = "running"
    postCard(view.optJSONObject("card"))               // null: cancel the card
    postAlerts(view.getJSONArray("alerts"), view.getJSONArray("summaries"))
    baselineStore.save(view.getJSONObject("baseline")) // after posting
    scheduleTick(if (view.isNull("nextDeadline")) null else view.getLong("nextDeadline"))
}
```

1. **Environments.** The same objects as `sidebar.view`, only for environments
   the service follows and is authorised for. While one is not connected, pass
   its last shell with a non-null `state`: its live rows read `stale` (status
   Waiting), it raises nothing and its baseline entry is kept. Leave out a
   revoked or forgotten environment: its rows and baseline entry go, with no
   Done alert.
2. **When.** After each shell change (at the stream's `repaint`), when the
   thread on screen changes, when preferences change and at `nextDeadline`
   (one delayed tick, rescheduled by every call; none when null). No polling.
3. **`lifecycle`.** `cold` on the first call after Follow is switched on (or
   after the user resets notifications): nothing alerts and the baseline is
   rebuilt. `resumed` on the first call after the process restarted with its
   stored baseline, or after Doze or a network loss; `running` otherwise. Both
   use the baseline, so a request that arrived meanwhile alerts and a result
   older than two minutes does not.
4. **Posting.** `card` is the foreground notification; `card.expiresAt` its
   `setTimeoutAfter`; `chip` (only while `active`) its short critical text,
   worded by the app (`working` → Working, `live` → "N live", `approve`,
   `answer`). Post each alert with its `id` as the tag, so posting the same alert
   again replaces it; a finished alert's `expiresAt` is its timeout. Post one
   summary per entry of `summaries`, in the alert group of its `group`.
5. **Baseline.** Store the returned `baseline` (DataStore) after the alerts are
   posted. If the process dies first, the next call with the old baseline returns
   the same alerts with the same ids, which replace rather than repeat. A lib
   upgrade that changes its `version` resets it silently (no alerts once).
6. **Stopping.** When `card` is null or not `active` for the idle period, post
   the last card (it expires by itself) and stop; nothing needs to run until the
   app is opened again.

## Events through the existing JSON ABI

```json
{"op":"capabilities"}
{"op":"subscribe","tag":"orchestration.subscribeThread","payload":{"threadId":"selected","afterSequence":10,"requestCompletionMarker":true,"acceptBoundedSnapshot":true,"acceptCompactTurnItems":true}}
{"op":"pollSubscription","subscriptionId":"returned-id","timeoutMs":1000}
{"op":"cancelSubscription","subscriptionId":"returned-id"}
{"op":"wakeSubscriptions"}
```

`capabilities` reports ABI/library versions and limits without connecting:
`maxSubscriptions` (8), `maxLargeSubscriptions` (2), `queueMaxItems` (64),
`queueMaxBytes`/`itemMaxBytes` (8 MiB), `largeItemMaxBytes` (64 MiB),
`largeQueueMaxBytes` (72 MiB), `maxPollMs` (1000), `probeTimeoutMs` (3000),
`idleTimeoutMs` (20000), `pollFields` and `subscribeBy` (`["tag","fn"]`).
Boundaries, idle, the completion marker, `wakeSubscriptions`, `cursor`, `start`
and subscribing by `fn` are additive, so `abiVersion` stays 3; a host can test
for `"wakeSubscriptions"` in `operations` or `"cursor"` in `pollFields`.
Subscribe returns `subscriptionId`; poll returns `{items, boundaries, idle,
completionMarker, closed, error, errorMessage, cursor, start}` inside the usual
`{ok,data}` envelope. `error` is a stable code; `errorMessage` carries a reason when the
failure had one. Items preserve T3's native snapshot/event/synchronized shapes.

Poll returns as soon as something is new (items, a chunk end, `idle` turning
true, or the end of the stream), otherwise after `timeoutMs`:

| Field | Meaning |
|---|---|
| `items` | Native items in server order, never dropped or reordered. |
| `boundaries` | Strictly ascending end offsets into this batch's `items`: `n` means a server chunk ended after `items[0..n)`. Apply items and repaint at boundaries, so a replay burst never shows half-applied state. Items after the last boundary belong to a chunk that continues in the next poll; `0` means the chunk that ended the previous batch ended right there. |
| `idle` | The last chunk is complete and no new frame completed during one 100 ms read tick: caught up for now. A large frame may still be in transit, and it can be `true` before the first item. Stays `true` until the next item; turning `true` wakes one waiting poll. Without `requestCompletionMarker` it is the only catch-up signal (the same rule ot3 uses); with it, wait for `completionMarker`. |
| `completionMarker` | Only with `"requestCompletionMarker":true` in the payload: `true` from the batch that carries the server's `{"kind":"synchronized"}` item (left in `items` at its position) onward. The initial snapshot or replay is complete; paint it. |
| `closed`, `error`, `errorMessage` | Once `closed` nothing more arrives. `error` is a stable code (`null` after cancel); `errorMessage` an optional one-line reason. |
| `cursor` | Where to resume: the payload's `afterSequence`, advanced over every item polled so far by the library's rule (a snapshot sets it, any other item only moves it on; thread events this build cannot decode count too). Subscribe again with it as `afterSequence`. `null` until known; for `terminal.attach` and `subscribeWorktreeSetup` it only orders items. |
| `start` | What the first polled item meant: `snapshot` (replace the held state), `resume` (continue from `afterSequence`) or `events`; `null` before the first item or for a tag the typed API does not know. |

```json
{"items":[{"kind":"snapshot","snapshotSequence":41,"projection":{}},{"kind":"synchronized"},{"kind":"event","sequence":42,"event":{}}],
 "boundaries":[2],"idle":false,"completionMarker":true,"closed":false,"error":null,"errorMessage":null}
```

Here the first chunk (snapshot and marker) is complete and can be painted; the
event belongs to a chunk whose end arrives in a later poll. One consumer polls each stream. Poll waits on a condition
variable rather than repeatedly calling Java; it does not hold the connection
lock and cannot stall sends or reads. At most eight streams exist per handle
(shell, open thread, command output, server config, preview, terminal, devices…),
each on its own WebSocket because T3 serves one request per socket. Each queue
holds at most 64 items and 8 MiB of serialized payload. The one exception is
`orchestration.subscribeThread` with `acceptBoundedSnapshot:true`: its socket
accepts 64 MiB frames, so one item may be a 64 MiB snapshot and its queue holds
72 MiB (that snapshot plus 8 MiB of events). At most two of the eight streams may
use that mode, so a handle never queues more than 2 × 72 + 6 × 8 = 192 MiB of
serialized JSON (in practice far less: each poll drains its queue). A ninth
stream, or a third bounded one, fails with `subscription_limit`. Without the
flag a larger frame is refused by the socket and the stream ends with `disconnected`.
Overflow ends the stream with `stream_backpressure`, preserving already queued events. Resume
from the last applied cursor or obtain a fresh snapshot; never silently drop gaps.
Close a finished subscription to release its slot. Disconnect or a successful
connection switch cancels all streams; a failed switch preserves the old session.

The worker handles Effect Chunk/Ack, Ping/Pong and Interrupt framing in Rust.
It pings every 5 s and closes a stream with `stream_timeout` after 20 s without
any frame, so a suspended device or a changed network surfaces quickly.

To check sooner, call wake when the app returns to the foreground or
`ConnectivityManager` reports a network change:

```json
{"op":"wakeSubscriptions"}
{"op":"wakeSubscriptions","subscriptionId":"returned-id"}
```

It returns `{"probed":n}` (open streams probed; an unknown id fails with
`unknown_subscription`). Each probed stream pings at once, on its worker's next
100 ms read tick, and ends with `stream_timeout` unless a frame arrives within 3 s
of the call: a dead socket surfaces in about 3 s (3.0 to 3.1 s measured), a live
one keeps streaming. The library never reconnects; on `stream_timeout` the host
cancels the subscription and subscribes again from its cursor, with its own backoff.
Cancellation wakes a blocked poll immediately; the network worker observes it on
its next read tick. DNS/connect/TLS negotiation already in progress can take its
normal timeout before the worker exits. No callbacks into the JVM occur from the
stream worker. Server cursors, projections and retry/backoff remain host-owned.

## Typed streams

Open streams by typed function, not by raw tag: the arguments are validated in
Rust before any socket, exactly like `call`, and the library computes the resume
cursor. `describe` lists the 13 stream functions (`"stream": true`) and, under
`streams`, how to read each one: `itemKey` and `items`, `starts`, `cursor`
(`afterSequence` resumes; `sequence` only orders), the items that end it and the
composed `firstItem` read, if any.

```json
{"op":"subscribe","fn":"orchestration.subscribeThread","args":{"threadId":"t-1","afterSequence":41,"acceptBoundedSnapshot":true,"acceptCompactTurnItems":true}}
{"op":"subscribe","fn":"orchestration.subscribeShell","args":{"afterSequence":120}}
{"op":"call","fn":"provider.installState","args":{"instanceId":"codex"}}
```

`subscribe` with `fn` fails locally with `unknown_function`, `not_a_stream` or
`invalid_<field>` (`invalid_thread_id`, `invalid_after_sequence`, …); mixing
`fn` with `tag` or `payload` is `invalid_input`. `call` on a stream function is
`stream_function`. Streams read for their current state only have a unary form
through `call`, with no queue and no poll: `provider.installState`,
`provider.authState`, `diagnostics.storageCleanupReport` and
`preview.discoveredServers`.

In Kotlin, `T3Client.subscribeFn(fn, args)` returns the usual `T3Subscription`
(it cannot overload `subscribe(tag, payload)`, which has the same JVM signature),
`T3Batch` carries `cursor` and `start`, and the generated `T3Api` wrappers return
subscriptions for stream functions (`T3Api(client).orchestration.subscribeThread(threadId, …)`).
`ThreadStream` reads a thread or shell stream as snapshots, events and a cursor:

```kotlin
// Worker thread. `held` is the app's own copy of the projection, keyed by thread.
val stream = ThreadStream.thread(client, threadId, afterSequence = held.cursor,
    completionMarker = serverSupportsMarker)
while (active) {
    val update = stream.poll()
    update.snapshot?.let { held.replace(it) }        // whole state: drop what was held
    update.events.forEach { held.apply(it) }          // in order, after that snapshot
    if (update.repaint) render(held)                  // chunk ended or caught up
    held.cursor = update.cursor                       // the library's resume rule
    if (update.closed) break                          // error says why
}
stream.close()
```

### Recommended Android flow

1. On connect, open the shell stream (`ThreadStream.shell(client, afterSequence = shellCursor)`)
   and keep it while the environment is in the foreground. Paint the sidebar from
   its snapshot and `project.*`/`thread.*` items.
2. When a thread opens, read its bounded window once (`boundedThread`), then open
   `ThreadStream.thread(client, id, afterSequence = window.snapshotSequence)`
   with `boundedSnapshot = true`. Close it when the thread view goes away; at most
   two bounded thread streams fit per handle, eight streams in total.
3. Persist `update.cursor` (or `stream.cursor`) per stream as you apply updates. On
   a closed update with a transient error (`stream_timeout`, `disconnected`,
   `rpc_connection_failed`), close the stream and, after the app's backoff, open a
   new one with `afterSequence` = that cursor. `start == "snapshot"` means the
   server sent a full state instead of resuming: replace, do not merge. On
   `stream_closed` or a rejection, start without a cursor.
4. On resume from background and on every `ConnectivityManager` network change,
   call `client.wakeSubscriptions()`. Live sockets keep streaming; a dead one
   closes with `stream_timeout` within about 3 s, and step 3 reopens it.
5. Treat the view as current after `completionMarker` when the server supports
   the marker, otherwise after `idle`. Repaint only on `repaint`, never in the
   middle of a chunk.

The library never reconnects, never retries and never stores a cursor; those
stay with the app, so a background process restart resumes from what it persisted.

## Long-running RPCs

```json
{"op":"rpcWithTimeout","tag":"device.configure","payload":{"enabled":true},"timeoutMs":120000}
```

Same request/exit as `rpc`, with an explicit deadline of 1 ms to 300 s
(`timeoutMs`, validated before connecting; otherwise `invalid_rpc_timeout`).
It is never retried: a deadline interrupts the request and returns
`rpc_outcome_unknown`, which the host reconciles. Failures keep `error` a stable
code and may add `message` with the server's one-line reason. Use it through
`T3Client.execute(JSONObject)` from a worker.

## Attachments

`T3Client.upload(fd, name, mimeType, sizeBytes)` accepts a borrowed readable file
descriptor. Open Android content with the resolver on a worker and keep its
ParcelFileDescriptor valid until JNI has entered the call. JNI owns a CLOEXEC
duplicate; Rust streams through T3's `attachments.createUploadUrl` grant and
signed same-origin POST. The duplicate is released on success and every error
path. There is no full-file byte-array copy. A nonresponsive provider read has a
bounded native deadline. Limits follow the core: 50 MiB/file, 10 MiB/image; the
host must also honor any stricter current provider/server policy.

Every adapter failure with a stable code is a `T3Exception`, including the ones the
JNI layer raises before Rust runs: `host_file_unavailable` (a descriptor it cannot
duplicate, e.g. `-1` or one already closed), `invalid_input` (a request over 8 MiB),
`native_error` and `native_allocation_failed`. `IllegalStateException` remains only
for misuse of the Kotlin objects (`client_closed`, `subscription_closed`).

`T3Client.image(id, name, mimeType)` returns validated image bytes through
`assets.createUrl` and its signed same-origin GET, with a 10 MiB bound. Kotlin
owns the returned byte array and supplies decoding/cache/UI. Images do not pass
through Base64 JSON. No bearer credential is sent to signed asset requests.

For other native hosts, `include/t3_protocol.h` exposes reader/release and image
callbacks. Upload transfers context ownership only when both callbacks exist;
release runs exactly once when Rust drops the reader, including invalid input.
Reader callbacks must be thread-safe and bounded. Image bytes are borrowed only
for the synchronous callback duration. Callbacks must not throw/unwind. Existing
JNI adapters can keep their four original C symbols and use the new JSON stream
operations; attachment callbacks require adapting their JNI marshaling.

## Transport options, pairing and assets

All of this is additive to ABI v2; `capabilities` lists it in `operations`,
`binaryOperations`, `connectionOptions` and `maxAssetBytes`.

```json
{"op":"connectBearer","httpBaseUrl":"https://environment.example","token":"stored-token","options":{"requestTimeoutMs":8000,"connectTimeoutMs":3000,"poolIdleTimeoutMs":90000,"maxBytes":8388608}}
{"op":"fetchDescriptor","httpBaseUrl":"https://environment.example"}
{"op":"pairBearer","httpBaseUrl":"https://environment.example","pairingCode":"from-the-link","environmentId":"optional-expected-id","client":{"label":"Omi","deviceType":"mobile","os":"android"},"connect":true}
{"op":"boundedThread","threadId":"selected"}
{"op":"requestOk","method":"POST","path":"/api/auth/clients/revoke","body":{"sessionId":"s-1"},"protocol":false}
```

`options` is optional on `connectBearer` and `connectRelay`; in Kotlin it is the
last parameter of both (`client.connectBearer(baseUrl, token, JSONObject().put("requestTimeoutMs", 8000))`),
and `connectRelay(relayUrl, cloudJwt, environmentId, clientId = "t3-mobile", options = null)`
lets the host name its relay client. Unknown keys,
non-numeric values, a zero timeout, a timeout over 300 s or `maxBytes` outside
1 byte to 64 MiB fail with `invalid_connection_options`; `null` clears the
connect or pool-idle timeout. Options govern HTTP. Streams and RPC keep their
own deadlines (see above).

`fetchDescriptor` and `pairBearer` need no connection and send no credential
until the descriptor is checked. `pairBearer` returns `{accessToken,
tokenType, environmentId, descriptor, scope, expiresIn, connected}`. Store
`accessToken` in the Android Keystore-backed store and clear the response buffer.
With `"connect":true` the handle switches to the new session after the exchange
succeeds, cancelling its streams; a failed pairing leaves the old one in place.
`boundedThread` returns the restored bounded snapshot (up to 64 MiB).

Signed assets use the binary path, never Base64 JSON. From Kotlin:

```kotlin
val clip = client.assetBytes(resource, maxBytes = 50L * 1024 * 1024, timeoutMs = 120_000)
// clip.status = {sizeBytes, mimeType}; clip.bytes is owned by the caller
val icon = client.projectFavicon(cwd = "/repo", path = "favicon.svg") // path is optional
if (icon.found) showIcon(icon.bytes!!, icon.mimeType)
```

`T3Asset` carries `status` (the JSON data), `bytes` (null when `found` is false),
`found` and `mimeType`. `maxBytes` defaults to `T3Client.MAX_ASSET_BYTES` (64 MiB).
Both wrappers pass this metadata to the JNI `image` native (`t3_client_image`), which
forwards it unchanged:

```json
{"op":"assetBytes","resource":{"_tag":"attachment","attachmentId":"a","fileName":"clip.mp4","mimeType":"video/mp4","disposition":"inline"},"maxBytes":52428800,"timeoutMs":120000}
{"op":"projectFavicon","cwd":"/repo","path":"favicon.svg"}
```

The status is `{sizeBytes, mimeType}`, or `{found:false, sizeBytes:0}` with no
bytes for a project without a favicon. Errors are `asset_*`,
`invalid_asset_url`, `invalid_asset_limit` and `invalid_asset_timeout`; through
`t3_client_call` these two operations fail with `binary_operation`. In Kotlin they
throw `T3Exception` with that code, like every other operation.

## Building from source

On an x86_64 Linux build host, explicitly install Rust's four Android targets and
the application's Android NDK. The build helper does not install or download SDKs
or targets. Omi's checked build uses NDK 28.2.13676358 and API 24:

```sh
rustup target add aarch64-linux-android x86_64-linux-android armv7-linux-androideabi i686-linux-android
ANDROID_NDK_HOME="$ANDROID_HOME/ndk/28.2.13676358" tools/build-android arm64-v8a x86_64 armeabi-v7a x86
```

Set `CARGO_NET_OFFLINE=true` with an already populated Cargo cache or vendor
configuration for offline builds. ELF libraries use 16 KiB maximum page alignment.
Outputs live under `dist/android/<abi>/libt3_jni.so`. Desktop Linux libraries are
not Android libraries, even when the CPU matches.

`tools/package-android` compiles the Kotlin adapter (`T3Client.kt` and `T3Api.kt`,
through `tools/compile-android-kotlin`) and assembles an AAR. It needs
`JAVA_HOME`, `ANDROID_JAR`, `KOTLIN_STDLIB` and `KOTLIN_COMPILER_CLASSPATH` pointing
to explicitly installed/cached tools. Run it in the clean, committed release
checkout after building all four ABIs. It produces the versioned AAR and SHA-256
checksum in `dist/`; it does not sign or publish an Android application.

### Releases

Pushing a `v*` tag runs [`.github/workflows/android-release.yml`](../.github/workflows/android-release.yml)
on `ubuntu-24.04`: it checks that the tag matches the `Cargo.toml` version, installs
the four Rust Android targets, NDK 28.2.13676358 and the `android-36` platform
through the runner's `sdkmanager`, JDK 21 and the Kotlin 2.3.21 compiler (the
compiler and org.json downloads are pinned by SHA-256 in
[`.github/actions/android-toolchain`](../.github/actions/android-toolchain/action.yml)),
runs `tools/build-android arm64-v8a x86_64 armeabi-v7a x86` and
`tools/package-android`, verifies the checksum and publishes
`lib-t3-rust-android-v<version>.aar` and its `.sha256` as a GitHub release. A manual
run (`workflow_dispatch`), or a pull request that touches the Android bindings or
the release tools, builds and packages the same way and only uploads both files as a
workflow artifact, which is how the workflow is checked before a tag.

Every push and pull request also runs the `kotlin` job of
[`test.yml`](../.github/workflows/test.yml): `tools/compile-android-kotlin` against
`android.jar`, then `tools/test-jni` and `tools/test-kotlin-jni` on x86_64, so no
Kotlin change is tagged without compiling.

## Validation

- `cargo test --locked`: local HTTP/WebSocket fixtures cover exact event delivery,
  long-poll concurrency, cancellation/Interrupt, queue bounds, attachment grants,
  transport options and proxy bypass, signed asset bytes, descriptor/pairing and
  bounded snapshots, through the Rust API and the JSON/binary ABI.
- `tools/test-c-abi`: a real C caller links the static Rust archive.
- `JAVA_HOME=... tools/test-jni`: real JNI/JVM calls, Unicode, error paths and ownership.
- `tools/test-kotlin-jni`: canonical Kotlin adapter, typed errors, `call`/`describe`
  and `domain` (sidebar, timeline, Markdown blocks, project chips, queue) through
  the generated wrappers, and concurrent close against a real native
  request; `StreamFixture` drives streams through a JDK-only fake WebSocket (chunk
  boundaries, completion marker, idle, wake probe and error reasons). Supply the cached compiler variables above
  and `JSON_JAR` for the host's org.json implementation; CI runs it in the `kotlin` job.
- Android cross-build/link checks cover all four ABIs. These are not Android
  device/UI/lifecycle E2E checks, which remain consumer work.

The upstream reference and inspected source paths are recorded in
[UPSTREAM.md](UPSTREAM.md). Provider calls and real conversations are not used by
these fixture tests.

Terminal context, output, read-only observation and reconnect bindings: [TERMINAL.md](TERMINAL.md).
