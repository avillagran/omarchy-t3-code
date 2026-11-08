# Actual browser profile reporting chain

The production QML singleton now has a complete consumer proof through the
admitted client executable, real bearer HTTP authentication, persistent framed
WebSocket RPC, and actual connection-owned server catalogs. This extends the
previous separately accepted helper, client-transport and server-RPC evidence.
It changes no production policy or global validated source pin.

The private QML fixture imports the actual `T3Backend`, remains hidden and
unselected, and reads genuine server catalog snapshots from the owning fixture.
The actual client watch hydrates a private client-settings file and reports the
Work profile to two independently authenticated server States. A third actual
State grants read access but withholds `preview:operate`; its built-in catalog
and absent default remain unchanged. QML uses the existing native preferences
CAS to change recording frame rate from 30 to 60 and then replace the profile
name. Both allowed catalogs receive both changes through real report RPCs. No
fixture target event, transport peer or action result is injected.

`examples/profile_reporting_acceptance.rs` owns three real loopback listeners,
State databases and issued sessions, and serves the production HTTP/RPC path
through `serve_io`. It never installs a provider or starts the standalone server
background services. The actual descriptor discovers an available browser
executable; no browser engine, page, user profile or device is opened. Its
read-only catalog snapshots are observation, never report inputs. All state and
credentials stay in the private container; only nonsensitive evidence is retained.

The example passes minimal strict Clippy and an example build in the existing
coordinated target. Its 72,393,680-byte executable has SHA-256
`4dd078f9347cdd2ebe53af1a4c537aded185e91e1677fc059ebde12819781c9c`.
The actual client remains the existing 217,847,048-byte `80e44787…` executable,
mounted read-only with no copy or rebuild. Source and binary hashes are checked
before and after runtime: 915 client inputs and 650 server inputs are unchanged.

The first complete runtime passes with Qt and server exits zero and an actual
server stop acknowledgement. The rootless container uses a pinned existing image,
private Xvfb/HOME/tmp/config, read-only source and binaries, no host display,
authentication or audio devices, and only its private loopback network. Every
fixture is retained; no recursive cleanup is introduced. Two preexisting
NativeUsageLimits declaration-order warnings remain in the Qt log.

Evidence: `/var/tmp/o3-profile-chain-7lqtp9xd`; hashes and exact source/artifact
identities are in `docs/nightly/browser-profile-chain-proof-4340.json`. The final
3.5-second hold observes stable values while the real worker stays active. It
does not independently count periodic snapshot cycles or accept reconnect,
revocation, separate worker EOF, or browser retirement. Earlier component
lifetime receipts keep their original scope. This is not standalone startup,
physical browser STATUS/GUI, or whole Nightly 4340 acceptance.

The client repository runs the proof with:

```sh
python3 tools/test-profile-reporting-actual-container \
  --server-source /absolute/path/to/o3-server \
  --server-build /absolute/path/to/accepted-example-build/result.json \
  --client-binary /absolute/path/to/the/admitted/client
```

Build-only preparation uses the existing target/global compile lock, two jobs,
no incremental output and at least 1.5 GiB free before each Cargo command. The
fixture executable is never run on the host. Ordinary server CI only compiles
the example; it does not fabricate the external client/QML resources.
