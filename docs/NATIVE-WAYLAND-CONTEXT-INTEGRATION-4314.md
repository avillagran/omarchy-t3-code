# Native Wayland context consumer integration

This optional consumer remains unselected. Strict all-target compilation and
nine focused parser/routing tests now pass on the integrated current-client
source. They do not establish actual GUI context delivery. The earlier accepted
factory proof does not cover this observer. Validation reuses an existing Cargo
target and mounts the exact libtest read-only; no executable or runtime copy is
made for these checks.

## Existing boundaries

The authenticated frontend forwarder owns each real engine/compositor socket
and SCM_RIGHTS lifetime. The new observation seam sees positive reads after
FD validation plus empty idle ticks for bounded GUI requests. It cannot alter
queued bytes or ancillary ownership. A parsing
failure must retire this exact connection and its context lease, without replay.
The original unobserved forwarding entry point retains its behavior.

`browser_wayland_context.rs` ports the previously physically exercised Python
text-input-v3 boundary into a Rust current-state observer. It tracks only selected
public interfaces, surface ancestry, the trusted host window's identity, pending
client state and the last client commit awaiting a matching compositor `done`.
Unrelated requests and preedit/commit events remain opaque. Unknown surrounding
text differs from a known empty string; cursor/anchor offsets remain UTF-8 bytes
until the existing GUI adapter converts them to UTF-16. Zero-width carets remain
valid. No raw protocol or text is logged.

There are two incomplete messages of at most 65,532 bytes, positive read chunks
of at most 65,536 bytes and at most 8,192 combined live/retired tracked objects. Expected host identities
are limited to 32 unique leases/titles of at most 256 bytes. Current/awaiting state
and at most one pending update/invalidation per context replace historical data.
An invalidation backlog exceeding the object budget is an error requiring the
consumer to retire the entire context lease. The observer's memory is separate
from the existing 1 MiB transport queue/64-FD budgets. These wrapper bounds are
resource limits, not a claim that the public Wayland protocol imposes them.

## Product binding implemented in source

When the actual GUI registration exists, the optional Wayland factory registers
`gui_input_context::Lease` with the admitted engine PID/pidfd. It allocates an
opaque namespace for each authenticated frontend connection. The Pool creates
a distinct UUID title nonce before writing each new tab's trusted open command.
NativeMain owns that host window title and prevents automatic document-title
replacement; the target document never supplies an admission identity.

The forwarder feeds the typed observer after ancillary ownership is validated.
Matching compositor `done` publishes current native state into the cohort owner.
Enable/disable, leave, role/surface destruction, title removal, socket failure,
hide and shutdown invalidate the previous query binding. Pending invalidation
also makes the existing GUI query-time health check reject its older cache before
a notifier callback runs. No context is added to worker stdout, QML, an outbox or
persistent storage.

Only an admitted actual frame can produce the expected GUI binding. The trusted
host queries its own webview rectangle and real window/content bounds before and
after capture; changed geometry supplies no query metadata. This is not a target
DOM or accessibility query. The GUI receives four opaque metadata fields and
content geometry alongside the real frame; scope remains the actual item/request
identity, independently added by the C++ owner. `BrowserSurface` preserves these
fields. `T3BrowserPage` binds them only after its image is ready. Composition
remains enabled independently while a new frame loads over retained pixels.

The cohort owner rejects mismatched frame requests, invalid geometry, multiple
active matching contexts and child surfaces whose transform is unproved. It
services bounded GUI requests during authenticated traffic and idle transport
ticks; state updates may refresh an already-bound frame but cannot mount another
item. The GUI lease still checks the actual engine and worker lifetimes.

Six observer cases and three explicit recording-sink policy tests pass in
private rootless Podman. The first strict build found two Clippy alignment
warnings; replacing those modulo expressions with `is_multiple_of` passed the
next strict build without changing protocol behavior. Build and runtime receipts
are `wayland-context-integrated-build-2/result.json` and
`wayland-context-integrated-runtime-1/result.json` under `/var/tmp/o3-build`.
Recording-sink tests do not impersonate GUI/kernel admission. Independent native
marshaller checks still must cover creation/framing: the first separate typed
client/server fixture aborted in its synthetic setup and remains unaccepted.
Actual GUI/factory acceptance
must cover title association, capture geometry, query delivery, immediate
retirement and default input noninterference. Child-surface transforms and
ambiguous active contexts remain a real gap rather than a fabricated offset.

## Clipboard migration is separate

The existing GUI clipboard registration is deliberately X-only: it delegates
an authenticated private X connection, X authorization and engine pidfd to the
GUI member. A Wayland display must not masquerade as this X cohort or receive
clipboard access merely because an IME connection is admitted.

Wayland hosting needs an independently authenticated GUI-to-owned-display lazy
selection bridge. Native Session permission policy remains the only permission
arbiter: denied programmatic reads fetch no system bytes; genuine native paste,
trusted sanitized writes and admitted agent input must not acquire extra cached-
read or human-only gates. Reads must pull the current offer at request time,
not mirror a snapshot after Allow. Egress retains at most the current bounded
offer after a tab/cohort closes while the GUI remains alive. Existing Qt seat,
data-device listeners, keyboard and drag/drop ownership stay with Qt.

No system clipboard migration, public selector, packaging completion or GUI
acceptance is claimed by this source draft. The next physical checks must use
the original typed native marshaller, actual frontend factory and real Qt owner
with the previously reviewed teardown ordering; no compositor crash replay or
unrelated runtime variation is justified by this draft.
