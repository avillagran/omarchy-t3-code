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

Admission retains a kernel pidfd for the actual parent, rechecks the parent
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
before the final `ww6k18mq` receipt passed.

The actual patched compositor still requires review and a separately pinned
build before execution. Wayland hosting, frontend engine authentication,
native lazy clipboard transfer and full GUI acceptance remain outstanding.
