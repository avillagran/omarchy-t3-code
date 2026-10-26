# Corrections

## Pending

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
