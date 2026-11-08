# Native voice resource discovery

The native watch process can select an explicit trusted startup configuration or
an application bundle whose addon digest and size were compiled into the client.
Selection never relaxes the existing runtime and helper admission checks.

## Explicit configuration

These startup variables form one complete group:

- `O3_NATIVE_VOICE_RUNTIME`
- `O3_NATIVE_VOICE_ADDON`
- `O3_NATIVE_VOICE_ADDON_SHA256`
- `O3_NATIVE_VOICE_ADDON_BYTES`

Supplying any member requires all four. Empty or invalid metadata and relative
paths are rejected. An explicit configuration takes precedence over a compiled
bundle. A partial or invalid override never falls back to bundled resources.
These values remain native startup inputs; voice wire actions cannot supply them.

## Executable-relative bundle

When no override is present, the client uses this fixed layout only if its build
included both `O3_NATIVE_VOICE_BUNDLED_ADDON_SHA256` and
`O3_NATIVE_VOICE_BUNDLED_ADDON_BYTES`:

```text
<client executable directory>/native-voice/
  electron-44.10.1-linux-x64/  # Exactly the 72 source-pinned runtime files
  electron-private-fd.node
```

The two values are compile-time environment metadata read through `option_env!`.
Setting them at application launch does not enable bundle discovery. Neither an
adjacent receipt nor the working directory, `PATH`, system Electron, a browser
profile, or a development cache supplies bundle identity. A build with neither
value leaves voice unavailable when no explicit configuration is supplied.
Supplying only one value is an invalid build configuration.

Both selection paths currently support Linux x86-64 clients only. The embedded
runtime manifest does not admit aarch64. Cross-building the Rust client must
target Linux x86-64 and pin an x86-64 addon; the metadata preparation host itself
does not determine that target.

Runtime admission still checks every member's full digest, size, immutable mode,
current-user ownership, single-link identity, canonical path and exact graph.
The addon is admitted and snapshotted with the same existing ownership, immutable
mode and identity checks. It must remain outside the exact runtime directory.
No resource is selected by a wire request or borrowed from another browser.

## Preparing build metadata

`tools/prepare-native-voice-build-metadata` reads an existing addon and prints JSON.
It validates the source-controlled runtime manifest and the addon's canonical
path, current-user ownership, single-link regular-file identity, immutable mode,
size limit and ELF64 little-endian x86-64 shared-object header. It detects changes
while reading. It does not compile, execute, install, copy or modify any artifact.
The header check does not prove the addon's implementation, ABI, exports or
provenance; the build pipeline remains responsible for producing it from the
reviewed `ui/browser/electron-private-fd.c` source.

Pass the JSON `environment` object as structured subprocess environment values
when building the client. Do not evaluate JSON through a shell. For example,
within an already authorized build job:

```python
metadata = json.loads(subprocess.check_output(
    [sys.executable, "tools/prepare-native-voice-build-metadata",
     "--addon", "/absolute/staging/native-voice/electron-private-fd.node"],
    text=True))
build_environment = {**os.environ, **metadata["environment"]}
# Pass build_environment to the existing coordinated client build process.
```

The addon must already have no write bits; runtime executable and nonexecutable
files normally use modes `500` and `400`, with private directories. A root-owned
system installation fails the existing current-user ownership requirement.

## Validation and remaining integration

Run the small Python fixture checks inside a private rootless Podman container,
without loading native code. The command inside that container is:

```text
PYTHONDONTWRITEBYTECODE=1 python3 tools/test-native-voice-build-metadata.py
```

Pure Rust resolver tests cover all 14 partial override combinations, override
precedence, malformed pins, target gating and executable-relative paths. They
must be included in the coordinated Rust validation; the Python checks do not
execute or replace those tests.

