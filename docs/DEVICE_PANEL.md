# Embedded device panel

The Devices tab renders native Android H.264 and iOS MJPEG sessions, forwards scaled pointer/keyboard input, and exposes host commands through the captured session target. Commands use the protocol action decoder; editable payloads cannot override the host or device. At most one action per environment/host/device runs at a time, including commands still running after the view closes.

`T3DeviceTools.qml` exposes foreground application launch/terminate, URLs, appearance, text size, Android orientation, supported accessibility settings, location and application permissions. iOS-only shake, Liquid Glass, color filters and push controls are conditional on platform. Host commands require a local or SSH host, and unknown settings remain disabled. No command is automatically retried.

## Verification

Six Rust device-screen tests validate native payload coverage, destination capture, lifecycle reservations and frame/input helpers. The actual tools component is exercised in a rootless container with a private software X display by `python3 tools/test-device-panel-container device-tools`. It covers platform controls, location bounds, application targeting, pending state, scope changes and unsupported hosts. The browser workspace fixture also passes in that harness. Translation coverage is 20 locales with 2,126 keys each.

These fixtures use synthetic device details. They do not claim a live emulator, iOS host or installed application acceptance. The complete side-panel route now passes in a rootless private Sway compositor, including device inventory, frames, touch mapping and close. Deferred singleton construction fixes the fixture's initialization order; a Wayland compositor is required for the application's layer-shell surface. Strict client Clippy errors were repaired separately rather than suppressed.

## Remaining source parity

Guided host installation/update, application installation, accessibility overlays, device event logs, recording/capture, fold/duo controls, 3D presentation and floating device windows remain separate implementation work. Backend platform capability errors must remain explicit rather than displaying successful placeholder behavior.
