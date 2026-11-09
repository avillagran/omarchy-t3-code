# Private compositor admission candidate

This is a source-reviewed candidate for an application-private Sway 1.12
build. It is not an installed compositor, an accepted hosting factory, or
permission to enable the public browser selector. The original seven signed
compositor artifact members and the 72 Electron runtime members are unchanged.

The original Sway global filter is installed before the Wayland listener.
The candidate prefixes that filter with actual kernel peer admission and
retains the original security-context checks. The same predicate rejects IPC
connections immediately after accept, before allocation or request parsing.
Registry advertisement and guessed global binds both use the Wayland filter.

Only sockets connected by the real supervisor PID and UID are admitted. The
supervisor must connect backend sockets itself, then delegate those connected
descriptors to its independently authenticated engine proxy. Merely creating
an unconnected descriptor in the supervisor is insufficient. Engine ancestry,
frontend peer identity and cohort ownership remain responsibilities of that
future factory; this compositor helper does not infer them from a PID label.

Admission retains a kernel pidfd for one captured actual parent PID, rechecks the parent
after opening it, checks liveness synchronously for every admission, and
terminates the private display when its supervisor dies. PID reuse cannot
revive the retained pidfd. Failure to allocate the pidfd or its event source
refuses initialization. The descriptor is close-on-exec and is reclaimed by
the display destroy listener.

Unauthorized clients receive a fatal error and are destroyed by an idle
callback, outside the libwayland client-created signal. There are exactly 32
rejection slots; a 33rd simultaneous creation terminates the private display
instead of growing an unbounded queue. Teardown removes pending callbacks,
client listeners, parent watch and display listeners. A failed event-source
allocation also terminates the private cohort.

The frozen native receipt and source hashes are recorded in
`tests/fixtures/private-compositor-owner-source-4314.json`. The test uses real
libwayland 1.26.0 in a private container, including connected-descriptor
delegation, foreign guessed binds, request-free idle rejection, exhaustion,
supervisor death and listener teardown. It does not launch Sway or Electron.
The earlier `6qmyki9h` receipt covers the smaller scalar-parent implementation;
it does not prove the final pidfd candidate. The `7f151jn6` compile failure is
retained; its const-correctness and indentation diagnostics were corrected
before the `ww6k18mq` receipt passed. Review then found a double-read parent race;
the corrected candidate captures one PID before validation and uses explicit
`-D_GNU_SOURCE`. Its final native receipt is `hudvnti0`.

The reviewed corrected candidate was subsequently built and exercised as a
separate app-private artifact. `tests/fixtures/private-sway-owner-proof-4314.json`
records its exact digest, source/tool archives, signed package verification,
header compatibility qualification and retained negative receipts. The final
`runtime-proof5` passed with real headless Sway, pixman, no host display or
input devices, no network, private HOME/runtime/config and hard core limit zero.
A child consumed Wayland and IPC sockets actually connected by the supervisor;
a foreign sibling could neither enumerate/bind globals nor interpret IPC.
The supervisor held its `exit` command socket until actual normal process exit.
No Electron engine was started. The source-controlled repeat runner accepts
only that frozen artifact and its exact two dependent library identities.
The source-controlled wrapper itself then passed once at
`private-sway-owner-3q2xgewl`, without rebuilding the artifact.

The build uses explicit `-D_GNU_SOURCE` only for the two patched translation
units. Applying it globally exposed a GNU `strpbrk` const diagnostic in unrelated
upstream code; the scoped compiler wrapper preserves upstream flags elsewhere.
The original signed Sway BUILDINFO pins libinput 1.31.3, whose authenticated
header was used to avoid a newly added enum diagnostic against the installed
1.32 headers. Runtime libinput was not replaced. No warnings were suppressed.
Meson/Ninja and source data remain in the private cache, without installation.

Wayland hosting, early frontend engine authentication, native lazy clipboard
transfer and full GUI/Pool acceptance remain outstanding. Compositor admission
alone does not authorize clipboard reads or prove native permission behavior.
The public selector remains disabled.

## Early engine admission

The renderer now has an opt-in `privateEngineSpawn` control on its already
confined private identity endpoint. Immediately after the actual successful
`Command::spawn`, it opens that child's kernel pidfd and sends it with kernel
sender credentials. It reports no user-supplied PID and retains close-on-exec
on its original endpoint. Failure kills and waits only that owned child.

`Pending::admit_spawn` validates the renderer sender, kernel pidfd identity,
actual child ancestry and current lifetime. `Pending::confirm` then requires
the independent trusted-main addon identity to name that same live engine.
The legacy single-stage `admit` API remains available. This ordering lets the
internal frontend factory authenticate early GTK/Ozone connections without
waiting for JavaScript to start. It does not authorize arbitrary connection
peers or allow clipboard access. The internal Pool factory now uses this caller; public selection is not installed.

`browser-engine-spawn-proof-4314.json` fences the production source and the
private immutable Rust receipt: the initial two actual kernel/process tests and strict
library Clippy passed. The senders are explicitly synthetic test processes;
this does not claim Electron, GUI or Wayland factory acceptance. No broad
runtime suite was collected or executed.

The completed internal caller and its final three identity/seven transport plus
actual Electron receipt are documented in `NATIVE-WAYLAND-FACTORY-4314.md`.
The original two-test ledger remains historical evidence, not a final-source
claim for the sender-order and lifecycle changes.
