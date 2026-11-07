# Buffered speech transport

This additive Rust API follows native nightly4314 (`83a3674ffd84ffb33de768210c806cb1ac53fd80`). It does not record a microphone, resolve a secret, download a model, or send anything automatically. The host must obtain recording consent and, separately, consent for cloud upload before explicitly calling `Connection::speech_transcribe`.

Buffered PCM requests, recognition options, cloud key presence, key/reference writes, OpenRouter model catalog and local/cloud transcription HTTP are implemented. [Local model controls and transcript postprocessing](SPEECH-CONTROLS-4314.md) complete the buffered HTTP surface. The [local live stream](SPEECH-STREAM-4314.md) adds owned WebSocket flow control and cancellation. New speech C/JNI entry points remain pending. No client UI, vendor copy or server capability is activated here.

## Audio and options

Audio is mono 16 kHz Float32 little-endian, nonempty and at most five minutes (19,200,000 bytes). Every sample must be finite and in [-1,1]. `encode_pcm_request` validates audio before transport. `decode_pcm_request` mirrors the native framing parser and returns the remaining audio without validating it; call `validate_pcm` separately when accepting a frame.

Framing is a four-byte little-endian UTF-8 metadata length, metadata, then unchanged PCM. Options are decoded through `TranscriptionOptions::decode`; unknown fields are discarded and omitted `speechSilenceFiltering` becomes true. Encoding retains native declared property order and the original dictionary spelling. String limits count UTF-16 code units. Duplicate dictionary spellings use ECMAScript trimming, NFC and contextual Unicode lowercase.

Native oracle evidence records Node 26.13, Unicode 17 and en-US default locale. The normalization dependency is pinned to Unicode 17, and Rust's Unicode version is asserted by the tests. The matching case conversion is the native en-US/default Unicode behavior; locale-sensitive alternative server locales are not certified. Rust strings/serde reject isolated UTF-16 surrogates before this API; that representational boundary remains explicit. No scalar-count approximation or ASCII-only dictionary normalization is used.

## HTTP behavior

Requests use actual same-origin `/api/voice/*` endpoints and admitted Connection credentials, including credential-generated headers for binary bodies. A separate no-proxy/no-redirect client uses the native client deadlines: 30 seconds for reads/control, 130 seconds for cloud PCM, and 30 minutes for local transcription/download/postprocessing. Cloud server execution retains its independent two-minute limit. There is no retry after audio transmission. JSON success responses are limited to 8 MiB; native cloud 422 errors are limited to 64 KiB and preserve the exact provider and tagged reason. Keys and dictionary/audio bodies are not logged. A cloud model query uses the exact author/model codec and URL encoding.

`CloudKeys.openrouter` is optional for older servers. Key writes accept a validated literal, configured provider reference, or null; success returns no secret. A secret reference is not resolved by this library. Server scopes/readiness and vault access remain authoritative. A transport failure never silently falls back to another account, provider, endpoint or local transcription.

## Evidence and execution

`tools/speech-native-oracle.mjs` executes the actual pinned Effect schemas and whole shared framing functions, with synthetic values only. `tests/fixtures/speech-4314.json` contains 64 source outcomes, including Unicode normalization/context/UTF-16 lengths, key/reference/model rejection and byte-exact framing. The Rust corpus compares acceptance, decoded encoding and frame bytes.

All Rust/HTTP tests and Android/JVM builds must run inside the official repository `tools/android-sandbox` wrapper. Synthetic loopback HTTP fixtures prove credentialed binary framing, typed provider errors, no cross-origin redirect, bounded response, no retry and invalid-audio refusal before transport. No live key, account, recording, provider call or audio upload is used. Fixtures do not delete trees, follow filesystem links or invoke host test harnesses.
