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

## Owned supervisor readiness leaf

`owned_backend::Owner` now implements a separate, unselected direct-child launch path. Its persistent profile is `XDG_STATE_HOME/omarchy-t3-code/owned-backends/o3`. The existing service controller is unchanged. Startup retains an exclusive profile lock, clears inherited environment variables, supplies a private bootstrap socket, and owns a child process group with parent-death handling. It omits browser descriptors while the server consumer remains gated.

Readiness requires version-1 runtime metadata with the actual child PID and an HTTP loopback origin, a successful bootstrap-seed OAuth exchange, and a successful authenticated `server.getConfig` RPC. Seeds and bearers are not stored by the launcher. Dropping the owner stops and reaps its child with bounded group termination and retains the persistent profile.

Five focused private-container cases pass: runtime identity/origin validation, final-file link refusal, private userdata link refusal, long control-socket path preflight, and actual CLI authentication/RPC/profile-lock/shutdown behavior. The actual CLI proof uses the supplied immutable readiness snapshot with SHA-256 `e88ec5f7cfad5e3737459ec63c5d33e0c695a339f366584386fc4fff371182dd`. This snapshot was captured from an active build around server `4c07f60`; it is not claimed to be a frozen exact-source build. Repeat against the frozen activation candidate before exposing the feature. `tools/test-owned-backend --server-fixture PATH` mounts only that explicit fixture file read-only and executes these selected cases in private rootless Podman.

Linux pathname control sockets require a path shorter than 108 bytes. The launcher refuses an overlong persistent `omabridge/control.sock` path before creating the profile; it does not relocate bridge identity into ephemeral storage. General long-profile support remains open. Initial long-fixture launch failures were diagnosed as this control-socket constraint, then the owned temporary fixture was shortened. No live services or shared profiles were changed.

## Pinned relay virtualization

`ui/browser/native-cdp-relay.js` is generated directly from the 257-line native `CdpRelay.ts` at the scoped revision above, with source SHA-256 `9c7aa34d4b110777c896f412be1f4eff6c60f08d39fe9b3bae2ad69f22f45754`. `tools/build-native-cdp.cjs` checks that hash before stripping TypeScript and wrapping the unchanged factory. Upstream copyright and MIT permission are retained in `licenses/T3-Code-MIT.txt`. This intake does not change the global nightly pin.

`tools/test-native-cdp.cjs` executes the actual pinned TypeScript module and the generated factory against the same synthetic target driver. The 33 traces match across 35 frames and 10 target calls: browser/page virtual sessions, target discovery/attach/detach, auto-attach announcements, child sessions, download routing, out-of-order completion, failures, malformed commands, and payload budgets. Oversized printable and escaped command results fail only the command; an oversized event closes the relay.

The native factory retains its 64-Mi-unit serialized payload estimate. The low-level debugger bridge separately bounds decoded JSON messages and queued output at 128 Mi UTF-16 units. It does not apply that limit to the outer escaped NDJSON envelope. The future descriptor parser needs independent framing headroom, including JSON escaping and UTF-8 byte accounting. A raw debugger response above the low-level limit retires the bridge; exact unbounded-response behavior is not claimed.

The ordinary hidden Qt frontend initializes debugger domains, target discovery, and metrics instrumentation of its own. The automation-only minimal mode now suppresses every frontend-origin request from DocumentCreation and leaves its promises unanswered. It sends only explicitly owned commands through the captured original native function. It does not fabricate successful frontend responses. Normal nonminimal mode still preserves frontend request/reply behavior.

The actual Qt fixture includes another same-profile page. Five concurrent owned commands prove the real target identity, correct title, absence of native bridge globals in the inspected application page, and completion of a `debugger` statement without a pause. Explicit Runtime enablement yields only the owned default context; the captured stream contains no frontend debugger/discovery/metrics events. Eight pure-boundary fixture groups pass, including minimal-mode suppression. Opening real DevTools must retire the automation lease and replace its inspector rather than enabling frontend traffic on the existing lease.

Composition remains gated on per-lease target/session filtering, dedicated-descriptor wiring, and actual input/focus behavior. Copying the pure factory and suppressing hidden frontend traffic do not by themselves prove a complete CDP endpoint.
