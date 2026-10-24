# Memory/Obsidian (development vault)

ot3 only. T3 keeps threads, projects and decisions per environment (each server's
own SQLite) and shares none of them between machines. The development vault is a
git repository of Obsidian notes that every machine clones: the shared record of
state, decisions and progress per project, read and written by people and agents.
Preferences → **Memory/Obsidian** sets it up on this computer with no terminal and
keeps it in sync.

Source brief: `~/Obsidian/Dev/Projects/omarchy-t3-code/Journal-Module-Brief.md`.

## This computer vs. the environment

**This computer** is the device the ot3 app runs on (for example mbp-m2), even when
the selected environment runs its agents on another host (for example node). The
clone, the agent instructions, the device entry and **Open in Obsidian** always act
on this computer. Connected hosts are only *read*, through T3's own
`projects.readFile` RPC; ot3 never writes, clones or runs git on them. The page keeps
the two apart: **On this computer** (local state) and **On your hosts** (what each
connected environment reports).

## Configuration

`$XDG_CONFIG_HOME/omarchy-t3-code/vault.json`, outside the connection catalog. It
never holds credentials; unknown fields are kept.

```json
{
  "version": 1,
  "enabled": true,
  "path": "/home/me/Obsidian/Dev",
  "remote": {"kind": "github-private", "url": "https://github.com/me/dev-vault.git",
             "environment": null, "transport": "direct"},
  "sync": {"pullOnOpen": true, "pullOnThreadStart": true, "pushOnSettle": true,
           "intervalMinutes": null},
  "agentInstructions": true,
  "projects": {"<T3 project title>": "<slug>"},
  "device": {"name": "mbp-m2"}
}
```

- `remote.kind`: `github-private`, `local-bare` (a bare repository on one of your
  hosts) or `custom-url`.
