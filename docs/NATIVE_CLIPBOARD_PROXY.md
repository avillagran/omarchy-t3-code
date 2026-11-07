# Private native clipboard selection proxy

This is an isolated prototype plan, not an enabled product capability. The owned native guest remains gated. Wayland calibration in `NATIVE_ELECTRON_GUEST.md` demonstrates that a hidden Electron process retains its private clipboard cache without receiving current compositor offers. Visible native hosting works after real seat focus. Injecting one snapshot at a permission callback cannot fix remembered Allow freshness.

## Owned topology

The proposed platform bridge keeps the actual Electron Clipboard API, native BrowserSession policy and Chromium data conversion. It does not replace `navigator.clipboard` or introduce a new permission grant.

- The real Qt application remains the compositor client that owns the Sidepanel's seat focus and operating-system clipboard interface.
- Electron and its native webview use a dedicated private X display. The display has no TCP listener and no host display connection. Its owner, socket, authorization material and lifetime belong to the actual native guest process cohort.
- A native member connects to that private display and owns a proxy CLIPBOARD selection. It reads system MIME data lazily in response to an admitted engine selection request. Clipboard offer-change notification invalidates metadata/cache identity without reading contents.
- In the other direction, an actual engine selection ownership change can publish a lazy Qt MIME offer. A later system paste requests data from that exact engine owner. It must not silently serve an older owner after replacement.
- The native guest's existing process/socket/inspector/tab lease remains authoritative. Clipboard admission is tied to that live physical owner, actual native permission callbacks and actual document lifetime, not a thread identifier or credentials hash.

The first isolated experiment uses a mapped Qt raster window as the platform member and a separate real Qt producer/receiver. This models the application's real focus requirement; it does not establish that a separate background helper can read the desktop clipboard.

## Permission and identity requirements

Actual native positive permission checks and request callbacks are the only permission authority. The synchronous remembered-Allow path must be supported without changing its return value. A granted permission query alone must not retrieve content. The bridge must receive a physical engine selection request before touching MIME data.

Selection requests must originate from the admitted native engine client. A private display is not a general-purpose X server. Check the requestor's server-reported local client identity; reject other clients, unsupported targets and synthetic selection events before requesting system metadata or bytes. The prototype will exercise these refusals. Possession of a fully trusted X authorization cookie is not claimed to isolate arbitrary mutually hostile X clients; production cookie and socket admission must confine access to the intended members.

Control messages travel only over a dedicated inherited private channel. A production channel needs verified peer credentials, bounded records, ordering between permission and selection messages, and fail-closed handling of backpressure. An unresolved race between permission admission and an X event must fail without fetching data. No permission failure may be converted into a remembered user denial.

Tab/frame navigation, inspector replacement, environment/profile changes, pipe closure and process failure revoke pending leases. Check revocation before fetching and again before publishing data. A stale clipboard callback cannot close or overwrite a newer owner. Multiple allowed origins must retain their actual native policy while concurrent requests and MIME formats remain independently bounded.

## Limits and failure behavior

The initial controlled prototype will cap metadata at 128 MIME targets/64 KiB, records at 4 KiB, concurrent transfers at eight, retained transfer bytes at 16 MiB, and each transfer lifetime at five seconds. These are explicit prototype limits, not native parity claims. Unsupported or oversized requests receive a failed selection response; they are never silently truncated.

Large X transfers need the real INCR handshake, property deletion acknowledgements, bounded chunks, completion and cancellation tests. A target or owner replacement must cancel outstanding transfers. Self-generated Qt and X offer notifications require explicit loop fencing; they must not become a clipboard watcher or cache of historical contents.

The public Qt `QClipboard` API can allocate a MIME payload before a caller checks its size. A controlled fixture can prove lazy semantics with bounded synthetic data, but a post-fetch cap is not a preallocation bound for arbitrary hostile clipboard offers. A production implementation must resolve that distinction, potentially with a native streaming offer reader on the application's compositor connection. No general memory-safety claim follows from the initial prototype.

No proxy payload enters console/status logs, RPC projections or general UI state. Diagnostics contain only bounded counts and static error codes. No root filesystem extraction, recursive fixture cleanup, host compositor changes or live user profiles are part of these experiments.

## Acceptance sequence

First prove lazy ingress and remembered-Allow freshness with actual native Session/Host, real text/PNG sources, and zero system data retrieval before native permission. Then prove denied foreign clients and synthetic events, unsupported metadata, owner replacement, large INCR transfer and revocation. Finally prove lazy egress, loop prevention, process teardown and the complete product Pool → actual Qt clipboard path. Keep the public selector gated throughout. Qt-native keyboard/IME production remains a separate frontend requirement.

## First isolated results

The initial ingress fixture `native-electron-owned-rd7bd__q` retrieved no system content before native Allow, then served fresh text, changed text under remembered Allow, and correct PNG pixels to the actual hidden native guest. It made exactly three content pulls totaling 158 accepted bytes. No navigator API was replaced and no permission was added.

The expanded physical protocol fixture `native-electron-owned-v2dn7kux` completes with the same native policy and real source/receiver applications:

- A client lacking the private X authorization fails to connect. A different client with a valid fixture cookie is refused using the server-reported local client PID. A synthetic selection event is also refused. These cases retrieve no system contents.
- A native 200,000-character read completes through real INCR property/deletion handshakes. An explicit paused-delivery boundary permits revoking a real pending transfer; the browser receives zero characters, pending transfers become zero and content-pull count does not increase after revocation.
- Too many MIME formats are refused before content retrieval. A 17 MiB text body is refused without returning a truncated success. The fixture makes the preallocation distinction measurable: maximum underlying Qt fetch is 17 MiB and aggregate fetched bytes exceed 53 MiB because the native reader tries fallback targets. The 16 MiB limit bounds retained accepted transfer data, not the underlying public Qt MIME allocation.
- Fresh text and the PNG survive subsequent remembered native checks. An earlier diagnostic exposed an ownership bug: rotating the X selection on permission refresh invalidated ClipboardItem data between `read` and `getType`. Ownership now changes only for actual offer notification or provider revocation; a permission refresh changes neither the offer nor its identity.

The fixture's control channel is an inherited parent pipe and admission is currently physical engine-process scoped. It does not yet implement the production inspector/tab/frame lease protocol. Its explicit pause is a test delivery boundary, not a browser permission override. Copy/write egress, hostile concurrent transfer-cap coverage, offer-loop prevention, parent/profile retirement integration and the actual application Qt member remain unfinished. The public selector stays gated.
