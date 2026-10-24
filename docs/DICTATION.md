# Dictation

The composer can turn speech into prompt text on this computer. Nothing is sent
anywhere: `bin/ot3-dictate` records with PipeWire and transcribes with
[whisper.cpp](https://github.com/ggml-org/whisper.cpp), the same approach as
on-device keyboard dictation (FUTO Voice Input uses whisper.cpp too).

## Using it

- **Microphone button** next to Attach and the screenshot button. Click to start,
  click again to insert the text. While it records, a bubble with the recording
  time grows out of the microphone with the last two seconds of microphone level
  against the room's noise: flat means nothing is heard, tall red bars mean it hears a voice. If the system
  microphone is muted the bubble says so and a click turns it on.
- Pauses are kept short before transcribing, and a long dictation is cut into
  pieces that each fit Whisper's 30 s window, so everything said after a long
  silence or past the first half minute still comes through.
- **Ctrl+Space** in the composer: hold to talk (released after ~450 ms, it stops
  and inserts), or tap to start and tap again. **Esc** cancels and inserts nothing.
- Text goes in at the draft's cursor, with a space added where words would touch.
  Undo removes it like any edit. A dictation started in one thread finishes in
  that thread's draft even after switching threads; a draft that is busy sending
  gets the text on the clipboard instead.
- Hiding the panel cancels a running dictation, so the microphone never stays
  open behind it.

Preferences → General → **Dictation**: the button, the model (download/remove),
the language, a vocabulary and how long recordings are kept.

## Lightweight by design

There is no daemon and no model server. `ot3-dictate listen` starts recording
at once and loads the model on a second thread while you speak, so loading
costs no waiting. When it stops it transcribes, prints one JSON line and exits:
idle memory is zero. The helper is 1.5 MB; models are downloaded on first use
to `~/.local/share/omarchy-t3-code/dictation/` and verified by SHA-256.

| Model | Download | 10 s of speech, M2 Pro CPU |
|---|---|---|
| Fast (`ggml-base-q8_0`) | 82 MB | ~0.6 s |
| Accurate (`ggml-small-q8_0`, default) | 264 MB | ~1.2–1.4 s |

Measured choices behind those numbers:

- **q8_0, not q5_1.** On ARM NEON q8_0 ran about three times faster for the same
  text (small: 4.1 s → 1.4 s).
- **Audio context sized to the clip.** Whisper encodes a 30 s window by default;
  the helper sets `audio_ctx` to the clip length plus a margin (20 ms per step).
- **At most 8 threads.** Using all 12 cores of the M2 Pro (with efficiency cores)
  was 3–10× slower.
- GPU: whisper.cpp's Vulkan backend was tried; on Asahi without a GPU Vulkan
  driver it only reaches lavapipe and gains nothing, so packages are CPU-only.

## Quality guards

- Silence is never transcribed (Whisper invents captions from it). Leading and
  trailing silence is trimmed by frame energy against the room's noise floor
  (the 10th percentile of 30 ms frames, so a clip that is mostly one long pause
  still finds its speech), every pause inside is shortened to 0.6 s, and a clip
  with under ~120 ms above the floor counts as silence.
- **One window per piece.** Whisper decodes 30 s at a time; handed a longer
  clip it slides the window itself and what falls in the second window comes
  back garbled or not at all (a 4-minute reading came back as 34 words, the
  last seven of them the same name). So a clip longer than 25 s is cut into
  pieces at the quietest 120 ms of the 1.5 s before each 25 s mark, and each
  piece is decoded on its own with `single_segment`. The language is detected on the first piece and fixed for
  the rest. A piece that decodes to nothing is retried with a beam search over
  the full window. Pieces are joined with a space, dropping a period Whisper
  put at a cut when the next piece carries on in lowercase.

## Kept recordings

Dictated prompts are instructions for code and come out of a train of thought
that is hard to repeat, so the audio is the one faithful record. Each
dictation's raw recording and its text are kept under
`~/.local/share/omarchy-t3-code/dictation/recordings/<thread>/<millis>.wav`
and `.txt` (the thread folder is the thread key with anything outside
`[A-Za-z0-9._-]` replaced by `_`). Resting the mouse on the microphone for a
moment lists this thread's recordings, newest first: a click plays one with
the system player, the trash icon deletes it, and the footer opens the folder.
A thread's recordings go with it when it is archived or deleted. Preferences →
Dictation → **Keep recordings** chooses the retention: until the thread is
closed (default), at most a month, a week or a day (pruned when the panel
starts and when the choice changes), or not at all. A kept recording can be
run through `ot3-dictate transcribe <file.wav>` to reproduce a transcription.
- Bracketed annotations (`[Música]`, `(risas)`) and known invented captions
  ("Subtítulos realizados por la comunidad de Amara.org", "Thanks for watching")
  are dropped.
- **Vocabulary**: the project, branch and thread title, plus the user's own
  words, go into Whisper's initial prompt (600 characters at most) so names come
  out spelled right.
- **Language** defaults to automatic. A fixed language helps very short
  phrases, but Whisper then *translates* speech in another language into it.

## Helper interface

```
ot3-dictate models                  {"models":[{"id","sizeBytes","installed"}],"default","recorder"}
ot3-dictate download <base|small>   {"type":"progress","bytes","total"}… {"type":"done"}
ot3-dictate remove <base|small>
ot3-dictate listen --model <id> --language <auto|code> [--prompt <text>] [--max-seconds <n>] [--keep <folder>]
    {"type":"recording"} {"type":"level","value":0..1}… {"type":"transcribing"}
    {"type":"text","text","language","seconds","elapsedMs"}
ot3-dictate transcribe <file.wav|raw> [listen options]   16 kHz mono s16le
ot3-dictate recordings <folder>     {"recordings":[{"id","file","time","seconds","text"}]} newest first
ot3-dictate delete <folder> <id>    remove one kept recording
ot3-dictate forget <folder>         remove a thread's recordings
ot3-dictate prune <seconds>         remove kept recordings older than that (0: all)
```

SIGINT (or SIGUSR1) stops and transcribes; SIGTERM cancels with exit 130. It
dies with its parent (`PR_SET_PDEATHSIG`). Errors are one code on stderr:
`model_missing`, `model_invalid`, `recorder_unavailable`, `download_failed`,
`download_corrupt`, `bad_arguments`. Recording uses
`pw-record --container raw --rate 16000 --channels 1 --format s16`, capped at
120 s. `OT3_DICTATE_MODEL_FILE`, `OT3_DICTATE_THREADS` and
`OT3_DICTATE_AUDIO_CTX=full` exist for benchmarks.

## Build

`bin/build-client` builds `native/dictate` when CMake and a C++ compiler are
present; otherwise the button and the Preferences section say the helper is not
installed. It is a separate Cargo package so the main client never links C++.
Local builds use `-mcpu=native`. Release packages (`tools/package-binaries`) set
`OT3_PORTABLE=1`: `GGML_NATIVE=OFF` with `armv8.2-a+dotprod+fp16` on aarch64
(Apple M1 and later, Snapdragon X, Raspberry Pi 5) or AVX2/FMA/F16C on x86_64.
The installer installs the helper when the package has it.

## Tests

- `cargo test --release --manifest-path native/dictate/Cargo.toml`: silence and
  click rejection, trimming, a short phrase after a very long pause, cutting
  long clips in pauses and joining the pieces, kept recordings' folder names and
  WAV framing, invented captions, JSON escaping.
- `cargo test --release ui_state`: persisted preferences reject unknown models,
  argument-like languages and multi-line vocabulary.
- `tools/test-qml dictation`: a stand-in helper; insertion at the cursor with
  spacing, prompt vocabulary, level wave, cancel, first-use model download, the
  kept-recordings list and the visibility preference.
