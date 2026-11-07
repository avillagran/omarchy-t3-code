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
| `i6pu87gd` | Reviewed role-first teardown: real edit observed, popup retirement acknowledged before native deactivation, engine and observer exit normally. No surrounding callback is observed. | Observation baseline succeeds; no Qt-query or stronger full-context acceptance. |

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
compositor or engine. The initial checkpoint paused runtime with
`tools/NATIVE_IME_CONTEXT_RUNTIME_PENDING`. Review approved one changed-lifetime
replay after binding intake to source-controlled pins: the published 72-member
runtime manifest and frozen seven-member compositor receipt. The small
compositor members are copied to a fresh read-only snapshot before execution;
self-reported acquisition receipts cannot authorize their identities. Earlier
failed closes are not replayed or relabeled successful. Runtime acceptance of
the changed lifetime was then measured separately in `i6pu87gd`: popup-closed
acknowledgement precedes native deactivation serial 9, with no fallback kill,
probe return code zero and no new coredump. Its intake receipt records source
HEAD 21223fe and both source-controlled manifest hashes. This accepts only that
fixed-field normal-lifetime observation, not arbitrary compositor failure,
engine death, complete Qt queries or the earlier stronger expectations.

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

## Typed client-protocol boundary prototype

The private `tools/test-wayland-context-boundary` runner now exercises the real
libwayland-client marshaller against a typed observer of the public text-input-v3
version-1 requests. The retained `wayland-context-wire-40gmb10k` receipt reports
428 checks and exit 0. Server enter/leave/done messages in this test are synthetic
protocol fixtures; it opens neither Electron nor a compositor. This is a protocol
boundary proof, not a Qt input-method query acceptance result.

The observer retains only current state and one pending commit. It distinguishes
unknown surrounding text from a known empty string, validates UTF-8 byte offsets,
and preserves signed surface-local cursor coordinates. Publication requires the
matching native done serial, enabled context, entered surface, and an expected
controller-owned window identity. Disable, leave, role destruction, connection
closure, and identity changes invalidate that state. Raw wire and text are not
logged. The socket owner must authenticate the engine separately; a title is
never sufficient authorization.

A production transport still needs an owned Unix-socket boundary that authenticates
both peers, forwards opaque ancillary descriptors once with close-on-exec, and
applies bounded backpressure without rewriting unrelated Wayland messages. The
compositor sees the process that creates its proxy-side connection; this identity
change must be accounted for explicitly rather than described as native engine
peer credentials. The native surface-local rectangle must then be projected using
actual window/content geometry and the admitted frame lease. The input-method
popup rectangle is not a substitute for that rectangle.

This proposed Wayland hosting route also changes the clipboard topology. The
existing lazy X selection bridge is proven for the private X engine and does not
automatically provide Wayland offers. Native permissions, lazy transfer, real GUI
seat ownership, current-offer lifetime, and cross-tab revocation require a genuine
Wayland data-device implementation and acceptance before selecting this route.
The public selector remains disabled; neither DOM nor accessibility reconstruction
is used to manufacture surrounding text or caret geometry.

## Actual native client-state measurement

The frozen `2a42116` transparent-transport probe passed in
`native-ime-context-c1ctemih` (exit 0). It admitted two independently authenticated
Electron connections to the owned compositor. The exact Chromium source initializes
GTK through `GtkInitFromCommandLine` in `ui/gtk/gtk_ui.cc` and independently opens
Ozone's display with `wl_display_connect` in
`ui/ozone/platform/wayland/host/wayland_connection.cc`. A one-connection listener
blocked startup in the preceding two retained negative fixtures; each connection
now has separate object IDs, serials, queues, and observation state.

