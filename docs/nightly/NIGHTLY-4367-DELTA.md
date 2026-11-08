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
| `09a54f01bc` | Whole-word title matches rank above busy prefixes | Pending implementation and validation review |
| `dff0b1159b` | Show the actual reason relay unlink failed | Pending implementation and validation review |
| `22dc6f9e98` | Keep scheduled task prompts clear of the mobile microphone | Pending Android/mobile review |
| `8e6a732305` | Move a scheduled task to another thread while preserving history | Pending server, SDK and client review |
| `af65a55dc7` | Codex tool calls proceed without waiting for reasoning summaries | Pending adapter and session review |
| `6417151c09` | Combobox search fields fill their available width | Pending native UI review |

Full Browser bootstrap/status/routing, GUI input context, managed authentication,
cloud and emulator acceptance remain open. The accepted GUI26 and scoped managed
peer20 receipts retain their exact source and runtime qualifications. GUI28 has
compiled the subsequent fault/quiet/burst fixture but has not executed it yet.
No new source comparison, library build or component test accepts the complete
4367 application. The global validated pin remains 3726.
