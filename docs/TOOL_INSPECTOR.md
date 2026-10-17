# On-demand native tool/item inspector

Parent integration is now present in lib/write/main, activity projection, backend/panel, qmldir and QML runner. `tool-inspector` and actual panel-click `activity-inspector` checks pass serially. Historical handoff text below describes the original module boundary.

## Delivered scope and evidence

The five files in this change are standalone modules/fixtures. They do **not** edit the parent's shared `lib.rs`, `main.rs`, `write.rs`, activity projection, `T3Backend.qml`, `T3Panel.qml`, `qmldir`, QML runner, catalogs or parity audit. Parent integration is required before a live activity row can open the inspector.

Reference verified locally: `/home/avillagran/Work/t3code-nightly`, commit `5a4e9980533705774dc3f45b35cacf76c3d78dbc`.

Native source contracts:

- `packages/contracts/src/orchestrationV2.ts:1407–1425`: native item identities, status and timestamps. `runId` and `providerTurnId` are nullable, **different** identifiers.
- `:1528–1537`: `command_execution.input: string`, optional `output: string`, `outputOmitted`, `outputIndicatesFailure`, `exitCode`.
- `:1658–1667`: `dynamic_tool.toolName: string | null`, unknown `input`, optional unknown `output`; not a fictional `{ stdout, stderr }` wrapper.
- `:1701–1707`: projected row `{position, visibility, sourceThreadId, sourceItemId, item}`. Inherited rows must fetch their **source** thread/item, not the viewer's thread.
- `:3273,3656–3667,3792–3795`: **Effect WebSocket RPC** `orchestration.getTurnItem`; payload `{threadId, itemId, revision?}`, result `{item: OrchestrationV2TurnItem | null}`. No HTTP output endpoint or `runId` request parameter is invented.
- `apps/server/src/orchestration-v2/ThreadManagementService.ts:1152–1155`: the persisted item goes through `projectTurnItemForDetail`.
- `apps/server/src/orchestration-v2/WireProjection.ts:14–16,193–220`: on-demand input/output are bounded to 256 KiB per native value; image bytes are omitted. Native transport truncation text is preserved, not presented as complete output.
- `packages/client-runtime/src/work-log/itemDetail.ts:126–150,153–208`: native live revision rules, legacy Bash stdout/stderr recognition and MCP text/resource block vocabulary.
- `apps/web/src/components/chat/V2ItemInspector.tsx:116–149,162–200,240–375`: native lazy read and missing/loading/error treatment; foreground input, muted output, reasoning and tool metadata.

Verification performed: `cargo test --release --offline --test tool_inspector` — **9 passed**, including a real loopback WebSocket Effect RPC fixture, source-vs-viewer identity, response mismatch, legacy command output, MCP/structured values, reasoning, UTF-8 bounds, and pinned native source-contract assertions. No live environment, account or user payload was requested. The pinned-source assertion checks the local contract fragments when that checkout exists; this is **not** execution of Effect's TypeScript schema decoder (Bun/native dependencies are not installed here).

QML runtime fixture is authored but **not executed by this sub-agent**, as requested; parent owns serialized QML execution. Static `qmllint --json -` parsing found no syntax/property-override/duplicate-binding diagnostics after fixing the inherited `Item.data` name collision. It still reports import/unqualified diagnostics without Quickshell's `qs.*` import context and fixture staging. This is not a clean runtime/visual verification claim.

## Rust module API

`src/tool_inspector.rs` provides:

- `compact_reference(native_row) -> Result<Value>`: payload-free reference, accepting a native projected row or a direct native item. A projected wrapper missing its explicit source IDs fails closed. Call **before** grouping activity rows.
- `scope(input) -> Result<Value>`: normalized *field selection*, not identifier normalization. No input identifier is rewritten.
- `request(input) -> Result<(&'static str, Value)>`: exact native method/payload, independently testable.
- `execute(&Environment, input) -> Result<Value>`: read-only RPC using the existing environment connection (native relay/proof routes included). It checks environment identity and connection errors, and uses the existing **read** credential fallback, never a new registry.
- `parse(input, native_result) -> Result<Value>`: checks envelope, source thread/item, type, nullable run/turn identities and essential item fields. It is a targeted inspector adapter, not a duplicate full native schema implementation. A changed `updatedAt` is allowed: native revision is a cache key, not an optimistic concurrency token.

