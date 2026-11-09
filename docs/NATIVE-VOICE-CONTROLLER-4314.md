# Native voice controller in Qt

`ui/js/NativeVoice.js` retains the complete original voice controller,
transcription errors and `replaceTextRange` from T3 Code revision
`83a3674ffd84ffb33de768210c806cb1ac53fd80`. Source hashes, compiler lock and
generated output hashes are recorded beside the artifact. No independently
written reducer replaces its state machine.

Qt's JavaScript engine needs standard Babel transforms for async functions,
class properties and optional catch bindings. Babel is build-only; its generated
helpers retain their original MIT notice in `licenses/Babel-helpers-MIT.txt`
and the application license manifest. Promises and generators run in Qt.

Qt does not recognize the original Unicode property expressions. The build
evaluates those four original expressions with pinned Node 26.13.0 / Unicode 17
over all 1,114,112 code points, including unassigned values and surrogate code
points. Two generated range tables implement the original letter/mark/number
and Latin/number properties. UTF-16 boundary handling preserves paired and lone
surrogates. The retained binary fixture records the original four-bit result
for every code point; the Qt test independently hashes all four predicates.

`VoicePlatform.js` supplies scoped dependencies for only this controller:
abort notification and the transcription error schema's constructor/type
predicate. It does not modify globals or prototypes. The schema type predicate
uses Effect's nominal class/HMR marker semantics; plain lookalike objects do not
become typed transcription errors. These adapters are not a general browser
AbortSignal or Effect implementation.

The shared test driver runs unchanged against the whole original TypeScript
modules with actual canonical Effect 4.1.2 and against the Qt artifact. It
compares 1,920 insertion cases, 53 asynchronous lifecycle scenarios, 60 error
type predicates and seven constructor outcomes. Scenarios cover microphone
refusals, deferred preparation/cancellation, multiple controllers, owner and
draft changes, selection changes, recorder interruptions, buffered retry,
live/streamed completion and transcript cleanup success/failure/skip. Strings
are compared as UTF-16 units, including lone surrogates. Async observations are
taken after draining microtasks at externally controlled steps, without a
wall-clock timing equivalence claim.

Capture, recording deletion and transport dependencies in this test are
synthetic in-memory callbacks. Deletion records an observation and never
removes a file. Both engines run in retained private rootless containers with
read-only sources, private HOME/tmp, no network, microphone, host display,
credentials or devices. All canonical Effect manifest files and immutable Node
bytes are checked before execution. The client-wide runtime audit remains open.

Rebuild with an already acquired compiler matching
`tools/voice-compiler/package-lock.json` (install dependencies with
`npm ci --ignore-scripts` in a separate build cache):

```sh
python3 tools/build-native-voice --native /path/to/pinned/t3code \
  --compiler /path/to/locked/compiler --node /path/to/pinned/node
python3 tools/test-native-voice-qt --native /path/to/pinned/t3code \
  --dependencies /path/to/canonical/effect/dependencies --node /path/to/pinned/node
```

`T3Backend.createVoiceController` binds one shared native runtime to the actual
selected draft. It reads source-text UTF-16 positions and commits synchronously
through the existing edit, undo, persistence and cursor-restoration paths. A
committed A→B→A edit changes the revision; edits to another draft do not. Owner
changes cancel the operation. Replacing a controller disposes the old one and
the native session mechanism waits for its underlying preparation to settle.
Hiding the panel interrupts capture. The composer disables submission while
the original controller blocks it and routes Escape to native cancellation.

The focused `voice-controller` QML fixture mounts the actual Backend and rich
Composer in a private software X server. It verifies real editor text/cursor,
undo/redo, submission refusal, edit/owner races, replacement settlement, physical
Qt Escape, background interruption and context-chip/source-position conversion.
Its worker and local dictation helper are explicitly inert, positively checked
before launch, and live in a separate private source namespace. Source hashes
are checked before and after the run. Run only this reviewed fixture with:

```sh
python3 tools/test-device-panel-container voice-controller
```

Independent audio/cleanup consent and actual capture-to-SDK buffered/streamed
transport now have separate physical acceptance in `NATIVE-VOICE-ASYNC-4314.md`.
The whole original browser platform also matches Qt across 500 boundary traces;
see `NATIVE-VOICE-PLATFORM-4314.md`. A persistent worker still needs to install
those real dependencies in this controller and connect their preference views.
The existing local dictation helper uses its own explicit settings.
