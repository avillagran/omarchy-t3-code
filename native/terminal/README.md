# Native terminal surface

`O3.Terminal 1.0 / TerminalSurface` embeds the same `libghostty-vt` revision as
T3 nightly 4262 and 4275: `9f62873bf195e4d8a762d768a1405a5f2f7b1697`.
The QML `engineRevision` property reports that pin. The plugin links Ghostty's
PIC static library and has no libvterm runtime dependency. `intermediate/`
retains the earlier libvterm implementation and its tests for reference; the
standard build does not compile it.

The surface parses terminal output, paints cells and encodes input. It never
opens a shell, owns a PTY, reads profiles, accesses a clipboard or opens links.
Rust owns terminal RPCs and process lifetime. QML routes `input`, `gridChanged`,
`copyRequested`, `pasteRequested`, `primaryPasteRequested` and `linkActivated`
to their existing backend operations.

## Build

Run `tools/build-terminal-surface`, or the standard `bin/build-client`.
`tools/build-ghostty-vt` fetches the exact upstream commit, verifies a clean
checkout and Zig 0.15.2, verifies the official compiler archive's size and
SHA-256 before extraction, and builds the native SDK. Builds use at most two
jobs and the shared `/var/tmp/o3-build/o3-parity-compile.lock`. The current
bootstrap supports x86_64 and aarch64 Linux using the official architecture-specific
compiler archives and checksums. The source, compiler and SDK stay under
`/var/tmp/o3-build`; `O3_GHOSTTY_BUILD_ROOT` can select another build cache.

The plugin is installed atomically into `bin/qml/O3/Terminal/`, together with
`qmldir`, `GHOSTTY_VERSION` and the upstream `Ghostty-LICENSE`. The launcher
adds `bin/qml` to `QML_IMPORT_PATH`. Release packaging includes this directory.

## Surface API

The surface exposes columns/rows, font family/size, foreground/background and
selection colors, selected text, selection presence, scroll offset/history,
title, cursor coordinates and alternate-screen state. `selectionRange` reads
Ghostty's tracked selection references, converts them to full-screen coordinates
including scrollback, and returns one-based `lineStart`/`lineEnd` plus zero-based
`startColumn`/`endColumn`. An invalid selection returns an empty map.

`feed(QString)` accepts decoded UTF-8 RPC output; `feedBytes(QByteArray)` also
accepts split byte sequences. `setGrid` uses the server's 1..1000 columns and
1..500 rows limits. `reset`, `clearSelection`, `copySelection`, `selectAll`,
`paste`, `selectRange` and `hyperlinkAt` expose terminal operations without
process or clipboard ownership. `rowText` and `cellAt` support inspection and
tests. The selection's end column in `selectRange` is exclusive.

Ghostty handles the VT parser, alternate screens, reflow, modes, graphemes,
wide cells, selections, key press/repeat/release encoding, mouse protocols and
paste filtering. Qt paints styled text runs, powerline triangles, decorations,
selection and cursors using T3's renderer rules. Keyboard shortcuts, history
scrolling, mouse/word/line selection, tracked drag anchors and selection
autoscroll are handled in the surface. Synchronized output preserves the last
complete frame and uses T3's 500 ms deadline for unterminated groups.

## Verification

Run `tools/test-terminal-surface`. Compilation happens on the host; all test
bodies run in rootless Podman with read-only source/binaries, private tmpfs
HOME/runtime, no network, dropped capabilities and a separate evidence mount.
No test owns a shell, host profile, device or clipboard.

`tools/generate-terminal-fixtures.mjs` executes the complete pinned T3
`core.ts`, `runtime.ts` and `keyCodes.ts` with the bundled Ghostty WASM. Only
TypeScript erasure and file import relocation are applied. The exact shifted-key
constant is taken from the native shared module. The generated fixture records
source and WASM SHA-256 values. Use `tools/test-terminal-surface --oracle-source /path/to/t3code-4262` to regenerate
and execute the oracle in the same rootless container isolation. The checked-in fixture covers cells/styles, Unicode and
oversized graphemes, ANSI/OSC, alternate screens, selection/history, legacy and
Kitty keyboard press/release, paste and mouse protocols. Qt tests also exercise
actual QML imports, painting, selection gestures and synchronized frames.

Ghostty's C header describes `max_scrollback` as lines, but the pinned
implementation (`Screen.zig`, `PageList.zig`) treats it as a byte budget rounded
to native terminal pages. We pass exactly 10,000, as T3's core does. Native
64-bit and WASM32 page layouts retain different row counts after pruning.
Tests compare visible state and selection and report that allocation difference
explicitly; they do not claim identical internal row capacity across targets.