Input (local IPC, **not** the native payload):

```json
{
  "environment": "selected-environment",
  "threadId": "viewer-thread",
  "sourceThreadId": "actual-source-thread",
  "sourceItemId": "native-item-id",
  "type": "command_execution",
  "runId": "source-run-id",
  "providerTurnId": "source-provider-turn-id",
  "revision": "2026-10-17T12:00:01.000Z"
}
```

Both `runId` and `providerTurnId` must be present, with `null` for genuinely unassigned identities. `revision` is `"live"` for idle/pending/running/waiting, otherwise the native `updatedAt` ISO string. Never substitute the provider turn ID for `runId`, `ordinal` for item ID, or a grouped `work/...` presentation ID for `sourceItemId`.

Success contains the exact local `scope`, `available`, bounded `title`/`status`, failure/exit-code state, `sections: [{key,label,text,truncated}]`, `metadata`, `metadataTruncated`, `raw`, `rawTruncated`, `emptyOutput`, `outputOmitted`. Missing persisted item returns `available:false` and no retained item contents. Raw is the native **item**, not an invented input/output schema.

Limits: each section/metadata text is capped at **64 KiB UTF-8**, raw at **128 KiB UTF-8**, title at 256 bytes, status at 128 bytes. Structured JSON serialization stops at its budget rather than allocating an unbounded pretty string. Truncation is explicit, including when a JSON prefix is no longer valid standalone JSON. JSON IPC escaping can enlarge the encoded envelope; the existing protocol bounds incoming frames to 8 MiB. Clipboard copies only the displayed bounded text. No disk cache, raw body logs, polling, automatic retries or snapshot hydration is added.

## Exact parent integration checklist

