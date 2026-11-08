# Native voice UI (nightly 4314)

The Composer selects the native voice controller only when its real host reports availability. The existing local dictation flow remains the fallback. The microphone button, cancel/settings actions, level indicator and plain-text preview use the same controller that owns draft replacement, undo and submission blocking.

Audio and cleanup consent are separate explicit choices. Opening settings grants neither. The microphone modal shows the current destination as plain text, initially focuses Block, denies on Escape and fences its session/request identity against draft changes. An uncertain microphone shutdown or lost host requires restarting the application; the UI does not silently reopen capture.

Voice preferences use the original nine-field codec through the host's revision-checked writer. The UI includes language, translation, shortcut mode, silence/filler handling, structured terms and aliases, cleanup instructions and correction cue. Ctrl+Space retains the native 300 ms auto/hold/toggle behavior and waits for preparation before stopping a released hold. Missing context does not imply microphone permission.

## Acceptance and limits

`python3 tools/test-device-panel-container voice-ui` runs the actual Backend, NativeVoiceHost, platform/controller, Composer and settings in private rootless Podman. Only the worker transport is synthetic; it never captures audio, calls a provider or opens account/profile data. The source-fenced receipt is `tests/fixtures/native-voice-ui-proof-4314.json`; its successful evidence directory is `/var/tmp/o3-device-panel-zjskqddd`.

The test covers explicit consent, all nine original preference writes and dictionary edits, Block/Escape/Allow, native controller start/stop and draft insertion, literal preview/identity, stale permission rejection after a draft switch, and physical held Ctrl+Space release while permission is pending. The earlier `_wud5edu` failure was an incorrect fixture lookup for QObject-owned popups; it is retained. The corrected lookup and expanded acceptance passed.

This is not physical microphone/provider acceptance. Device enumeration is not exposed by the current host: only system default and a previously saved device identity are shown. Stability of device IDs across private capture profiles is unverified. The language field accepts an explicit code; destination-supported language discovery is not exposed. New cloud destinations remain unavailable until the original key-save and explicit acknowledgment workflow is connected; existing grants are displayed without creating a new grant. These limits must not be represented as complete voice parity or automatic activation.

## Streaming preview correction

The first UI acceptance used a synthetic `{text}` preview object, which did not match the native contract and masked an empty-preview defect. That assertion is superseded. Native `SpeechStreamText` has `committed` and `tentative` strings; nightly 4314 ChatComposer renders adjacent spans without adding whitespace, and PreviewAnnotationSpeech explicitly concatenates those fields. The corrected consumer and transport fixture preserve that concatenation exactly. The new actual UI proof asserts HTML-like text in both fields remains literal `Text.PlainText`. Native source hashes are recorded in the receipt; previous passes are not reused as evidence of this correction.
