# Owned native browser audio capture

`ui/voice/browser-capture-owner.mjs` consumes the complete pinned
`NativePcmCapture.js` factory with actual Chromium MediaStream, AudioContext,
AudioWorkletNode, Blob and AbortController objects. The trusted host supplies
the packaged factory/worklet URL and local callbacks. This owner has no server
credentials, model downloader, public capture endpoint or network destination.
The original producer owns device constraints, graph connections, silent output,
chunking, metering, five-minute duration, stop acknowledgement and flush order.

The owner is single-use. It tracks the actual permission promises, tracks and
AudioContext close promises, including a stream returned after cancellation.
It confirms close only after those resources settle. An unsettled system
permission still requires the enclosing process owner's deadline and killable
lifetime; no early successful receipt is manufactured. Cleanup uncertainty is
sticky. Streaming callbacks are fenced immediately on closure or a failed sink.
Buffered output stays in memory, is capped at 19,200,000 bytes, and is returned
once. The owned Blob URL is revoked on consumption or closure. A cancellation
during an asynchronous Blob read refuses the eventual audio result.

The renderer host must supply a sandboxed private origin and permission handler
bound to the exact capture frame, current capture lifetime and current user
decision. A preceding settings read or remembered destination alone cannot
authorize audio transfer. The host must independently recheck current server
authority and the audio consent grant before every SDK feed. Those process,
consent, permission UI and SDK consumers are not installed by this checkpoint.
No application preference or public capability selects this owner yet.

## Runtime evidence

`tools/test-native-pcm-browser` verifies the exact admitted Electron 44.10.1
runtime before and after execution. All checks run in a retained rootless
container with read-only sources/runtime, private HOME/tmp/Xauthority/display,
256 MiB shared memory, no network, host audio socket, device or profile.
Chromium's test-only fake media device supplies synthetic audio. Renderer
sandbox, context isolation and Node isolation remain enabled. The fake device
flag and permission handler exist only in the private fixture.

Seven actual media-graph cases cover streaming, buffered capture, cancellation
after preparation, cancellation while getUserMedia is pending, cancellation
during a buffered read, a failed streaming sink, and an unavailable exact device.
The fixture observes nonzero finite Float32 PCM, live graph before recording,
native meter events and stopped/closed tracks/contexts afterward. Late audio is
refused. Raw PCM is neither logged nor included in JSON receipts; only synthetic
buffer size/hash and lifecycle metadata are retained. Sample counts depend on
real audio scheduling and are observations, not fabricated golden values.

The independent renderer numeric comparison matches the entire compiled DSP
resource against the original native modules over 73 cases, all 4,800,000 limit
samples and 24,582 primitive math observations. It also retains the demonstrated
negative comparison with Node 26.13.0. The historical Qt raw-device adapter is
unselected and does not establish browser audio processing parity.

This evidence accepts synthetic device acquisition and the actual renderer
owner. It does not accept a physical microphone, OS permission dialog, acoustic
accuracy, full compositor integration, independent audio/cleanup consent,
cloud transport or public voice activation.
