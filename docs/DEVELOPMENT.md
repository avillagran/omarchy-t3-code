# Development and advanced setup

[Back to the project overview](../README.md).

## Requirements

- Linux with **Omarchy's Quickshell shell and plugin manager**. This is not a
  standalone application or a Waybar module.
- T3 Code installed and initialized, with a running server and its connection
  catalog. The default data directory is `~/.t3`; `T3CODE_HOME` overrides it.
- Rust/Cargo, a C/C++ toolchain and Git to build from source.
- `secret-tool`/libsecret for the encrypted Linux connection catalog; OpenSSH for
  SSH connections. `wl-clipboard` supports clipboard integration.
- Optional: Qt Quick and Qt WebEngine development packages, `pkg-config`, and
  Qt's `moc` for interactive HTML reports. The build detects these packages.
- Node.js18+ for collecting the exact application license notices during source
  builds, translation checks and the HTML renderer test. Python3 converts the
  compiler-provided HTML copyright summary to plain notice text at build time
  and runs reference/asset tools; neither is needed by the backend at runtime.

Build on the target architecture. Generated binaries are not committed.
Development has been exercised on Linux ARM64 against T3 nightly versions;
compatibility with every T3 release is not guaranteed.

## Architecture builds

The installer downloads the latest published release for aarch64 or x86_64.
It checks SHA-256 and the source commit before installing the Rust client and Qt
HTML frontend. Runtime libraries must match the Arch Linux build; missing
libraries cause installation to fail with a source-build option, never a silent
compiler installation. Keep Omarchy up to date.

To build main locally through the installer, use:

```sh
curl -fsSL https://raw.githubusercontent.com/avillagran/omarchy-t3-code/main/install.sh | OT3_BUILD_FROM_SOURCE=1 bash
```

