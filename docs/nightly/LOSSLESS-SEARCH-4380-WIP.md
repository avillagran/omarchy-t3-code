# Lossless UTF-16 search and visit transport

This component implements the Nightly4380 search/selection/visit contract through
the real Rust application and Qt consumer. It does not establish full Nightly4399
parity, complete shell/stream reduction, or composed O3 server search SQL acceptance.

The exact public SDK is `1c6c8c82fed581ce09e2e0cd9c0d19c00a772af8`, published in
draft SDK PR43 on top of PR42. All seven public CI jobs passed that commit.
Every one of the141 canonical vendored leaves matches the public commit; there
is no private SDK patch. Only the existing serde dependency edge was added to
the client's t3-protocol lock entry, without changing dependency versions.

## Transport and ownership

The existing scalar pipe carries nested JSON syntax for text that cannot be
represented by a Rust String. Queries, matches, snippets, older-settled rows,
native failure causes and picked-query visit values retain their original UTF-16
units. Legacy top-level search/mount/visit packets are recognized before scalar
Value decoding. Scalar public search methods retain their result shape and
explicitly refuse an unrepresentable result; Qt uses the raw resultJson path.
Isolated units are never replaced, dropped or converted into a Rust String.

The UI keeps its original trim/slice200/trim policy. Rust admission measures
ECMAScript whitespace and UTF-16 units. Native V8 retains the original search
planning, opening claim and pickedQuery32 algorithm, including
its intentional removal of a trailing high surrogate. A remaining low surrogate
is transported intact. visitedAt remains the original String without client-side
RFC parsing or trimming; other timestamp validators are unchanged.

NativeJson escapes isolated units before both inner JSON and the outer QProcess
write. Normal BMP characters and valid pairs stay literal to preserve modern
JSON.stringify key identity. The actual Qt probe preserved isolated units in its
own stringify output instead of escaping them, so an explicit product encoder
is required before UTF-8 pipe encoding. Undefined object fields, null array
entries, controls, keys and native numeric formatting were checked separately.
The specialized writer applies only to the four lossless product packet kinds.

Trusted environment identity overrides server-supplied row/match identity.
Search mounts retain independent palette/sidebar owners, ordered replacement
and release, atomic malformed refusal, removal/cut handling and authoritative
shell precedence. Raw-row metadata is internal and cannot be spoofed through an
ordinary scalar mount. This search projection is not a general substitute for
an original full-shell reducer or its text-dependent decisions.

## Qualified private evidence

Four actual-library Rust fixtures passed against real private SDK peers: RPC
concurrency/error handling, mount ownership, raw rows and real search/visit with
the original V8 worker. Their original overall run remains FAIL because its
subsequent Qt configuration did not load; those four component passes are
recorded separately. A corrected Qt-only transport observation then passed using
the same genuine Rust driver and actual V8/SDK path. No four-case replay was used
to hide the parent failure.

The canonical src/main.rs application was then linked through the normal Cargo
example target. SDK/root library reuse was proven against the actual source
origin; the main artifact was genuinely fresh=false. The new406-file graph and
ring native directory were reviewed independently. Only the two former main
hardlink aliases were released; the other27 executables stayed read-only.

The final canonical observation used actual product T3Backend/T3Panel and their
own watch/voice processes, not a replacement client driver. A private Qt observer
read the real process command/stdout/exited signals without replacing handlers,
changing product ownership or injecting results/state. It required all four
corpus queries at both actual peers, clip200, snippet240, raw rejection, trusted
row/key identity, original picked32 and low-surrogate visit, independent sidebar
ownership after palette release, final removal and canonical main normal exit0.
All four real visits passed. Peer/compositor supervisors exited0, peer threads
joined, and all14 tracked pidfds were terminal. Source/target and29 executable
fences passed before and after; the container and actual client were terminal.

Canonical result SHA256:
`947aeac22e880c47c38b24486fd7a72354e6da9897905cadb040423831b7d67e`.
Detailed observations:
`/var/tmp/o3-build/client-lossless-search-canonical-acceptance/observations.json`,
SHA256 `4499f0e07b3878ad49658cb60f16ad54abd89ae433e43ce5b9b0a5d883544a67`.
These fences describe completed owners, not a perpetual freeze of later caches.

Earlier canonical failures remain retained: unsupported QML children counting
(`0bdbd130…`), then a deadline (`d1f2c1d5…`), then diagnostic deadline
(`039c7971…`). The diagnostic run proved search/results/raw rows and showed that
the standalone host omitted app/shell.qml's real panel visibility forwarding.
Only that missing fixture lifecycle connection was added; the product's hidden
panel visit veto and all18 assertions/90-second deadline stayed unchanged.
The earlier library-owner failure from an ordinary rlib hardlink fence also
remains FAIL; its successful compiler graph was qualified without recompilation.

No host tests, runtime/executable copies, new target, cleanup, installation or
default-branch merge were performed by these validation gates. Controlled peers
are not the actual O3 codec/SQL implementation. Complete original shell/watch
transport/reduction and broader4399 changes remain separate work.
