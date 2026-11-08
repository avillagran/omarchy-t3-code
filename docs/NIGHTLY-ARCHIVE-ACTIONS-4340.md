# Nightly 4340 archive actions

The archive row now exposes a direct Delete button beside Unarchive, with the
translated tooltip and a title-specific accessible name. It uses the existing
scoped native action and delete-confirmation preference. Opening confirmation
locks both row actions immediately, including an already-open context menu;
cancel releases the row without sending a command. Accepted or uncertain
commands retain the existing synchronous action guard and explicit retry flow.
The confirmation names the archived thread even when no active thread shell
is present. Changing the selected project scope closes a stale confirmation.

This increment follows the complete `ArchivedThreadsPanel` and
`useThreadActions` sources at nightly 4340 revision
`a9e709f15f825cacac1b6d665fa1920c328d68b7`. The exact original and local source
hashes are recorded in `docs/nightly/archive-direct-delete-proof-4340.json`.
No global validated upstream pin changes.

The existing grouped archive fixture now clicks the actual row Delete button
and confirmation button in Qt. It verifies cancellation, both pending states,
duplicate and competing-action refusal, real context-menu deletion, stale scope,
120 virtual rows, retained scroll position, immutable origin, explicit durable
retry, and the shared delete-confirmation preference. It passes through
`python3 tools/test-device-panel-container archive`, using the existing pinned
rootless software-display image, read-only repository, private HOME/tmp/config,
inert worker and dictation helper, and no network or host display/devices.
The six exercised source files remain hash-identical before and after runtime.

Evidence: `/var/tmp/o3-device-panel-8xowm5ip/{result.json,qml.log,command.json,archive-source-fence.json}`.
The transport supplies explicit synthetic read/action receipts. This verifies
real QML behavior; it executes no live server RPC, real thread deletion, Rust
backend, provider credentials, or complete Nightly 4340 application.
