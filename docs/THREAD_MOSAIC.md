# Host filters, thread mosaic and quick replies

The sidebar host selector scopes active, working, snoozed and settled shelves,
and combines with the existing project, status and text filters. The selection
is saved locally. Waiting and Idle are available as status filters.

The grid button beside the host selector opens the thread mosaic. Choose 1–8
views; use the arrows for more matching threads. Each tile shows a live text
transcript, project, host and status. Expand a tile to use the full conversation
and its composer, attachments, tools and approval controls. The mosaic is a
reader; opening it does not change the selected conversation or its draft.

Each tile reuses the existing bounded snapshot and resumable thread stream.
Events have their own namespace and serials, so stale results cannot replace a
new tile. Only visible tiles are subscribed, with at most eight mosaic readers.
Closing the mosaic, opening settings or hiding the window cancels its readers.
The regular selected-chat reader remains independent. Transcripts stay in memory,
not in presentation preferences.

Hover over the animated sparkle button next to the microphone for quick replies.
Click or use the keyboard to open it as well. Selecting a reply inserts it at the
saved cursor/selection without submitting. “Stop” is text, not an interrupt RPC.

Preferences → General → Quick replies controls visibility, automatic/manual
language and the list for each language. Edit one reply per line, then Save;
Reset restores that language's defaults. An empty saved list disables its entries.
Lists allow up to 32 replies, 250 characters per reply. Defaults cover all 20 UI
languages. Automatic language selection uses local word/script hints from the
draft, then user messages, then the UI language. It is heuristic: ambiguous short
phrases and mixed-language/code-heavy text may need a manual language choice.
No draft is sent to a translation service.

Validation: `tools/test-qml mosaic-replies` exercises host/project/status intersections, eight distinct
threads, 8→1 tiles, stream routing/stale events, cancellation, language selection,
actual mouse hover and menu clicks,
draft preservation and preference editing. Rust `ui_state` tests validate saved
bounds. `cargo test --test mosaic_live --test live -- --test-threads=1` verifies
eight real WebSocket subscriptions across two local protocol fixture servers,
streamed deltas, retaining one connection while cancelling seven, stopping on
hide, and the existing resume/reconnect behavior. Live multi-host performance still needs evaluation with real workloads;
fixtures do not establish throughput or reconnect timing on eight remote hosts.
