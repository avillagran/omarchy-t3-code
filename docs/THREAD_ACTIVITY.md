# Sidebar activity clocks

Nightly 4301 (`4cb6945dc60ed03f567dc5daafd083e52308e82b`) changes sidebar relative labels to the native `resolveThreadActivityTimestamp` helper. Its clock is independent of the existing LastReply sorting preference.

For native-shaped shell records, the normalizer preserves a separate `activityClock` input. The generated native helper selects the latest valid user-message, requested, started or completed timestamp. It falls back to `updatedAt` only when those activity stamps are unavailable. Settled labels first use a valid persisted `settledAt`. Tied activity times preserve the native candidate order. `latestUserMessageAt` is intentional; it is not replaced by `latestUserAuthoredMessageAt`.

The shell adapter follows the actual `models.presentThreadShell` boundary: a null latest run ID suppresses the run clock; an omitted completion uses `updatedAt` for idle or terminal runs, while an explicit null stays null. Native shell stream deltas update this input through the existing live shell reducer. Bounded message pages do not infer or overwrite the authoritative shell clock. Legacy numeric or incomplete records retain their existing presentation fallback. Existing reply comparators and reply timestamps remain unchanged.

Relative labels consume the original native timestamp formatter and sidebar label method. A shared native `minuteTick` store supplies wall-minute snapshots; its actual Qt timer starts with subscribers and stops after the last unsubscribe. Only label bindings consume the minute property. Working durations keep their separate elapsed-work clock.

Qt's built-in Set iterator does not visit a newly added listener during the same notification, unlike the pinned native runtime. A factory-local listener-set service preserves live iteration, removal and re-subscription. It does not patch global prototypes or replace the native clock method body. The Qt owner uses a real timer handle. Only `minuteTick` is exposed; this checkpoint does not activate the native deadline or second-tick families.

## Evidence

- The complete native `threadSort.ts` executes 105 activity/settlement cases; Qt compares both returned clocks and reports 12 separate date-parser diagnostic cases.
- The complete native presentation module, its official typed fixture and 83 actual source modules execute 144 shell cases with real Effect 4.1.2 DateTime. PR projection is isolated to an empty result for the no-PR fixture, and subagent title formatting is not exercised. These cases do not certify those unrelated projections.
- Rust compares both the shell adapter and the actual normalized row producer with all 144 native presentations. An actual live shell delta test checks that a stale event cannot restore an older clock.
- The full native timestamp module and original row label method produce 36 relative-label cases. The full native clock produces ten subscription, cache, boundary, teardown and reentrant traces. Qt matches those traces, including the platform listener-set adapter.
- The physical Qt timer test verifies alignment, a real Text binding update, shared listeners and last-reader teardown. A separate actual T3Panel/sidebar-delegate test preserves LastReply order while checking independent activity labels, sorting-mode changes and settlement precedence. It waits for the real 60-second native timer period, checks the resulting row label updates, and destroys the panel to verify shared-timer teardown; the period is not shortened. Its private transport feed performs no account access, network operations or file mutations; the production clock and row logic remain active.

All runtime proofs run in rootless Podman with private profiles, read-only source/dependencies and no external network. Rust tests use immutable runtime snapshots. No fixture deletes file trees. The full client runtime audit marker remains in place.

Run source generators only inside that reviewed private Node 26.13/Effect 4.1.2 environment. Generate the full presentation graph before the relative-label oracle: its erased type-only settings import resolves to the same real, canonical contract/Effect graph. `tools/test-rust-container --test thread_activity` and the `thread-activity` / `thread-row-activity` modes of `tools/test-device-panel-container` exercise the focused consumers.

The selected UTC, timezone, early/expanded-year and invalid-date diagnostic cases match between the pinned native runtime and Qt. This is not a universal certificate for implementation-defined date strings. Native-shaped inputs use the server's encoded date fields; numeric legacy dates are not guessed into that representation.
