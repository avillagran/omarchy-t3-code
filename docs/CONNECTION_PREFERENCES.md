# Native Connections preferences

Source parity: `apps/web/src/components/settings/{LoadBalancingSettings,GitHubRoutingSettings}.tsx`, `packages/client-runtime/src/{load-balancing.ts,connection/githubRoutingPermissions.ts,platform/storageDocument.ts}`, `apps/desktop/src/settings/DesktopClientSettings.ts`. Native Electron stores client weights in `userdata/client-settings.json`; routing trust entries belong in the **encrypted** `userdata/connection-catalog.json` under `githubRoutingPermissions`. This implementation introduces no second registry and exposes no credentials.

## Integrated application behavior

`T3Backend.qml` reads the native preferences and refreshes them after catalog changes. `connections_management.rs` dispatches `preferences-read`, `client-update`, and `github-update` under the existing catalog lock. It returns the actual primary environment UUID, rejects unknown or disabled targets, checks the client/ciphertext revision, and saves GitHub permissions through encrypted catalog persistence. QML shows folded controls for enabled environments and preserves prior values while requests finish.

`connection_route.rs` resolves automatic, attachment-free new drafts against the native repository grouping key and corresponding destination project ID. It checks provider availability/authentication and fresh native host-resource snapshots before native weighted scoring. Explicit environment choices and branch/worktree launches retain their chosen destination. `T3Backend.qml` requests destination workspace preferences after routing; launch and retries stay bound to that environment.

`connection_github.rs` honors these permissions for PR details, checks and activity reads. Both endpoints must permit sharing, have trusted saved routes, and report the same account and GitHub host. Permissions and environment bindings are refreshed before dispatch. Credentials stay on the environment running the operation. Existing PR writes are not newly routed by this adapter.

Client settings preserve unknown fields, legacy wrappers and symlinks. Electron does not take O3's file lock: revision checks immediately before replacement detect stale writes but cannot guarantee an atomic cross-process CAS.

## Test boundary

`cargo test --test connection_preferences` uses only isolated temporary client JSON and synthetic decrypted catalog fixtures, exercises revision conflict, legacy wrapper/symlink preservation, malformed input, route-key stability/revocation, redacted projection, and CPU/memory scoring. No live native preferences, auth sessions, messages, network, or installed runtime are modified. The encrypted management integration test also exercises preference writes through the production dispatcher, stale revisions, unknown environments, and ciphertext preservation.
