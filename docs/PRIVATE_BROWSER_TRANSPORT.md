# Private browser transport foundation

The optional owned-server path is not exposed yet. Existing service mode and shared profiles remain unchanged. The planned owned path uses a separate environment and persistent profile, a directly supervised child process group, inherited bootstrap/browser descriptors, and actual authenticated readiness. It must never adopt or stop an existing shared daemon implicitly.

## Qt transport evidence

An isolated Qt 6.12 experiment creates a hidden `devToolsView` without remote-debugging TCP flags. A MainWorld user script installed at DocumentCreation, before assigning `devToolsView`, can intercept the native `InspectorFrontendHost.sendMessageToBackend` and `DevToolsAPI.dispatchMessage` boundary before frontend initialization.

`ui/browser/private-cdp-bridge.js` is an internal foundation, currently not wired into the product. It remaps every frontend and server request into one monotonic physical-ID namespace, restores the original IDs, routes frontend responses back to their dispatcher, and queues server responses privately. Events retain normal frontend delivery. Pending requests, messages, queued output, and chunk assembly are bounded. Closing retires pending state; it never replays a request.

The bridge contains no console or status output. Product integration must carry drained raw messages exclusively through the inherited private browser descriptor. The inspected application page must never receive this script or a native bridge. Each owner must fence it by environment, profile, lease, thread, tab, and actual target identity, and release it when that owner disappears.

`tools/test-private-cdp-qt` builds its standalone fixture and runs it only in private rootless Podman with read-only inputs, no network, and disposable fixture storage. Run long checks through a transient user service. The experiment sends concurrent `Runtime.evaluate` and `Target.getTargetInfo` commands through the actual Qt channel. Both original request IDs are restored, the returned synthetic title matches, and the actual target ID equals the page's `devToolsId`; the frontend reports no wrong-ID protocol error. Separate pure-boundary tests exercise collisions between frontend/server IDs, out-of-order responses, event delivery, chunk assembly, bounds, and release. Synthetic chunk tests do not establish that Qt's large-message transport emitted chunks: a 1.2-million-character actual response arrived through the ordinary dispatcher.

## Remaining integration

The raw transport is not complete native browser parity. The pinned native source is `32a0531748c44b25072d7495ce0b9bd7e9450a87`, especially `DesktopBrowserHost.ts`, `CdpRelay.ts`, and `GuestInput.ts`. It includes owner-scoped screencast/focus/painting behavior, child sessions, detach cleanup, and Electron-specific unfocused input/reload handling. Qt equivalents require actual behavior proofs.

The owned launcher, private descriptor writer, preview-open observer, guest lease registry, target correlation, lifecycle teardown, and privacy checks are still pending. Guest creation must react to authoritative opened events before an open RPC completes, and attachment must be announced only after acquiring the actual target lease. No UI capability should be advertised from this foundation alone.
