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

## Allocator profiling and tuning

The installed Quickshell links jemalloc 5.4. The existing glibc arena/trim
settings therefore helped the Rust helper, but did not tune the Qt UI. A local
mallctl probe measured about 98 MB allocated versus 198 MB allocator-resident
memory. Qt's separate JS heap reserved about 13 MB; an explicit collection
freed only about 1.4 MB, so no periodic full GC was added. A heaptrack sample
reported a 57 MB peak tracked malloc heap; that excludes QV4 mmap storage and
changes allocator behavior, so it is not a total-memory figure.

`bin/ot3` now supplies app-local jemalloc defaults: two arenas, background page
purging, one-second dirty/muzzy decay and an 8 KiB maximum thread-cache size.
Explicit inherited `MALLOC_CONF` entries are appended last and override these
defaults. No preload, allocator replacement or new runtime dependency is used.
The existing glibc settings remain for the native helper. GTK platform-theme
replacement was also measured, but was not shipped for its small extra gain.
Allocator options: [jemalloc manual](https://jemalloc.net/jemalloc.3.html).
JS accounting: [Qt memory management](https://doc.qt.io/qt-6/qtqml-javascript-memory.html).

The benchmark now accepts a deterministic conversation and records CPU time,
elapsed time and top RSS mappings. Run each command separately, repeat in reverse
order, and compare PSS **plus** SwapPSS:

```sh
env -u MALLOC_CONF ./tools/measure-sidebar-memory --panel --messages 50
MALLOC_CONF='narenas:2,background_thread:true,dirty_decay_ms:1000,muzzy_decay_ms:1000,tcache_max:8192' ./tools/measure-sidebar-memory --panel --messages 50
```

The fixture has 24 threads, 50 alternating messages, 10 paragraphs per message,
disabled animations and a 1200×900 window. Launcher settings are intentionally
not applied implicitly by the benchmark; `--revision` changes QML only.

| Run | Allocator | RSS KiB | PSS KiB | SwapPSS KiB | CPU ms | Elapsed ms |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| 1 | Default | 285696 | 234330 | 0 | 1500 | 4971 |
| 1 | Tuned | 249888 | 197701 | 0 | 1490 | 4996 |
| 2 | Default | 242544 | 191370 | 11616 | 2200 | 5695 |
| 2 | Tuned | 251024 | 199741 | 0 | 1620 | 5027 |

Savings were **35.8 MiB** and **3.2 MiB** PSS+SwapPSS. System memory pressure
caused substantial variance; these results support a modest allocator change,
not a universal 36 MiB saving or a claim that RSS alone always falls.

Installed runtime verified `MALLOC_CONF` in `/proc/PID/environ`, both hosts
ready, `stale=false`, and the same selected thread immediately after restart.
Before restart the UI sampled 268032 KiB RSS / 228770 PSS / 126448 SwapPSS;
after restart 371184 RSS / 312538 PSS / 0 SwapPSS. This is **not a controlled
allocator comparison**: restart, history pagination and warm-up differ. The
current UI still exceeds the original budget substantially.

Validation: Bash syntax, benchmark Python compilation, launcher defaults and
explicit-user-override checks, QML sidebar-keybindings, mosaic-sidebar, and panel-reopen.
The first panel-reopen run hit its known collapsed-state timing failure;
an untuned control and a tuned repeat both passed all ten cycles.
No Rust rebuild was needed. Local launcher backup:
`~/.cache/ot3-jemalloc-20261026-210458`.

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

## Deferred review controls

Diff review, working-tree review, Git controls and tool inspection now create
controls only on first opening. Diff/working review and the tool inspector
release their controls after 30 seconds closed. Open views cannot release;
reopening reconstructs the current scope. Diff context/tree presentation choices
survive unloading. Tool replies are forwarded only to a mounted inspector,
which still validates its request and scope. Git controls remain mounted after
first use, retaining the existing tab and lifecycle behavior. Existing native
scope/readiness invalidation still clears pending Git confirmations and text;
this change does not add another unload/discard path for that form.

A proposed QObject-backed conversation-row model was discarded: the isolated
comparison without overlapping test processes did not reduce anonymous memory
(179824→180960 KiB). Initial apparent gains were not repeatable. The current
conversation model, streaming behavior and delegate reuse policy remain intact.
The measurement helper now also reports PSS anonymous/file/shared components.

Final comparison: same 24-thread, 250-message, 40-paragraph fixture, jemalloc
settings from the preceding section, baseline `216b025`. No concurrent test
Quickshell process was run during these paired measurements.

| Order | Version | RSS KiB | PSS KiB | SwapPSS KiB | PSS anon KiB | CPU ms |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| Baseline first | Baseline | 315168 | 255533 | 0 | 179568 | 1840 |
| Baseline first | Candidate | 309376 | 249829 | 0 | 173696 | 2070 |
| Candidate first | Candidate | 309632 | 250254 | 0 | 173952 | 1750 |
| Candidate first | Baseline | 315344 | 254917 | 0 | 179536 | 1790 |

This saves **5.57 MiB** and **4.55 MiB** PSS+SwapPSS before those views are used.
CPU samples vary; no general CPU improvement is claimed. Visited Git controls
remain allocated; the other three views can return to their unloaded state.
The live UI after installation sampled 369616 KiB RSS / 313098 PSS /
2704 SwapPSS on the user's current chat. This is a state check, not a controlled
before/after comparison; the total 50–100 MB target is still open.

Focused checks passed: work-details (cold controls, guarded unload, diff reload,
presentation restoration and late replies), activity-inspector (actual row
click, scoped replies, close on thread switch, unload and recreation),
tool-inspector, Git flow (same controls/tab after reopening), working-review
flow, native-diff flow, sidebar-keybindings and scroll-stability. No Rust rebuild.
Installed T3Panel was merged against its prior base to preserve local activity
announcements and recent-thread palette changes. Backup:
`~/.cache/ot3-lazy-reviews-20261026-212042`.
Both hosts ready and selected thread identical after restart. Desktop capture:
`~/Pictures/ot3-evidence/deferred-reviews-20261026.png`.
