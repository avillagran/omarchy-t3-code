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

`stateDir` defaults to OmaBridge's `$XDG_DATA_HOME/omabridge`, and ot3 never creates an identity. `relayUrl` must be HTTPS. A malformed document disables every OmaBridge route instead of guessing. ot3 only runs the bundled `bin/omabridge` next to the client (see below), never one from `PATH`; `OT3_OMABRIDGE_BIN` overrides it for development. Pairing, approval and revocation happen in OmaBridge (`omabridge-gui` or its CLI), not in ot3.

Diagnostics: `omarchy-t3-client omabridge-read` prints the public snapshot (peer ids, names, granted services, routes). `omarchy-t3-client omabridge-status` also connects every environment and reports which ones OmaBridge carries (`active`) and any connection error. `connections-view` prints the credential-free catalog view.

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
- `tools/test-qml connection-omabridge` covers the automatic default, eligible connections and services, the confirmed pin/strict/automatic/saved-only/default payloads and the live status.
- The `connections`, `connection-network`, `connection-access`, `preferences` and `i18n` QML checks pass, and translations cover 20 locales.

**Live check:** with an empty route file (automatic), `omabridge-status` connected a paired SSH host through its `ssh` service with no configuration, verified against the real `known_hosts` entry. A raw `t3` forward also answered HTTP 200.

Away from the LAN, a NAT-ed peer needs an OmaBridge relay (`relayUrl`).
