# Embedded terminal panel

The chat header and terminal shortcut open the Terminal side-panel tab. Each conversation retains its own terminal tab groups, active pane and split direction. The renderer attaches to the selected server's PTY; the desktop never starts a local shell for a remote conversation.

`src/terminal_surface.rs` captures the environment, thread and terminal identifiers for each view. It forwards UTF-8 input, native grid changes, clear and explicit close through typed terminal RPCs. A snapshot replaces the displayed history, replayed output at or before its sequence is ignored, and reconnecting resumes the same terminal without repeating code-block commands. Detaching or hiding the app leaves the server process running. Explicit close requests history deletion, matching the native terminal action.

`T3TerminalPanel.qml` and `T3TerminalPane.qml` provide terminal tabs, horizontal/vertical splits, selection, copy/paste, clear, explicit restart and close. Native keybinding context follows terminal focus, including Ctrl+J toggle and Ctrl+D split. Read-only environments cannot send terminal mutations. Layout persistence contains identifiers only, with a bounded size; terminal history remains on the server.

The native Qt plugin is loaded through `O3.Terminal 1.0`. It uses Ghostty revision `9f62873bf195e4d8a762d768a1405a5f2f7b1697`, the engine pinned by the reference web terminal. The launcher adds the native module directory to its import path because Quickshell resource URLs do not resolve a relative native plugin. Standard builds invoke `tools/build-terminal-surface`; release overlays include the plugin, module descriptor, engine revision and upstream license. The installer verifies runtime libraries and retains compatibility with source releases predating this surface.

## Verification

The `embedded_terminal` Rust integration fixture runs the actual desktop worker against a synthetic remote HTTP/WebSocket server. It covers captured destinations, UTF-8/control input, sequence deduplication, resize, clear, reconnect, one-time command execution, detach and explicit close. Unit tests cover target/grid bounds, view cancellation and private layout projection.

`python3 tools/test-device-panel-container embedded-terminal` exercises the actual backend singleton, terminal component and native renderer. `native-terminal` exercises the full application header, keyboard toggle, focus/split routing, draft retention, per-conversation layouts and stale-event rejection. `side-panel` covers the device route as a regression. The full application fixtures use a private Sway headless/pixman compositor; the smaller components use a private software X display. All run in rootless containers with read-only source, no network, private HOME/XDG directories and no host display access.

Build the optional Wayland test image with `podman build -t localhost/o3-qml-wayland-tests:local -f tests/containers/qml-wayland.Containerfile tests/containers`. The runtime has the compositor's file capabilities removed so test containers retain `--cap-drop=ALL` and `no-new-privileges`.

These checks are synthetic and do not establish installed application acceptance. Structured terminal selections in the composer, terminal picker/search, accessibility and complete native menu/link behavior remain separate source-parity work. The shared engine and working VT display do not by themselves establish every surrounding native terminal function.