The native client sent acknowledged surrounding state of 12, then 13 UTF-8 bytes,
with cursor/anchor offsets advancing from 5 to 12 to 13. Its surface-local cursor
rectangle advanced through `(23,63,0,18)`, `(47,63,0,18)`, `(82,63,0,18)` and
`(90,63,0,18)`. The input-method observer still reported no surrounding text and
popup-local `(0,-18,0,18)`. The earlier missing callback therefore did not prove
that the engine lacked surrounding text. The typed boundary measures the engine
requests directly and does not relabel that popup rectangle.

Both socket peers were PID/UID verified. Sway correctly attributed the proxied
window to the creator of the backend socket, a documented topology difference.
Original GuestInput, the genuine keyboard edit to seven UTF-16 units, popup-role
retirement acknowledgement, normal engine/peer exit, and relay shutdown passed.
Only typed metadata was retained; text and protocol bytes were not logged.

The native diagnostic exposed outer bounds `(0,0,1280,1024)` and content bounds
`(0,29,1280,995)` alongside the captured guest's `1280x995` viewport. These are
concrete inputs for a content-offset projection, not permission to assume every
surface has the same origin. Production admission must bind the actual native
surface/window, tab instance, frame geometry and current GUI lease before exposing
an input-method query. High-DPI, resizing, selected-tab changes and stale frames
still require coverage. Qt queries and Wayland clipboard migration remain open.

## Qt query consumer

`BrowserInputContext` and `BrowserInputItem` now admit typed native state through
C++ owner entry points. These are not page-JavaScript invokables. The current
connection, surface, tab, captured frame and mount scope must match before an
acknowledged state can replace the cache. Earlier serials cannot replace newer
ones. Current-lease invalid data clears the cache; a foreign stale result cannot
clear a newer binding. Scope replacement, disabling, hiding, focus loss and native
window deactivation clear the state before subsequent queries.

`tools/test-browser-native-context-query` builds with `-Wall -Wextra -Werror`
inside a private container and exercises real `QInputMethodQueryEvent` delivery
on an actual mapped Qt item. Its source-pinned input is the final native state
and trusted window/content geometry from `c1ctemih`; the text range checks use the
fixed synthetic field, without retaining text from the native protocol log. The
consumer retains a zero-width caret, translates surface coordinates using actual
content offsets and painted-frame geometry, converts UTF-8 byte offsets into Qt
UTF-16 positions, and distinguishes unavailable from known-empty surrounding text.
Native hints and purpose are retained; only explicit equivalent Qt constraints
are translated, without inventing a predictive-text policy for other flags.

The preceding focused proof `browser-native-context-query-xbsmq39o` passed 54
checks covering foreign ownership, malformed ranges, invalid geometry, stale
serial, selection, empty/unknown and sensitive/digits flags. The stronger
`browser-native-context-query-ip_0aglj` failed the new window-hide cache-retirement
assertion: hiding does not necessarily change QWindow active state immediately.
The correction explicitly observes QWindow visibility. The final frozen proof
`browser-native-context-query-yjx9xzy8` passed all 60 checks, including explicit
window hiding and stale-owner retirement refusal. The earlier failure remains a
negative receipt, not a pass.
Delivery from the
live owned Wayland engine into the product GUI is not yet connected, and the
current X hosting route is not presented as providing this context.

## Live native state to the actual Qt query owner

The frozen `85db7cb` probe passed in `native-ime-context-q62gjdj0` (exit 0).
The current authenticated native snapshot (serial 9, connection 1, 13 UTF-8 bytes)
was delivered directly to the mapped `BrowserInputItem` consumer through a bounded
inherited private descriptor. The receiving process validated the owned FIFO,
close-on-exec, parent lifetime and 16 KiB framed input limit before parsing. Text
never passed through stdout, QML properties or an archived protocol log.

The GUI and native engine used separate owned compositor seats, preserving the
engine's genuine focused frame while Qt received real query events. The Qt owner
used its own item dimensions and the trusted native window/content geometry to
project the surface-local caret into the frame's aspect-fit presentation. Its
query event returned the current surrounding text, UTF-16 cursor/anchor and caret;
the sensitive text value itself was not printed. The full query/lifecycle check
program exited 0. The original popup roundtrip acknowledgement preceded native
view closure, and engine, observer, Qt consumer and relay shutdown succeeded.

