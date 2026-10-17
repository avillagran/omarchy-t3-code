# Application open-source notices

The native T3 Code screen is `settings.open-source-licenses.tsx` and
`components/settings/OpenSourceLicenses.tsx` at the pinned source revision
`5a4e9980533705774dc3f45b35cacf76c3d78dbc`. It loads its own static web manifest,
not server configuration for the selected environment. ot3 follows that ownership
with a compiled manifest of its own dependencies and supplied notices.

## Original behavior and adapted data

General → Diagnostics → View licenses opens the page; General stays selected in
the sidebar. No additional top-level tab is invented. The application page hides
the environment selector and requires neither a T3 account nor settings permission.
A translated General settings search can find the link.

The screen provides the native heading/count/search affordance, all-term filtering
by name/version/license/bundles, empty/loading/error/retry states, optional project
source links, compact rows, adjacent versions and a single expanded notice. Notice
text is selectable plain text, wrapping to at most76monospace columns; it is never
interpreted as provider HTML. Filtering/collapse and page disposal preserve or
release the selected notice appropriately. Rust supplies metadata first and one
notice lazily; the full manifest does not enter the QML model on every read.

The original shared `thirdPartyLicenses.ts` supplies filter, identity and bundle
formatters verbatim after TypeScript erasure. Qt requires a named catch binding;
that syntax adaptation is recorded in `NativeLicenses.sources.json`. The original
manifest decoder runs at build time with standard URL. QML calls only the original
filter/key/bundle helpers; runtime Rust decoding checks the same required fields,
nullable version/source, safe HTTP(S) links and duplicate identities, with local
resource bounds. IDs preserve original
`encodeURIComponent(JSON.stringify([kind,name,version]))` semantics.

`licenses-read` and `licenses-notice` are application-only read operations on the
existing independent preferences worker. There is no environment lookup or
network request. Notice reads carry the exact manifest digest and entry key.
Request nonces reject late receipts; a mismatched digest/key cannot display another
entry's text. IPC loss becomes an explicit retry state. No preference is mutated.

## Building the actual manifest

`node tools/build-licenses.cjs` uses locked, offline Cargo metadata for the native
build target, exact crate notice files, the locked renderer's collected license
files, both Lucide notices, copied T3 code/artwork attribution and ot3's own notice.
Build-only renderer dependencies are classified separately. The Rust standard
library is not in Cargo metadata, so its compiler-provided copyright summary and
license texts are collected separately. Python's standard HTML parser converts
that supplied summary to plain text and retains visible text/external links; it
runs only during the build. System-provided Qt/Quickshell/platform libraries are
not redistributed as application dependencies by this plugin.

The current native ARM64 manifest contains238entries and1,801,708bytes. Its
provenance records target, Rust compiler version, Cargo.lock, renderer lock,
individual notice hashes, original helper hash and final manifest hash. It
contains no source-machine paths or connection/provider credentials. Metadata and
notices are embedded in the Rust executable, so a prebuilt installation needs no
Node/Python/Cargo or loose notice-file service.

`bin/build-client` fetches locked sources for the native target, collects notices,
then builds. Source installs now include Node.js18+ and Python3 as build tools;
prebuilt/runtime installs do not add them. Missing supplied notice text fails
regeneration instead of fabricating a license. A sibling T3 checkout is unnecessary
for ordinary builds. To regenerate the original JS helpers explicitly, pass the
exact clean pinned checkout to `node tools/build-licenses.cjs /path/to/t3code`;
this TypeScript regeneration needs Node exposing `stripTypeScriptTypes`.

## Verification and limits

Rust `third_party_licenses` tests check actual compiled metadata/lazy notices,
no-account dispatch, all required native fields, duplicate keys, unsafe URLs,
plain text, the Unicode identity vector, manifest revision and current
lock/target/provenance agreement.

Serial QML `licenses` checks original search/key/bundle logic, one lazy expanded
row, selectable exact text, collapse and retry. `licenses-preferences` uses the
real isolated worker and compiled manifest: translated General search/link,
application scope, actual Apache notice, wrong/stale receipts, explicit retry and
page disposal. Existing Preferences/keybindings/archive/composer tests and the
mocked installer suite are regressions. These tests do not modify user accounts,
projects or preferences.

The saved native light screenshot and Spanish Omarchy fixture were visually
compared for hierarchy, rows, versions and expansion. Different platform font,
DPR, theme and actual dependency manifests prevent a pixel-identity claim. Qt's
text spacing and the scoped ownership adaptation remain visible differences.
The broader screen parity goal remains active.

Scoped installation verified2026-10-17: backup
`/tmp/ot3-before-licenses-screen-9xn2z3p_`, installer
`/tmp/ot3-install-licenses-screen.py`. All12runtime/documentation files match the
source.13draft hashes, native client-settings, selected thread and closed-panel
state were preserved. Both local and native-ID node are ready; the hidden panel
was not forced open for validation. The existing isolated worker fixture verified
actual238-entry metadata/notice loading. Build-client, mocked installer and
translation816-key checks pass; no user preference/account mutation was used.
