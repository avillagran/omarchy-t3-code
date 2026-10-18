# Subscription limits

The native QML panel uses the same account identity, workspace disambiguation,
freshness, pooling, window ordering and pace helpers as T3 Code, pinned to
`5a4e9980533705774dc3f45b35cacf76c3d78dbc`.
`tools/build-usage-limits.cjs` generates `ui/js/NativeUsageLimits.js` from clean
original source and records input/output hashes and Qt compatibility adaptations.
There is no Node or Python runtime dependency.

## Discovery and host ownership

Each saved environment gets a cancellable `subscribeServerConfig` connection with
`usageLimitSources: true`. Provider instances come from the snapshot and
`providerStatuses`; hubs replay and update through `usageLimitSourcesUpdated`.
Reading only `server.getConfig` would omit hubs. Unknown drivers can contribute
valid limits without a client provider list change. Unsupported window kinds are
skipped rather than misinterpreted. Instance homes, credentials and provider
requests stay on their owning T3 server. The client neither copies remote tokens
nor uses local credentials on behalf of external OpenCode servers.

The subscription preflight requires the original `orchestration:read` scope.
The read-only projection retains only instance/account identities, labels,
subscription windows, public reset credit counts and validated external usage
links. Models, provider environment variables, settings, management keys, raw
transport errors and credential values never reach QML. Credential fingerprints
are opaque identity metadata, not reusable credentials.

Native status refresh is coalesced per environment, at least five minutes between
automatic attempts, including failures. The refresh uses `server.refreshProviders
{}` without `refreshModels`; no agent conversation is started. The explicit
refresh button requests another check. Connections retry with bounded backoff;
a removed/replaced environment cancels its old reader and rejects late receipts.
Disconnects label cached readings as potentially outdated; authorization failures
clear native observations. The CLI `quotas` reads the same sanitized subscription
including its hub replay, with a bounded observation interval.

## Supplemental local accounts

Existing local Hermes/Codex support is retained as another compatible source.
Discovery enumerates all credential-pool entries, not just the first. Duplicate
credentials are probed once per adapter. Codex also reads its existing CLI login;
Kimi uses its existing environment key. Registered local adapters currently cover
Codex, Kimi Coding, Nous and OpenCode Go. Arbitrary new native T3 drivers do not
require adding a local adapter.

OpenCode Go reads local Hermes credentials, `OPENCODE_GO_API_KEY`,
`OPENCODE_API_KEY`, or the local OpenCode `opencode-go` API credential. It queries
`https://opencode.ai/zen/go/v1/usage` directly, using an identifying application
User-Agent, and follows the original rolling/weekly/monthly response contract.
Its fingerprint uses the same `SHA256("opencode-go\\0" + key)` identity as T3.
These are **local supplemental** accounts; a remote Hermes-only credential pool
is not discovered through a T3 provider that does not publish usage limits.

Local probes use four workers at a time and publish partial results between
batches, so a large pool does not hold up native host readings or draft/message
workers. Failed reads retain a matching account's last good windows and original
checked timestamp with a failure notice. Completed discovery removes accounts no
longer configured. This reader does not refresh or rewrite OAuth credentials.

## Presentation

The original helpers deduplicate by driver and normalized email or credential
fingerprint, including workspace disambiguation. Native instances and hub reports
of the same account share the freshest observation. Accounts with unknown
identities remain separate, as in T3; the client does not guess that two accounts
are identical. Each driver has generic session, weekly, monthly or other windows,
with aligned account segments, remaining percentages and reset countdowns.
Expanding a group shows each account's plan, reporting hosts/source and windows.
The view scrolls at a bounded height and shows probe/disconnection notices.

Hub credentials remain configured per T3 environment using the original Usage
providers settings. This change consumes those hubs; it does not add a second hub
configuration store or a native hub editor. Reset-credit redemption and the full
Usage cost/token page are outside this read-only panel.

## Validation

- `node tools/test-usage-limits.cjs`: 47 unchanged original T3 cases against the
  generated Qt-compatible helpers, including Go identity, workspaces, native/hub
  duplicates, freshness, missing windows, pool arithmetic and external links.
- `cargo test --release --test quota_sources --test live --bin omarchy-t3-client`:
  real authenticated WebSocket subscription, explicit hub opt-in/replay, projection,
  permission denial, generic drivers/windows, local discovery/parser and retained
  good snapshots; existing chat streaming remains independent.
- `WAYLAND_DISPLAY=wayland-1 QT_QPA_PLATFORM=wayland tools/test-qml usage-limits`:
  original algorithms in Qt, multiple hosts/accounts, dynamic drivers, monthly
  windows, actual controls, refresh and a captured native render.
- Existing `layout` fixture verifies header placement and collapse behavior.

Live read-only verification found both configured environments connected and
reporting their native provider catalogs, alongside local Codex/Kimi accounts.
Go consumption was unavailable: the default HTTP client was blocked by
Cloudflare error 1010; an identifying application User-Agent reached the API and
received HTTP 401 for the existing local credential. Adapter/fixture success is
not evidence of a successful live Go quota read. No credentials were changed.

The scoped runtime install replaced eight files from an isolated HEAD-based Rust
build and a copy of the actual installed QML baseline. It excludes concurrent
Browser/Storage work. Staged installed-baseline `usage-limits` and `layout` QML
checks passed. The shell was restarted and its selected/open thread restored;
17 draft hashes, native preferences and the uncapped HTML renderer were preserved.
Backup: `/tmp/ot3-before-subscriptions-ggv_l3oo`. No commit, push or release.

The compact follow-up places provider names beside their windows, removes the
extra usage heading in the chat header view, reduces padding, and puts unavailable
readings behind one warning tooltip. Account details remain expandable. Every
account segment with a known window duration/reset has an optimal remaining
marker from the original `elapsedShare` clock. Available quota below that marker
uses the urgent theme color immediately; equality/above uses the accent. Windows
without a usable clock have no invented marker. The original pooled pace helpers
and their five-point tolerance remain unchanged; this stricter visual threshold
implements the requested per-account comparison against the visible marker.
