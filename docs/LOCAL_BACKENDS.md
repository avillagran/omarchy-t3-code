# Choosing the local server

The sidebar's offline play button opens a keyboard accessible chooser for O3
Server or T3. Opening or cancelling it never starts a server. Connections uses
the same chooser. Running servers retain their ownership checked stop/restart
controls; an installed desktop application's server is read only.

O3 and T3 can have separate user services, profiles and CLIs. Installers can
register them in `$XDG_CONFIG_HOME/omarchy-t3-code/backends.json` (the default
config root is `~/.config`):

```json
{
  "version": 1,
  "default": "o3",
  "servers": [
    {"id":"o3","unit":"o3-server.service","profile":"/absolute/o3-profile","cli":"/absolute/o3-server"},
    {"id":"t3","unit":"t3code.service","profile":"/absolute/t3-profile","cli":"/absolute/t3"}
  ]
}
```

These are installed user services, not shell command templates. The unit's
`Environment=T3CODE_HOME=...`, including user drop-ins, must match its profile.
The registry accepts only `o3` and `t3`, distinct unit names, and
absolute paths. Missing or incompatible installations remain unavailable. A
malformed registry disables lifecycle control instead of launching a fallback.
Without a registry, existing T3 installations retain their original behavior;
the O3 choice is marked not installed.

## Installed by install.sh

Choosing O3 Server in `install.sh` (or `O3_SERVER=o3`) writes, only when each is absent:

- `~/.local/share/o3-server/bin/o3-server`, the release pinned in
  `integrations/o3-server/release` and installed by `bin/fetch-o3-server` (verified against the
  pinned hash and the release's `SHA256SUMS`, like OmaBridge);
- `~/.config/systemd/user/o3-server.service`, marked `# Written by the O3 Code installer.`:
  `o3-server serve --no-bridge --base-dir ~/.local/share/o3-server/profile` with a matching
  `T3CODE_HOME`, listening on `127.0.0.1:3773`, enabled at login. The bundled OmaBridge CLI keeps
  the device's single bridge identity, so the embedded bridge stays off;
- a registry with only `o3`, its default.

Later runs update the pinned binary and restart only a unit carrying that marker. A unit or
registry written by hand is kept as is; a registry without `o3` stops the installer instead of
being rewritten. T3 is neither installed nor opened for O3 Server.

An explicit choice is stored in the client state directory's
`backend-choice.json`. Local transport discovery and credential issuance use
that server's profile and CLI; saved remote connections and UI preferences stay
in the client profile. Each registered server has a separate local token file.
A verified server already running at login takes precedence over a stale saved
choice. A control lock and live-runtime checks serialize lifecycle actions and
prevent competing launches.

## Shared-profile webhook boundary

Both registrations may explicitly name the same profile. The runtime identity
then determines which server owns it. Before starting T3 on that profile, the
client invokes the matching O3 CLI's `shared-profile-readiness --base-dir PROFILE`
diagnostic. An older O3 CLI without this command cannot verify handover and the
client reports that verification is unavailable.

Legacy queued webhook deliveries and retained duplicate-protection claims block
handover. These rows lack the original native request and cannot be converted
losslessly by the client. Even claims with old expiry values remain significant
because legacy admission checked their presence rather than expiry. The check
never clears them. O3 remains available to preserve and reconcile that data.

A switch checks readiness before stopping O3 and again after shutdown, so a
delivery admitted during shutdown cannot be silently abandoned. A failed initial
check leaves the running process alone; a failed post-shutdown check leaves both
servers stopped and preserves the O3 selection. Unknown, failed or inconsistent
reports also refuse the switch. This boundary covers the client's lifecycle
controls; an external direct launch of T3 bypasses it. A clean report does not
certify every aspect of shared-profile parity.

Starting resets the chosen unit's failure limit and waits for its native runtime
and HTTP readiness. Failure is reported without trying the other server. No
profile is copied, migrated, removed or repointed. This does not enable or change
boot services. Installers must preserve the user's existing autostart policy.

Native T3 network configuration and its updater are limited to the original T3
unit/profile; custom/O3 units do not receive T3-specific launch/update overrides.

Checks include Rust targets `backend_service`, `backend_shared_profile`,
`backend_handover`, `connection_network` and `server_updates`, plus QML fixtures
`backend-choice`, `backend-start` and `backend-owner`. Execute them through the
container runners described in [RUST-TESTS.md](RUST-TESTS.md). Lifecycle checks
use private services/profiles, including a real temporary process that admits
retained work while handling shutdown.
