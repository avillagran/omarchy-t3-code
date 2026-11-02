# Server capabilities and large shell snapshots

`gitControls.actions` and `gitControls.present` accept an optional `serverConfig`
object from `server.getConfig` (or `subscribeServerConfig`). Rust callers use
`git_controls::actions_with_config` and `present_with_config`; the original
functions remain available for older integrations.

The exact T3 field is `serverConfig.environment.capabilities.pullRequests`.
Explicit `false` disables both `create_pr` and `commit_push_pr`, while leaving
commit and push available. `true` or an absent field preserves the existing Git
state rules. `present` retains only this capability in its output and revision;
`buildAction` checks it again, and `buildVerifiedAction` rejects a changed
capability as stale. Hosts must pass the current server config when presenting
and refreshing a Git sheet. The library does not infer support from a forge name
or the client's installed tools. This is an environment capability, not a claim
that every repository host or account can create PRs; servers must advertise
unsupported environments as false. Per-repository/authentication failures still
need server-side validation.

Evidence is **source-only**, from T3 nightly commit
`680152e27b2a961686389ca69a1557cbb2c55f08`:

- `packages/contracts/src/environment.ts:84`: optional boolean `pullRequests`.
- `apps/server/src/environment/ServerEnvironment.ts:224`: server advertises true.
- `packages/client-runtime/src/state/presentation.ts:308`: client reads
  `serverConfig.environment.capabilities.pullRequests`.
- `packages/contracts/src/git.ts:216`: VCS status has no PR creation capability.
- `packages/contracts/src/sourceControl.ts:159`: discovery reports provider
  installation/authentication, without a boolean PR creation capability.

Protocol GET `/api/orchestration/shell` and `orchestration.subscribeShell` accept
at most **64 MiB on desktop, 16 MiB on Android by default** (decoded bytes).
Thread snapshot limits remain 64 MiB. The Android limit still admits the audited
9 MiB profile, while bounding the substantial cost of JSON presentation.
Explicit larger connection byte budgets remain an application opt-in.

FFI admission allows one shell separately from two bounded thread subscriptions,
with eight total subscriptions. Desktop serialized queues are bounded by
3 × 72 MiB + 5 × 8 MiB = 256 MiB; Android shell queues have a 24 MiB budget.
JNI accepts up to 72 MiB to match Rust's domain-input envelope; Rust still
rejects non-domain inputs over 8 MiB. This accommodates the shell wrapper and
multi-environment sidebar input without removing the input bound.

## Host JNI memory experiment (2026-11-02)

Java 21, Linux x86_64, `-Xmx2g`, release Rust archive and host JNI. Each fresh
process constructs 10,000 threads with long ASCII titles, sends the resulting
`sidebar.view` domain request, parses/presents in Rust and materializes the JSON
response as a Java string. `wait4().ru_maxrss` records the whole process peak,
including JVM, request construction, native allocations and response copies.

| Nominal title payload | Peak RSS (KiB) |
| --- | ---: |
| 9 MiB | 769196 |
| 16 MiB | 913080 |
| 32 MiB | 1189672 |
| 64 MiB | 1966304 |

All four calls succeeded. Actual input includes about 250 KiB of JSON overhead;
9 MiB produced 9,688,998 input bytes and 43,876,084 output characters.
Run `tools/measure-jni-sidebar` after `tools/test-jni` to repeat the workload.
`mobile-limit-red.log` records the failing 16 MiB platform-budget assertion. These are synthetic whole-process host measurements,
not Android ART heap measurements or Compose rendering measurements. A 64 MiB
phone default is unjustified at this cost; 16 MiB is a conservative compatibility
compromise, not a guarantee on low-memory phones. Pagination or a persistent
native shell handle would be preferable future work.

## Audit corrections

Base: `origin/main` = `1498ad6656fed511d1c7ea61795dfd46c0e01ce5` (0.3.8),
also Android's pre-change pin. Desktop previously used 046bfa9 derived from
7cbad5d (0.3.7). Rebased the capability change onto 1498ad6 to retain gzip/brotli.

Regression evidence in `/var/tmp/o3-build/client-independent-audit/`:
`admission-red.log` reproduces `subscription_limit`; `jni-red.log` reproduces
9 MiB `invalid_input`; `lib-green.log`, `lib-clippy.log`, `jni-build.log` and
`memory.json` record the corrected validation. The compression-bomb test now
uses 65 MiB (the previous exact 64 MiB fixture is within the desktop allowance).

The nightly source defines no shell pagination or bounded opt-in on the shell
HTTP endpoint or subscription. This remains a source-only conclusion, not a
live-server test. No owner server, device, or emulator was contacted.
