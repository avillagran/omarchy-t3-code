# Native Electron IME boundary audit

Public native-browser selection remains disabled. These private diagnostic
controls preserve the unsuccessful complete GUI route and separate its renderer
failure from a second, resolved container resource limit. They do not establish
native surrounding-text/caret queries or complete browser parity.

The [tracked case ledger](../tests/fixtures/native-ime-boundaries-4314.json)
records return codes and hashes of each retained input/command manifest. Its
experimental NativeMain inputs are identified by bytes, rather than relabeling a
committed parent as tested. Nonzero cases are retained failures, not passing tests.

## Source and capture lifecycle

The dedicated 4314 reference is 83a3674ffd84ffb33de768210c806cb1ac53fd80.
Its entire `apps/desktop/src/preview/Manager.ts` is byte-identical to 4301:
SHA256 `cf324dad5fd05a688e38d879055b0e8584b91e6ca763dd52a274483636e2f44e`.
Its picture-in-picture consumer performs an initial capture, then a completion
paced 84 ms sleep/capture loop, with a 1,000 ms capture-attempt timeout. Human input
does not itself request an extra capture. The audit implementation follows that
scheduling, skips empty images, and fences publication against the actual
window/tab/instance/lease/view. It retains the legacy Qt scheduling separately.

Frame-format/fingerprint parity is not claimed: this wrapper transports PNGs,
does not yet reproduce native JPEG quality 80/fingerprint suppression, and currently
publishes focused-frame availability with frames. Background native painting
ownership also still needs the original keyed consumer, rather than treating
every Host painting event as a selected-page screenshot request.

## The independent shared-memory failure

The observed default container shared-memory capacity was 65,536,000 bytes.
`k5kqxpc1` reached 63,410,176 mapped bytes in 64 mappings and only 1,024,000 free
bytes, less than one 640 × 480 × 4-byte frame. Root descriptor count peaked at 119 and
the admitted private descriptor retained CLOEXEC. Private cgroup counters showed
no memory OOM or PID-limit event.

