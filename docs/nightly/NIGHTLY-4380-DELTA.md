# Nightly 4380 source delta

The current target is
[v0.1.8-nightly.20261108.4380](https://github.com/pingdotgg/t3code/releases/tag/v0.1.8-nightly.20261108.4380),
published on 2026-11-08 at 18:48:01 UTC, commit
`ade473018d32003658b7d38693e083ee2e314bd8`. The GitHub stable `/releases/latest`
endpoint still returns 0.1.7; the release collection includes this newer
prerelease. The requested Nightly goal follows the prerelease.

The comparison with 4367 (`6417151c093e54dbba0e6c824c69d03983793b9a`) contains
one commit and eleven changed paths. The build-number increment does not mean
thirteen source commits. Commit `ade473018d` changes provider maintenance so
mise-managed installations update through mise.

Changed source:

- Server Claude maintenance and Codex driver declarations.
- Provider-core `maintenanceResolver.ts` and its original tests.
- Grok, Muse, OpenCode and Pi maintenance/driver integration.
- Provider internals and installation documentation.

The maintenance resolver and provider declarations require implementation and
validation review in O3; a passing earlier component gate does not cover them.
All older pending functionality remains in scope.

## Exact reusable source observations

| Scope | Observation | Limits |
| --- | --- | --- |
| Browser and Status | 112 of 112 files byte identical | Includes the admitted 84-source closure, the complete preview subtree, callbacks, formatter, viewport and host helpers. Does not accept new staged foreground integration. |
| Managed auth | 116 paths unchanged | The complete 114-source closure plus registry hydration and WebSocket settings behavior are unchanged. Current lifecycle integration is still source work. |
| Search and visits | 13 paths byte identical | Complete generated search input sources, contract, server open helper/orchestrator, visit tracker and palette-pick behavior. Does not establish SDK publication or combined client acceptance. |

No unchanged runtime tests were repeated solely for this reference update.
Historical receipts retain their exact source, dependency and execution scope.
The global validated application pin remains 3726; selecting a newer target or
showing source equivalence does not advance that validated pin.

Evidence:

- Browser: `/var/tmp/o3-server/status-native-4367-to-4380-2ragbree/equivalence.json`, SHA256 `0140ad6274291816f51c119ec2a3930cff5a3c7414aac2d66189c990c76f9fac`.
- Search/visits: `/var/tmp/o3-build/thread-open-4380-source-equivalence-o2j6ptbn/review.json`, SHA256 `828c75c0c29b0a8090fa6f497bc2c12398aa2712b3090c8d3d2a41db93a68799`.
- [Native search runtime and consumer candidate](NATIVE-SEARCH-RUNTIME-4380.md) distinguishes real V8/protocol passes from the uncompiled Rust/QML integration.
