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

Protocol GET `/api/orchestration/shell` and `orchestration.subscribeShell` now
accept at most **64 MiB**, matching the existing bounded-thread allowance.
A synthetic 10,000-thread shell with long titles exceeds 9 MB, so the former
8 MiB ceiling rejected normal profiles. This gives roughly seven times that
profile's size while preserving a finite serialized-body/frame bound. Parsed
JSON has additional allocation overhead. Other subscriptions retain 8 MiB unless
already opting into bounded thread snapshots; explicit per-request limits still
apply. A shell subscription now uses one of the FFI handle's two large-stream
slots (72 MiB queue each); the eight-stream total and 192 MiB aggregate queue
budget stay unchanged. No server change or new request flag is required.

The same nightly source defines no pagination or bounded opt-in on the shell
HTTP endpoint (`packages/contracts/src/environmentHttp.ts:680`) or subscription
(`packages/contracts/src/orchestrationV2.ts:3202`). Separate archived/older-settled
thread APIs do not paginate the active shell snapshot. Tests cover a shell above
8 MiB over HTTP and WebSocket, including a reconnect with `afterSequence`.