This closes the live native-protocol-to-Qt-query boundary proof. The fixture uses
one owned field and controlled mount identity. Product delivery still needs a
separate authenticated GUI member channel, actual worker/engine/socket ownership,
NativeMain tab-instance binding, current request/frame lease and invalidation
forwarding. The existing clipboard registration channel intentionally carries
ownership metadata only and must not silently become a text-context channel.
The current product factory still owns an X display; neither a product Wayland
supervisor nor a Wayland lazy clipboard bridge is implied by this proof.

## Dedicated GUI channel and real-frame admission

The GUI member now has a separate bounded `SOCK_SEQPACKET` input-context channel.
Registration transfers that channel and an engine pidfd using ownership metadata
only. The registration owner verifies the actual worker, renderer/engine ancestry
and engine pidfd. The channel independently verifies peer credentials and each
message's kernel `SCM_CREDENTIALS`; it accepts no ancillary file descriptors in
state traffic. EOF, malformed messages, backpressure on GUI requests or engine
pidfd readiness retire its admitted contexts without replay. Its current-state
set is bounded to 16 GUI mounts; incoming records are bounded to 16 KiB.

`T3BrowserPage` provides only expected connection/surface/tab/frame identity and
actual content/painted-frame geometry to its native item. These fields remain
empty until a real frame producer supplies them. State packets cannot replace
the expected binding. The item is disabled until its image is actually ready;
this fixes the former fixture's assumption that a page with no frame can accept
input. The stronger fixture first proves refusal without a frame, then loads an
owned regular PNG and verifies actual page input, metadata and aspect-fit geometry.
Ambiguous simultaneous mounts are refused and invalidate earlier state. Retiring
a channel uses its retained guarded item pointer and matching binding, so another
mount or frame cannot be cleared by an old channel. GUI focus/frame requests carry
ownership metadata only in the reverse direction; text never enters QML.

The frozen `browser-native-context-query-cy7jfpu7` proof passed 115 checks,
including real per-message credentials, a foreign child sender, unknown rights,
exact FD-count reclamation, engine pidfd death, EOF, stale frame and duplicate
mount refusal. These actors are synthetic owned processes, not a claimed new
Electron end-to-end result. `browser-input-method-ihecezii` separately passed the
actual page/plugin IME suite with all compilation and runtime inside private
containers. `tests/fixtures/native-input-channel-proof-4314.json` records exact
source hashes and receipts. The new registration operation is compiled but still
needs its worker caller and actual admission proof; the product Wayland factory,
private compositor intake and native lazy clipboard migration remain unselected.

## Receive-error correction after source review

Review of the GUI channel and preexisting registration member found that a
non-retry `recvmsg` error could leave the original ancillary-buffer length and
uninitialized stack data in place. Traversing that buffer after the failed
syscall could mistake garbage for received rights. Both paths now return/retire
before ancillary traversal on errors other than `EAGAIN`/`EINTR`, and initialize
the control buffer defensively. Successful truncated packets still traverse and
close actual received rights; zero-length packets containing rights are not
mistaken for an ancillary-free EOF.

`browser-native-context-query-56o4ex98` passed 116 checks including an actual
`ECONNRESET`, context retirement and exact FD-count reclamation.
`browser-input-method-xhj2qkb9` exercised the real registration member with a live
owned worker: it left the admitted reply unread and closed its socket, producing
an actual `ECONNRESET` while the worker remained alive. The member retired, its
FD count returned to baseline, and the actual page/plugin IME suite passed.
The source-matched follow-up receipts are recorded separately in
`tests/fixtures/gui-receive-reset-proof-4314.json`; the earlier channel ledger is
unchanged. No fixture tree or external process was deleted.
