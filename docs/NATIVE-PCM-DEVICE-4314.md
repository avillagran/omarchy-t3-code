# Qt device-byte consumer for the original PCM processor

**Scope correction (2026-11-07):** this unregistered adapter matches Node
26.13.0, whose V8 math differs from the actual admitted Electron renderer.
The renderer comparison in `NATIVE-PCM-CAPTURE-4314.md` supersedes any native
browser parity interpretation of the observations below. No microphone
consumer selects this adapter.

`native/voice/pcmdsp.cpp` consumes device-format bytes described by `QAudioFormat`
and executes the complete pinned native worklet in a private QJSEngine. It uses
the same unchanged V8 math dependency as the historical Node comparison in the
[capture resource](NATIVE-PCM-CAPTURE-4314.md). Original channel averaging,
resampling, phase, metering, chunks, flush and duration limits execute in that
resource. C++ supplies the device and MessagePort boundaries.

The class is thread-confined and single-use for one capture context. Its owner
provides a trusted packaged resource and the actual device format; the resource
is not a user expression, HTTP/MCP argument or preference path. Preparation
rejects invalid formats, including the Qt sample-format sentinel with a zero
frame size, and preserves the native minimum 16 kHz requirement. It does not
restrict a valid device to the sample rates in the test corpus.

Each callback accepts at most 1 MiB. The audio owner splits larger device reads.
Complete frames are processed in batches of at most 4,096 frames; an incomplete
frame carries into the next callback. Stop discards only an incomplete frame
and asks the unchanged processor to flush. It never fills missing input with
invented samples. The capture context cannot restart a stopped processor.

PCM, level, limit and stopped messages are queued outside the processor's
JavaScript stack. A receiver can cancel from a level/PCM callback without
re-entering `process()`. QObject lifetime cancels queued deliveries, and a guard
stops a delivery batch if its receiver destroys the owning object. The PCM
boundary is an ArrayBuffer/QByteArray conversion, without audio JSON or a
decimal representation. No device, filesystem or network is opened by this class.

## Native sample conversion

The converter uses the complete unchanged Chromium sample traits from revision
`92a6f15143569cf59b340472d0b5030465d95722`, version 152.0.7977.169, selected by
[Electron 44.10.1 DEPS](https://github.com/electron/electron/blob/v44.10.1/DEPS).
The original [header](https://github.com/chromium/chromium/blob/92a6f15143569cf59b340472d0b5030465d95722/media/base/audio_sample_types.h),
BSD notice and source hashes are retained under `native/voice/vendor/chromium/`.
Only format selection and unaligned-safe reads are adapted. A second Chromium
runtime is not embedded.

The generic Qt normalization helper has different unsigned-zero and negative
integer scaling in the reviewed
[Qt implementation](https://github.com/qt/qtmultimedia/blob/dev/src/multimedia/audio/qaudioformat.cpp).
The selected original Chromium traits supply U8/S16/S32 conversion and unchanged
Float32 values. The worklet continues to own mixing, filtering and clipping.

## Validation

The actual C++ consumer passes the unchanged 73-case whole-module Node oracle,
including Float32 mono/stereo, cancellation-safe unstarted/stopped behavior,
varied blocks, synthetic device writes split after three bytes, nine sample
rates, meter-only capture and the complete five-minute output limit. Its PCM,
meter and event bytes match exactly. The shared oracle also retains the existing
coefficient and 24,582 scoped-math observations through the resource test adapter;
the C++ consumer does not expose an internal coefficient inspection API.

Five invalid-format cases verify refusal before processing. A real queued Qt
receiver cancels during a level callback and receives the stop acknowledgment;
the fixture proves that callback runs after the feed stack has returned.

A separate Clang-compiled executable runs the complete original Chromium traits
to produce 393,219 raw-sample/normalized-Float32 observations. U8 and S16 cover
every integer value; S32 includes precision boundaries, extrema and deterministic
32-bit samples. The actual device adapter is compiled separately with the usual
C++ compiler. Eighteen mono/stereo cases at 16, 44.1 and 48 kHz feed its raw device
bytes while the original Node worklet receives the native corpus's normalized
values. PCM, meter and stop-event bytes match exactly, including split frames.
The expected values do not come from the adapter's own conversion method.

```sh
python3 tools/test-native-pcm-qt --component device \
  --native /path/to/pinned/t3code --node /path/to/pinned/node \
  --reuse-node-artifact /absolute/private/pinned/node
python3 tools/test-native-pcm-qt --component integers \
  --native /path/to/pinned/t3code --node /path/to/pinned/node \
  --reuse-node-artifact /absolute/private/pinned/node
```

The build includes Qt6Multimedia and retains the exact-math compiler flags and
`-Wall -Wextra -Werror`. Runtime runs inside private rootless Podman with retained
fixtures, read-only sources/binaries, no network, microphone, host display,
credentials or devices. The final source and admitted executable are checked
after execution. A retained verified Node snapshot avoids another large copy.

## Remaining capture owner

This is the reusable device-byte consumer. It is not yet registered as a public
QML type or called from the product's microphone controls. Actual QAudioSource
acquisition, physical device-format negotiation, permission and late-cancel
handling, a killable capture-process lifetime, private binary transport, browser
primitive bindings, consent and SDK audio delivery remain required consumers.
The original browser capture lifecycle has separate accepted callback evidence;
that evidence does not establish the remaining physical device path.
