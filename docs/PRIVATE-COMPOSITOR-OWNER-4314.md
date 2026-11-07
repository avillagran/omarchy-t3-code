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