Build, package and installer source preserve this layout. Coordinated checks
cover the compiled client's default discovery and a real client/bundle helper
installation in container RAM. The whole installer and an actual isolated source
build still require separate acceptance. This change does not establish physical
microphone, cloud credentials, model setup, transcription or native browser
activation acceptance.

## Building and distributing the bundle

Set `O3_NATIVE_VOICE_BUILD_RUNTIME` to an absolute, already acquired and admitted
Electron runtime directory when invoking `bin/build-client`. The builder checks
all 72 pinned files, compiles the reviewed `electron-private-fd.c` with the local
C toolchain and Node N-API headers, seals the addon, then supplies the preparer's
JSON environment values directly to Cargo. Inherited compile-time bundle pins
are cleared. It reads the resulting client's internal metadata command and
requires the exact pins it supplied before staging or activating the bundle.
No adjacent receipt can enable a client compiled without those pins.

The builder never acquires a runtime or changes a system Electron installation.
An ordinary source build without this input explicitly compiles without bundle
pins. Linux x86_64 release packaging requires the explicit input; aarch64 release
packaging remains usable without native voice resources.

`tools/package-native-voice` copies only individually enumerated nofollow files
into a brand-new private staging directory. The runtime stays an exact 72-file
graph. Its original `LICENSE` and `LICENSES.chromium.html` remain within that
graph; the reviewed addon and the application's exact `LICENSE.o3-code` are
siblings. `build-metadata.json` describes the build but is never a runtime trust
anchor. The overlay writer checks actual compiled client metadata and streams
the sealed bundle directly into the release archive, without a second staged
runtime copy.

The installer reads package tools from the fetched, expected source commit
before changing an existing checkout. The extractor accepts only the fixed
bounded voice subset, rejects archive links, duplicates, traversal and unexpected
members, and writes fresh owned files with modes `500/400`. It compares the
receipt with the actual files, then invokes the new client's read-only default
bundle admission command. That command ignores startup overrides and never
creates a profile, display, consent helper or audio owner. Only after these
checks can the bundle be activated beside the installed executable.

Activation retains an old private bundle under `.native-voice-retired-*` instead
of recursively deleting it. Cross-filesystem installation first copies the
enumerated files into a new sibling staging directory, then renames the old and
new directories. Failed staging leaves the active bundle untouched. Retained
build and retired directories require explicit review and cleanup by their
owner; this workflow does not recursively remove them. Existing releases without
the bundle tools retain their original installer path.

The standalone builder uses the `install-client` transaction. Before
activation, it copies and checks the new client, prepares any cross-filesystem
bundle copy, and preserves the old client bytes in a new owned inode. The client
backup is deliberately a copy: replacing the active executable must still mark
the running worker's `/proc/self/exe` as deleted so its existing update watcher
can restart normally. After activation it checks default resource admission on
the installed client. A successful transaction retains the old client copy and
bundle. A caught replacement or admission failure restores that pair, retaining
failed bundle candidates for review. Restoring the previous client unlinks the
failed active executable inode so a concurrently restarted worker can also
observe its normal deleted marker. Rollback uses prepared files and renames
instead of requiring another client data copy.
Its boundary is the client/bundle pair. It does not cover the other build assets
or checkout. A first-install failure in this older pair helper retains the
failed executable under its transaction directory; it does not establish the
deleted executable marker for that particular failure case.

The modern installer uses `tools/install-native-overlay` for one larger group:
the source revision, all ten fixed executable/library/terminal assets, and the
optional voice bundle. Its tools are read from the fetched expected commit before
changing an existing installation. The fixed overlay extractor opens each output
individually with nofollow/exclusive creation and rejects missing, duplicate, link
or oversized asset members. All new assets, complete new bundle resources, and
byte-exact old asset backups are staged and checked before any active replacement.
The asset backups are copies, so a running old executable still gets its deleted
inode marker. After replacing assets and fast-forwarding source, the installed
client must pass real default resource admission.