1. Export `pub mod tool_inspector;` in `src/lib.rs`. Add a **local** `"tool-inspector" => crate::tool_inspector::execute(&e,input)` read arm in `write::execute` after its existing environment resolution. This local operation name is not a new native protocol method. Preserve existing connection selection.
2. Keep this read on the existing asynchronous read/action path, not the mutation/outbox path and not the UI thread. Emit its `{op,requestId,result,error}` action envelope for **both** success and failure. Bound outstanding work (one visible inspector request; no worker per timeline row). Reject or discard work when the panel is hidden; never replay a detail read as a mutation.
3. In `conversation_timeline`, immediately after `protocol::activity::describe(row)` and **before grouping**, attach `activity["inspector"] = tool_inspector::compact_reference(row)?` (handle invalid references locally by leaving that activity non-inspectable, not failing the whole timeline). Current `protocol::activity::describe` retains only `id,type,category,title,command,runId,status,active`; it loses **sourceThreadId, sourceItemId, providerTurnId, revision**. The helper adds exactly those four fields plus `type,runId`, with **no input/output/reasoning**.
4. Retain the reference in **every** `activity.entries[]` child. A group header should select one actual entry or reveal the entry list, never synthesize an item reference from the group's presentation ID. Single-entry activity must also be clickable. For inherited timeline content use the native projected `visibleTurnItems` rows when appropriate; do not derive their source from the currently selected thread. Existing direct `turnItems` are also accepted by the helper.
5. Register `T3ToolInspector 1.0 T3ToolInspector.qml` in `ui/qml/qmldir`. Put the component inside the existing panel popup or an inline activity disclosure. Set an explicit size/anchors (its implicit size is 640×440 scaled units). Supply `bridge`, `environment` and **viewer** `threadId`. Bridge supplies existing `copyText`, `copyFinished` and `scrollPolicy`; no additional clipboard service is needed.
6. On explicit row activation call `inspector.openItem(entry.inspector)`. Handle its `requested(token,scope)` signal by `bridge.send(Object.assign({op:"tool-inspector",requestId:token},scope))`; never replace the token with a different UUID unless mapping it back. Forward the action event into `inspector.accept(event.requestId,event.result,event.error || "")`. The component rejects stale tokens, mismatched scopes, old item results, old viewer/environment results, late errors, closed/reopened results, and double refresh while busy. Tokens include an instance nonce to avoid recreation collisions. **Do not** overwrite `detail` directly.
7. Call `dismiss(true)` when the hosting popup/panel closes. The component automatically dismisses when `environment` or viewer `threadId` changes; selection bindings must actually update those properties. `closed()` tells the parent to close its containing popup. Hiding the popup without dismissing is insufficient. Discard late backend events too; dismissal cannot cancel an already-sent native RPC.
8. Construction, transcript loading and tab switching must cause **zero** reads. Explicit open, Retry/refresh make one read. While an inspector remains open on a live item, parent may reopen/refetch **once** when its compact revision changes from `live` to the completed timestamp (native's cache behavior); never on every streaming timestamp. No automatic refresh loop is implemented inside this module.
9. Add `tool-inspector` to `tools/test-qml`'s allowed cases, then run `tools/test-qml tool-inspector` **serially**, with its existing isolated HOME/XDG staging. It exercises lazy dispatch, scope/turn identity, plaintext safety, tabs/no refetch, copy acknowledgement, unavailable/error/retry, stale success/failure and dismissal. Do not launch it alongside another focus-sharing QML suite. Existing singleton i18n/controls use the runner's isolation; no live backend bridge is injected in the fixture.
10. Parent owns translations and audit updates. `CHAT_PARITY.md` row 26 / screen audit gap 1 cannot be marked fully implemented until the actual activity click/bridge/result path and QML runtime are verified. Record image and rich-rendering differences below instead of claiming full native inspector parity.

## Geometry and deliberate differences

Native body uses `space-y-2`, mono `text-2xs`/relaxed wrapping, and separate scroll regions with `max-h-80` (320 CSS px). This component uses `Style.space(8)`, caption/monospace, foreground input, muted output, theme failure colors and a **scaled 320-unit maximum** per section. `T3Wheel`, `T3PickerButton`, `T3IconButton` and `T3CopyButton` are reused. It adds Details/Metadata/Raw tabs appropriate to a panel; native normally expands the body inline. Parent can embed it inline without changing native reads. No new hardcoded palette colors.

Current limitations, not placeholders: reasoning is selectable **plaintext**, not native Markdown; structured dynamic arguments are pretty JSON rather than native flat `key value` pairs/syntax highlighting; output images are native image **markers in raw/structured output**, not fetched image previews. Native file search/web search bodies are structured results, not clickable native result layouts; no diff/fork/checkpoint/subagent mutation/navigation buttons are added by this read-only module. Other native item types remain inspectable via Metadata/Raw. No claim of screenshot/pixel parity or live clicking is made.

## Strings for parent catalogs

Every UI label goes through `T3I18n.tr`; server-origin text and JSON keys remain literal, plaintext data. Add/verify these exact English source keys (including section labels emitted by Rust):

- `Activity details`, `Details`, `Metadata`, `Raw item`, `Input`, `Output`, `Reasoning`, `Results`, `File`, `Changes`, `Diff`, `Plan`, `Steps`, `Prompt`, `Questions`, `Message`, `Error`, `Progress`, `Result`.
- `Retry`, `Close`, `Copy`, `Copied!`, `Copy failed`, `Exit code`, `Loading output…`, `Couldn't load output`, `Output is no longer available.`, `No output.`
- `This activity has no inspectable item reference.`, `The item no longer matches this activity.`, `The server returned an invalid item.`, `Output was withheld by the server. Retry to refresh.`, `Display limit reached. Copy includes only the displayed text.`
- Native status source keys: `idle`, `pending`, `running`, `waiting`, `completed`, `failed`, `cancelled`, `interrupted`.

Local errors: `invalid_tool_reference`, `tool_scope_mismatch`, `malformed_tool_item` are explicitly localized by the component. Other native/transport errors use existing `T3I18n.error` after the localized load-failure prefix. Existing copy component strings/acknowledgements remain in use.
