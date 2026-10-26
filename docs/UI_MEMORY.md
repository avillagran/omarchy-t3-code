# QML memory work — 2026-10-26

## Changes

- Sidebar rows create snooze/context menus and action dialogs on demand. A
  30-second idle timer can unload them; it preserves open dialogs and in-flight
  management operations. Hover cards remain independent. Lazy loaders occupy
  no input surface; popups attach to the real visual parent when loaded.
- Usage, scheduled tasks, agent-session import and process diagnostics are
  created on first use. Usage and diagnostics unload after 30 seconds closed.
  Usage retains period/metric; forms retain their controls and unsaved text.
- Video playback has its own dynamically loaded component. Text/image views no
  longer instantiate a MediaPlayer, audio output or video sink. Closing a video
  destroys playback resources; reopening reconstructs the viewer.
- Drafts, thread data, connection polling and the native Rust service are unchanged.

## Reproducible measurements

Run on Omarchy/aarch64 with the same 24-thread workload and 1200×900 window:

```sh
./tools/measure-sidebar-memory --panel --revision 1f29b2f
./tools/measure-sidebar-memory --panel
```

The helper replaces only T3ThreadRow/T3Panel from the requested Git revision.
Other files remain current. It isolates HOME/config/state and uses an empty
fixture environment list, so it never connects to accounts or T3 servers.
The complete panel workload has no conversation history. Row-only mode is the
same command without `--panel`. Both modes sample the actual Quickshell PID.

Full-panel results, KiB (baseline `1f29b2f`, candidate working tree):

| Run | Version | RSS | PSS | SwapPSS |
| --- | --- | ---: | ---: | ---: |
| 1 | Baseline | 317728 | 228384 | 0 |
| 1 | Candidate | 269440 | 186730 | 0 |
| 2, reversed order | Baseline | 317824 | 212530 | 0 |
| 2, reversed order | Candidate | 232912 | 148414 | 25840 |
| 3, final input-surface fix | Baseline | 324384 | 227472 | 0 |
| 3, final input-surface fix | Candidate | 267376 | 175389 | 0 |

PSS + SwapPSS decreased by **40.68 MiB**, **37.38 MiB** and **50.86 MiB**. This sum avoids
mistaking swapped-out memory for freed allocation. RSS/PSS also vary with shared
libraries, graphics resources and concurrent memory pressure; these are focused
workload results, not a universal application-memory guarantee.

A row-only repeat without swap decreased RSS 259216→218688 KiB and PSS
169361→129855 KiB. This supports reducing per-row controls as a useful change.

The installed app, with 231 real threads and a live conversation, sampled
370656 KiB RSS / 306040 KiB PSS / 0 SwapPSS. Its selection and content changed
while the user worked, so this is **not** a controlled before/after comparison.
A later sample on the longer Quick Fixes conversation was 418880 KiB RSS /
353466 KiB PSS / 0 SwapPSS. No FFmpeg media plugin was mapped in either
text-only startup. Live memory still reaches roughly 400 MiB on this chat.

The original **50–100 MB total budget is not achieved**. Remaining work needs
heap/object/render-resource profiling against realistic long chats and mosaics.

## Validation

Focused QML checks: thread-menu (lazy creation, unload/recreate, open-editor
protection, recycled target), thread-hover, row-click, usage-page (unload and
period/metric restoration), scheduled-tasks (unsaved form retained), live-media,
video-lifecycle (local MPEG-4 playback/pause/seek/teardown/reopen), mosaic-sidebar,
connection-feedback-motion, preferences, sidebar-navigation and panel-reopen.

A mosaic repeat encountered a QtTest click failure during fixture startup; the
fixture now waits for its actual window/rendering and reports target geometry
on failure. The repeated check passed. A separate hover regression was caught:
empty lazy loaders must not cover a row or window. They now have no input
surface, and loaded popups receive the intended visual parent. Hover/actions
then passed, alongside the baseline comparison. The failing fixture exit was
also deferred to avoid aborting Quickshell inside its own QML signal handler. No Rust source changes or release build
are required for this QML-only change.

## Sidebar shortcuts included in this pass

- Ctrl+B: show/hide the sidebar, including while editor text is selected.
- Super+B: reveal/focus the active chat, open its shelf/parent chain and scroll
  it into view. Explicit reveal clears only filters that exclude the active row.
- Arrow keys then move through the sidebar; focusing alone never switches chat.
- Ctrl+K retains its existing sidebar-focus fallback when the native palette
  does not claim it. Help and palette labels reflect the new B shortcuts.
- QML sidebar-keybindings covers real key events, selected text, scroll position,
  arrows, collapsed Working/Settled shelves, hidden sidebar and a filtered child.
  sidebar-navigation, keybindings-runtime, composer-enter and composer also pass.
