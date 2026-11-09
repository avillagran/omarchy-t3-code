# Nightly 4353 source delta

The current target is recorded in [the Nightly 4367 delta](NIGHTLY-4367-DELTA.md).
This preceding target is [v0.1.8-nightly.20261108.4353](https://github.com/pingdotgg/t3code/releases/tag/v0.1.8-nightly.20261108.4353)
at `3cd97a1ee230437b1bfd8441dffe65eac55be1e5`. The original checkout retains
its 4314 HEAD; the new tag was fetched without switching it. The preceding
[4340 ledger](NIGHTLY-4340-DELTA.md) and component receipts remain intact.

The exact comparison records 11 commits and 24 changed paths. Of the 70
previously reviewed original source footprints, 65 are byte identical and five
changed. [The complete source comparison](reference-delta-4353.json) includes
per-file SHA256 values and each commit's paths. Its external receipt is
`/var/tmp/o3-build/nightly-4353-reference-20261108/equivalence.json`, SHA256
`16ed515a6dbf91c953dc95bb7f49e9c5239c5aac98110bbd3159a5b81f1df4c1`.

The changed reviewed footprints are `McpHttpServer.ts`, its test, preview
`handlers.ts`, `ServerBrowser.ts` and `ServerBrowserPage.ts`. Their delta keeps
an optional full-resolution screenshot for saving while retaining the capped
agent image. Earlier screenshot/handler receipts do not establish this new
behavior. Unchanged files retain only their earlier qualified component
proofs; callers, changed contracts and dependencies still require review.

| Commit | Original change | Current evidence |
| --- | --- | --- |
| `3cd97a1e` | fix(server): saved preview screenshots keep full resolution (#28255) | pending review |
| `e11c2fdc` | fix(mobile): share PDFs from the Android attachment viewer (#32266) | pending review |
| `7b680b61` | feat(antigravity): support agy CLI as custom binaryPath (#32503) | pending review |
| `8a84e942` | perf(server): skip oversized history during handoff selection (#23821) | published on server `2e1721d`; exact scalar selector matches 63 native cases, full container CI passes both modes |
| `a1af1841` | fix(scripts): report:renders starts on Windows without a vp shim (#32777) | pending review |
| `71c2d3a3` | fix(server): storage report lists project worktrees nested inside plain folders (#25883) | pending review |
| `28eff11d` | fix(web): lineage phase headers truncate instead of overflowing the details card (#27582) | pending review |
| `ecf7f1ab` | fix(web): mobile sidebar keeps its width when shelves expand (#27541) | pending review |
| `f475de2f` | fix(web): device tool update buttons fit their popovers (#27030) | pending review |
| `57ded10b` | fix(web): restore bold weight in rendered Markdown (#25694) | explicit 700 weight in native rich-text CSS; rendered UI acceptance pending |
| `0450f854` | fix(muse): a declined /compact reports its reason and no longer wedges the thread (#28454) | pending review |

The native Markdown stylesheet now explicitly gives `b` and `strong` weight
700, matching the new reference rule instead of relying on rich-text defaults.
This small style change has source review only; it does not claim rendered
font or whole-client acceptance.

All other entries remain pending until their actual O3 behavior and relevant
proofs are reviewed. Android PDF sharing requires explicit Android review and
the canonical project sandbox. Windows tooling and mobile web behavior need
an explicit platform decision; neither is silently excluded. Server provider,
handoff and storage work in parallel branches must be integrated and validated
before counting it here. No repeated test is required solely because an
unchanged original file has a newer release tag.

The global validated pin remains 3726. Client auth draft PR #36 and its local
integration retain their 4340 strict/UI/RPC scope. Full Browser Status routing,
GUI input context, managed auth storage, cloud and emulator acceptance remain
open. No source comparison or individual component result accepts the full
Nightly 4353 application.
