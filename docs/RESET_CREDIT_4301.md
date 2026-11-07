# Reset-credit consumer, nightly 4301

Reference: T3 Code `4cb6945dc60ed03f567dc5daafd083e52308e82b`.
The client uses the published lib-t3-rust reset-credit input/API without private
SDK changes. No real account credit is used by these acceptance checks.

Account rows retain the native redemption destination and credit snapshot.
The confirmation dialog lives outside the expandable account details, identifies
the account and warns that redemption cannot be undone. Cancel and Escape close
it without a mutation. An account with banked credits remains visible even when
its current usage-window list is empty.

The UI checks provider-management access before opening confirmation. The actual
write dispatcher independently fetches the current authenticated session with the
write credential, checks `providers:manage`, decodes the typed input and sends
exactly one `provider.consumeResetCredit` RPC. A lost response is reported; it
never authorizes retry. Accounts have separate pending/results state, and stale
request IDs or another environment cannot complete an operation. Warning text
wins over the four native outcome labels. Progress/results remain outside account
details after confirmation closes; the quota refresh is read-only.

`tools/build-reset-credit-ui.cjs` extracts the complete native summary function.
`tools/build-usage-limits.cjs` updates account presentation to the same revision,
including fresh plan selection and native hub/default-label handling. The latter
is a presentation-only module: new exhausted-window/model-fallback selection
exports are deliberately outside this module and remain separately scoped.
Provider display names come from the actual canonical contracts module export,
not a hand-built driver table. Source/output hashes are recorded next to generated
modules. The native oracle verifies every staged input and Effect 4.1.2 before
importing the real module graph.

Private acceptance:

- `tests/reset_credit.rs`: real loopback HTTP/WebSocket through `write::execute`,
  separate read/write credentials, fresh scope revocation, invalid input, all four
  outcomes, hub credit payload, warning and dropped-response no-replay behavior.
  One integration test passes in rootless Podman; fixtures are retained.
- `tests/qml/reset-credit.qml`: actual backend and confirmation component,
  denied/revoked scope, cancel, confirm, repeated confirm, stale completion and
  error/outcome presentation. It also compares 48 complete summary results and
  48 complete native account projections. The incidental worker is an explicit
  inert fixture; this visual proof does not consume server credits.
- Strict Clippy for the library, CLI and reset-credit target passes. Translation
  catalogs cover all 20 locales. Full client runtime collection remains gated by
  the existing transitive filesystem audit; no broad-suite claim is made.

Evidence is retained under `/home/node/BuildCache/o3-reset-credit-ui` and
`/var/tmp/o3-client-rust-62aak85v`; the final isolated QML checkpoint is
`/var/tmp/o3-device-panel-wdzzgtol`. No deployment, profile change or account
mutation is part of these checks.

Pending redemption belongs to the stable account identity. A quota refresh that
rotates `nextCreditId` cannot enable a second redemption for that account while
the first request remains pending; unrelated accounts remain independent. The
actual modal regression reproduces the old bypass and passes with stable account
admission (`/var/tmp/o3-device-panel-v617oo95`).
