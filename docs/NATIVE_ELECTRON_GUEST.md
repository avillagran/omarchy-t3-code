# Native Electron guest foundation

The optional owned browser is being evaluated with Electron 44.10.1 and the actual T3 Code Nightly 4301 BrowserSession, DesktopBrowserHost, GuestInput, and CdpRelay modules. The production Sidepanel still uses the existing Qt guest. These tools do not install Electron or enable a renderer selector.

## Pinned inputs

Native source revision: `4cb6945dc60ed03f567dc5daafd083e52308e82b`. BrowserSession SHA-256: `9e66dd7d6c8d6e376d7f9a35ae1f58d82225fb5a5549781551baaba89adc3f03`.

The official Linux x64 Electron application ZIP has SHA-256 `2be868b71b0aa71f30e6b5567a106a35a049385e963183c074aad1975ead5167`. `tools/acquire-owned-electron-runtime` checks the official release metadata, checksum and size, rejects archive links and special files, and retains a per-file manifest. It runs acquisition in a private container. This is an application archive; operating-system root filesystems are not extracted.

`tools/build-native-electron-services.mjs` consumes a source directory whose `SOURCE.json` contains `revision` and a `files` map of source-relative paths to SHA-256 digests. It requires Effect 4.1.2, @effect/platform-node 4.1.2 and Rolldown 1.2.16. All Effect imports resolve to one canonical dependency tree. The bundle retains real Electron and Node builtins; the native dialog service receives an explicit platform dialog boundary. Native algorithms are not replaced with fixture implementations. Generated source/dependency hashes accompany the bundle. Distribution must retain `licenses/T3-Code-MIT.txt`, `licenses/Effect-MIT.txt`, and the official Electron runtime licenses.

## Private physical acceptance

`tools/test-native-electron-owned --runtime DIR --services BUNDLE` freezes inputs, verifies every runtime file against the manifest and starts a private X11 display inside rootless Podman with networking disabled, read-only inputs and a private home. A separate Qt application owns the synthetic operating-system clipboard. Neither the host display nor the host clipboard is mounted.

The first passing evidence is `native-electron-owned-8cybzvdd` under the private desktop build cache (result 0). It demonstrates:

- A real `webview` guest using the native preferences: sandbox enabled, Node integration disabled, and context isolation disabled. The sandbox preload confirms the actual sandbox; the document has no require, process, Buffer or native-host globals.
- Real host-webContents, partition/session and DevTools target identity, with actual native attached-event registration before CDP commands.
- Block and remembered Block, new-origin Allow and remembered Allow, with fresh text and PNG pixels obtained from the separate operating-system clipboard owner. Another session still prompts and can deny.
- Explicit human `webContents.copy()` transfers selected synthetic text to the other application without adding a clipboard-read prompt.
- Native root relay rejection of a foreign target.

In this actual native baseline, a DevTools `Runtime.evaluate` call to `navigator.clipboard.writeText`, even with `userGesture: true`, requests clipboard-read permission and is denied after Block. A DevTools `execCommand('copy')` returns false. These outcomes are preserved rather than converted into automatic grants. The successful explicit native copy operation is a separate tested path.

A second private physical proof, `native-electron-owned-cp68mcf1` (result 0), confirms that Electron's raw page debugger can inspect a different real guest with `Target.getTargetInfo`. The existing transport-independent target guard rejects nine foreign-target, discovery, browser-context, Browser-domain and foreign-session cases on actual Electron, while preserving opaque Runtime strings. Closing the guard rejects subsequent commands. This guard must sit below the native Host in the eventual adapter; the root relay alone is insufficient. Child-session admission and the complete adapter remain separate acceptance gates.

## Internal product route

The explicit `browser_guest::Config::with_native_electron` option now routes the existing product Pool through the real client `browser-render` wrapper to the native main-process adapter. It is not selected by the default configuration. The wrapper retains its profile lock while supervising Electron, admits the original private socket before launch, and supplies a distinct high descriptor. A small N-API boundary rechecks the original peer PID/UID, UNIX stream type, and sets/verifies close-on-exec before creating guest contents. Electron data uses a separate `native-electron` subdirectory; Qt's cookie/cache layout is not reused.

