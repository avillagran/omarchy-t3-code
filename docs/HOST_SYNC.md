# Sync this computer's setup

O3 only. **Preferences → Connections → Sync this computer's setup** leaves a connected
host with the same working setup as this computer, from the Obsidian vault to the
harnesses' configuration. It writes to the host only when you press **Sync**, after a
preview. It never deletes anything, and it backs up each file it replaces on the host first.
You can run it again at any time. It sends only what changed.

## What is synced

Each item has its own switch. An item with nothing to do cannot be selected.

| Item | From this computer | On the host |
| --- | --- | --- |
| Development vault | The vault set up in [Memory/Obsidian](JOURNAL_VAULT.md): its folder relative to the home (`~/Obsidian/Dev`) and its network remote (`origin`, without credentials) | Cloned into the same folder when it is missing or empty. When it is a clone of the same remote, `git pull --rebase --autostash`. A rebase that stops is aborted (`conflict`) and nothing is discarded. A folder holding anything else is left untouched. The vault's commit identity (`user.name`/`user.email` from its `.git/config`) is set there only when the host's clone has none. |
| Agent instructions | `~/.claude/CLAUDE.md`, `~/.codex/AGENTS.md`, `~/.config/opencode/AGENTS.md`, `~/.gemini/GEMINI.md`, when present | The same files |
| Skills | Each entry of `~/.claude/skills`, `~/.codex/skills` and `~/.agents/skills`. A link inside the home is copied with its content resolved. | The same paths |
| Harness settings | `~/.claude/{settings.json,keybindings.json}`, `~/.claude/{agents,commands,output-styles,hooks,themes}/`, `~/.codex/config.toml`, `~/.codex/{prompts,rules}/`, `~/.config/opencode/opencode.json(c)` and its `agent(s)`, `command(s)`, `mode` and `plugin` folders, `~/.gemini/settings.json`, `~/.gemini/commands/`. MCP servers from the `mcpServers` key of `~/.claude.json`. | The same files. For `~/.claude.json`, only the servers the host lacks are added. Its account, caches and other servers stay. |

What is never sent:

- Credentials and sign-ins: `~/.claude/.credentials.json`, `~/.codex/auth.json`, `.netrc`,
  `.env*`, SSH keys, `*.pem`/`*.key`/`*.p12`, and any configuration file with a literal
  token, key or password, whether by known prefix (`sk-ant-`, `ghp_`, `github_pat_`,
  `xoxb-`, `AKIA…`, private key blocks) or by a secret-named key with a literal value
  (`"apiKey": "…"`). These files are listed as *Not sent · Looks like a credential*.
  References such as `${VAR}`, `$VAR` or `env:` are allowed. Each host signs in on its own
  (see *Sign in needed* in [OmaBridge](CONNECTION_OMABRIDGE.md#names-and-sign-ins-on-each-host)).
- History, sessions, projects, caches, logs and databases. Only the allowlisted paths
  above are read. Inside them, `.git`, `node_modules`, `__pycache__`, `.venv`, `history*`,
  `*.log`, `*.db` and `*.sqlite*` are skipped.
- Skills that belong to the system or to the harness: an entry linked outside the home
  (Omarchy's `omarchy`, `omarchy-app`, `diagnose-crash`, which every Omarchy host has),
  hidden entries (Codex's `.system`) and Claude's account-synced `~/.claude/skills/synced`.
- `~/.claude/themes/omarchy.json`, which Omarchy writes on every host for its theme.

Limits: 8 MiB per file, 128 MiB and 20,000 files per sync. Anything larger is listed as
*Too large*.

## Paths

The host's home comes from the host itself (`$HOME`). In every text file, this
computer's home is rewritten to the host's home, but only as a whole path prefix:
`/home/me`, `/home/me/x` and `"/home/me"` change, while `/home/meow` and `/x/home/me` do not.
Binary files are copied unchanged. `~/` paths need no change.

## What the host keeps

- A skill folder that the host has as a link of its own (for example, Omarchy's) is
  *Kept: the host's own*. Nothing inside it is written.
- A file whose location on the host resolves outside its home, through a link, is kept.
- A path that is a folder or something else on the host is *Not a file on the host*.
- Nothing is ever deleted. A file that exists only on the host stays.

## Preview, sync and backup

1. Choose the host (enabled connections with an SSH route) and press **Preview**. A
   single read-only SSH command (`host-sync-probe.sh`) reports the host's home, the
   SHA-256 of every file that would be written, which skill folders are links, the vault
   folder and its `origin`, `git ls-remote` access to the remote, and whether `gh` is
   installed and signed in. Each item shows how many files are new, to update, up to
   date, kept or not sent. Each file or skill that changes is listed.
2. **Sync** asks for confirmation and names the items and the host. O3 probes again and
   then sends one tar archive to `host-sync-apply.sh`. That script writes a file only if
   the host still holds what the probe saw: a file changed meanwhile is reported as
   changed and kept. Before replacing a file, it copies it to
   `~/.local/state/omarchy-t3-code/sync-backups/<UTC time>/<path>` on the host. Files are
   written through a temporary file and an atomic rename, keeping their mode.
3. Results show per item: added, updated, up to date, kept, changed meanwhile and
   failed, plus the backup folder. The vault shows cloned, updated, left untouched,
   conflict, or why it could not sync. The last result per host is kept in
   `$XDG_STATE_HOME/omarchy-t3-code/host-sync.json` and appears as *Last synced*.

### GitHub access on the host

The vault uses the host's own git access, and no credential is copied. When `git
ls-remote` there fails for lack of authentication, the vault item says so. If the
remote is on GitHub and `gh` is installed there, **Sign in to GitHub on <host>…** opens
a terminal here running `gh auth login --web` and then `gh auth setup-git` on that host,
so git can also push. When `gh` is missing, the item says to install it. Press
**Check again** afterwards.

## Transport

Only SSH environments are offered. Pairing-link hosts cannot be written to.

- The connection's own SSH profile (`connections::ssh_args_for`): `BatchMode`, and a
  private OpenSSH master (`ControlMaster=auto`, `ControlPersist=600`). The preview,
  the probe before a sync and the sync itself share that one connection. A host that
  rate-limits port 22 (for example, `ufw limit`) therefore sees a single connection,
  not one per file.
- Or the same SSH through the paired peer's OmaBridge `ssh` service:
  `ProxyCommand=omabridge --state-dir <dir> --name O3 connect --peer <id> --service ssh`
  with `HostKeyAlias=<the profile's host>`. The host key is still checked against
  `known_hosts` for the host already verified (never for a stale alias, and host key
  checking is never disabled). The peer that OmaBridge uses for that environment right
  now, or the pinned one, is tried first. Otherwise, the saved route is tried first,
  then the other peers that grant `ssh`. A pinned `only` route uses nothing else.

The result says whether the host was reached over SSH or through OmaBridge.

## Front ends

- Panel op `host-sync` with `action`: `read` (last results), `preview` (`environment`),
  `apply` (`environment`, `items` from `vault`, `instructions`, `skills`, `config`,
  `"confirmed":true`), `github-login` (`environment`). Only one preview or sync runs at
  a time (`host_sync_busy`).
- CLI: `omarchy-t3-client host-sync` with the same request on stdin, for example
  `echo '{"action":"preview","environment":"<id>"}' | omarchy-t3-client host-sync`.

## Tests

- `cargo test --release --test host_sync`, with two temporary homes, the host running
  the same scripts in a local shell:
  - the preview, then a sync, copy only the setup;
  - the home is rewritten as a whole prefix;
  - modes are kept;
  - links into the home are resolved;
  - the host's own linked skills and Omarchy's are kept;
  - credentials, sessions and Omarchy's theme never leave;
  - a configuration file with a literal key is not sent;
  - MCP servers are merged while the host's account stays;
  - replaced files are backed up;
  - a second sync does nothing.

  The vault test clones it, pulls it with the commit identity, and leaves an occupied
  folder alone.
- Unit tests in `src/host_sync.rs` cover secret detection, the home rewrite, OmaBridge
  arguments that keep `HostKeyAlias` and never weaken host key checking, and a file
  changed on the host between probe and sync that is not overwritten.
- `tools/test-qml host-sync` checks the page.