Caught activation, source-advance or admission failures restore source, client,
libraries and bundle together. A failed first install restores the known original
absence of each exact new asset; open executable descriptors then have no links.
Rollback requires the exact expected HEAD, branch, index, tracked bytes, affected
path identities, installed assets and bundle identity. Its nofollow source plan
covers affected paths and ancestors in both directions, including an old tracked
path that is absent from the new commit. An ignored/untracked collision there,
an unexpected directory child during a directory/file transition, or a user edit
blocks the whole rollback. Unknown outcomes retain artifacts and report an error;
they do not reset or delete the user's data. Noncolliding untracked files remain.
Git hooks and filesystem monitors are disabled. The installer never uses a hard
reset, clean, or recursive directory cleanup; retained checkouts and downloads
require explicit owner review.

Source builds use `tools/prepare-source-overlay` to clone only the expected tracked
source into a separate retained checkout. The original builder completes every
asset there before the active installation enters the same overlay transaction.
An explicit `CARGO_TARGET_DIR`, or the installed checkout's target directory by
default, lets coordinated builds reuse their existing artifacts. The four original
generated license outputs may change for the selected compiler/architecture;
unexpected tracked edits, index changes, revision changes or an incomplete asset
group reject the stage. This workflow requires CMake as well as the existing
source toolchain and does not silently omit dictation or preview assets.

These transactions handle caught failures. They do not provide a durable journal
for process death between filesystem operations. User configuration, dependencies,
backend acquisition, service restarts and later installer hooks are outside the
group. Historical prebuilt releases without the new overlay helper retain their
legacy installation path. The full installer and actual isolated compilation have
not been executed for this change.

`tools/test-native-voice-package.py` exercises small synthetic package fixtures
inside a private container. `tools/test-native-voice-package --runtime <absolute
runtime> --addon <absolute addon>` checks the real graph and archive stream using
read-only mounts and a hashing sink; it neither copies nor executes Electron.
Add `--client <coordinated compiled client>` only when that binary's embedded pins
match the selected addon. The optional client check verifies real default lookup
with the runtime mounted beside the executable and a deliberately invalid
startup override, without performing an installation or opening audio/display.

The metadata nine-case and package fourteen-case suites were rerun with retained
private RAM fixtures after removing directory finalizers. Earlier receipts made
with `TemporaryDirectory` are not accepted as safety-compliant evidence. The
fifteen overlay cases cover bounded extraction, joint rollback, deleted executable
markers and source/user collision fences. Seven isolated-source cases substitute
an explicitly synthetic asset builder; they prove staging behavior, not a native
build. All runtime fixtures run in private rootless Podman, retain their RAM data
until container exit, and never recursively delete a directory tree. Separate
real client/bundle helper checks copied only into container tmpfs and verified
compiled pins, restored bytes and both replaced executable descriptor markers.

The actual overlay-helper proof also uses the compiled client, all 72 original
runtime members and the reviewed addon, with nine explicitly synthetic auxiliary
assets and private fixture Git revisions. After a second source fast-forward and
real installed-client admission, an injected failure restores the previous
source/branch/index, all ten asset hashes, the original bundle directory and all
75 member inodes. Old and failed executable descriptors both have zero links.
This proof uses only private container RAM and does not launch auxiliary native
libraries or the full installer. Receipt: `voice-real-overlay-ram-fbmntez_`.

## Microphone identity and metadata

Each `Resources` owner reserves its private `capture-profile` directory beneath
the durable application state/consent namespace, separate from cache assets and
per-recording directories. The consent codec still addresses only its unchanged
hashed grant filenames; it never enumerates or reads the profile subdirectory.
The directory's device/inode and its exact lock identity are checked throughout
a retained, nonblocking exclusive file lock. A second application cannot open the
same profile while that lease remains alive. Captures and asynchronous child
reapers retain the profile lease. A failed reaper spawn, failed wait or missing
group fence quarantines the child and lease until process termination; it does
not release an uncertain owner. This profile is created by the application; it
does not adopt a user's Chrome profile or accept paths from a voice action.

