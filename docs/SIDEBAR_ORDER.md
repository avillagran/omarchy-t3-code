# Sidebar ordering

The native rules come from T3's `client-runtime/state/threadInbox.ts` and
`threadSort.ts`. Timestamp normalization lives in `lib-t3-rust::sidebar` so Omi
can use the same anchors.

- Grouped inbox: newest creation, un-settle, run request or run completion.
  Observed returns from Working also cover mid-run questions and background work.
- Working: latest authored user message, with the native request-time fallback.
- Settled: explicit settlement, otherwise latest message/run timestamp.
- Ungrouped active list: native manual order keys and creation/un-settle anchors.
- Pinned rows retain their independent order and stay above unpinned rows.

Opening the panel does not invent a return. Refreshing metadata does not move a
thread. Live completion events update the sort anchor before the next shell poll.
Regression checks: `tools/test-qml sidebar-order`, shared library timestamp tests,
and the desktop live-completion normalization test.
