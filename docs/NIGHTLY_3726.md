# Nightly 3726 integration

Reference: `v0.1.3-nightly.20261026.3726`, T3 Code commit
`3f9448747bb8848050dc2cacf83affa6f94a66a1` (2026-10-26).
Shared protocol: lib-t3-rust `6e1c19474f793efa9560b9031057679c18818372`
(v0.3.6 candidate, branch `feat/nightly-3726`). No release or Android AAR was published.

## Integrated surfaces

- Compact snapshot/history IDs and deduplicated live bodies, with resnapshot on
  a missing body source. Bounded item loading and older server responses remain compatible.
- Shell omission capabilities, older settled pages/counts and selected thread families.
  Snapshot generations, per-thread sequence tombstones and page cutoffs prevent stale
  pages from resurrecting deleted threads or skipping the first page after reconnect.
- Guest link creation/copy/close and owner approval/dismissal of queued guest messages.
  Approvals dispatch the server-owned guest message ID; guest text never becomes UI markup.
- EAS account inspection and host CRUD alongside SSH hosts. Settings retain existing
  credential references on edits and unrelated hosts. Views expose only token presence.
  Sessions and device operation reuse the existing native device contract.
- Ignored file listing, worktree occupant navigation, host-backed PR author/label
  filters and change breakdown. All reads retain their environment/project scope.
- Connect reachability uses the native POST endpoint. Explicit SSH discovery can use
  a hidden Zenity askpass prompt; background probes stay noninteractive and reuse a
  private SSH control socket. Cancel does not persist credentials.
- Default follow-ups steer; an explicit queue preference is retained. Queue-edit
  acknowledgements preserve newer text/attachments. Settling offers a short undo
  action, releasing its shortcut on a later edit. Selected text keeps Cmd+B.
- Locale/12/24-hour timestamps, inline audio loaded on demand, usage-limit auto-resume
  and snooze, and scheduled-task worktree strategy/base/thread/mode preservation.
- Automatic compaction appears as one maintenance row and does not make the thread
  look busy. Only awaited commands/monitors hold Working; delegates keep their own hold.
- Native browser, preview, usage, device, keybinding and license helpers regenerated
  from this exact reference. Translation catalogs contain 20 locales.

## Server-owned behavior and native equivalents

The guest open/redeem/submit events belong to the server or public guest page;
OT3 does not acquire authority to dispatch them. Public guest URLs open the native
T3 guest page. Existing message/stop/approval APIs inherit Nightly's PDF document
routing, attachment claims, subagent stop and requesting-agent approval fixes.

`redesignEnabled` selects a React presentation and does not configure the native
Qt interface. The legacy WebSocket ticket endpoint remains supported by this
Nightly; route-specific tickets do not require a client migration yet.

The drift inventory is a lexical discovery aid, not proof of complete UI parity.
Older unreviewed entries are deliberately not marked complete just because the
shared API registry knows a method. This change closes the 3386-to-3726 integration
delta and does not claim every historical web/desktop feature was ported.

## Validation

- Shared library: 365 tests passed across 59 suites, including exact public RPC
  payloads/scopes, a real local WebSocket compact snapshot and body recovery.
- QML: 29 scenario fixtures cover the new screens plus queue, composer, sidebar,
  search, preferences, connections, PRs, tasks, live media and shortcuts.
- Native client validation: 52 unit tests and the focused integration suites cover
  the shell watcher, eight simultaneous mosaic streams,
  preferences CAS, EAS credential preservation, bounded HTTP, durable queue/outbox,
  owner guest commands and the Connect POST endpoint.
- After the guest approval fix, 15 focused library tests passed; an ordinary empty
  message still fails validation. Vendor hashes, drift inventory, translations,
  unchanged native usage/preview cases and helper provenance checks pass.
- SSH askpass is checked with a synthetic prompt/cancel driver. EAS provisioning,
  a real password/OTP login and a deployed Nightly server were not exercised.

The installed OT3 binary/service and original T3 checkout are unchanged.