The capture process uses a persistent `session.fromPath` partition and the fixed
secure standard origin `o3-voice://capture`. Its asset route, renderer IPC and
permission request still require a fresh per-process nonce and the exact owned
main frame. Assets are immutable snapshots, remote requests are denied, custom
scheme CORS is disabled and renderer sandboxing remains enabled. The profile's
`user-data` and `session` directories must be canonical, private, owned directories
without symlinks. Successful shutdown uses `app.quit()` so Chromium can finish
its ordinary persistent-profile shutdown.

Media permission starts denied in every capture process. A prior profile does
not authorize recording: each valid audio request goes through the original
native permission decision. The complete original Nightly
`listBrowserMicrophones` module supplies device IDs and fallback labels after
that decision. Metadata travels as a separate bounded control event and reaches
Qt only for the current live voice owner. PCM continues exclusively over the
private binary capture channel. Exact selected-device constraints are unchanged;
an unavailable device fails instead of switching to another microphone.

The current selector receives metadata after a successfully prepared recording.
The same explicitly authorized capture observes `devicechange`, and the settings
refresh button can request a new list from that existing capture. Enumeration
uses the complete original helper without opening another stream or changing
the selected device. One enumeration runs at a time, with one coalesced follow-up
and at least 250 ms between requests. A three-second timeout disables that
observer for the current capture rather than accumulating unresolved native
requests; recording and PCM remain independent.

The refresh button is disabled while idle. The last accepted list remains
selectable between recordings, so a different microphone can be saved for the
next recording. A failed or timed-out initial enumeration reports unavailable
metadata and preserves that cache. A successful empty enumeration replaces it.
Owner/profile realm changes and helper loss invalidate cached metadata. Late
events or refresh completions after stop, cancellation or owner replacement
cannot update the current view or change preferences. Repeated synchronization
of the identical owner tuple leaves an active observer intact. Metadata revision
numbers increase up to JavaScript's safe-integer limit; exhausting that limit
closes only the metadata observer, without changing recording or PCM state.

On a fresh application launch it offers the system default and any saved ID as
unavailable until a recording supplies the current list. It does not request
permission or open a stream merely because settings were opened. Background
hotplug refresh while no capture is alive and first-use device discovery remain
separate integration work. Deleting cached capture assets does not reset device
IDs. Removing the durable state profile resets Chromium device IDs; an old
selected ID then fails without a fallback.

`tools/build-native-voice-microphones.mjs` strips only TypeScript types from the
whole source-pinned module using the private Node 26.13.0 build runtime.
`tools/test-voice-microphone-identity --runtime <absolute admitted runtime> --addon
<absolute immutable addon>` runs the exact capture scripts, addon and original
DSP with synthetic media in private rootless Podman. It checks sequential device
identity, a selected nondefault device, unknown and foreign-profile IDs,
permission refusal, cancellation during permission, PCM delivery and confirmed
process closure. It never mounts host audio devices, audio services or a display.
This test does not compile or accept the Rust profile/discovery/host integration
or the mounted Qt settings UI; those require coordinated build checks.

## Active capture refresh validation

The 2026-11-08 refresh change has the following local evidence. Receipt paths are
development artifacts on the validation machine; the scopes below distinguish
compile checks, metadata peers and actual capture/SDK execution.

