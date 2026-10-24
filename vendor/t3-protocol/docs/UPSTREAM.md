# T3 source reference

The transport and Android boundary were checked against original T3 Code source
at commit `5a4e9980` in the local `t3code-nightly` checkout on 2026-10-16.

| Contract | Original source |
| --- | --- |
| Streaming RPC declarations | `packages/contracts/src/rpc.ts`: `WsOrchestrationV2SubscribeThreadRpc`, `stream: true` |
| Thread replay/snapshot/live events | `apps/server/src/ws.ts`: `subscribeOrchestrationV2Thread`, `afterSequence`, bounded/compact snapshots, completion markers |
| Attachment grants | `packages/contracts/src/assets.ts`: `AttachmentCreateUploadUrlInput/Result`, `AssetCreateUrlInput/Result` |
| Provider send limits | `packages/contracts/src/chatAttachment.ts`: 10 MiB/image, 50 MiB/file, supported image MIME types |
| Desktop behavior using the same core | Omarchy T3 Code's pinned `t3-protocol` dependency and integration fixtures |

Version 0.3.0 brings back the changes Omarchy T3 Code made to its vendored copy
of this crate while following T3 0.1 and 0.1.2:

| Behavior | T3 reference |
| --- | --- |
| Stream liveness (5 s ping, 15-20 s timeout) | client runtime `rpc/protocol.ts` |
| Sidebar status, snooze wake and woke time | client-runtime `models.ts`: `resolveSidebarThreadStatus`, `presentThreadShell`, `threadRaisedHandWhileSnoozed`, `threadWokeAt` |
| MCP app references | `mcpAppFromToolItem` (`t3McpApp` in a completed tool call's output) |
| Partial-payload projection events | T3 0.1 events `thread.pull-request-link-updated`, `provider-session.detached`, `run.background-work-cancelled`, `checkpoint.rollback-requested`, `context-handoff.*`, `context-transfer.*` |
| Device screens | T3's native device hub proxy for expo-device-hub (serve-sim / serve-emu) |

The JNI bridge marshals UTF-8 bytes and file descriptors only; it introduces no
alternative HTTP/WebSocket/authentication implementation. Queue limits are local
host safeguards, not values supplied by T3. Existing native envelopes and cursors
are preserved. New T3 releases can change contracts: update the shared library
and consumers at explicit revisions after checking source and fixtures again.

This is an independent community library, not an official T3 SDK.
