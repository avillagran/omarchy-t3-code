# Internal native Wayland factory

`Config::with_owned_native_wayland` now selects a real internal Pool factory.
It launches the separately pinned private Sway artifact, creates an authenticated
frontend namespace, and starts the actual renderer and Electron runtime. This
does not add a public selector or adopt the user's compositor, display, profile,
backend service, input devices or clipboard.

The compositor accepts only backend sockets connected by its actual supervisor.
The worker connects each backend itself. It accepts an engine frontend only
after checking the real spawned engine's kernel credentials and live pidfd.
Requests from another PID are rejected without reading protocol bytes or
creating a backend connection. Both sockets must be close-on-exec.

The renderer reports its actual `Command::spawn` child pidfd before waiting for
JavaScript. The later trusted-main addon independently confirms the same live
engine. If scheduling delivers the addon record first, intake retains only that
one authenticated record and requires the corresponding renderer spawn record.
It never admits a connection just because an arbitrary packet supplies an ID.
Renderer death kills its actual Electron child through `PR_SET_PDEATHSIG`; the
parent PID is checked after installing the guard. Owned X and Wayland builders
clear the other owned-display mode.

The transport preserves opaque bytes and SCM_RIGHTS ordering. A first positive
partial send transfers ancillary ownership once; suffix writes do not resend
descriptors. Each direction retains at most 1 MiB of allocated byte capacity
and 64 queued rights. Capacity remains charged until the entire chunk retires,
including any already-sent prefix. At most eight authenticated connections are
accepted per engine lifetime. These are explicit wrapper resource budgets, not
a claim about every possible native workload or platform limit.

Backend connection attempts are nonblocking and close-on-exec, own at most one
new socket, and check cancellation and the actual engine/compositor pidfds before
and after connection. A full backlog fails closed immediately. There is no
reconnect, command replay or extra native command deadline. Established streams
watch both process lifetimes and cancellation; retirement closes queued rights.
Thread teardown polls at most 50 ms between cancellation checks. No protocol
payload is logged.

Private display sockets use a fresh short temporary namespace rather than
truncating a profile path to fit `sockaddr_un`. Profile storage and its lock stay
in the existing profile directory. Directories are created atomically with mode
0700. The compositor receives explicit headless/pixman configuration with Xwayland
and swaybg disabled. Cleanup retains directories/configuration and unlinks only
the exact owned frontend socket inode. It does not recursively remove fixtures.

`tests/fixtures/browser-native-wayland-factory-proof-4314.json` records the exact
source, immutable executables, assets, runtime pins and invocation-script hashes.
The isolated build retains SDK pin `74f0052`; it does not relabel a later primary
SDK combination or replace the required combined source check.
The final receipt is `/var/tmp/o3-build/wayland-factory-coherent-final`:

- Three kernel identity tests and seven transport tests passed, with no ignored
  tests. These include sender ordering, foreign identity, actual rights transfer,
  partial sends, peer loss, cancellation, full backlog and retained allocation.
- Strict library and actual example Clippy passed. This is not a full matrix.
- Actual Pool → renderer → Electron → authenticated Sway → unchanged native
  Host/Guest passed CDP page evaluation, absence of Node globals and real frame
  delivery. An idle foreign frontend was refused while the legitimate guest
  continued evaluating. The normal Pool close path ran; its void API does not
  separately assert graceful process status.
- A second actual cohort was used for an intentional owned-renderer pidfd kill;
  its real Electron engine pidfd became readable. This proves that engine death
  boundary, not complete abnormal-exit reaping of every Chromium descendant.

The tests run only in private rootless containers with read-only artifacts,
private HOME/runtime/config, no network, no host display/devices, Chromium's
sandbox intact and soft/hard core limit zero. The 72 original Electron files
and seven original signed compositor members are unchanged. Older failed or
more limited receipts remain qualified in the ledger, including the corrected
stale unit-runner mount and the partial-send fixture's previous failure to drain
its bounded receive queue.

This acceptance does not prove actual Qt/GUI input, context projection or system
clipboard transfer on the new factory. The next consumers must bind context
metadata only through the actual frame producer and preserve native permission,
origin, fresh-read and current-offer lifetime semantics. Packaging, full owned
supervisor/provider integration and public selection remain gated on those
concrete consumers. No production focus workaround or engine flag was added.

A subsequent test-only follow-up initializes the same queue fields in one
literal for combined Clippy. The production prefix is byte-identical; its hash
and the old/new full test-source hashes are recorded separately. The original
test artifact hashes remain the exact executed receipt, without relabeling the
follow-up as another executed test build.
