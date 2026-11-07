# Native voice consent, revision 4314

This checkpoint retains the whole original `voiceConsent.ts` and
`useLocalStorage.ts` producers at
`83a3674ffd84ffb33de768210c806cb1ac53fd80`. The whole destination, EnvironmentId,
cloud-model and UUID declarations supply their actual Effect 4.1.2 codecs and
equivalence. There is no separately implemented consent state machine or schema
parser. Babel 8.1.0 selects complete declarations; Node 26.13.0 erases TypeScript;
Rolldown 1.2.16 bundles the actual Effect dependency graph. Source and dependency
hashes are recorded in `ui/voice/native-consent.sources.json`.

The framework adaptation replaces `useSyncExternalStore` with a fresh snapshot
read. Qt must subscribe to the separate storage notification and request a new
snapshot. This accepts the data producer, not React rendering or a Qt settings
consumer. `window`, `CustomEvent` and the redacted diagnostic sink are explicit
realm-local bindings. UUID generation remains the whole original function using
the runtime's real cryptographic random source.

## Private persistent storage

`ConsentStorage` admits only the two original consent keys and their original
pending-save marker namespace. Ordinary preferences receive no storage handle.
Each key is hashed from its UTF16LE code units, so a legacy ID can never become a
filesystem path. The framed key/value bytes retain arbitrary UTF16, including
unpaired surrogates. EnvironmentId has its original trimmed nonblank type, with
no invented 64-character limit. Unknown fields and cloud-model syntax are handled
by the original codecs.

The namespace is a canonical owned 0700 directory with a held descriptor. Every
operation checks its identity again. Records are owned 0600 regular files with
one link; reads use `O_NOFOLLOW` and verify the held inode around the read.
Writes use fresh exclusive staging files, fsync and exact-key atomic rename.
Removal renames one known entry to a retained tombstone. There is no recursive
cleanup, rootfs extraction or traversal through a grant-supplied path. Old
workers deny access after namespace replacement.

An individual key/value record is bounded by Chromium 152's 10 MiB storage-area
quota constant ([original source](https://raw.githubusercontent.com/chromium/chromium/152.0.7977.169/components/services/storage/dom_storage/dom_storage_constants.h)).
This adapter does not yet reproduce Chromium's aggregate multi-client quota
accounting. Tombstones and failed staging records remain retained. Completing
quota accounting and a bounded retention policy remains packaging work; neither
may restore a grant or remove arbitrary trees.

## Helper and Rust owner

The small helper executes the original producer under the same admitted
Electron 44.10.1 graph as capture, using its native RunAsNode mode and embedded
Node 24.22.1. No runtime flags change its fuses. The existing native descriptor
helper checks the inherited AF_UNIX peer PID/UID and sets CLOEXEC before work.
The helper accepts bounded, sequenced metadata commands only. It has no audio,
server credential, network operation or generic preference API.

Bindings and cloud acknowledgement closures stay inside their originating
helper. The Rust `AudioGrantGuard` contains an opaque handle and a weak reference
to that owner. Every `is_current()` performs a fresh original `bindAudioGrant`
check. No snapshot authorizes a send. Reselecting the same destination produces a
new original identity; a late acknowledgement can acknowledge only its own
pending choice. Dropping an acknowledgement forgets its closure without
promoting a pending grant. Cleanup uses the independent original key.

Rust keeps inner grant/destination JSON as a string. This preserves raw native
UTF16 across serde's UTF8 boundary for Qt to decode later. A dead helper,
uncertain protocol, replaced namespace or failed storage read denies access.
The owner retains the runtime/assets and an unreaped child identity. Retirement
requires the actual close response, pidfd exit, terminal owned-group fence
before reap and success status. Forced or uncertain retirement stays Unknown;
Drop cannot revive a grant. This is a local application factory, not a public
MCP or CLI action.

## Acceptance and remaining consumers

- 20 public-operation traces run on the unbundled whole original modules under
  pinned Node 26 and on the compiled producer under actual Electron 44.10.1. Only
  fresh opaque UUIDs are normalized; all resulting choices and consent decisions
  match. Cases include independent cleanup, stale ordinary preferences,
  same-destination reselection, pending markers, late ACKs, invalid models,
  corrupt storage, unknown fields and long/unpaired UTF16.
- Nine physical cases exercise three actual private helpers, cross-process
  reads and notifications, marker read failure, hardlinked records, namespace
  replacement, real descriptor admission/CLOEXEC and clean exits. No host
  settings, devices, display or network are mounted.
- Seven actual Rust-owner scenarios cover the same grant/ACK boundary, raw UTF16
  transport, namespace replacement, Drop, helper death, stopped-helper retirement
  within one shared five-second budget, and invalid-addon startup.
  Build and strict all-target Clippy pass. All runtime execution is in a private
  rootless container; there is no host test execution.
- Effect's exact MIT notice is included in the compiled application catalog,
  now 273 entries, alongside the original T3 notice. The retained Electron graph
  continues to carry its original notices.

This checkpoint does not activate microphone capture, SDK upload, transcript
cleanup, settings controls or the composer. Those consumers must bind the current
grant, retain Unknown retirement, check current server authority and consent
immediately before every audio operation, and retain the original controller's
owner/revision and cancellation behavior. Original cleanup additionally compares
the recording's captured cleanup grant with a fresh `readCleanupGrant()` before
its request. No SDK private patch is introduced.

Reproduction: build with `tools/build-native-voice-consent`, run the whole-source
and helper checks with `tools/test-native-voice-consent`, and run the real Rust
owner with `tools/test-voice-consent-process`. Supply the already admitted pinned
runtime, compiler/dependency artifacts and the worktree's exclusive Cargo target.
Failed receipts and fixtures are retained; never rerun unchanged failures.