For a release, commit and push the source, then run
`CARGO_BUILD_JOBS=2 tools/package-binaries` natively on each architecture.
mbp-intel provides the x86_64 builder; ARM64 builds run on the development machine.
Packages include the Rust client, document and browser Qt libraries, and the original browser secret helper. Manifest/Cargo versions, `--version`, archive filenames and `BUILD-INFO` carry the same release version. Create an annotated `vVERSION` tag for the source commit.
Write the release notes in `docs/releases/vVERSION.md` (what changed for users,
upgrade notes and the archive checksums). Gather both versioned archives and
their checksum files into `dist/`, then run `tools/publish-release vVERSION`;
it refuses to publish without the notes. The publisher refuses stale packages and
publishes a draft only after both matching architecture assets are uploaded.
QML-only changes still need a release for installer updates, but Rust binaries
need rebuilding only when their sources/dependencies change (the packaging tool
uses Cargo's incremental cache for consistency).
Generated binaries are excluded from Git. Packages target Omarchy on Arch Linux,
not arbitrary Linux distributions. The portable Omi library remains a separate
host-platform build; these desktop packages do not replace an Android build.

## Automated installer

The root [installer](../install.sh) installs runtime dependencies and prebuilt binaries, enables the widget and starts the local service only if stopped.
It adds the keyboard integration to an existing Lua bindings file after making
a backup: Super+Shift+A replaces ChatGPT/earlier Ares bindings with this client,
and Enter sends inside ot3 without changing the terminal shortcut. Repeated runs update a clean checkout; local changes are never reset.
T3 Code itself must already be installed and initialized. The installer is not
run as root; only Pacman uses sudo.

## Manual source installation

Clone directly into the user plugin directory and build before enabling:

```sh
git clone https://github.com/avillagran/omarchy-t3-code.git \
  ~/.config/omarchy/plugins/omarchy-t3-code
cd ~/.config/omarchy/plugins/omarchy-t3-code

bin/build-client
omarchy plugin validate .
omarchy plugin enable io.github.avillagran.omarchy-t3-code
omarchy restart shell
```

Press Super+Shift+A, run `bin/ot3` or click the T3 icon in the status bar to show the app. Configure local and remote
connections in the official T3 client; this plugin inherits them. It does not
maintain a separate server list or ask you to paste provider tokens.

To update a source installation:

```sh
cd ~/.config/omarchy/plugins/omarchy-t3-code
git pull --ff-only
bin/build-client
omarchy restart shell
```

Save any active draft before restarting the shell. Existing drafts and UI
preferences are stored outside the plugin checkout.

### Default agent and send shortcuts on Omarchy

Omarchy normally reserves Super+Enter for the terminal. On installations using
Lua Hyprland bindings, add this line to `~/.config/hypr/bindings.lua`:

```lua
dofile(os.getenv("HOME") .. "/.config/omarchy/plugins/omarchy-t3-code/integrations/hyprland/bindings.lua")
```

Reload your Hyprland configuration afterwards. Super+Shift+A opens this plugin
in place of the default agent shortcut. Enter sends inside ot3; the
Super+Enter terminal binding is untouched. Review the
binding file before enabling it if you already customize these shortcuts. With a
manual installation, the binding is not installed automatically.

## Local and remote connections

T3 remains the source of truth for local runtime discovery, remote environments,
connection names, credentials and model configuration. The Rust adapter reads
T3's runtime and encrypted catalog and uses its native integration credentials.
SSH connections reuse your existing SSH configuration, agent and known hosts.
T3 Connect/relay authorization follows the account and environment permissions
already granted in T3.

**Closing the official GUI only saves resources safely if its server is running
independently.** A desktop-owned server stops with that desktop app.
The sidebar footer detects who owns the local server. A desktop-managed server
shows a GUI indicator and hides lifecycle controls. When stopped, the play button
installs or starts T3's native headless user service. A headless server exposes
Stop and Restart in its control menu. Verified manually launched headless servers
can be stopped gracefully; restarting them adopts the native service. The plugin
rechecks ownership before each operation and never stops a GUI-owned or
unidentified process. It does not migrate an active desktop session or start a
competing server over the same data directory.

Starting waits for T3's native readiness check and reports failures in the panel
and Connections. If the GUI has removed its runtime file while the native service
is still active, starting restarts that profile's service so it can publish a new
runtime; systemd's successful acknowledgement alone does not count as success.
Lifecycle actions run independently of remote snapshot queries.

SSH startup uses T3's native `__ssh-helper runtime-port` and `wait-ready` checks.
A stopped remote server starts through its existing `t3code.service`; if absent,
T3 installs its own service. The adapter verifies the saved environment ID and
service data directory before starting it. Existing servers are reused without
restart. Healthy SSH tunnels survive local runtime changes; failed connections
expire early for the next refresh. This requires an installed native T3 CLI and
working noninteractive SSH authentication on the inherited target.

### Live synchronization

Each environment has one `orchestration.subscribeShell` stream. Its items are
applied in memory (`src/shell_state.rs`, after T3's shell reducer) and the
sidebar is rebuilt from that shell; `/api/orchestration/shell` is read only for
environments whose stream is down. A reconnect resumes with `afterSequence`
(T3 replays up to 1000 events, otherwise sends a fresh snapshot). The visible
chat resumes the same way from the envelope it already holds. Completion
markers are requested only when `server.getConfig` advertises
`shellResumeCompletionMarker`/`threadResumeCompletionMarker`.

Streams ping every 5 s and drop a socket after 20 s of silence. Retries back
off from 1 s to 5 min with jitter and start over after 30 s connected
(`src/reconnect.rs`). Resuming from suspend (logind `PrepareForSleep`) or a
NetworkManager connectivity change wakes every stream: open sockets must answer
a ping within 3 s, waiting ones reconnect after a quick probe. Signals are read
with `gdbus monitor` (glib2); without it, resume is detected from the
boot-time/monotonic clock gap. Stream failures and wakeups are written to the
client's standard error, once per change of error.

Preferences write only an allowlisted subset of native settings via
`server.updateSettings`, then reload `server.getSettings`. The shared
`t3-protocol::settings` module validates values and exports safe field descriptors
without provider credentials. General settings apply to the selected local or
remote server; appearance follows Omarchy, and favorites use T3's existing store.
Source Control includes native branch naming/prefix/instructions, writing style
and pull request templates, merge method, worktree options and review guide
preferences. Text edits require an explicit Save; staged edits survive unrelated
settings responses and failed writes until the preferences view is closed or its
environment is changed. General also exposes nullable inactivity days for
auto-settle and automatic resume after rate limits.
Connection management remains owned by T3's catalog.

Preferences temporarily replace the chat sidebar with their own category
navigation, then restore the original chat layout on return. Sections follow
T3's grouped cards with muted headings and divided rows; switches and inputs use
Omarchy colors. The centered content column adapts to narrow windows. Search
filters the supported native setting fields across categories (translated labels
and English source labels). Hidden sections unload their content. The design
check renders real QML screenshots to `/tmp/t3-preferences-*.png` and explicitly
sets the narrow content viewport so compositor tiling cannot mask layout bugs.

## Everyday use

- **Super+Shift+A** opens Omarchy T3 Code as the default agent.
- **Enter** submits the prompt in the normal composer. **Shift+Enter** inserts a
  new line and continues supported lists. The expanded editor keeps Enter for
  new lines; **Ctrl+Enter** sends in either mode. Other shortcuts follow the panel's keyboard help; system
  bindings can affect availability.
- Select a model through the native-style picker; favorites come from T3.
- Use `@` for file/folder completion, `$` for skills and `/` for supported commands.
- Right-click a chat row for thread actions, or its project name/initials for
  project actions and settings.
- Open usage from the header button. Bars appear below the conversation title.
  Missing quota windows are omitted rather than displayed as zero.
- Open Pull Requests from the sidebar header. Select a chat to return.
- Click a file reference or attachment to preview it inside the panel.
- Scrolling back preserves your reading position; returning to the end or
  sending a prompt resumes following new content.

Optional preventive compaction can request one compaction per inactivity period
before estimated cache expiry. Review its settings: provider cache lifetimes
vary, estimates can be wrong, and compaction consumes quota even if you do not
return to the thread. It requires the plugin and T3 server to remain running.

## Desktop app and bar widget

Omarchy T3 Code runs as its own Quickshell instance, not inside the Omarchy
shell. `bin/ot3` builds a small config folder in
`$XDG_STATE_HOME/omarchy-t3-code/app` (links to Omarchy's `Commons` and `Ui`
and to this installation's `ui`, `bin`, `translations`, `licenses`, `data` and `upstream`,
plus `app/shell.qml`) and starts it as the transient user service
`omarchy-t3-code`, so neither a stuck bar nor a shell restart takes it down.
The window has the app id `io.github.avillagran.omarchy-t3-code`.

```bash
bin/ot3            # toggle: show, focus, or hide when focused
bin/ot3 start      # start hidden (login autostart and the bar widget)
bin/ot3 hide|quit|restart|status
bin/ot3 install-desktop   # launcher entry, autostart, ~/.local/bin/ot3
```

The app writes its status (counts, shown, offline, OmaBridge, a translated
tooltip) to `$XDG_RUNTIME_DIR/omarchy-t3-code/status.json`. The bar widget
(`ui/qml/BarWidget.qml`, the plugin entry point) only reads that file and runs
`bin/ot3`; it loads no backend or translations. A status older than 90 s reads
as a stopped app.

The header cycles panel → floating → tiled. Panel mode shows the app under the
bar's T3 button as the in-shell popup did: the bar widget sends the button's
place (`ot3 anchor JSON`, kept per screen in `$XDG_RUNTIME_DIR/omarchy-t3-code/anchors.json`)
and the app draws a transparent, click-through twin of that bar to anchor the
popup. Without the bar widget there is no panel mode, only windows. Closing through the
compositor or the header's – button hides the window; the backend keeps
running, so subscriptions, notifications and drafts continue.

## Keeping up with T3 Code nightly

ot3 is validated against one T3 Code revision, recorded in `upstream/t3code.json`;
every generator in `tools/` reads it from there. `tools/upstream-drift.cjs`
compares that revision with a newer checkout (default `../t3code-nightly`) and
reports what ot3 does not cover: RPC methods, orchestration command/event types,
HTTP endpoints, client/server settings and keybinding commands added or removed,
removed items ot3 still uses (breaking), stale generated tables, and the commits
grouped by the part of ot3 they touch.

```bash
git -C ../t3code-nightly pull --ff-only
node tools/upstream-drift.cjs --write          # upstream/DRIFT.md
node tools/upstream-drift.cjs --list rpc        # one surface with ot3's coverage
node tools/upstream-drift.cjs --check           # inventory covers the pin; supported items still referenced
node tools/upstream-drift.cjs --sync            # add the new items to upstream/parity.json, unreviewed
node tools/upstream-drift.cjs --pin             # move the pin once every item is in the inventory
```

`upstream/parity.json` records each item as `supported`, `partial`, `missing` or
`ignored` (with a note); new items enter with `reviewed: false` until someone
decides. Moving the pin requires regenerating the tables (`tools/build-*.cjs`)
with the nightly checkout at the new revision. `node tools/test-upstream-drift.cjs`
tests the tool against a fixture repository. `tools/upstream-watch` fetches the nightly
tags (without moving the checkout), compares the newest one and prints a
summary; a T3 scheduled task runs it daily and leaves the summary in a thread.
Preferences → General → T3 Code compatibility and the sidebar's environment list
show each server's T3 version and mark the ones newer than the pin.

## Architecture and resource use

```text
Quickshell / QML interface
        │ JSONL over stdin/stdout
        ▼
Persistent Rust desktop adapter
        │
        ├── t3-protocol: native HTTP / RPC / live events / signed assets
        │        └── T3 local server or remote environment
        ├── Linux integration: keyring, SSH, drafts and notifications
        └── On-demand HTML frontend → frames inside the QML preview
```

The visible chat subscribes to native events. Other thread states use adaptive
polling, and provider usage has its own slower refresh cycle. Rendering reuses
unchanged content; animations stop when hidden. There is no persistent Node.js,
Python or Electron runtime in the plugin backend.

Interactive HTML uses an isolated, on-demand Qt WebEngine process with a small
C++ frontend bridge. Its frames and input stay inside the QML modal; it creates
no visible desktop window. Closing the preview, switching chats or hiding the
panel releases that renderer and its private frame cache. Text and raster-image
previews do not need the browser renderer. Reports inherit the Omarchy palette
when opened, run inside an opaque sandbox, and cannot access app credentials or
local files through the document.

QML previews (`qml` code blocks) use the same frame transport with a smaller
Qt Quick-only renderer (`bin/libt3-qml-preview.so`, no WebEngine). It runs with
an empty environment, `nice` 10, a 512 MiB data limit (`RLIMIT_DATA`) and a
Landlock domain, imports only QtQuick, QtQuick.Controls, QtQuick.Layouts,
QtQuick.Shapes, QtQuick.Effects and QtQml, and exits with
`qml_preview_unresponsive` when a component blocks its event loop for 4 s.
Animated components are captured at most 20 times a second as uncompressed BMP
in `$XDG_RUNTIME_DIR` (PNG encoding cost ~13 ms per frame and dominated the
renderer's CPU). Measured on an ARM laptop at 730×330: first frame 50–150 ms,
25–70 MB PSS (WebEngine pages: ~150–240 MB over five processes), no CPU when
static; a looping animation costs ~2% in the renderer and ~9% in the shell.
`tests/qml_preview_renderer.rs` keeps these limits; `tests/qml_preview.rs`
covers the import checks and the sandbox.

Every frame the panel presents is recomposed by the compositor, so continuous
motion shares one clock (`T3Ticker`) and each animation asks for the pace it
needs: an idle sky drifts at 8 fps, the busy sky at 30, spinners, the working
shimmer and thread comets at 60, and shooting stars at the display rate while
one is on screen. Preferences › Animations › Animation smoothness switches
between adaptive, saver (30 fps) and maximum. Live transcript rows carry a
content revision (`rev`) so streamed updates only touch the rows that changed,
and the detached window uses an opaque surface when the theme background is
opaque.

To measure on a running panel, enable Preferences › Animations › Frame rate
meter, or use the diagnostics IPC:

```bash
bin/ot3 call profileScroll  # 4 s of touchpad-style scrolling
bin/ot3 call profileResult  # fps and frame interval percentiles
bin/ot3 call skyState       # sky mode, meteor and clock pace
bin/ot3 call sidePanel devices  # side panel tab: browser, renders, devices ("" closes)
```

Resource consumption depends on transcript size, attachments, the shell and
optional previews. The T3 server and provider workers still use resources.
No fixed memory saving is promised, and an active HTML renderer adds browser
process overhead.

### Shared library for Omi

[`lib-t3-rust`](https://github.com/avillagran/lib-t3-rust) is the independent source
of the portable Rust transport, authentication and protocol logic, including a C ABI for a JNI host.
It does not depend on Quickshell, SSH or the Linux keyring. Omi supplies its own
platform integration; the Android application is not built or modified here.
See its [Android/JNI guide](https://github.com/avillagran/lib-t3-rust/blob/main/docs/ANDROID.md)
and the desktop [adapter contract](protocol/t3-direct-v1.json). Portable protocol
tests now run in the library repository; this repository tests desktop integration.
Credentials stay in the host adapter and are not passed to QML.

QML files import `QtQuick 6.11` on purpose. Qt 6.12 added a `QtQuick/Color`
singleton that shadows Omarchy's `qs.Commons` `Color`, which leaves every
theme colour undefined (white surfaces, black text). The versioned import
keeps the theme singleton visible; keep it on new files. `QtQuick.Window`
re-imports the latest QtQuick, so it is pinned to 6.11 as well.

## Project layout

```text
.
├── src/                     # Rust desktop adapter and process entry point
├── ui/
│   ├── qml/                 # Panel, composer, sidebar and shared controls
│   ├── js/                  # UI models, theme helpers and generated icon data
│   ├── assets/              # SVGs, shaders and third-party license notices
│   └── document/            # Optional isolated Qt HTML frontend
├── translations/            # English source, locale batches and runtime catalog
├── integrations/hyprland/    # Default agent desktop binding
├── docs/protocol/            # Shared transport contract and fixtures
├── tests/                   # Rust, isolated QML and legacy reference checks
├── examples/                # Opt-in integration diagnostics
├── tools/                   # Build helpers, generators and validation scripts
└── bin/                     # CLI launcher, build entry point and local binaries
```

`manifest.json` points directly to `ui/qml/BarWidget.qml` (the bar widget);
the app's entry point is `app/shell.qml`, started by `bin/ot3`. Rust follows the
standard Cargo layout; the portable crate is a pinned Git dependency on
lib-t3-rust, shared with Omi. Update its full revision in Cargo.toml and commit
Cargo.lock after validating both repositories. To develop locally without changing
the committed dependency, use Cargo’s command-line patch:

```sh
cargo test --workspace --config 'patch."https://github.com/avillagran/lib-t3-rust".t3-protocol.path="../lib-t3-rust"'
```

A local patch can require a temporary lockfile update; do not commit a path-based
lock entry. Run the normal locked checks again after removing the patch.
Generated binaries, local measurements and development evidence
are excluded from Git. The root `hypr-bindings.lua` is a compatibility entry point
for existing user configurations; new installations use `integrations/hyprland/`.

## Validation

```sh
cargo test --workspace --locked
node tools/check-translations.cjs
node tools/test-installer.cjs
omarchy plugin validate .

# Requires the installed Omarchy shell and a graphical session:
tools/test-qml thread-menu
tools/test-qml content-panels
tools/test-qml documents
tools/test-qml backend-owner
tools/test-qml sidebar-pr
tools/test-qml dpi-lifecycle
tools/test-qml panel-reopen
tools/test-qml window-mode
tools/test-qml preferences
tools/test-qml preferences-source-control
tools/test-qml preferences-design
node tools/test-remote-bootstrap.cjs
tools/test-qml context-divider
tools/test-qml copy-actions
node tools/test-copy.cjs
tools/test-qml upward-scroll

# Requires the optional Qt HTML frontend:
tools/test-document-viewer
T3_DOCUMENT_MIN_FPS=40 tools/test-document-scroll

# Optional user-systemd fixture; never stops the real T3 server:
tools/test-backend-ownership
```

QML checks use isolated fixtures. The `examples/` directory also contains opt-in
integration diagnostics; review their arguments and behavior before running
against your own server. Some diagnostics can create temporary test metadata.

## Current limits

This client is under active development and does not yet reproduce every T3 UI
feature. PR mutations, general workspace editing, PDF previews and HTML sibling
asset bundles are not implemented. HTML capture follows loading and interaction;
it does not continuously stream web animations. Recent-history reads can be
bounded by the server, and some approval flows are not yet supported. Remote
permissions and expired sessions must be resolved through T3 itself.


## Remaining feature gaps

See [the interaction parity audit](FEATURE_PARITY.md).

Sidebar groups keep their expanded/collapsed preference across panel unloading
and grouping toggles. Selection, search and status filters do not force a closed
group open. Working, Snoozed and Settled headers have horizontal separators and
are hidden when their filtered group is empty.

Shelf headers use the native inline geometry: label, horizontal rule, chevron
(`CollapsibleSectionHeader` in T3). Ctrl+K focuses the current card or its closed
shelf header. Up/Down traverse one order across all lists and headers; Left/Right
collapse/expand a focused shelf. Enter/Space toggle a header or open a card.

The timeline extends behind the prompt stack, matching T3 ChatView's composer
overlay. An equal trailing inset keeps the last message above the prompt at the
live edge. The prompt surface stays opaque so scrolling text does not obscure
editing. `tools/test-qml composer-overlay` verifies overlap and end visibility;
`sidebar-navigation` exercises actual Ctrl+K and arrow keys. The longer upward
scroll fixture uses a floating window so desktop interaction cannot dismiss it.

Desktop notifications pass a private 96px PNG to notify-send. Native project
icon overrides take priority, followed by the native signed project-favicon
asset and the bundled T3 logo. Input/waiting/plan, completed/review and failed
notifications add an action badge. Asset downloads are bounded, same-origin and
use no credentials on the signed GET; images are cached privately for one hour
with a 128-file cap. SVG, emoji and monogram rendering uses librsvg; raster icons
include PNG/JPEG/GIF/WebP/ICO support (native T3 extracts ICNS to PNG). The Lucide
0.564.0 atlas matches T3's dependency and includes its license. Regenerate it with
`tools/generate-project-icons` and the official lucide-static npm archive.

Markdown links use inline Nerd Font glyphs, following native `ChatMarkdown`'s
GitHub brand treatment (including branch/commit/pull URLs), file references and
web fallback. Decoration is view-only: raw Markdown, full-message copy and search
metadata remain unchanged. Unlike native remote favicons, generic web links use
an offline globe. `tools/test-qml markdown-links` checks actual rich-text output,
clickable icons, file types, host boundaries, literal content, copy and find.

Timeline spacing follows native `MessagesTimeline`: no list-wide gap or extra
padding around work rows (24 px line height), 8 px after commentary and 16 px
after terminal/user messages, scaled through Omarchy's spacing tokens. Author
headings remain accessible without a visible row. Commentary omits terminal
metadata; timestamps and compact copy controls sit below terminal/user bodies.
`tools/test-qml timeline-spacing` checks row geometry and produces a visual
fixture. Link glyph spacing also reserves a real font-run space to clear Nerd
Font glyph overhang; its test measures the label position against glyph bounds.

The broader source-backed chat/sidebar review, including remaining differences,
is recorded in [NATIVE_VISUAL_AUDIT.md](NATIVE_VISUAL_AUDIT.md). The comfortable
736px lane is shared by the composer and timeline; user bubbles shrink to their
content with an 80% cap and keep hover metadata outside the background. Markdown
uses an explicit Qt-supported stylesheet. Sidebar shelves render native 36px
slim rows, with persistent PR actions, project icon overrides, read/draft/pin
cues and static work status. `tools/test-qml visual-parity` checks actual geometry,
source-dependent state and bundled project icon rendering.

Document previews read text through native `projects.readFile` with the selected
thread's worktree (or project root) as `cwd`. Signed asset grants remain reserved
for media and attachments; they reject ordinary Markdown/text files. Markdown
opens as safe rendered rich text with a Source/Preview toggle, preserving the
original text and handling links through the existing reference router.

Angle-only wheel events retain a bounded 1000 ms ease even when Hyprland reports
native coasting. The compositor's distance is preserved without a second
acceleration multiplier; precise native pixel streams still pass through. Tiny
edge distances settle faster. The `inertia` and `wheel-scroll` checks verify
continued movement after release with both fallback and native policies.

Scroll capability probing also checks the compositor's window exclusions at the
current pointer position. Layer popups can sit over a browser whose class
suppresses native touchpad coasting. While those exclusions are configured, the
visible panel refreshes its effective policy every 300 ms through the compositor
socket. Hidden/inactive clients are excluded from the underlay lookup; floating
windows take precedence over tiled clients. Mouse support remains independent.

The local fallback seeds a bounded velocity coast after 80 ms of silence, using
the patch's EMA, minimum velocity and quadratic-decay integral over 1000 ms.
This adds actual travel after release rather than only easing the input distance.
Native streams never receive this extra local coast. Both document text views
use the same controller as the sidebar and chat.


Native checkout diff renderer: `tools/build-diff-renderer` rebuilds the deterministic gzip asset from the exact npm lock (Node/npm are build-only). Rebuild the Rust client after changing the asset, because it is embedded in the executable. `tools/build-document-viewer` rebuilds the on-demand Qt bridge. `tools/test-diff-renderer` exercises the real component with synthetic full-file data and native mouse/keyboard events; ordinary HTML still uses `tools/test-document-viewer`. See `docs/GIT_CONTROLS.md` for the scoped native-read/security contract and remaining checkpoint parity.


Historical renderer regression: `OT3_DIFF_HISTORICAL=1 tools/test-diff-renderer` checks the native no-checkout-loader path. QML checks `checkpoint-diff-flow` and `diff-preferences` exercise range/file identity and canonical native layout persistence in an isolated home. Native layout writes must start with a fresh client-settings read; per-view whitespace and wrap do not rewrite their global defaults.

Archived-thread regression: `cargo test --release --test archive_management --test connection_preferences`, plus serial `tools/test-qml archive` and `tools/test-qml archive-preferences`. The fixtures use isolated local state and synthetic native HTTP/WebSocket servers. Live smoke is read-only: `bin/omarchy-t3-client archive-read <environment-id>`. See [ARCHIVE.md](ARCHIVE.md); never use real archived-thread mutation merely to validate the screen.

Native compact bounded snapshots: chat seed reads, plan/approval preflight and automatic compaction reads now request `compactTurnItems=1`; live subscription opts into `acceptCompactTurnItems`. `bounded_snapshot::restore` restores local visible turn items before reducers/caches. Legacy unmarked responses remain unchanged. Protocol GET `/api/orchestration/threads/:id/bounded` and `orchestration.subscribeThread` with `acceptBoundedSnapshot=true` have a bounded 64 MiB response/frame allowance; generic HTTP, RPC and other streams retain 8 MiB. `cargo test --release --test bounded_snapshot --test live --test approvals --test plan_transport --test auto_compact` covers exact reconstruction and the real HTTP limit. The long development chat exceeded the legacy form limit during archive installation; its compact response was then approximately 5.5 MB and grew beyond 9 MB on October 17. Regression tests cover compact HTTP and live snapshots above 8 MiB, unchanged generic limits, and rejection above 64 MiB.

Keybinding editor integration is in progress: [KEYBINDINGS.md](KEYBINDINGS.md). `node tools/build-keybindings.cjs` deterministically reuses the pinned original TS logic in Qt (no npm/runtime dependency). Verify with `tools/test-qml keybindings-logic` and `cargo test --release --test keybinding_settings`. The native editable screen and supported runtime dispatch are now installed, with primary/active ownership, native config-stream invalidations, localized labels/search and script shortcuts through the existing journaled pipeline. Also run serial `shortcut-routing`, `keybindings-runtime`, `script-shortcuts` and `keybindings-flow`. These fixtures and live read-only checks do not establish parity for missing workspace commands or alternate-layout/IME/compositor key handling.


Application licenses: [LICENSES.md](LICENSES.md). `bin/build-client` collects exact
locked Cargo, renderer, asset and compiler notices before compiling; Node and
Python are build-only. `node tools/build-licenses.cjs` uses fetched locked crate
sources offline and the checked-in original helpers. Run release
`third_party_licenses` tests and serial QML `licenses`/`licenses-preferences`.
The real-worker fixture runs without a T3 account and never modifies user data.

Chat Markdown images use native rich-text image placeholders restored only after
the Rust image worker resolves a private cache URL. Filesystem destinations,
including absolute paths and `file:` URLs, use T3's exact thread/path
`media-file` asset grant; remote host paths are never passed to Qt as image
sources. Public HTTP/data images carry no environment credentials. Image size
is proportional to the chat lane and clicking opens the existing native viewer.
The generic document/plan Markdown renderer keeps its existing image policy.
Run `cargo test --release --test markdown_images --test images --test history
--test documents` and serial QML `markdown-images`, `live-media`, `history-find`.

File references and changed-file rows expose a Nerd Font eye action that opens
the existing native document viewer (HTML preview/source, rendered Markdown,
UTF-8 text/code and supported images). Changed-file row clicks still open the
diff; deleted files have no current-file preview. Composer attachments expose
the same eye after upload completes: supported images reopen the full image
viewer and other attachments use the document viewer, without sending or
removing the draft. Run serial QML `file-previews`, `documents` and `clipboard`.

HTML document capture has no application-imposed FPS cap: pending repaint
requests coalesce on the next event-loop turn, with fast lossless PNG encoding.
Requests remain demand driven, so idle documents stop capturing. Actual
throughput still depends on Chromium repaints, rendering and desktop presentation. `tools/test-document-scroll` exercises
real wheel input at 900×620, reports changing-frame throughput and frame gaps,
and checks idle/close behavior. This measures renderer transport throughput,
not the final desktop presentation rate; `T3_DOCUMENT_MIN_FPS` is an optional
host-specific performance gate.

Subscription account discovery, original T3 pooling, host ownership and focused
checks are documented in [SUBSCRIPTIONS.md](SUBSCRIPTIONS.md).