Only bounded native PCs, public error file/line locations, fixed EGL/Mojo
categories and numeric GPU exit codes were retained from stderr. They identify
`viz.mojom.CopyOutputResultSender` message 1018638595 deserialization failures,
then GPU exits and the browser's intentional unusable-GPU fatal. The pinned
[bitmap shared-memory traits](https://github.com/chromium/chromium/blob/152.0.7977.169/services/viz/public/cpp/compositing/bitmap_in_shared_memory_mojom_traits.cc)
allocate/map each copied bitmap; an unreadable bitmap can be refused by the
[CopyOutputResult reader](https://github.com/chromium/chromium/blob/152.0.7977.169/services/viz/public/cpp/compositing/copy_output_result_mojom_traits.cc).
Diagnostic reports retain no process memory, core, minidump, raw stderr or pixel bodies.

Changing only the private SHM budget to 256 MiB resolves this sustained failure.
`irbh5taa` and `iy707dnb` each deliver 167 additional actual capturePage frames
over 15 seconds, 170 total, then complete native Japanese composition. The latter
uses the original product GLX-disabled display; its shared mappings peak at
76,922,880 bytes. Default GLX is therefore unnecessary for that correction and
the product display arguments remain unchanged. Initial EGL fallback errors
are distinct from the resolved shared-memory limit.

This is a bounded fixture-resource correction, not evidence that all capture
sizes or lifetimes fit 256 MiB. It does not fix the original editing failure:
the complete GUI/Manager still fails in `nbzm3loa` with sufficient SHM.
Manager controls use frozen server 0b9a5d3, SHA256
`23b9f821ddc2d3da7597ff9f65cbb3ab7daa6e5eb84c1887301b1977595a977c`,
with the reviewed desktop-origin-only guard. Local Chromium fallback, clearProfile
and download paths are not admitted. This does not validate newer server code.

## Editing boundary controls

`83canvot` records a complete fixed synthetic Qt timeline: press at 17.201 ms,
release at 17.369 ms, move at 56.550 ms, composition at 96.370 ms and commit at 117.240 ms after
the actual IME-ready phase. Preedit is exactly three UTF-16 units, selection 3..3,
with no replacement range. Only these known synthetic strings are admitted to
the replay fixture. Physical pointer commands complete before composition.

The same five packets replay successfully in standalone Arch (`kvc6cz04`),
with measured maximum dispatch lateness of 1,947 µs. A Qt frame-consumer replay with
no physical Qt pointer/IME producer fails (`81ahr6zj`), as does the same Qt image
without any GUI (`ms6n9k4k`). Actual compositor-to-Qt input also fails without
Rust Manager/Pool/backend or an active clipboard cohort (`xkcv0svx`). These
observations prevent blaming Rust dispatch or compositor IME generation merely
because the complete route contains them.

Private compositor geometry is read as a complete JSON document, not parsed
line by line. Physical fixture coordinates use its actual window rectangle;
QtWayland's unavailable global origin is not assumed to be the compositor's
floating-window origin. This fixture correction never mutates DOM focus or
fabricates a focused-frame flag.

Immediate read-only precomposition snapshots are identical in the successful
and unsuccessful image controls: connected, enabled, active TEXTAREA; empty
native value; selection 0..0; DOM Caret with one range with BODY anchor at offset 1.
The snapshot uses the existing owned executeJavaScript route, not an agent
Runtime.evaluate that renews NativeHost focus holds. It is diagnostic only.
DOM selection observations do not establish Blink's internal EndingSelection
or provide a production native caret/surrounding-text boundary.

The failed Qt-image control also survives 15 seconds and 168 captured frames before
editing (`benqtgz2`). The canonical focus-emulation idle ACK changes 1 → 0 before
input, yet native composition still fails. No fixed product delay or focus
override is installed.

## Environment comparisons and remaining work

Paired input-file SHA maps are identical; normalized Podman arguments differ
only in the image. Image entrypoint, working directory and PATH/LANG settings
match. Forced precomposition sampling shows Fcitx loaded in both cases; an
earlier periodic sample missed its late initialization and is not a supported
cause. Observed public static module manifests match for 104 main-process,
95 renderer, 104 GPU and 95 network-service mappings. Direct font-file mappings
have not been observed, so this is not a complete font-data equivalence claim.
Process identities come from actual native APIs and are checked against kernel
descendant relationships before read-only mapping metadata is inspected; JSON
PIDs never admit a browser capability or replace the trusted FD handshake.

Candidate GTK/X11/dconf/xdg/locale settings match or are absent. Masking the
private system font cache (`snqw9bz8`) and mounting the exact bounded linker
cache from the successful image (`fknkx0x6`) do not prevent the renderer crash.
Neither operation changes the host. The editing cause remains open; no new
graphics or sandbox flag is used to hide it.

The Qt producer still returns invalid surrounding-text, selection, caret and
hint query variants, and refuses replacement operations requiring those missing
values. No DOM-derived context or successful insertText-only claim closes that
gap. Actual engine query/lifecycle integration remains required before public
activation, together with the complete GUI editing acceptance.

All runtime cases execute in private rootless Podman with network disabled,
read-only inputs, private HOME/XDG/display/profile, native guest sandbox/Node
preferences, inherited core limits of 0:0 and retained fixtures. Inner Python/QML
fixtures are never host test commands. No user clipboard, existing user profile, shared service or physical device
participates. There is no recursive fixture teardown or whole-client
runtime-audit closure.


## Preserved boundaries after the environment comparisons

Read-only authenticated X11 input-focus queries match the actual NativeWindow
XID in both images immediately before composition: `d033s3mf` succeeds and
`6z74zwgu` reproduces the renderer failure. The query checks the socket peer's
PID/UID against the owned Xvfb and never changes focus or properties. Lack of
native X11 window focus is therefore not a supported explanation for this pair.

The complete GUI/Manager contrast on the passing image has not been executed.
Both images share the same two official Arch base layers. Their observed IDs are
`1b9c2f3a1a72268cab659ab3f20956bb6ce730e8dd716fe90754ba03a8870e19`
and `cf00ddb64a7d8e72a205fff49fbdaf443ecec648130b2e729320726575679462`.
The existing GUI runner uses compositor files inside the latter image.
No already-admitted regular-file compositor bundle was found among the existing
local artifact paths. Automatic approval review rejected the proposed image
mount with a generic reason; that route was stopped, no successful image mount
or GUI counterpart ran, and no rootfs was exported. An unexecuted contrast is
not acceptance of the destination package or a correction to the failing image.

Native surrounding-text/caret work must use existing engine APIs and actual
lifecycle state. It may not replace those values with guessed DOM geometry,
relabel a snapshot as the internal editing anchor, change product focus policy,
or add a fixed delay based on these unsuccessful experiments.