The focused proof `native-electron-pool-ch7e2ig7` passes one actual product integration test with zero ignored tests. It proves nonce/expected/proof/admitted ownership, the actual native attached event and virtual target/session, page Node-global isolation, foreign-target rejection, and a 640×480 PNG from the same guest controlled by CDP. The center pixel matches a synthetic DOM background (18, 52, 86, 255), and `close_profile` reports successful process exit. The ordinary Electron webview `capturePage` call fails in this setup; the adapter captures the owned page through the guarded `Page.captureScreenshot` boundary instead.

The private socket proof `native-electron-owned-uvl4xa1r` additionally constructs the real Node socket before verifying descriptor flags and scans every inspectable process in its private PID namespace, including possible reparented crash helpers. No other process holds the admitted socket inode, except the explicit fixture peer. Runtime hashes, adapter hashes and immutable Cargo executable/library hashes are retained by the runners.

The native Session's actual two-choice question options now cross the existing global application permission broker as typed metadata. The modal renders message/detail literally and uses the original button labels and cancel default; it does not classify permissions by parsing English text. Busy/cancelled sources resolve the platform boundary without a remembered user choice. The focused actual QML modal proof `o3-device-panel-ixpve53c` verifies the legacy clipboard flow and this native-options flow. A complete physical native read → application modal Allow → clipboard bytes roundtrip through this new Pool adapter remains to be proven.

## Wayland calibration and input roles

The private Wayland fixture uses a mapped, raster-painted Qt peer. It verifies compositor focus and the real Qt window's active state before each clipboard operation. Visible native hosting additionally focuses the real Electron host and its webview before copying or reading. Native guest sandbox/Node preferences and permission handlers remain unchanged. All future runs enforce and verify hard and soft core limits of zero. A failed initial XWayland graphics fixture entered NVIDIA EGL libraries during software fallback; the read-only host crash metadata identifies the private libpod process. No core payload was extracted or host configuration changed. The corrected private runner selects the same Mesa vendor as the existing Wayland QML fixture.

The diagnostic `--calibration-only` mode records negative outcomes; exit zero in that mode means the observations were collected, not that clipboard parity passed. The visible Wayland baseline `native-electron-owned-1m2p5w4a` passed the full clipboard assertions. Diagnostic visible `8pnl7ax0` and hidden `i144vof3` retain the following comparison (the final visible focused-main-read check is `j7td0zo6`):

| Operation | Visible native host with seat focus | Hidden/offscreen host |
| --- | --- | --- |
| Main-process write → own read | Current synthetic control | Current synthetic control |
| Main-process write → focused Qt receiver | Current control | Empty |
| Fresh Qt offer → main read | Delivered after native seat focus; empty while Qt retains focus | Previous private cached copy |
| Guest copy → focused Qt receiver | Current selected text | Empty |
| Allowed guest reads after fresh Qt text/image offers | Fresh text and correct PNG pixels | Stale text and no PNG |

The private mixed Qt-Wayland/Electron-XWayland experiment also fails copy delivery. Installing a native runtime or setting a permission therefore cannot establish clipboard availability for hidden hosting. Snapshot injection at a single permission callback is insufficient: the actual native synchronous permission-check handler can return cached Allow on later reads without another asynchronous request callback. No snapshot mirroring, navigator API shim or extra grant is implemented.

`electron-human-input.cjs` translates bounded selected-surface Qt event records into the pinned native `GuestInput.sendPageCommand`. Historical `native-electron-pool-4d402sij` proved delivery of typed composition commands directly to the engine, alongside pointer/keyboard/Unicode/pixels/profile-close behavior. Its composition branch called the raw guarded debugger, so that PASS did **not** cover native GuestInput's focused-frame admission. The correction and replacement evidence below supersede that narrower composition inference.

## Remaining admission gates

