# Nightly 4367 source delta

The current target is [v0.1.8-nightly.20261108.4367](https://github.com/pingdotgg/t3code/releases/tag/v0.1.8-nightly.20261108.4367),
commit `6417151c093e54dbba0e6c824c69d03983793b9a`, published on
2026-11-08 at 12:24:26 UTC. The original checkout retains its 4314 HEAD;
only the immutable new tag was fetched. The [4353 ledger](NIGHTLY-4353-DELTA.md)
and its pending requirements remain part of the full target.

[The complete comparison](reference-delta-4367.json) records eight commits and
70 changed paths. Of the 70 previously reviewed source footprints, 67 are byte
identical; `mcp/toolkits/core.test.ts`, `mcp/toolkits/html/handlers.ts` and
`mcp/toolkits/html/tools.ts` changed. The 84 original Page runtime source files
staged from 4353 are also byte identical at 4367. This preserves only their
earlier scoped evidence; it does not validate changed callers, new features,
dependencies or a whole application.

The source-comparison receipt is
`/var/tmp/o3-build/nightly-4367-reference-508jqfxd/equivalence.json`, SHA256
`da308ce5503b5f843c29a55f74528cf40a9668ae904cdb0c291676a9e40ebb07`.

| Commit | Required behavior | Current evidence |
| --- | --- | --- |
| `20fcc6ca57` | Refresh linking tools in existing agent sessions | Pending implementation and validation review |
| `d6c2b09c52` | HTML preview/render accepts a file path, with native source checks and caller permission enforcement | Pending implementation and validation review |
| `09a54f01bc` | Whole-word title matches rank above busy prefixes | Complete original core passes 34 callbacks/192 assertions in V8; Qt regex incompatibility and product/history integration remain open. [Evidence](NATIVE-SEARCH-4367.md). |
| `dff0b1159b` | Show the actual reason relay unlink failed | Pending implementation and validation review |
| `22dc6f9e98` | Keep scheduled task prompts clear of the mobile microphone | Pending Android/mobile review |
| `8e6a732305` | Move a scheduled task to another thread while preserving history | Pending server, SDK and client review |
| `af65a55dc7` | Codex tool calls proceed without waiting for reasoning summaries | Pending adapter and session review |
| `6417151c09` | Combobox search fields fill their available width | Pending native UI review |

## Subsequent scoped progress

The server feature branch now publishes canonical birth installation before
author navigation, retaining the 4367 native source manifest and fullScreenshot
behavior. Three physical lifecycle cases and six combined Chromium capture
cases passed. Head `19132bf` corrects two scoped-reply test fixtures after CI at
`65beeff` exposed their missing session IDs; exact-head
[CI 42497001835](https://github.com/avillagran/o3-server/actions/runs/42497001835)
passes both modes. Complete Status producers and consumers remain a separate,
uncompiled source candidate.

[Restored controls](RESTORED-CONTROLS-4367.md) records local T3/O3 wording,
device-code copying and preference-page persistence, two QML cases and an actual
four-process state-module probe. The normal client library build passes after
targeted invalidation of a stale shared-cache fingerprint. A rebuilt application
and its complete worker integration suite remain pending.

GUI31 alias and poison cases passed; busy exposed an incorrect fixture
expectation about a shared registration. GUI32 cleanup and genuine sibling/
capacity coverage are preserved in a local checkpoint, without compilation or
runtime acceptance. Managed authentication now stages the full 4367 native
worker closure and linked-environment lifecycle source; that new candidate has
not been compiled or activated.

Full Browser status/routing, remaining GUI acceptance, managed authentication,
cloud and emulator acceptance remain open. Earlier accepted and failed receipts
retain their source and runtime qualifications. No component gate accepts the
complete 4367 application. The global validated pin remains 3726.
