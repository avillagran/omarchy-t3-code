# Inline code file candidates, nightly 4340

The original `packages/shared/src/markdownLinks.ts` at
`a9e709f15f825cacac1b6d665fa1920c328d68b7` classifies `allocator.cc:217` as a
file candidate. `.cc` belongs to the country-hostname set, which permits a
position suffix, rather than the generic-hostname set, which always rejects it.
The old O3 implementation already accepted this specific example but differed
on hostnames with positions, shortened paths, trailing folders, Windows paths,
JavaScript whitespace and version-shaped final segments.

`src/markdown_inline_path.rs` ports the complete candidate classifier, including
both hostname sets and its actual ordering. Windows absolute paths retain their
separators; other backslashes normalize to slashes. Elided path segments stay
plain code. JavaScript whitespace is represented explicitly instead of using
Rust Unicode whitespace. A file candidate grants no filesystem authority.
The existing renderer length/control/scheme guards remain at the rendering
boundary. This change does not establish full file-link resolution or UI parity.

## Evidence

`/var/tmp/o3-build/inline-files-oracle-4340` retains four complete original source
files, their pinned hashes, commands, logs, the native result and a Rust probe.
The actual original Node 26.13 classifier passed its 12 original regression rows
and produced 87 scalar-string cases. The exact production Rust module, compiled
without Cargo into the existing client target, matched all 87 in one private
rootless Podman test. Strict rustc compilation passed; both containers finished
and disappeared. No host tests, credentials, network, filesystem fixtures or
recursive cleanup were used. No Cargo target or persistent cache was created.
The native process passed, but the first host result parser split literal U+0085
inside valid JSON; LF-only parsing recovered the retained output without rerun.

The checked-in fixture records these exact results and source hashes. A library
regression test consumes it, but the complete library and renderer have not yet
been rebuilt or exercised for this source change. The probe establishes only
classifier behavior, not the full application or global nightly acceptance.
Isolated UTF-16 surrogate strings are outside Rust scalar-string ingress.
