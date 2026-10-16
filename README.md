# Omarchy T3 Code

**A lightweight T3 Code client for the Omarchy desktop, built with QML and Rust.**

Manage local and remote coding conversations from your status bar: follow live
work, write prompts, attach files, respond to questions, inspect changes and
track subscription usage. The interface follows T3 Code's familiar layout and
iconography while using your active Omarchy theme.

The desktop client runs inside Quickshell with one persistent Rust backend.
T3's server still does the coding work; you can use this panel without keeping
the official desktop GUI open when the server runs independently.

> This is an independent community project. It is not affiliated with T3 Code.
> It targets Omarchy's Quickshell plugin system and T3's evolving native APIs.

## Features

- **Familiar workspace:** resizable panel and sidebar, search, status filters,
  project identities, working timers, grouping, settled threads and context menus.
- **Live conversations:** streamed updates for the visible chat, contextual tool
  activity, animated work indicators, Markdown, changed-file summaries and tasks.
- **A capable composer:** model picker with inherited favorites, options, queued
  messages, stop controls, file/folder mentions, skills and supported commands.
- **Durable drafts:** per-thread text and attachments, expanded editor, list
  continuation, prompt history and undo. Switching chats keeps your draft.
- **Attachments and previews:** file picker, drag and drop, clipboard images,
  clickable thumbnails, and text/image/HTML previews inside the panel.
- **Native thread actions:** pin, rename, mark unread, settle, snooze, archive and
  delete, with environment capability checks and confirmation where required.
- **Pull requests:** browse PRs, descriptions, checks, conversation and diffs in
  the main conversation area. This view is currently read-only.
- **Subscription usage:** available quota, weekly/short windows when reported,
  and expected-pace markers, preserving the original Ares bar design.
- **Desktop integration:** work/ready/input badges, attention notifications,
  keyboard navigation, persisted view state and smooth scrolling.
- **20 languages:** translations use the flat English-key format from
  [omarchy-i18n](https://github.com/avillagran/omarchy-i18n).

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
- Node.js for translation checks and the HTML renderer test; Python is used only
  by legacy reference tests and asset-generation tooling, never by the backend.

Build on the target architecture. Generated binaries are not committed.
Development has been exercised on Linux ARM64 against T3 nightly versions;
compatibility with every T3 release is not guaranteed.

## Installation

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

Click the T3 icon in the status bar to open the panel. Configure local and remote
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

### Super+Enter on Omarchy

Omarchy normally reserves Super+Enter for the terminal. On installations using
Lua Hyprland bindings, add this line to `~/.config/hypr/bindings.lua`:

```lua
dofile(os.getenv("HOME") .. "/.config/omarchy/plugins/omarchy-t3-code/integrations/hyprland/bindings.lua")
```

Reload your Hyprland configuration afterwards. This integration sends through
the panel while it is open and launches the terminal otherwise. Review the
binding file before enabling it if you already customize Super+Enter. It is
not installed automatically.

## Local and remote connections

T3 remains the source of truth for local runtime discovery, remote environments,
connection names, credentials and model configuration. The Rust adapter reads
T3's runtime and encrypted catalog and uses its native integration credentials.
SSH connections reuse your existing SSH configuration, agent and known hosts.
T3 Connect/relay authorization follows the account and environment permissions
already granted in T3.

**Closing the official GUI only saves resources safely if its server is running
independently.** A desktop-owned server stops with that desktop app. The panel's
backend-start action can request T3's native user service after the server stops;
it does not automatically migrate an active desktop session or start a competing
server over the same data directory.

## Everyday use

- **Super+Enter** submits the prompt. **Enter** inserts a newline and continues
  supported lists. Other shortcuts follow the panel's keyboard help; system
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

Resource consumption depends on transcript size, attachments, the shell and
optional previews. The T3 server and provider workers still use resources.
No fixed memory saving is promised, and an active HTML renderer adds browser
process overhead.

### Shared library for Omi

[`crates/t3-protocol`](crates/t3-protocol/README.md) contains the portable Rust
transport, authentication and protocol logic, including a C ABI for a JNI host.
It does not depend on Quickshell, SSH or the Linux keyring. Omi supplies its own
platform integration; the Android application is not built or modified here.
See the [protocol contract](docs/protocol/t3-direct-v1.json) and fixtures for shared
behavior. Credentials stay in the host adapter and are not passed to QML.

## Project layout

```text
.
├── src/                     # Rust desktop adapter and process entry point
├── crates/t3-protocol/       # Portable Rust library and C ABI for Omi
├── ui/
│   ├── qml/                 # Panel, composer, sidebar and shared controls
│   ├── js/                  # UI models, theme helpers and generated icon data
│   ├── assets/              # SVGs, shaders and third-party license notices
│   └── document/            # Optional isolated Qt HTML frontend
├── translations/            # English source, locale batches and runtime catalog
├── integrations/hyprland/    # Optional Super+Enter desktop binding
├── docs/protocol/            # Shared transport contract and fixtures
├── tests/                   # Rust, isolated QML and legacy reference checks
├── examples/                # Opt-in integration diagnostics
├── tools/                   # Build helpers, generators and validation scripts
└── bin/                     # CLI launcher, build entry point and local binaries
```

`manifest.json` points directly to `ui/qml/BarWidget.qml`. Rust follows the
standard Cargo layout; the portable crate remains separate from Linux desktop
integration. Generated binaries, local measurements and development evidence
are excluded from Git. The root `hypr-bindings.lua` is a compatibility entry point
for existing user configurations; new installations use `integrations/hyprland/`.

## Validation

```sh
cargo test --workspace --locked
node tools/check-translations.cjs
omarchy plugin validate .

# Requires the installed Omarchy shell and a graphical session:
tools/test-qml thread-menu
tools/test-qml content-panels
tools/test-qml documents
tools/test-qml upward-scroll

# Requires the optional Qt HTML frontend:
tools/test-document-viewer
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

## Credits and license

Evolved from [Omarchy Ares](https://github.com/avillagran/omarchy-ares).
Interface behavior and bundled T3 marks follow
[T3 Code](https://github.com/pingdotgg/t3code). Action icons use
[Lucide](https://lucide.dev). See [asset attribution](ui/assets/README.md),
[T3's MIT notice](ui/assets/T3-LICENSE) and [Lucide's ISC notice](ui/assets/LUCIDE-LICENSE).
Provider names and marks belong to their respective owners.

Plugin source is available under the [MIT License](LICENSE).