The internal adapter is not publicly selected. The actual GUI/Pool/Manager clipboard and permission evidence is recorded in [NATIVE_CLIPBOARD_PROXY.md](NATIVE_CLIPBOARD_PROXY.md), including fresh native read decisions, PNG, cross-cohort lifetime and bounded custom MIME. This is human desktop acceptance, not a claim of a completed provider/MCP native-engine path. Cookie import, popup/download/file-picker binding, broader failures and public runtime selection still need their own acceptance. Qt IME surrounding/caret queries and complete compositor-to-engine input remain explicit gates.

## Native focused-frame admission and private mapped hosting

The corrected composition adapter calls the unchanged native GuestInput just like keys and text. The retained negative run `native-electron-pool-6z55hoh9` demonstrates that an offscreen hidden host can lack `webContents.focusedFrame` even after CDP pointer input. Native GuestInput then refuses `Input.imeSetComposition`; the adapter must not fabricate focus or bypass that rejection. Expected native focus rejection now returns a scoped input-rejected record instead of poisoning the ordered stdin queue and shutting down every page. Late replies cannot attach to a replaced surface. Other transport/scope failures retain their existing failure behavior.

`with_mapped_native_host()` is a separate internal factory option requiring a factory-owned authenticated display. It maps/focuses the genuine native BrowserWindow on that private display and hides other windows when selecting/deactivating their view. It never adopts the GUI's desktop display. `native-electron-pool-l8akh0sq` passes the real Pool/Host/GuestInput path with actual native composition start/update events and the correct final Unicode value. The initial host trace still reports no focused frame immediately after show; the subsequent authoritative frame/input proof observes the real focused frame rather than substituting a constant. The commit event is observed with `isTrusted=false` through the pinned native insert-text API; the adapter does not upgrade that flag. The matching unmapped negative test `native-electron-pool-2nyz8tv0` passes native rejection, absence of injected composition and continued keyboard/CDP/page usability. Each runs one test, zero ignored, using immutable CLI/test/library snapshots against the published SDK pin.

## Actual Qt input-method producer

The GUI plugin now contains BrowserInputItem, a real QQuickItem accepting QInputMethodEvent. T3BrowserPage loads it only for an admitted native-engine frame whose actual focused-frame observation permits native composition. Existing Qt renderers do not load the optional plugin. Pointer focus and tab focus reach the native input item; ordinary key events still bubble through the existing page handler. A late-loaded item inherits focus only if the page image already held it, rather than stealing it from the address bar. Scope tokens bind every event to its original surface, including reentrant callbacks replacing that surface.

`browser-input-method-v08vdolw` uses the real plugin and actual T3BrowserPage in a private Wayland GUI. Actual Qt input-method events exercise Unicode preedit/commit/cancel, cursor offsets, bounds, disabled input, a different focused window, reentrant scope replacement and the real page signal. This is explicit Qt event injection, not a claim that a compositor IME has yet delivered those events end to end. Both native compilation with warnings as errors and focused Rust lib/bin/Pool strict Clippy pass. The tools use a software GUI backend, private HOME/display, read-only inputs and verified zero soft/hard core limits. No recursive fixture cleanup runs.

The producer returns invalid query variants for surrounding text, selection/cursor positions, hints and caret rectangles. It refuses replacement/selection edits requiring that unavailable context instead of assuming an empty field or guessed caret. The [exact Electron44 public WebContents API](https://raw.githubusercontent.com/electron/electron/v44.10.1/docs/api/web-contents.md) supplies the focused-frame observation but no IME-context query. [Electron's pinned DEPS](https://raw.githubusercontent.com/electron/electron/v44.10.1/DEPS) selects Chromium152.0.7977.169; that revision's [GTK input-method implementation](https://raw.githubusercontent.com/chromium/chromium/152.0.7977.169/ui/gtk/input_method_context_impl_gtk.cc) leaves SetSurroundingText unimplemented, so an IBus proxy alone cannot recover it from that path. Native query-protocol/hosting investigation remains required; no DOM heuristic is installed.
