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

The unchanged native Session and engine are the permission authority. A physical selection request from the exactly admitted engine may represent normal paste even when programmatic clipboard-read remains blocked. The bridge must not impose a second cached-Allow requirement or a human-only write window. A permission query alone must not retrieve content; a real admitted engine selection request is required before touching MIME bytes. The initial grant-gated prototype described below is historical evidence and requires this correction before product use.

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

## Human-copy egress feasibility

The isolated native-copy fixture `native-electron-owned-tmcjv60v` admits an explicit human-copy action before the actual webContents copy operation. It verifies the native X owner and selection timestamp, obtains bounded target metadata, and publishes a Qt MIME offer. A different real Qt application receives the exact selected text. A subsequent programmatic read in the same blocked native session remains denied: publishing a human copy does not grant clipboard-read permission, and system-content retrieval count is still zero before the native Allow step.

Qt 6.12 Wayland requests `text/plain` while setting this offer: one 28-byte pull from the engine-produced copy occurs during publication, followed by a second pull when the other application pastes. This is current admitted copy data, not a system clipboard watcher or a history cache. The fixture records this behavior and does not claim strictly lazy Qt egress. Retrieval checks the captured owner and timestamp and bounds X property/INCR data before accumulating it. Unknown formats and changed owners fail rather than serving another owner's data.

This remains a process-scoped feasibility fixture. It does not establish the full production human-command nonce, tab/frame retirement, simultaneous writes, current-copy retention across profile-process shutdown, all custom MIME formats or product Pool-to-application Qt integration. The normal selector is unchanged.

## Next native platform member

The installed public Qt 6.12 `QNativeInterface::QWaylandApplication` exposes the actual application display, seat, last input serial and last input seat. An additional native data device on that same compositor connection can receive real offers and stream MIME data through nonblocking pipes, enforcing byte limits before allocation. A native data source can publish engine data using the actual Qt application's input serial without calling the Qt MIME setter that eagerly requests text. This needs physical validation; the existence of these public interfaces is not proof of correct offer ownership or delivery.

That member must remain inside the actual Qt application's compositor client, use a separate event queue without reading or dispatching Qt's private queue, and release only resources it owns. It must retain the actual offer object's lifetime, refuse another seat or revoked engine source, stream with backpressure, and avoid data retrieval on metadata notification. Production integration additionally needs authenticated local parent/engine registration and the existing real Pool/process lease. No browser-visible API shim or permission-state change is part of this plan.

## Native offer streaming result

The shared `native/browser/waylandoffers.h` member now replaces the original whole-payload Qt getter for native Wayland ingress in the isolated fixture. It uses the public Qt display/seat interface, a private wrapper and new registry/data-device/offer proxies. It neither changes listeners/queues of Qt-owned proxies nor reads or dispatches the default queue. Registration is asynchronous; Qt retains socket reading and normal GUI dispatch, while a timer dispatches pending events only on the member's private queue.

`native-electron-owned-4i1v4g8u` accepts a complete 16,777,216-byte valid text value through the native browser and rejects a 17 MiB value without a truncated success. Before Allow, stream and reserved-byte counts remain zero. Fixed 64 KiB blocks bound reserved read storage to 16 MiB; successful assembly can add at most another 16 MiB, and the observed estimate reaches 32 MiB in the exact-cap case. These figures describe this member's payload buffers, not the entire process, toolkit or browser memory. Metadata notification retrieves no contents. The original Qt whole-MIME allocation problem remains documented above as a measured rejected approach, not a property of this streaming ingress.

`native-electron-owned-ls6p2kl3` revalidates the same native clipboard, copy, permission, identity, INCR, cap and revocation assertions after removing synchronous Wayland roundtrips from member initialization. Existing Qt keyboard/drag-and-drop behavior with the additional data device still needs explicit enabled/disabled validation. A GUI-process QML member, authenticated worker/cohort FD registration and current-copy lifetime handling remain product-integration gates.

## Toolkit noninterference proof