| Check | Result and boundary | Receipt |
| --- | --- | --- |
| Rust strict and builds | `clippy --all-targets -- -D warnings`, library test build and client build passed; these commands do not execute the full test suite. | `/var/tmp/o3-build/voice-microphone-refresh-build-1/result.json` |
| Typed watch example build | Current example linked against the same frozen refresh sources, with no executable copy. | `/var/tmp/o3-build/voice-refresh-watch-example-build-1/result.json` |
| Rust metadata parser | All eight current parser cases passed; 168 other tests were filtered out. Covers initial/repeated metadata, ordering, payload bounds, unavailable status, stop/close and safe-integer exhaustion. | `/var/tmp/o3-build/voice-microphone-parser-8-w58qipzg/result.json` |
| Monitor in Node 26.13.0 | Six cases passed for the unchanged monitor module: original enumeration, coalescing, cancellation, errors and timeout. This older receipt's eight controller cases are superseded by the next row. | `/var/tmp/o3-build/voice-microphone-refresh-ke1f7hsq/result.json` |
| Current controller/Host in Qt | Nine cases passed using the actual original controller/platform and Qt QJSEngine with an explicit metadata transport peer. Covers idle/prepermission refusal, current scope, stale completions, owner loss, retained cache and successful empty lists. | `/var/tmp/o3-build/voice-microphone-refresh-8nu26_be/result.json` |
| Mounted settings QML | Actual backend/settings/select/button passed with the metadata peer: three refresh requests, two explicit permission scopes, original preference revisions `10 → 11 → 12`, literal labels, idle selection and unavailable/empty distinction. | `/var/tmp/o3-device-panel-v7zbrbr_/result.json` |
| Actual Electron capture JavaScript | Two sequential, explicitly authorized synthetic captures passed with stable selected device IDs and confirmed shutdown. A fixture-only second descriptor injects fixed DOM device-change events and enumeration failures/empty results; production scripts remain unchanged. | `/var/tmp/o3-build/voice-refresh-runtime-probe-2-f3jh53bz/result.json` |
| Actual Qt/watch/Rust Host/SDK | Passed through the production watch parser and typed synthetic example: one refresh RPC coalesces two API calls, two three-device lists, one explicit permission, 16,064 bytes of synthetic PCM, original draft insertion, idle cache and EOF shutdown with no remaining namespace processes. | `/var/tmp/o3-build/voice-microphone-watch-_v0xri5i/result.json` |

The current compiled client is 215,940,744 bytes with SHA-256
`bbb597ae1edab9b7fd4a43fa3bf7fa626a9b00563d8a0ad5e3fc4d5605c38983`.
The parser test artifact is 151,132,864 bytes with SHA-256
`fe38ecaa79d28231b78ba84fab6f39249c4fdb13c14e687b0160d53c974c1dfd`.
The actual watch example is 160,748,544 bytes with SHA-256
`d4120ad12d75684ebd253df5c9636ec211e461a3d2c2db1330d9417eb4202904`.
The runtime remains the exact 72-file Electron 44.10.1 graph and the reviewed
16,408-byte addon remains pinned to
`6cbce632628b75d8e214d76a0f84fdc543071a9e8f3b0406746079a08aa1ccc2`.

Runtime checks use private rootless Podman. The native watch proof mounts source,
executables, runtime and addon read-only, creates its own private HOME/config and
application state in RAM, and makes no binary/runtime copies. The QML metadata
peer writes only its own retained evidence directory. No check uses host audio,
host display, real credentials, a user's browser profile or an external server.
The SDK proof's HTTP peer runs on the container's private loopback interface.

Injected DOM events prove device-change plumbing, not physical microphone
hotplug. Fresh/idle device discovery remains unimplemented: denied enumeration
was separately observed to expose one blank-ID audio input both before and after
a prior recording. No permission grant or hidden stream is added to expand that
list. Physical microphone reconnection, cloud/provider setup, the full installer
and the full Rust runtime test suite remain outside this refresh acceptance.

Electron's pinned [session API](https://github.com/electron/electron/blob/v44.10.1/docs/api/session.md)
defines persistent path sessions. Its pinned
[protocol API](https://github.com/electron/electron/blob/v44.10.1/docs/api/protocol.md)
requires scheme registration before readiness and handlers on the selected
session. Runtime characterization also verifies that the secure custom origin
supports AudioWorklet and that the permission handler receives its serialized
origin with a trailing slash.
