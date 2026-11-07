# Embedded device panel

The Devices tab renders native Android H.264 and iOS MJPEG sessions, forwards scaled pointer/keyboard input, and exposes host commands through the captured session target. Commands use the protocol action decoder; editable payloads cannot override the host or device. At most one action per environment/host/device runs at a time, including commands still running after the view closes.

`T3DeviceTools.qml` exposes foreground application launch/terminate, URLs, appearance, text size, Android orientation, supported accessibility settings, location and application permissions. iOS-only shake, Liquid Glass, color filters and push controls are conditional on platform. Host commands require a local or SSH host, and unknown settings remain disabled. No command is automatically retried.

## Verification

Six Rust device-screen tests validate native payload coverage, destination capture, lifecycle reservations and frame/input helpers. The actual tools component is exercised in a rootless container with a private software X display by `python3 tools/test-device-panel-container device-tools`. It covers platform controls, location bounds, application targeting, pending state, scope changes and unsupported hosts. The browser workspace fixture also passes in that harness. The current recursive UI translation check covers 20 locales with 2,169 keys each.

These fixtures use synthetic device details. They do not claim a live emulator, iOS host or installed application acceptance. The complete side-panel route now passes in a rootless private Sway compositor, including device inventory, frames, touch mapping and close. Deferred singleton construction fixes the fixture's initialization order; a Wayland compositor is required for the application's layer-shell surface. Strict client Clippy errors were repaired separately rather than suppressed.

## Nightly 4275 discovery metadata

The client retains `hostStatuses[hostId].incompletePlatforms` from nightly 4275
instead of dropping it during snapshot normalization. The native array preserves
order and duplicate entries; only `ios` and `android` values are accepted, and
the older `androidDiscoveryIncomplete` flag remains available. Eighteen cases
execute the actual native host-status schema with Effect 4.1.2. All nine device
integration checks pass in the private Rust container runner.

This checkpoint preserves the metadata for consumers. The full native behavior
that keeps previously discovered devices during partial platform failures,
restores discovery details and distinguishes stale entries after boot still
requires backend and visible-control integration.

## Remaining source parity

Guided host installation/update, application installation, accessibility overlays, device event logs, recording/capture, fold/duo controls, 3D presentation and floating device windows remain separate implementation work. Backend platform capability errors must remain explicit rather than displaying successful placeholder behavior.

## Shared SEMU v2 decoder checkpoint

The client pins published SDK `7548724acca15c0c01c4ec2b797695cc6a638825`
with123 exact crate inputs and no private patches. Its frame parser recognizes
16-byte SEMU v1 and24-byte SEMU v2 headers; the latter timestamp bytes are no
longer sent to H.264. The actual pinned hub0.19.1 writer/parser provides338
observations, with documented legacy empty-v1 compatibility. The parent SDK88411c6
passed422 Rust tests, C ABI, actual JNI and baseline-aware Clippy inside
`android-sandbox`. Cargo dependencies and Android/JNI/Kotlin sources are unchanged.

Real emulator frame, input and view-close acceptance still requires its own
frozen client/server/Qt proof. Decoder corpus success and a booted inventory entry
do not establish that full UI flow.

## Native media ticket purpose

The device SDK requests `/api/auth/websocket-ticket/media`, matching the complete
native4301 deviceHubAccess resolver. Eight HTTP cases, six native resolver cases
and one actual stable-server purpose check pass. The generic ticket helper is
unchanged and the device consumer does not silently fall back to it.

The stable0b9 server accepts unbound tickets for media and refuses RPC-bound
ones. Both unbound and media requests reached a separate403 gate when agent
access was disabled. The server viewer-access correction has its own native
policy and physical proof; changing ticket purpose alone does not establish
video acceptance or justify changing authentication policy.
