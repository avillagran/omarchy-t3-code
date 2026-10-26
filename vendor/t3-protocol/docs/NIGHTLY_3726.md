# Nightly 3726 integration

Reference: T3 Code `v0.1.3-nightly.20261026.3726`, commit
`3f9448747bb8848050dc2cacf83affa6f94a66a1`. The extracted RPC/permission
fixture and Android `describe`/Kotlin wrappers use this exact contract.

- Bounded HTTP reads request `compactTurnItems=1`, `dedupeBodies=1` and
  `compactIds=1`. History reads request `compactIds=1`. Plain responses from
  older servers still work, and the bounded response limit stays 64 MiB.
- ID tables expand at the HTTP/stream boundary, only on the routes and stream
  defining this encoding. Expansion follows T3's record-level `id`, `*Id`,
  `*Ids` and nested `item` rule; arbitrary tool JSON is untouched.
- `bounded_snapshot::restore` restores bodies before local visible items.
  `projection::try_apply` restores live bodies from the current projection
  and reports missing sources, so a host can resnapshot without advancing its
  cursor past lost text. Legacy `projection::apply` remains available.
- `ThreadSub` exposes compact-ID/body opt-ins; `ShellSub` exposes the two
  omission flags. Hosts must gate shell omissions on `shellThreadFamilies` /
  `shellOlderSettled`, fetch selected thread families, page older settled
  threads, reconcile counts, and preserve stream sequence ordering.
- Typed reads: thread families, worktree occupants, older settled pages,
  ignored files, PR filter options and change breakdowns, and Expo accounts.
  Guest creation requires `access:write`; Expo account inspection requires
  `settings:write`. Other reads retain their contract scopes.
- Owner commands close guest shares, dismiss pending guest messages and send
  approved messages through `message.dispatch.guestMessageId`. Share opening,
  redemption and submission remain server-only commands.
- Automatic idle compactions render as one quiet activity row. Failures and
  manual compactions remain visible. Commands/monitors hold completion only
  when awaited; subagents/background tasks still hold it. Old shell vectors
  without a task kind retain their legacy row hint.

New struct members require downstream Rust struct literals to use
`..Default::default()` or constructors. Existing C ABI symbols/versions remain
unchanged. Generated Kotlin source is updated; this change does not publish
or replace an Android AAR.
