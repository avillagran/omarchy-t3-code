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

## Remaining admission gates

The internal adapter is not publicly selected. Human pointer/keyboard forwarding, cookie import, popup/download/file-picker consumers and native service failure propagation still require implementation or physical acceptance on this route. Complete provider/MCP → server FD → guest command/event integration, native dialog/clipboard composition and lifecycle failure cases remain admission gates. Page-session Target/Browser escape prevention is enforced below the native Host; legitimate child-session acceptance still needs physical coverage. Production Wayland clipboard behavior has not been established by the private X11 proof. No public Electron capability or selection is enabled by these tools.
