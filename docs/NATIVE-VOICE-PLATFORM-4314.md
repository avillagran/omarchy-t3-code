# Whole original browser voice platform in Qt

`NativeVoiceInput.js` compiles the complete original
`createBrowserVoiceInputPlatform` and its nominal error predicate, together with
the complete original `cloudSpeechLanguages` and `effectiveSpeechLanguage`
declarations. The pinned source is nightly 4314, revision
`83a3674ffd84ffb33de768210c806cb1ac53fd80`. No independent language, mode or
translation reducer replaces those declarations. Source hashes and the pinned
Node/TypeScript/Babel compiler lock accompany the output.

Imports become explicit factory dependencies. Standard Babel transforms adapt
async functions, class properties and optional catch bindings to Qt; no globals
or prototypes change. The host must supply the same Schema/error class realm
used by its voice controller. Capture, SDK work, cancellation and binary storage
remain host capabilities. The platform retains original cloud-versus-local
routing, preparation-time option capture, locale selection, streaming-versus-
buffered selection, callbacks and typed failure behavior.

The whole original module runs in Node with actual canonical Effect 4.1.2. Its
shared language declarations are selected intact from the original AST. The
same unchanged test driver runs against that module and the generated Qt
artifact: 500 outcomes and ordered boundary traces match, with 489 successful
synthetic transcripts, six cancellations, three unavailable outcomes and two
wrapped transcription failures. The grid covers streaming, buffered streaming,
translation, absent/empty/single/multiple language lists, detection, supported
and unsupported intents, all three cloud providers, missing OpenRouter model,
captured options, callback identity, lone UTF-16 units, and nominal versus plain
lookalike errors. It tests the platform's own abort checks; SDK and capture
boundaries here are explicit in-memory doubles.

Evidence: `/var/tmp/o3-build/native-voice-platform-ib6yl5h6/result.json`.
All 2,343 canonical Effect manifest files were verified. Both engines execute
in private offline rootless containers with read-only sources, no microphone,
host display, credentials or devices. The preceding
`native-voice-platform-luqwvj7a` receipt remains failed: Qt rejected the first
test driver's async syntax before execution. The shared driver now uses ordinary
Promise chaining in both engines; production output did not change.

Rebuild and compare using existing verified caches:

```sh
python3 tools/build-native-voice --component platform --native /path/to/t3code \
  --compiler /path/to/compiler --node /path/to/pinned/node
python3 tools/test-native-voice-platform --native /path/to/t3code \
  --dependencies /path/to/effect --compiler /path/to/compiler \
  --node /path/to/pinned/node
```

This factory is not installed in the product UI. The real persistent worker
must still connect original capture preparation with the one-shot SDK stream,
keep recordings as owned binary handles outside QML, decode status with the
original full codec, and connect original settings/options/cleanup callbacks.
Physical capture-to-SDK transport has separate accepted evidence in
`NATIVE-VOICE-ASYNC-4314.md`; it does not certify simultaneous live capture or
this platform's product integration. The existing local dictation helper keeps
its separate explicit settings until that integration is accepted.
