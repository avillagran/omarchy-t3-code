# OmaBridge connection routes

[OmaBridge](https://github.com/avillagran/omabridge) provides private device-to-device links and ships with ot3. Once two devices are paired, ot3 carries inherited T3 connections over that link first and keeps the network address saved in the T3 catalog as the fallback. You don't need a VPN, an open port or a public endpoint. T3 still owns every credential and authenticates every session. Without the bundled binary, or without a paired identity, nothing changes.

## What is routed

| T3 connection | Peer service | Behaviour |
| --- | --- | --- |
| SSH profile | `ssh` (peer's `127.0.0.1:22`) | Discovery and the `-L` tunnel both use OpenSSH with `HostName=127.0.0.1`, `Port=<forward>` and `HostKeyAlias=<original host>`. The user, identity and `known_hosts` checks stay the same. `ProxyJump`/`ProxyCommand` are disabled for the bridged hop. |
| Pairing link | `t3` (peer's T3 port) | The catalog bearer session is sent to a loopback forward instead of `httpBaseUrl`. |
| T3 Connect relay | — | Never routed. |

For each attempt, ot3 spawns `omabridge --state-dir <dir> forward --peer <id> --service <s> --listen 127.0.0.1:0`. It reads the forward's readiness line, accepts only a loopback listener, and ties the child to the connection's lifetime with PDEATHSIG.

## Choosing the peer

Each environment follows one of three plans:

- **Automatic** (the default): every paired peer that grants the matching service is tried in turn. A peer is used only after it proves it serves that environment, so no configuration is needed and a wrong peer is never trusted:
  - SSH: the bridged hop must pass the `known_hosts` check for the profile's host, and the remote bootstrap must report the same T3 environment id.
  - Pairing link: the peer's public `/.well-known/t3/environment` descriptor must name the environment *before* the bearer credential is sent.
  If no peer answers, the saved T3 route is used.
- **Pinned**: one peer and service. `prefer` falls back to the saved route; `only` reports `omabridge_route_failed`. Pairing links are still checked against the descriptor.
- **Saved route only**: OmaBridge is never used for that environment.

`"default":"off"` turns automatic routing off for every environment that is not pinned.

## Bundled binary

The installer (and every update, which reruns it) downloads the OmaBridge release pinned in `integrations/omabridge/release` for the device's architecture. `bin/fetch-omabridge` accepts the archive only when its SHA256 matches both the pinned hash and the release's `SHA256SUMS`, checks that it loads on this system (it needs glibc 2.39 or newer), and installs `bin/omabridge` next to `bin/omarchy-t3-client`. An already-current copy is not downloaded again. If any step fails, the installer reports it and continues: ot3 works as before, with saved routes only. To bump OmaBridge, change the version and both hashes in that one file.

## Configuration

**Preferences → Connections → OmaBridge** has a global *Use OmaBridge for every connection* switch and, per SSH or pairing-link connection, a selector: *Automatic*, *Saved route only*, or a paired peer that currently grants the matching service. Each row shows whether the connection is live through OmaBridge and which peer carries it. Every change asks for confirmation. Settings are stored in `$XDG_CONFIG_HOME/omarchy-t3-code/omabridge-routes.json` (mode 0600, compare-and-swap on its revision); a missing file means automatic routing:

```json
{"version":1,"default":"auto","stateDir":"/home/user/.local/share/omabridge","routes":{
  "<pinnedEnvironmentId>":{"peer":"<64-hex endpoint id>","service":"ssh","mode":"prefer","relayUrl":null},
  "<directEnvironmentId>":{"mode":"off"}}}
```

`stateDir` defaults to OmaBridge's `$XDG_DATA_HOME/omabridge`. ot3 creates an identity there only when you ask it to (see below). `relayUrl` must be HTTPS. A malformed document disables every OmaBridge route instead of guessing. ot3 only runs the bundled `bin/omabridge` next to the client (see below), never one from `PATH`; `OT3_OMABRIDGE_BIN` overrides it for development.

Diagnostics: `omarchy-t3-client omabridge-read` prints the public snapshot (peer ids, names, granted services, routes). `omarchy-t3-client omabridge-status` also connects every environment and reports which ones OmaBridge carries (`active`) and any connection error. `connections-view` prints the credential-free catalog view.

## Managing devices

The same section manages OmaBridge itself, so `omabridge-gui` is not needed. Every action runs the bundled CLI against the private state directory and reads its JSON output. Each change of trust asks for confirmation first.

- **This device**: its name and public device ID. *Create identity* runs `omabridge init` with the host name. The private key never leaves the state directory.
- **Invite a device…** (needs this device to be [published](#publishing-this-device)): choose the services to share (T3 Code, SSH) and whether to show a QR code. OmaBridge writes a one-use invitation to `~/Downloads` (or `~`) with mode 0600, and it expires in 10 minutes. The dialog shows the file path and the optional QR once. Closing the dialog forgets both, and *Delete invitation file* removes the file. Treat the file and the QR as secrets.
- **Pair from invitation file…**: the chosen file is copied to `$XDG_STATE_HOME/omarchy-t3-code/omabridge-pairing/` (0700 directory, 0600 file), and ot3 runs `omabridge pair` with the copy. Your original file is never changed. If no identity exists, ot3 creates one first. Until the other device approves, the request is listed as waiting, with *Check again* (which repeats `pair` and completes pairing) and *Cancel*. The copy is deleted once pairing is approved, or once the invitation expires.
- **Waiting for your approval**: claims on this device's invitations, with the requested services. *Approve…* confirms the device name, ID and services.
- **Paired devices**: what you can reach on each peer and what it can reach here. *Revoke…* removes the pairing after confirmation.

*Delete invitation file* only removes an `omabridge-invite-*.json` owned by you, with mode 0600, that is still a valid invitation, so it also works for invitations created by another front end.

Only peer IDs, names, services and expiry times reach the UI. Invitation tokens, invitation IDs, peer addresses and keys stay with OmaBridge. CLI error output is mapped to fixed error codes and is never shown or logged.

## Publishing this device

Publishing is optional and off by default. *Publish this device* (with confirmation, and *Also share SSH* only when an SSH server answers on `127.0.0.1:22`) writes a `systemd --user` unit, `omarchy-t3-omabridge.service`, and enables and starts it:

```ini
ExecStart="<plugin>/bin/omabridge" --state-dir "<state dir>" serve --service t3=127.0.0.1:<T3 port> [--service ssh=127.0.0.1:22]
```

The T3 port comes from T3's `server-runtime.json`. If T3 later moves to another port, the section marks the unit as stale and offers *Publish again*. Only peers you approved can reach the published services, and T3 still authenticates every session. ot3 creates an identity first if none exists. It only rewrites or removes a unit that starts with its own `# Managed by Omarchy T3 Code` header, and refuses paths that systemd would reinterpret. Turning the switch off (also confirmed) disables and removes the unit, and keeps pairings. When the installer updates the bundled binary, it restarts a published unit with `systemctl --user try-restart`. Invitations need a running published device.

## Bar indicator

*Show OmaBridge in the bar* (in the same section, off by default) adds a small indicator after the T3 status in ot3's bar widget. It refreshes the snapshot every minute and when opened. A red dot means requests are waiting for your approval. Its popup shows this device's state (published services and number of paired devices) and lets you approve a request in two steps (the full device ID is shown before *Approve*) or *Check again* on a pairing you sent. *OmaBridge settings…* opens Preferences → Connections for everything else. Middle click refreshes. Invitations and their secrets are never shown in the bar.

## Other front ends

Other Omarchy front ends (for example the control panel) manage OmaBridge through ot3 instead of the CLI, so validation, confirmation, secret handling and the published unit stay in one place. The client lives at `~/.config/omarchy/plugins/omarchy-t3-code/bin/omarchy-t3-client` (or under `io.github.avillagran.omarchy-t3-code`).

- `omarchy-t3-client omabridge-read` prints the snapshot: `available`, `version`, `identity`, `device` (`{id,name}` or null), `peers` (`id`, `name`, `services` it can reach here, `remoteServices` you can reach there), `pending` (`id`, `name`, `services`, `expires`), `awaiting` (`id`, `name`, `expires`), `published` (`enabled`, `active`, `services`, `stale`, `sshAvailable`), `default`, `routes`, `active`, `revision` and `error`.
- `omarchy-t3-client omabridge-action` reads one JSON request on stdin (at most 64 KiB, never on the command line) and prints the new snapshot. Every action except `read` needs `"confirmed":true`, which the front end must only send after the user confirms:

| `action` | Fields | Adds to the snapshot |
| --- | --- | --- |
| `identity-create` | — | — |
| `invite` | `services` (`["t3"]`, `["ssh"]` or both), `qr` (bool), optional `ttl` (60–3600 s, default 600), optional `directory` | `created`: `path`, `expires`, `services`, `qrImage` (SVG data URI or null). Secret: show it once and drop it. |
| `invite-discard` | `path` | — |
| `pair` | `path` of an invitation file | `paired`: `status` (`pending` or `approved`), `name` and, while pending, `id` |
| `pair-retry` / `pair-discard` | `id` from `awaiting` | `paired` (retry only) |
| `approve` | `peer`, plus `expires` from `pending` | — |
| `revoke` | `peer` | — |
| `publish` | `ssh` (bool) | — |
| `unpublish` | — | — |
| `set` / `clear` / `default` | route fields as in Preferences, plus `revision` | — |

On failure it prints `{"error":"<code>"}` and exits 1. Codes include `confirmation_required`, `invalid_command`, `omabridge_unavailable`, `omabridge_identity_missing`, `omabridge_not_published`, `omabridge_service_not_published`, `omabridge_ssh_unavailable`, `omabridge_invite_expired`, `omabridge_invite_invalid`, `omabridge_invite_unreadable`, `omabridge_peer_not_found`, `omabridge_unit_foreign`, `omabridge_publish_failed` and `omabridge_command_failed`. A running ot3 sees changes made this way on its next refresh.

## Verification

`cargo test --release --locked --test omabridge_route` covers the following, using a fake `omabridge` binary and a loopback T3:

- document validation and the automatic/pinned/off plans;
- automatic routing with no configuration, and refusal of a peer whose descriptor names another environment (the credential is never sent to it);
- the global and per-environment `off` settings;
- loopback-only readiness parsing;
- the public peer projection;
- compare-and-swap, confirmation and grant checks;
- 0600 storage with no T3 credential;
- the bearer request through the bridge;
- `prefer` fallback, the `only` failure and clearing a route.

`cargo test --release --locked --test omabridge_manage` drives the real bundled CLI with two throwaway identities on loopback (it is skipped when `bin/omabridge` is missing). It covers identity creation, the refusal to invite while unpublished, a 0600 invitation with a QR whose token never reaches the UI, pairing through a private copy, approval with confirmation, *Check again*, discarding the invitation, and revocation. Publishing goes through a stand-in `systemctl` that runs the unit's real `ExecStart`, so the generated unit is what serves the invitation. The test also covers the stale-port flag and unpublishing.

Other checks:

- Unit tests pin the bridged SSH arguments.
- `tools/test-qml omabridge-indicator` covers the bar indicator's refresh, status, pending badge, two-step approval, *Check again* and the settings hand-off.
- `tools/test-qml omabridge-device` covers identity creation, the confirmed approve, revoke and pair payloads, *Check again*, invitation services and QR, and clearing the shown-once invitation when its dialog closes.
- `tools/test-qml connection-omabridge` covers the automatic default, eligible connections and services, the confirmed pin/strict/automatic/saved-only/default payloads and the live status.
- The `connections`, `connection-network`, `connection-access`, `preferences` and `i18n` QML checks pass, and translations cover 20 locales.

**Live check:** with an empty route file (automatic), `omabridge-status` connected a paired SSH host through its `ssh` service with no configuration, verified against the real `known_hosts` entry. A raw `t3` forward also answered HTTP 200.

Away from the LAN, a NAT-ed peer needs an OmaBridge relay (`relayUrl`).