- `remote.transport` for `local-bare`: `ssh` (the environment's T3 SSH profile) or
  `omabridge` (the same SSH reached through the peer's OmaBridge `ssh` service:
  the clone gets `core.sshCommand` with `ProxyCommand=omabridge connect`).
- Sync results (last pull/push, errors) are kept apart in
  `$XDG_STATE_HOME/omarchy-t3-code/vault-state.json`.

## Detection on connected hosts

On opening the page, each connected environment other than this computer is asked,
read-only and with `allowMissing`, for at most these files:

1. `~/.config/omarchy-t3-code/vault.json` (an ot3 configured there);
2. `~/.claude/CLAUDE.md` and `~/.codex/AGENTS.md`, only the path inside the
   `<!-- dev-vault-journal:start/end -->` block, or a `~/…` path written next to
   "vault" in a "Dev Vault" heading (vaults set up by hand);
3. the default `~/Obsidian/Dev`.

For each candidate folder it reads `AGENTS.md` (it must exist: a development vault)
and `.git/config` (the `origin` URL). Personal folders are never listed or scanned.
Credentials embedded in a URL (`https://user:token@…`) are dropped and never shown,
stored or copied.

Each host ends in one of: **online** with vaults, **online without a vault**,
**offline** (connection error or the RPC failed: unknown, never "no vault"), or
**unsupported** (a server whose `projects.readFile` rejects absolute paths).
Vaults are grouped by their canonical remote (`github.com/owner/repo`,
`host:/path`): clones of the same remote on several hosts are one vault. With more
than one distinct vault the page asks which one to use.

### Remote as seen from this computer

- A network URL (`https://`, `ssh://`, `git@host:`) is used as it is.
- A path on the host (`/srv/git/dev.git`, `file://…`) is a bare repository there:
  this computer reaches it through that environment's SSH profile
  (`ssh://user@host:port/path`) or, when OmaBridge carries the environment, through
  the peer's `ssh` service. Without either the page explains that SSH is needed.
- A vault without `origin` cannot be shared: the page says so and offers to
  configure a remote manually.

## Set up on this computer

1. Choose the local folder (default `~/Obsidian/Dev`).
2. **Inspect the folder**: missing or empty → clone; a clone of the same remote →
   reuse it (offer **Repair**); another remote, a non-git folder with files, or a
   file → *occupied*: never overwritten, the page asks for another folder.
3. **Check access** with `git ls-remote` using this computer's own authentication
   (`GIT_TERMINAL_PROMPT=0`, SSH `BatchMode=yes`). No credential is copied from any
   host. Failures are classified: `vault_auth_missing` (shows how to connect it:
   `gh auth login` + `gh auth setup-git` for GitHub, an SSH key for SSH),
   `vault_remote_unreachable`, `vault_remote_missing`.
4. Clone (`git clone`), set `core.sshCommand` for OmaBridge.
5. Install the agent protocol (below), register this device in `Machines.md`,
   commit and push that change.
6. Save `vault.json`. The page then shows the path, sync state and
   **Open in Obsidian**.

The wizard is idempotent: running it again reuses the clone, rewrites the same
instruction block and the same device row. The page never says *configured* while a
step is pending (access, clone, instructions, Obsidian); each step shows its own
state.

Without hosts (or with every host offline) the page still offers **Clone a remote**
(URL typed by hand) and **Create a vault**: initialise the folder from embedded
templates, optionally create a private GitHub repository with `gh repo create
--private`, and push.

## Agent instructions

A marked block is written to `~/.claude/CLAUDE.md` and `~/.codex/AGENTS.md` on this
computer:

```
<!-- dev-vault-journal:start -->
…protocol pointing at the vault path…
<!-- dev-vault-journal:end -->
```

Only that block is replaced; the rest of the file, including other marked blocks, is
kept byte for byte. Writing it twice gives the same file. A symlinked instruction
file is written through its target only when that target is a regular file.

## Automations

All on this computer, in the background, one git operation at a time:

- **On opening ot3** (`pullOnOpen`) and when a thread starts working
  (`pullOnThreadStart`, at most once a minute): `git pull --rebase --autostash`.
- **When a thread settles** (`pushOnSettle`, a thread that was working stops,
  debounced 30 s): `add -A`, `commit -m "journal(<slug>): update from <device>"` if
  anything changed, `pull --rebase`, `push`.
- **Optional interval** (`intervalMinutes`): pull, and push local commits.
- **New T3 project**: `Projects/<slug>/MEMORY.md` from the template and a line in the
  `## Proyectos` index of `MEMORY.md`, only if missing. The slug is the mapped name
  or the project folder name; only `[A-Za-z0-9._-]`.
- **Conflicts**: a rebase that stops is aborted (`git rebase --abort`), the local
  commits stay, and the state shows `conflict` until the user resolves it. Nothing is
  forced or discarded.

Every write is confined to the configured vault folder: paths are built from
validated slugs, and symlinks leaving the vault are refused.

## Obsidian

Detected from `obsidian` on `PATH`, the Flatpak `md.obsidian.Obsidian`, an
`obsidian.desktop` entry, or `/Applications/Obsidian.app` on macOS. **Open in
Obsidian** registers the folder in Obsidian's `obsidian.json` when absent (other
vaults kept) and opens `obsidian://open?path=…`. When missing the page shows how to
install it (Omarchy: `omarchy-pkg-add obsidian`; elsewhere obsidian.md).

## Status

`vault-status` reports: configured, path state, remote (without credentials),
branch, ahead/behind, uncommitted changes, rebase in progress, last pull/push with
time and error. The bar badge is optional and off by default.

## Front ends

- Panel ops: `vault-read` (config + local status), `vault-discover` (hosts),
  `vault-action` (`inspect`, `check`, `setup`, `create`, `repair`, `pull`, `push`,
  `open`, `save`), each answered as an `action`.
- CLI: `omarchy-t3-client vault-status`, `vault-discover`, `vault-sync`.

## Tests

`cargo test --test journal_vault` with temporary bare repositories, clones and an
isolated HOME: create, clone, pull/push, rebase conflict, idempotent instruction
block with foreign blocks kept, project stub and index, no write outside the vault,
multihost detection with grouping by remote, offline hosts, existing clone reuse,
occupied folder, missing local authentication, and a client (mbp-m2) whose
environment is node: discovery only issues read RPCs to node and every write lands
in this computer's HOME. `tools/test-qml journal-settings` checks the page.

Out of scope (v1): a note editor in ot3, graphical conflict resolution, phone sync,
automatic migration of old vaults.
