# Native PCM capture resource

`NativePcmCapture.js` contains the complete original `pcmResampler.ts`,
`pcmWorklet.ts` and `browserPcmCapture.ts` from T3 Code revision
`83a3674ffd84ffb33de768210c806cb1ac53fd80`. The compiler removes TypeScript
and module linkage and uses pinned Babel transforms for async functions,
class properties, optional catch bindings and arrows. The arrow transform
avoids a demonstrated Qt lexical-`this` failure in the original processor's
constructor callback. Original bodies and callback ordering are retained.

`createDsp` takes explicit AudioWorklet and math dependencies. `createCapture`
takes explicit browser/media/blob/timer dependencies. Neither factory changes
global constructors or prototypes. Hardware capture and the Qt implementations
of those media dependencies are still pending. The separate callback comparison
below covers the complete browser capture producer with controlled primitives.

## Renderer correction (2026-11-07)

The exact admitted Electron 44.10.1 renderer reports Chromium 152.0.7977.169
and V8 15.2.124.45-electron.0. Running the complete original modules there
demonstrates that Node 26.13.0 is not an equivalent numeric oracle: 64 of 73
coefficient cases and five PCM cases differ. The prior Node/Qt observations
below remain valid only for that pair of engines. They do not accept the C++
DSP adapter as a native browser capture implementation. That adapter is
unregistered and has no microphone consumer.

The entire compiled `NativePcmCapture.js` resource, using the actual renderer
Math implementation, matches all 73 original-module cases byte for byte,
including 24,582 math observations and all 4,800,000 limit samples.
`tools/test-native-pcm-renderer` records both that positive comparison and the
explicit negative Node reference comparison in one private sandboxed renderer.
It does not claim an AudioWorklet graph, microphone or consent integration.

Chromium capture also includes browser-selected audio processing. A raw Qt
audio source plus a resampler cannot establish that layer. The next capture
consumer therefore uses the complete original browser producer and actual
Chromium MediaStream/AudioContext/AudioWorklet primitives.

## Historical Node math dependency

Qt's ambient libm produced different filter coefficients and five different PCM
outputs in the initial corpus. The implementation now uses the complete,
unchanged V8 IEEE754 source shipped with Node 26.13.0 / V8 14.6.202.34, with a
small scoped QObject exposing sine, cosine and exponential. The original V8 and
Sun notices are retained and included in the application license manifest.

`native/voice/vendor/v8/UPSTREAM.json` pins the immutable Node source commit and
every original file hash. Three separate compatibility headers supply private
symbol visibility, `std::bit_cast` and portability macros without editing V8
function bodies. No V8 runtime or process is embedded. The math symbols remain
hidden so a second JavaScript engine cannot interpose them.

The native math build requires C++20, `-O3`, `-fno-fast-math`,
`-ffp-contract=off` and hidden visibility. The accepted build retains
`-Wall -Wextra -Werror`; no diagnostic is disabled. An earlier GCC build at
`-O2` reported a possible uninitialized local in the original large-argument
range-reduction routine. Both the failure and the accepted build are retained.
The complete source compiles cleanly at `-O3`.

## Validation and remaining integration

The unchanged shared driver executes the original TypeScript modules directly
in pinned Node and the compiled artifact in actual QJSEngine. It hashes raw
Float64 coefficients, Float32 PCM and Float64 meter values, without decimal
rounding or tolerance comparisons. It covers 73 cases over nine sample rates
from 16 kHz to 192 kHz: silence, impulse, alternating values, ramp, deterministic
noise, stereo mixing/cancellation, varied input block boundaries, meter-only
mode, repeated stop/flush and the complete five-minute limit of 4,800,000 output
samples (19,200,000 PCM bytes). Another 24,582 primitive observations compare the
scoped math dependency with the actual Node Math implementation, including
negative zero. All values match exactly.

The test uses a scoped C++ SHA-256 sink through the documented
[ArrayBuffer/QByteArray conversion](https://doc.qt.io/qt-6/qtqml-cppintegration-data.html).
All executable runtime checks run in private rootless containers with retained
fixtures, read-only sources, immutable executable copies, private HOME/tmp and
no network, microphone, host display, credentials or devices. Original V8
source hashes and generated artifacts are checked before execution and the
reviewed source fence is checked afterward.

```sh
python3 tools/build-native-voice --component capture \
  --native /path/to/pinned/t3code --compiler /path/to/locked/compiler \
  --node /path/to/pinned/node
python3 tools/test-native-pcm-qt --native /path/to/pinned/t3code \
  --node /path/to/pinned/node
python3 tools/test-native-pcm-qt --component lifecycle \
  --native /path/to/pinned/t3code --node /path/to/pinned/node
```

The lifecycle comparison executes the complete original `browserPcmCapture.ts`
directly in Node and its compiled resource in QJSEngine. The same controlled
media primitives drive 25 sequences: preparation refusal, already-aborted
signals, permission refusal and late permission after cancellation, selected
microphone constraints, module/resume failure or cancellation, exact buffered
blob bytes, streaming feed, final flush, stop timeout, processor error, device
disconnect, both duration-limit producers, cancellation during pending stop,
repeat stop/cancel and recording URI ownership. Event order, cleanup callbacks,
remaining timers, result/error strings and buffered bytes must match exactly.
The module/resume cancellation cases assert that the intended asynchronous
boundary was entered before cancellation. These synthetic browser primitives
do not establish Qt device acquisition or actual microphone permission behavior.
Cancellation uses the actual production `VoicePlatform.AbortController` in Qt
and Node's actual `AbortController`, with identical explicit cancellation reasons.
The media fixture does not supply a second imitation of the cancellation signal.

For subsequent focused checks, `--reuse-node-artifact` can mount a retained
private pinned Node snapshot without creating another persistent executable
copy. The runner refuses symlink components, a different owner, multiple links
or a mode other than `0500`. The pinned executable hash and file identity are
checked before and after private execution. This does not reuse a Cargo target
or authorize an executable from the live profile.

The default `--component controller` continues to generate the previously
accepted voice controller byte-for-byte. The compiler lock gains only the
pinned arrow transform required by capture. Device enumeration/acquisition,
independent consent, real audio transport and microphone acceptance still
require their actual consumers and validation.
