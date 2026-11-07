# Native input context baseline and observer lifetime

This is a private original-engine investigation, not an implemented Qt query
adapter. Public native-browser selection remains disabled. The accepted complete
Arch Browser/Manager flow in [the editing audit](NATIVE-ELECTRON-IME-AUDIT-4314.md)
uses a different fixture without this native input popup and retains its own
immutable client/older-server qualifications.

## What the original engine actually supplied

The fixed original-engine probe uses Electron 44.10.1, its unchanged canonical
native Session and GuestInput module, sandboxed webview preferences, and the
signed regular-file Sway artifact. `XDG_SESSION_TYPE=wayland` describes only its
owned private session. No ozone, IME or graphics command-line switch is added.
The engine's existing rootless sandbox launcher switch is retained.

Exact Electron DEPS revision
`92a6f15143569cf59b340472d0b5030465d95722` selects the Chromium implementation.
The inspected GTK context, Wayland context, text-input-v3 implementation and UI
feature definitions are byte-identical between that immutable revision and
the 152.0.7977.169 version tag. Source paths do not alone prove runtime context
availability.

The owned input-method-v2 observer never grabs the keyboard or logs text. It
records bounded offsets, a comparison bit against known synthetic text,
content hints and popup rectangle numbers. Its one transparent SHM pixel is
size-sealed but permits the server's writable mapping. A previous write-sealed
version correctly failed with `wl_shm` protocol error `INVALID_FD`; that was a
fixture error, not evidence against the engine.

The observer acknowledges actual done serials with an empty commit, without
inserting or deleting text. Chromium queues subsequent text-input-v3 state
until that exchange completes. An activation-only observer therefore cannot
establish absence of context updates.

| Retained case | Actual observation | Result |
| --- | --- | --- |
| `_wtnf17a` | Hosted webview receives the unchanged GuestInput pointer; native focusedFrame and document focus are true; real keyboard moves selection to UTF-16 offset 6. Native caret height is 18; no surrounding callback occurs. | Nonzero stronger full-context expectation retained. |
| `75hj70ht` | Real keyboard appends a fixed character; known text changes from 6 to 7 UTF-16 units, with selection 7..7 and native key receipts. Eight native state updates still contain no surrounding callback. | Nonzero stronger full-context expectation retained. |
| `sjm2u88s` | The same real edit and caret observations complete, followed by a compositor failure when the view closes before the observer popup. | Lifetime failure; not acceptance. |

Names refer to `native-ime-context-<case>` artifacts. Read-only synthetic DOM
snapshots diagnose focus and confirm the fixed edit; they never supply a Qt
context value. Popup-local `(0, -18, 0, 18)` is not a guest-local caret rectangle.
It cannot be published as one merely because its dimensions look plausible.

These observations do not establish a universal absence of surrounding text
in Electron or all original T3 surfaces. They do prevent assuming that a
hypothetical complete getter is an unconditional native acceptance requirement.
The remote original surface's sentinel-only textarea is separately documented
in the editing audit; it must not be conflated with the local hosted view.

## Compositor failure and static evidence

At the lifetime failure, authoritative systemd metadata identifies owned Sway
PID 3314538, SIGSEGV/SEGV_MAPERR, with storage `none`; hard core limits remained
zero. There was no OOM event in the bounded journal interval. No core, minidump
or process-memory payload was read. Further execution of that variant stopped.

The kernel reports an instruction in libwayland-server 1.26.0. Static ELF build
ID `0844735a67b53b10f35139455b7bed3e5887ddc8` and disassembly place offset
`0x867f` in `wl_list_remove+0xf`, writing to the previous link plus eight. The
reported fault address is eight, consistent with a null previous link. This
identifies the operation, not its caller; no caller stack was available.

The official Arch Sway PKGBUILD SHA256
`e4ad2f490a89e9d9985f524c50629d2fa3ea5dc39cff388cb17329eb58f51ef1`
matches the signed package's `.BUILDINFO`. Its sole upstream source patch
changes Meson version formatting. In
[`text_input.c` at Sway 1.12](https://github.com/swaywm/sway/blob/1.12/sway/input/text_input.c),
`input_popup_set_focus` removes the focused-surface listener before resolving
the view. Its unsupported-view return does not reinitialize or reattach that
link. A subsequent unmap/destroy can therefore encounter an invalid link.
This is a concrete source risk consistent with the failure, not proof of the
unavailable caller stack.

## Prepared lifetime change: not yet executed

The revised observer accepts `close-popup`: it destroys its own popup role,
then its surface and buffer, and waits for an actual compositor roundtrip before
acknowledging retirement. Only after that acknowledgement may the fixture close
the native view. Error reporting in the engine no longer independently destroys
the view. Failure cleanup uses the same ordering; if retirement cannot be proved,
it kills only the owned private compositor before stopping the engine, preserving
the failure. That fallback is not a claim of graceful abnormal-exit handling.

The prepared Python, Node and strict C source checks pass without starting a
compositor or engine. Runtime remains paused by
`tools/NATIVE_IME_CONTEXT_RUNTIME_PENDING` until checkpoint review. Earlier
failed closes are not replayed or relabeled successful.

## Concrete adaptation boundary

A GTK module cannot recover text discarded by Chromium's empty GTK
`SetSurroundingText`; its caret delivery is tied to GTK key dispatch, which the
current CDP input route does not exercise. It is not a demonstrated complete
adapter for the current private X11 host.

A Wayland-native host has a concrete public protocol boundary:
`zwp_text_input_v3.set_cursor_rectangle` carries the engine's surface-local
rectangle before compositor popup positioning. An owned, typed protocol boundary
could observe that value and committed native state, preserving serials and
descriptor delivery, rather than deriving a caret from DOM or popup geometry.
It would still need actual engine/window ownership, surface-to-tab correlation,
lease invalidation, content-frame coordinate mapping and a proved GUI update.

That route would change the current private X11 hosting topology. The existing
lazy X selection bridge cannot simply be declared compatible with it. Clipboard
permission, lazy bytes, GUI-seat hosting, native input, isolation and teardown
would need their own concrete acceptance before selecting such a route. No new
engine hosting mode, query fallback or public capability is implemented by this
baseline.
