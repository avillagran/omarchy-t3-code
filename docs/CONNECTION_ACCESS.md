# Native connection and access settings

The QML Connections page follows T3 Code's `ConnectionsSettings.tsx` and
`ConnectionsSettings.logic.ts` in the local native checkout, plus its
`EnvironmentAuthHttpApi` and shared `authScopeOptions.ts` contracts. It uses the
existing picker buttons/fields, preference cards and switches.

The page contains the selected environment's version, network reachability,
server-supported machine icon, linked T3 Connect activity/webhook preferences,
a folded authorized-clients and pairing-links section, and the existing
native encrypted environment/route catalog. Changing the preferences environment
changes the access-management target without navigating the current thread.

Access management calls the same authenticated HTTP endpoints as native T3:

- `GET /api/auth/session`, `/api/auth/clients`, `/api/auth/pairing-links`.
- `POST /api/auth/pairing-token`, `/api/auth/pairing-links/revoke`,
  `/api/auth/clients/revoke`, `/api/auth/clients/revoke-others`.
- `GET /api/connect/link-state`, `POST /api/connect/preferences`.
- Environment icon changes use `server.updateSettings`, gated by the server's
  `environmentIcon` capability and `settings:write` scope.

`permissions` is authoritative when supplied, including an empty list. Read and
write access are separate; a failed/unauthorized client list is never presented
as proof that no other clients exist. Current-session revocation is disabled.
Revocation requires an explicit confirmation scoped to the selected environment.
Pairing grants default to the native standard-client set intersected with the
current grant. The terminal-operation checkbox also grants terminal-read;
removing terminal-read removes operation. Backend validation repeats these checks.

Only explicit creation returns a one-time credential. Copyable URLs use T3's
`/pair#token=...` format, with an editable destination address. Loopback and
unspecified IPs are never offered as QR targets. Optional `qrencode` generates
SVG locally with the URL on stdin, not process arguments. A missing QR encoder
never loses the created credential. Mutation receipts are delivered independently
of subsequent list requests, so a list error cannot silently create another link.
Closing or changing the view clears the credential; stale replies are ignored.

Local access management lazily upgrades the existing private local client token
through `t3 auth session issue`. It reuses that same token for chat and access
management; revoking other clients therefore keeps this client's grant. SSH
bootstrap likewise requests the native administrative grant through the remote
CLI. Bearer/relay pairing grants are never escalated: their actual scopes gate
the controls. SSH onboarding and access requests run outside the send/draft queue.
No live user links or sessions are created/revoked during fixture validation.

## Desktop-only boundaries

This is a QML port, not an embedded Electron settings page. Network exposure
uses the same read-only behavior as T3's web client: the server owner controls
its launch/restart configuration. Electron-only Tailscale exposure, cloud-account
sign-in/registration and client load-balancing/GitHub-sharing preferences are not
ported by this change. Existing native relay routes remain managed in the saved
catalog. Windows-only WSL controls do not apply on Omarchy. These differences
must not be described as complete desktop feature parity.

## Checks

`cargo test --release --test connection_access --test connection_management
--test connections --bin omarchy-t3-client` covers native HTTP contracts,
permission bounds, credential reuse, metadata-only refreshes, confirmed revocation
and catalog preservation. `tools/test-qml connection-access` and `connections`
exercise scope changes, native grant defaults, terminal permission dependency,
secret clearing, copying, lifecycle and removal guards. Preferences design and
backend reconnect checks cover the surrounding UI/runtime. Translation validation
covers all 20 catalogs.
