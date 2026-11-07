# Test filesystem safety

Runtime tests run in private rootless Podman with read-only sources and dependencies, a private home, and no network. Android and JVM checks use `android-sandbox` exclusively. Tests retain filesystem fixtures for sandbox teardown: they must not recursively erase trees or follow fixture links outside their owned temporary directory.

The full Rust runtime collection remains blocked in `tools/test-rust-container` while the transitive audit is open. Explicit target selection is a collection boundary, not a declaration that every target has been reviewed. Do not bypass this boundary to rerun the historical full matrix.

## Production behavior and explicit test boundaries

Production browser-profile deletion and vault rollback retain their deletion behavior. Reviewed tests inject an explicit retaining filesystem adapter that quarantines an owned directory and records the requested removal. These tests prove validation, orchestration, retained contents, and namespace changes; they do not prove physical recursive deletion.

Build-cleanup tests use the same traversal through an in-memory filesystem implementation. They exercise ownership, race, quarantine, and mount-ID rejection. The former physical bind-mount case is now a fake-filesystem mount-rejection proof, not a physical mount test.

Vault rebase fixtures intercept abort and continue at an explicit Git-command boundary, retain sequencer metadata, and verify the requested recovery plan and working content. They do not execute Git's recursive sequencer teardown. Ordinary Git metadata operations remain functional fixture behavior; automatic fixture garbage collection is disabled.

Host-sync archives are constructed in memory. Their PAX tests pass bytes to GNU tar through pipes without extracting a filesystem tree. Installer tests record and quarantine production cleanup requests through an explicit command double.

## Bounded temporary resources

Frame cleanup unlinks only the fixed renderer frame slots and removes an empty owner directory. It never traverses unknown entries. QML import mirrors remove their recorded aliases and empty directories individually. Unknown artifacts and mirrors left after abnormal process exit can remain; exact abnormal-exit reclamation parity is still open. This bounded policy can retain temporary storage until sandbox or owner cleanup.

Filesystem symlink fixtures point within their own temporary fixture. QML loader aliases separately reference read-only mounted runtime or source dependencies; these are import plumbing, not evidence that arbitrary filesystem symlinks are safe to traverse.

## Checkpoint evidence

The safety checkpoint has 13 focused Rust cases passing across archive, vault, host-sync, frame, and browser-profile behavior, plus 21 build-cleanup cases. Three reviewed installer/bootstrap/restart scripts pass in private containers. The native QML preview helper builds, and all-target Clippy compilation passes. These scoped results do not replace a newly audited full runtime matrix.

Remaining review includes transitive production helpers reached by other tests and native browser/UI workflows. Retained fixtures and explicit doubles must stay visible in evidence; no environment switch, silent no-op, ignored test, or alternative host runner may stand in for destructive behavior coverage.

## Immutable runtime inputs

The Rust container runner keeps the shared compilation lock until it has copied
its test executables, embedded CLI and runtime libraries into a retained artifact
directory beside the resolved Cargo target. Containers mount that copy at the
original embedded debug path; later Cargo builds cannot replace their runtime
inputs. Evidence records each artifact's SHA-256 and byte length. The copier uses
`O_NOFOLLOW`, accepts regular files only, and never recursively removes artifacts.
Private regressions cover later source-file replacement and rejection of owned
symbolic links and paths outside the debug directory.

This freezes build artifacts. Source worktrees still need separate immutable
checkpointing for exact-commit acceptance. The full runtime audit marker remains
in force; artifact copying does not authorize unreviewed tests or establish
complete native browser/UI coverage.

The focused `tools/test-owned-browser-qt` runner executes only the reviewed new
`owned_browser_qt` target in private Podman. Its profiles, HTTP document fixture,
Qt inputs and immutable artifacts are retained. It performs no tree deletion,
uses no symlink fixtures, and launches only its explicitly frozen private server
and Qt binaries. Shutdown addresses only those owned child processes and socket
endpoints. This focused admission does not clear the full runtime-audit marker.

## Browser transitive audit follow-up

The real Qt coordinator fixture in `tests/preview_transport.rs` previously
constructed the production `Manager::start` adapter and later called
`cleanup.clear`. That path could reach physical recursive profile deletion.
It now uses the existing explicit retaining adapter: the active renderer must
still stop and flush, the selected partition must leave its original namespace,
one quarantine receipt must exist, and the retained partition must remain a
directory before the profile is reopened. This is orchestration and namespace
coverage, not physical deletion coverage.

All four real-manager import fixtures now inject the same retaining boundary,
including possible import rollback. Invalid-lock and symlink-storage checks
in `browser_profile_storage` pass an adapter that fails immediately if
admission incorrectly reaches removal. These checks cannot fall through to the
production recursive remover when an admission regression occurs.

This follow-up does not clear `TEST_RUNTIME_AUDIT_PENDING`. In particular,
whole-CLI test inputs, external command implementations and other transitive
helpers still need review before broad collection is authorized.
