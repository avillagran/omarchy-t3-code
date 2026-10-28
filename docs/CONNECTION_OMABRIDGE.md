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
{"version":1,"default":"auto","routes":{
  "<pinnedEnvironmentId>":{"peer":"<64-hex endpoint id>","service":"ssh","mode":"prefer","relayUrl":null},
  "<directEnvironmentId>":{"mode":"off"}}}
```

`stateDir` (optional) defaults to OmaBridge's own `$XDG_DATA_HOME/omabridge`, the device's identity. `relayUrl` must be HTTPS. A malformed document disables every OmaBridge route instead of guessing. ot3 runs the same OmaBridge binary as the Control Panel: `OT3_OMABRIDGE_BIN`, `omabridge` on `PATH`, `~/.local/bin/omabridge`, then the copy bundled next to the client.

Diagnostics: `omarchy-t3-client omabridge-read` prints the public snapshot (peer ids, names, granted services, routes). `omarchy-t3-client omabridge-status` also connects every environment and reports which ones OmaBridge carries (`active`) and any connection error. `connections-view` prints the credential-free catalog view.

## Managing this device

Preferences → Connections → OmaBridge shows the same OmaBridge page as the Omarchy Control Panel, on the same device: ot3 sends each request unchanged to `omabridge manage`, with OmaBridge's own state directory and publishing unit (`omabridge-serve.service`). Both front ends therefore show the same identity, peers and state, and offer the same management. OmaBridge validates every request, requires `"confirmed":true` for changes (sent only from a dialog's confirm button), and keeps keys and invitation tokens. The binary is found as the Control Panel finds it: `OT3_OMABRIDGE_BIN`, `omabridge` on `PATH`, `~/.local/bin/omabridge`, then the copy bundled with ot3. An OmaBridge without `manage` is reported as outdated.

The page covers this device's identity and name, publishing T3 Code and SSH, the relay (public, a custom HTTPS relay or direct only), one-use invitations (optionally asking to manage the device that accepts, with a QR shown once), pairing from an invitation file (inspected first; one that asks to manage this device needs explicit consent), approvals, revocation, letting a paired device manage this one, and managing paired devices that allow it: rename, publish, relay, a hosted relay, approvals, revocation and public gateways. *Refresh* also asks paired devices for their current names.

Public gateways let T3 Code's own apps, which cannot run OmaBridge, reach this device: a managed device with a public IP opens an HTTPS address that leads only to this device's T3 Code, guarded by T3's own pairing. Open, check and close it from that device's management dialog. In Access, the pairing dialog for this device offers the running public addresses as endpoints, so the link and its QR carry that address.

Per-connection routes use the same device and binary. While this device is published, an OmaBridge with the `forward-through-server` feature (listed in `omabridge manage`'s `features`) opens each route's forward inside its running server, so the device's identity never connects twice. An older OmaBridge needs a separate identity for routes, set with `stateDir` in the routes file.

## Connection assistants

Expand *Connection assistants* to choose where the agent runs and either *Set up a new host…* or *Connect existing…*. Both open a new chat in the host's scratch project and send the prompt at once: the agent works over SSH from start to end, and the user only watches.

*Set up a new host…* installs another host and pairs it with this one. Choose the SSH host, what it should run (OmaBridge only for its SSH, a headless server with T3 Code as a background service and OmaBridge, Omarchy with ot3, or T3 Code and OmaBridge on another desktop), whether this device may manage it, and whether it may also reach this device's SSH. The prompt pins the OmaBridge release and its hashes, uses `omabridge manage` on the new host, and asks only before sudo, firewall or port changes.

*Connect existing…* pairs a host whose OmaBridge already has an identity and publishes T3 and/or SSH. It never installs, updates, recreates the identity, publishes services or changes T3, SSH or firewall configuration, and it keeps the host's peers and grants; a missing prerequisite stops it. It always runs on this device.

When the agent runs on this device, the other host invites and this device claims: the agent writes the invitation on the host into a private directory, copies it here over SSH without printing it, pairs here with `omabridge manage`, and approves this device's identity on the host (starting from ot3 is the approval). If management was chosen, it sets this device as the host's admin (`admin-set`); if SSH sharing was chosen, it grants the host this device's SSH. This device only dials out, so pairing works even when this device uses a private relay that admits only its paired devices, or a firewall drops inbound traffic. The agent verifies by peer ID, since an older identity of the same device can share its name, and quotes OmaBridge's error codes when a step fails. Finally, when the host runs T3 Code, it adds it to ot3's connections over SSH with `omarchy-t3-client connections-manage`.

When the agent runs on another host (for a new host only that one reaches), *Start* first makes this device write a one-use invitation that expires in an hour (asking to manage the new host when chosen); it is moved to that host over SSH, the prompt names it by path only, the new host claims it, and you approve it here.

## Bar indicator

*Show OmaBridge in the bar* (in the same section, off by default) adds a small indicator after the T3 status in ot3's bar widget. It refreshes the snapshot every minute and when opened. A red dot means requests are waiting for your approval. Its popup shows the same OmaBridge page as Preferences. Closing the popup closes any open dialog and forgets a shown invitation. *OmaBridge settings…* opens Preferences → Connections, where per-connection routes and the bar toggle live. Middle click refreshes.

## Other front ends

Other front ends manage this device by calling `omabridge manage` themselves, as the Omarchy Control Panel does. ot3 adds:

- `omarchy-t3-client omabridge-manage`: one `omabridge manage` request on stdin (at most 64 KiB), answered as `{"output":<OmaBridge's JSON>,"status":<exit code>}`, with the same binary lookup as Preferences.
- `omabridge-install-prompt` (a backend operation): `{host, profile, existing, manage, shareSsh, invitation, device, deviceId, runsOn}` to that prompt; run here (`runsOn` empty) it needs this device's `deviceId` and no invitation.
- `omarchy-t3-client connections-manage`: one connections request on stdin, as Preferences sends it (`save-route`, `rename`, `enabled`, `remove`, with the `revision` from `connections-view`), answered with the saved connections or `{"error":<code>}`. The assistants use it to add the paired host's T3 Code; the running app notices the change by itself.
- `omarchy-t3-client omabridge-read`: the routing snapshot (`available`, `version`, `identity`, `peers`, `default`, `routes`, `active`, `revision`, `error`).
- `omarchy-t3-client omabridge-action`: one routing request on stdin. `set`, `clear` and `default` change routes (with `revision` and `"confirmed":true`). `gateways-read` and `gateway-check` read this device's public addresses from every device it manages, for the Access pairing dialog; `gateway-open` and `gateway-close` (confirmed) open or close one. On failure it prints `{"error":"<code>"}` and exits 1.

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


Other checks:

- Unit tests pin the bridged SSH arguments.
- `tools/test-qml omabridge-indicator` covers the bar indicator's refresh, status, pending badge, two-step approval, *Check again* and the settings hand-off.
- `tools/test-qml omabridge-device` covers identity creation, the confirmed approve, revoke and pair payloads, *Check again*, invitation services and QR, and clearing the shown-once invitation when its dialog closes.
- `tools/test-qml connection-omabridge` covers the automatic default, eligible connections and services, the confirmed pin/strict/automatic/saved-only/default payloads and the live status.
- The `connections`, `connection-network`, `connection-access`, `preferences` and `i18n` QML checks pass, and translations cover 20 locales.

**Live check:** with an empty route file (automatic), `omabridge-status` connected a paired SSH host through its `ssh` service with no configuration, verified against the real `known_hosts` entry. A raw `t3` forward also answered HTTP 200.

Away from the LAN, a NAT-ed peer needs an OmaBridge relay (`relayUrl`).
