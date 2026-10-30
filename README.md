# O3 Code

**T3 Code as an Omarchy desktop app, with a status widget in your bar. A lightweight QML interface with a Rust backend.**

![O3 Code — desktop overview](docs/images/preview.png)

| Models and favorites | Live chat and file changes |
| --- | --- |
| ![Native model picker and favorites](docs/images/screenshot-2026-10-16_04-50-21.png) | ![Chat, sidebar and changed files](docs/images/screenshot-2026-10-16_04-51-01.png) |
| **Thread actions** | **Pull requests** |
| ![Thread context menu](docs/images/screenshot-2026-10-16_04-51-34.png) | ![Pull request browser inside the panel](docs/images/screenshot-2026-10-16_04-52-30.png) |

Follow local and remote coding sessions, write prompts, attach files and respond
to questions without keeping the official T3 desktop GUI open. The panel uses
your Omarchy theme and inherits T3's connections, models and favorites.

## Highlights

- Live messages, contextual work indicators, tasks, queues and changed files.
- Durable drafts, expanded prompt editor, mentions, skills and commands.
- Image/file/HTML previews inside the panel; copy messages and images.
- QML previews of `qml` code blocks, inline or in the side panel, in a sandboxed
  Qt process (allowed Qt imports only; no network, file access or launches;
  Omarchy colours as `omarchy.background`, `.foreground`, `.accent`…).
- Thread actions, branch/PR badges, conversation minimap and a PR browser.
- Subscription usage bars and work/ready/input badges on your desktop bar.
- A desktop app in its own process: a stuck bar never blocks it; minimize it to the bar widget.
- Resizable layout, keyboard navigation, smooth scrolling and **20 languages**.
- Native headless server controls and automatic startup for inherited SSH connections.
- Preferences for native general/Git settings, Omarchy appearance, connections and model favorites.

## Install

Requires **Omarchy's Quickshell shell/plugin manager**. Run as your desktop user, from your
desktop session:

```sh
curl -fsSL https://raw.githubusercontent.com/avillagran/omarchy-t3-code/main/install.sh | bash
```

The [installer](install.sh) installs runtime dependencies through
Pacman and, when T3 Code is missing, **T3 Code nightly** (`t3code-nightly-bin`, from Omarchy's
repository or the AUR through `yay`/`paru`). If T3 has never been opened it opens it once, so T3
creates its profile and `t3` CLI. It then downloads verified **aarch64 / x86_64** binaries and installs O3 Code as a **desktop
app**: an "O3 Code" entry in the app launcher, `o3` on your PATH, a hidden start at login and
**Super+Shift+A** as your default agent on Omarchy's Lua configuration. It also enables the optional
**bar widget**, which shows work/ready/input badges and minimizes the window to the bar
(`OT3_BAR_WIDGET=0` installs the app alone). It reuses an existing T3 server or starts the native
headless service when stopped. It also installs the pinned, checksum-verified
[OmaBridge](docs/CONNECTION_OMABRIDGE.md) CLI next to the client. Your T3 connections and data stay in place.
The same command updates a clean Git installation. It may request your sudo
password and restarts the Omarchy shell when finished.

The app runs in **its own process**, not inside the Omarchy shell: if the bar gets stuck, Super+Shift+A
and the launcher keep working, and restarting the shell does not close your chats. Installed from the
Omarchy plugin marketplace instead, the same folder works as a plugin: its bar widget starts the app
hidden and shows or hides it on click; run `~/.config/omarchy/plugins/omarchy-t3-code/bin/o3 install-desktop`
to add the launcher entry and `o3` command as well.

## Use

**Preferences → Appearance → Interface layout** offers **Classic** (the default)
and **Focus**, plus **By project** for a project → state → thread hierarchy.
Project and state headers fold independently; matching repositories across hosts
share a project. In By project, projects and threads sort alphabetically; Classic and Focus sort by latest reply by default. Focus adds two-line thread rows with host labels, project/host/branch
context in the composer, a quieter conversation and a **Task board** in the header.
The board follows the sidebar filters; click a card to open its conversation.
Switching back restores your existing appearance choices and keeps drafts intact.
The chevron beside the sidebar grouping button switches layouts directly.


Press **Super+Shift+A**, open **O3 Code** from the app launcher or click **O3** in the status bar
to show the agent; the same shortcut or click hides it again (the header's **–** button and closing the
window minimize it to the bar, the app keeps running). `o3 quit` stops it and `o3 restart` applies an
update. **Enter** sends; **Shift+Enter** inserts a new line. **Ctrl+Enter** also sends. **Super+Enter** keeps Omarchy’s
terminal shortcut. The header’s window button cycles **panel** (under the bar’s T3 button, with the bar widget)
**→ floating → tiled**, keeping the same chat and draft. Add projects through the embedded
environment and folder picker, including remote folders.
Use the header buttons for subscription usage and pull requests, and right-click
a thread for its actions. **F1** opens keyboard help.

Manage saved connections in **Preferences → Connections**: add SSH hosts or pairing
links, edit routes, rename, reconnect, switch off or remove them. Ot3 uses T3’s
native encrypted catalog. If the
official GUI owns the server, close it and use the sidebar's start button to run
headless. The plugin does not stop a GUI-owned server. Inherited SSH connections reuse a running
remote server or start its native headless service when stopped. Open the sidebar
gear for preferences; server settings apply to the selected environment.
O3 bundles [OmaBridge](docs/CONNECTION_OMABRIDGE.md): once devices are paired, SSH and pairing-link
connections go through them first, without a VPN, and fall back to the saved route. Pair, approve,
revoke, publish and manage your devices in Preferences → Connections → OmaBridge or from the
optional bar indicator: the same page as the Omarchy Control Panel, on the same device.
**Sync this computer's setup**, on the same page, leaves a connected SSH host with your
[working setup](docs/HOST_SYNC.md): the development vault, agent instructions, skills and
harness settings, after a preview and without credentials.

Some T3 features are still being ported, and the PR view is currently read-only.
See the [feature parity checklist](docs/FEATURE_PARITY.md).

## Development

Build instructions, architecture, the shared Rust library for Omi, keyboard
integration and validation commands live in the [development guide](docs/DEVELOPMENT.md).
The portable protocol is maintained separately in [lib-t3-rust](https://github.com/avillagran/lib-t3-rust),
shared with Omi and consumed at a pinned revision.

## Credits

Evolved from [Omarchy Ares](https://github.com/avillagran/omarchy-ares), inspired by
[T3 Code](https://github.com/pingdotgg/t3code), with [Lucide](https://lucide.dev)
icons. Independent community project. [MIT License](LICENSE) ·
[Asset attribution](ui/assets/README.md).
