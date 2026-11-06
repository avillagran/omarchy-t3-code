# Embedded terminal panel

The chat header and terminal shortcut open the Terminal side-panel tab. Each conversation retains its own terminal tab groups, active pane and split direction. The renderer attaches to the selected server's PTY; the desktop never starts a local shell for a remote conversation.

`src/terminal_surface.rs` captures the environment, thread and terminal identifiers for each view. It forwards UTF-8 input, native grid changes, clear and explicit close through typed terminal RPCs. A snapshot replaces the displayed history, replayed output at or before its sequence is ignored, and reconnecting resumes the same terminal without repeating code-block commands. Detaching or hiding the app leaves the server process running. Explicit close requests history deletion, matching the native terminal action.

`T3TerminalPanel.qml` and `T3TerminalPane.qml` provide terminal tabs, horizontal/vertical splits, selection, copy/paste, clear, explicit restart and close. Native keybinding context follows terminal focus, including Ctrl+J toggle and Ctrl+D split. Read-only environments cannot send terminal mutations. Layout persistence contains identifiers only, with a bounded size; terminal history remains on the server.

The native Qt plugin is loaded through `O3.Terminal 1.0`. It uses Ghostty revision `9f62873bf195e4d8a762d768a1405a5f2f7b1697`, the engine pinned by the reference web terminal. The launcher adds the native module directory to its import path because Quickshell resource URLs do not resolve a relative native plugin. Standard builds invoke `tools/build-terminal-surface`; release overlays include the plugin, module descriptor, engine revision and upstream license. The installer verifies runtime libraries and retains compatibility with source releases predating this surface.

## Verification

The `embedded_terminal` Rust integration fixture runs the actual desktop worker against a synthetic remote HTTP/WebSocket server. It covers captured destinations, UTF-8/control input, sequence deduplication, resize, clear, reconnect, one-time command execution, detach and explicit close. Unit tests cover target/grid bounds, view cancellation and private layout projection.

`python3 tools/test-device-panel-container embedded-terminal` exercises the actual backend singleton, terminal component and native renderer. `native-terminal` exercises the full application header, keyboard toggle, focus/split routing, draft retention, per-conversation layouts and stale-event rejection. `side-panel` covers the device route as a regression. The full application fixtures use a private Sway headless/pixman compositor; the smaller components use a private software X display. All run in rootless containers with read-only source, no network, private HOME/XDG directories and no host display access.

Build the optional Wayland test image with `podman build -t localhost/o3-qml-wayland-tests:local -f tests/containers/qml-wayland.Containerfile tests/containers`. The runtime has the compositor's file capabilities removed so test containers retain `--cap-drop=ALL` and `no-new-privileges`.

These checks are synthetic and do not establish installed application acceptance or complete wrapper parity. Context production and visible link/accessibility interactions have separate evidence; importer, cache lifecycle and drawer ownership remain unfinished. The engine alone does not establish every surrounding native terminal function.


## Structured context and local resource bounds

A terminal selection becomes a native inline reference and a versioned context
record. Drafts, frozen retries, queued edits and recall retain the record separately
from message text. The native producer normalizes CRLF, removes trailing blank
lines and reserves space for its exact truncation marker; it avoids splitting a
surrogate pair at the 64,000 UTF-16-unit text bound.

Each native context retains the upstream 200-record and 16-million serialized
UTF-16-unit limits. Queue editing can hold four contexts: the current draft,
frozen attempt, original queued message and backup draft. Local draft storage is
bounded to 200 MiB and worker IPC to 256 MiB so these allowed contexts survive
without discarding backups. Ordinary draft text/file metadata keeps its original
750,000-byte aggregate allowance. Large valid contexts temporarily require more
memory; idle process memory is unaffected. The separate O3 shared-editor and
shared-draft synchronization budgets are not raised by this change.

## Context persistence fidelity

The client preserves incoming JSON object insertion order, including future-kind
context payloads. Upstream projects an unknown record with `JSON.stringify`, so
non-index property order inside that payload is meaningful recursively. Retry,
queue-edit, shared-draft and stash comparisons retain that distinction. Ordinary
object fields and integer-index property order remain structural. Structural
revision hashes keep their previous sorted representation; already frozen outbox
requests retain their original identity and file bytes on retry after an upgrade.
Maintenance reports add an order-sensitive confirmation proof while retaining the
legacy durable effect claim, preventing an upgrade from repeating a claimed effect.

Stash and cross-thread prompt transfers persist context records with their inline
references and clear both together. Conflicting standalone excerpts receive fresh
identities and rewritten references. Conflicting attachment/annotation records
with dependent screenshot identities currently fail atomically rather than
performing the native dependent-record import. The private stash remains bounded
to 8 MiB and 20 entries; a capacity failure leaves the composer intact. These
importer differences and validation of every known non-terminal record kind are
still open parity work; terminal-context coverage does not establish complete
composer-context parity. Arbitrary future payload strings containing a lone
escaped UTF-16 surrogate also remain unsupported by the Rust JSON decoder;
paired Unicode strings are retained. A private watch probe confirmed that the
paired-string command receives its draft acknowledgement while a lone-surrogate
JSON command does not. The terminal producer avoids creating such half-pairs;
this does not establish unrestricted JavaScript JSON-string fidelity.

## Remaining native wrapper consumers

The completed engine checks comprise 52 Qt test cases, including 40 source-engine
oracle cases; the structured-context producer has 20 separate source cases.
Source-derived link detection and cell hit ranges pass 219 helper calls and 273
actual viewport cell checks, with explicit provenance. The visible terminal is
connected to them; private routing and QML checks cover preferences, metadata
admission, editor positions and current-thread fencing. This does not establish
the full pending-validation or mounted metadata lifecycle.
The native terminal surface activates a link on a plain first left click,
unless Shift is held. Its current drawer consumer uses Ctrl or Meta to force
the system browser; the generic platform-exclusive modifier helper belongs to
markdown link activation. It is not the terminal surface activation rule.

**Run in terminal** already creates an embedded tab and writes `command + "\r"`
once after its first live snapshot. Native T3 can reuse eligible idle drawer
terminals while excluding side-panel terminals; separate drawer ownership and
eligible reuse remain fidelity work. The legacy external frontend is an explicit
CLI path. The input and scrollback accessibility wrapper has its own private
QML evidence and 768 native geometry/pointer comparisons. Nightly 4288 has no
terminal search action; native picker/importer fidelity is still pending.

Native file-path admission rejects UNC and mixed leading separators before any
filesystem stat, requires an absolute or home-relative path, and enforces the
512 UTF-16-unit metadata limit. Windows paths normalize separators and case for
metadata while the selected editor receives the original position-bearing target.
Twenty-two source-derived metadata cases exercise these rules through actual
private RPC calls, including proof that rejected paths reach neither metadata
nor editor RPCs. The current final-open stat is temporarily stricter than the
native mounted-thread/session positive metadata cache. Porting that cache and
its invalidation lifecycle remains required work after initial visible wiring.

Consumer source intake is pinned to nightly 4288 commit
`32a0531748c44b25072d7495ce0b9bd7e9450a87`. Read that commit with `git show` or a
clean detached reference; an older working checkout is not current evidence.
The actual drawer validates path links through `getFileMetadataQueryAtom` before
opening the preferred editor. The original preferred-editor resolver and exact
catalog order/labels have 598 source-derived cases at that commit. Missing cache
entries expire after 60 seconds; known retained entries survive while the thread
and physical RPC session remain mounted. Exact invalidation/reconnect handling
and pending validation cancellation remain required work.
