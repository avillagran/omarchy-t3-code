# Native terminal

Use the terminal icon in the chat header or **Ctrl+J** to open the embedded
terminal in O3’s side panel. The chat, composer and application window remain
available. **Ctrl+D** splits a focused terminal. Tabs and horizontal/vertical
splits retain their layout per thread. The renderer uses the exact pinned native
Ghostty engine rather than a terminal approximation; see
[terminal panel implementation and validation](TERMINAL_PANEL.md).

Commands run in the selected T3 environment and the thread’s project or worktree.
Remote environments use the existing T3 connection. No separate SSH host,
password or terminal configuration is required. Selection, copy/paste, PRIMARY
selection, scrollback and VT input stay inside the embedded terminal. **Add
selection to prompt** creates a native structured context record and inline
reference; drafts, send/retry, queue editing, recall and stash retain its text.

Closing a panel detaches the frontend and preserves the server shell and jobs.
The panel restores terminal output after attaching again; explicit restart is
available for an exited shell. Input is never replayed automatically after an
uncertain write. Read-only connections cannot mutate terminals. Existing
connection grants must include the native terminal scopes; credentials remain
in Rust’s connection and never enter terminal launcher arguments.

## Implementation and remaining fidelity work

`src/terminal_surface.rs` owns native terminal session RPCs and subscription
lifetime. `native/terminal/terminalsurface.cpp` renders and encodes through the
pinned Ghostty C ABI. `T3TerminalPanel.qml` and `T3TerminalPane.qml` supply the
side-panel interaction, theme and session layout. The legacy external frontend
in `src/terminal.rs` remains available for existing explicit CLI actions;
it is no longer the chat-header implementation.

The terminal engine suite covers 52 Qt cases, including 40 native engine oracle
cases, with 20 separate native terminal-context producer cases. Private Qt interaction checks exercise the
actual chat header, renderer, focus, Ctrl+J/Ctrl+D, draft retention and selection
attachment. Rust checks cover the real terminal RPC and context persistence
paths. These checks run with isolated profiles and displays, without live user
projects. Source-derived link matching has 219 helper oracle calls and 273
actual viewport cell checks. Another 22 isolated routing checks and an actual
pane/viewport QML check cover URL/browser preferences, editor positions, hover
admission, selection, modifiers and stale-thread replies. Plain first left click
activates a link without changing selection; Ctrl or Meta forces the system
browser for URLs. The generic Ctrl/Meta-only activation helper belongs to the
markdown consumer, not the terminal surface.

Resolved UNC/network paths are rejected before requesting metadata. Path actions
require current thread and connection permissions. The first wrapper wiring uses
a bounded 60-second metadata cache and rechecks metadata before opening an editor.
This is a temporary fidelity gap: native positive metadata remains retained for
the mounted thread and RPC session, including after its global cache TTL. Exact
retention, invalidation and mounted-session identity remain to be ported. The terminal frontend stays mounted when switching between side-panel tool tabs,
preserving its stream, renderer and selection while an app-browser link opens.
Closing the side panel still detaches it. Search,
accessibility and native picker/importer details are still being ported. The resource and importer limits in [TERMINAL_PANEL.md](TERMINAL_PANEL.md)
remain explicit.

## Run a command from a message

Completed shell code blocks expose **Run in terminal** and **Copy code** in their
header. Running requires a click; receiving or rendering a message never runs
anything. As in T3 Code, only a closed, single-line shell fence without control
or invisible formatting characters offers the run action. Streaming, incomplete
or multiline blocks remain readable and copyable.

A code action opens a fresh embedded terminal tab in the current chat's
environment and project/worktree directory. The command travels through the
existing private desktop worker IPC and is written once as `command + "\r"`
after the initial live terminal snapshot. An uncertain write is never replayed
automatically. The chat header and Ctrl+J use the same embedded panel.

Native T3 code actions target the drawer and may reuse an idle drawer-owned
terminal, excluding terminals owned by the side panel. O3 currently exposes the
embedded side-panel ownership path and creates a fresh terminal for code actions;
separate drawer ownership and eligible idle-drawer reuse remain fidelity work.
The legacy external frontend is still an explicit CLI path. Closing either
frontend detaches without killing its server shell.
