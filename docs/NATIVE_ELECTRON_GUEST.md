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

## Remaining admission gates

The proof is not a production adapter. Existing Pool/private-FD process ownership, per-guest lease admission, lifecycle cancellation, bounded framing, frame delivery, native input, and the global application dialog broker must be connected and physically tested. Page-session Target/Browser escape prevention requires more than the root relay check. Production Wayland clipboard behavior has not been established by the private X11 proof. No public Electron capability or selection is enabled by these tools.