The private `wayland-member-interop-g_irf_yl` fixture exercises the same real QtQuick target with the member disabled and enabled. A real compositor virtual keyboard types `QtKeyboard42`; an independent Qt client drags `owned-wayland-drop` through the normal Wayland drag protocol into a real DropArea; and the default Qt clipboard transfers text between the clients. Both variants record exactly one drop, the expected keyboard/clipboard values and zero member content-stream bytes. No QDropEvent or accepted result is injected. The virtual-pointer test protocol is pinned to wlr-protocols revision `b010a03648b88d143236de193bddbfea0c08bc84`, preserving its original MIT notice and source manifest. This validates the additional data-device/private-queue behavior for these toolkit roles; it does not establish all production GUI hosting and lease behavior.

## Native input and lifetime baseline

The exact 4301 Session/Host with Electron 44.10.1 passes `native-electron-owned-5_x03toe` on an owned compositor with a real pointer and keyboard. Normal Ctrl+V pastes external text while programmatic `readText` stays blocked. A genuine page button click has both `isTrusted` and active user activation; its sanitized `writeText` succeeds and a separate Qt application receives the exact bytes, without a new read prompt. The lock-pinned Playwright 1.60.0 Keyboard and Chromium RawKeyboard classes also copy and paste exact text through the unchanged native Host while programmatic read remains blocked. The fixture appends exports to the verified original bundle and initializes only those original modules; it does not copy a keyboard algorithm or invoke browser/artifact cleanup.

Current copied text remains available after destroying its source tab. A 2×2 PNG with RGBA `[18,52,86,255]` remains available to the actual Qt peer after destroying its source tab and closing the native services while the native main process remains alive. This is the native lifetime baseline, not proof that a separate private-engine process can be destroyed without a GUI-owned current-offer keeper. Production must retain at most the one current bounded engine-produced offer until it is replaced or the GUI closes.

The existing O3 `preview_press` backend currently omits modifiers and must be corrected independently before claiming end-to-end MCP chord parity. This fixture uses the actual pinned Playwright classes and exact native Host, not that incomplete O3 consumer.

## Corrected hybrid policy and current offer

`native-electron-owned-uz7gs2pi` applies the measured native policy to the hidden-engine/private-X topology. Native GuestInput paste and a trusted/activated page click write succeed, and actual Playwright Ctrl+C/V succeeds, while programmatic read remains blocked. The first blocked call has zero system stream bytes/content pulls; another blocked call after real paste adds neither. Foreign, synthetic and unauthenticated clients remain refused. No cached read-Allow or human-only publication gate remains. Native checks are observed without changing their return values; they do not grant or refresh the bridge lease.

`native-electron-owned-w2e6i8bi` passes the existing real INCR, exact 16 MiB body, oversized refusal, metadata cap, hostile-client and paused-transfer revocation cases with this corrected policy. Its main-process calibration explicitly reads the clipboard, so blocked-read assertions compare content counters immediately before/after the denied API calls rather than incorrectly attributing the privileged calibration reads to the page. The earlier grant-gated count of three pulls/158 bytes remains scoped to its historical fixture.

For egress, publication now captures only the current engine-produced supported formats, deduplicating PNG aliases, within a 16 MiB aggregate encoded-body limit. Each next receive is limited to the remaining candidate budget before accumulation. The one current Qt MIME offer owns these bytes independently of the engine selection owner; it preserves exact text/PNG after tab/services closure and explicit private-X selection clearing. Preparing a replacement can temporarily coexist with the old current offer; this is a bounded candidate, not historical clipboard storage. Decoding a supported PNG can additionally allocate up to 4 million pixels. No system-offer body is cached. Actual separate engine-process termination and worker/GUI process ownership remain production acceptance gates, rather than being inferred from clearing an X selection.

The actual engine-exit cases `native-electron-owned-w1qjpza4` (text) and `native-electron-owned-3hblmuo8` (PNG) now pass. Electron exits successfully before the private supervisor starts a new, independent Qt receiver. The same mapped GUI member remains on its original compositor connection, owning only the current offer. With genuine seat focus, the fresh receiver reads exact text or the exact 2×2 RGBA image. The fixture's explicit keeper mode disables its closed engine-control notifier and retires engine admission on EOF while retaining the current Qt offer; ordinary fixture mode still quits on EOF. Container teardown reclaims the retained GUI. This proves current-offer lifetime across actual engine exit in the isolated topology; it does not yet prove the real Quickshell worker/cohort FD registration or public product activation.
