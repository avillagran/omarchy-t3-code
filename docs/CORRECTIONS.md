# Corrections

## Pending

- [ ] **2026-10-26 — Restore the desktop RAM budget.** Initial RSS: Qt UI
  393 MiB, Rust worker 404 MiB. Worker reduced to approximately 61 MiB by
  returning freed allocator pages, avoiding simultaneous history copies and
  caching successful idle checks by metadata revision with a five-minute TTL.
  UI remains near 400 MiB; full 50–100 MB target is not achieved. Serialized
  thread/catalog/translation data alone does not account for UI RSS.
  Follow-up: lazy sidebar actions, utility popups and multimedia are installed;
  the fixed 24-thread panel saves 37–51 MiB PSS+SwapPSS. The live UI still
  sampled ~409 MiB RSS on a long chat; the target remains open. See `UI_MEMORY.md`.
  Allocator follow-up: Quickshell uses jemalloc, so app-local arena/cache/decay
  tuning is installed. Fixed long-chat fixture saves 3–36 MiB PSS+SwapPSS in
  two comparisons under varying system pressure. UI still exceeds the budget;
  restart snapshots are not treated as a controlled before/after measurement.

- [ ] **2026-10-26 — Intermittent Spanish accents in text input.** User reports
  that typing accented letters sometimes works and sometimes fails despite a
  Spanish keyboard layout. Identify the affected input and reproduce with real
  dead-key sequences (`´` then `a/e/i/o/u`), upper case and repeated typing.
  Inspect input-method composition, shortcut handling and editor re-rendering;
  verify text and cursor preservation. Root cause is unconfirmed. Pasting
  precomposed Unicode alone does not validate this report.

## Fixed locally

- [x] **2026-10-26 — Node reconnect loop and red connection indicator.**
  Multiplexed SSH forward ownership fixed in `15cc34d`; seven connection tests
  pass, including cleanup after abrupt owner exit. Release binary installed on
  Omarchy with backup; both environments ready, `stale=false`, 231 threads.
  Existing QML process retained. Removed 101 abandoned forwards while retaining
  the shared SSH master. No remote server restart or version change.

- [x] **2026-10-26 — Explain connection failures without agent assistance.**
  Persistent notice in the chat and Connections shows the affected host, a
  readable reason, a selectable error code and Reconnect/Connections actions.
  Retrying retains the cause; recovery clears it. The hosts popup and bar tooltip
  also expose the reason. Unknown codes remain visible rather than guessed.
- [x] **2026-10-26 — Global animation switch at the start of Appearance.**
  Turning it off freezes decorative clocks and the bar, disables transition and
  effect controls, and makes transitions immediate. Individual choices remain
  stored and return when enabled. Default remains enabled for existing users.

- [x] **2026-10-26 — Locate sidebar threads in the mosaic.**
  Clicking a sidebar thread keeps the mosaic open, selects its page and adds
  a 3.2-second accent border. Repeat clicks renew the emphasis. Works with
  global animations disabled; manual pagination clears the temporary highlight.
  Verified with real Qt test clicks and existing mosaic motion checks.

- [x] **2026-10-26 — Separate sidebar visibility and keyboard focus.**
  Ctrl+B toggles the sidebar. Super+B opens it and focuses/reveals the current
  chat row, expanding its section and ancestors. If filters exclude that row,
  this explicit reveal clears them. Arrows continue from that row; selection
  and drafts are retained. Native duplicate bindings are suppressed, and the
  rich-text composer routes these keys without applying text formatting.
