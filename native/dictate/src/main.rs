//! Local voice dictation for the composer.
//!
//!   ot3-dictate models                 installed and available models (JSON)
//!   ot3-dictate download <model>       fetch a model, printing progress lines
//!   ot3-dictate remove <model>         delete a downloaded model
//!   ot3-dictate listen [options]       record until SIGINT, then print the text
//!   ot3-dictate transcribe <file>      transcribe 16 kHz mono s16le raw or WAV
//!
//! listen options: --model <id>, --language <code|auto>, --prompt <text>,
//! --max-seconds <n>. It lives only while one dictation runs: PipeWire records
//! from the start, the model loads alongside, and the process exits after
//! printing one result. SIGINT stops and transcribes; SIGTERM cancels.
//! Every stdout line is one JSON object; errors are one short code on stderr.
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicBool, AtomicI32, Ordering},
    thread,
    time::{Duration, Instant},
};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

const RATE: usize = 16_000;

struct Model {
    id: &'static str,
    file: &'static str,
    size: u64,
    sha256: &'static str,
}
// Multilingual whisper.cpp models from ggerganov/whisper.cpp; sizes and hashes
// are the Hugging Face LFS objects. q8_0 runs about three times faster than
// q5_1 on ARM NEON (small: ~1.2 s for a 10 s clip on an M2 Pro) for the same text.
const MODELS: &[Model] = &[
    Model { id: "base", file: "ggml-base-q8_0.bin", size: 81_768_585, sha256: "c577b9a86e7e048a0b7eada054f4dd79a56bbfa911fbdacf900ac5b567cbb7d9" },
    Model { id: "small", file: "ggml-small-q8_0.bin", size: 264_464_607, sha256: "49c8fb02b65e6049d5fa6c04f81f53b867b5ec9540406812c643f177317f779f" },
];
const DEFAULT_MODEL: &str = "small";
const SOURCE: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/";

static STOP: AtomicBool = AtomicBool::new(false);
static CANCEL: AtomicBool = AtomicBool::new(false);
static RECORDER: AtomicI32 = AtomicI32::new(0);

extern "C" fn on_signal(sig: libc::c_int) {
    if sig == libc::SIGINT || sig == libc::SIGUSR1 {
        STOP.store(true, Ordering::SeqCst);
    } else {
        CANCEL.store(true, Ordering::SeqCst);
    }
    // Closing the recorder ends the read loop at once.
    let pid = RECORDER.load(Ordering::SeqCst);
    if pid > 0 {
        unsafe { libc::kill(pid, libc::SIGTERM) };
    }
}

fn fail(code: &str) -> ! {
    eprintln!("{code}");
    std::process::exit(1)
}
fn emit(line: String) {
    let mut out = io::stdout().lock();
    let _ = writeln!(out, "{line}");
    let _ = out.flush();
}
fn quote(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn models_dir() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/share"));
    base.join("omarchy-t3-code/dictation")
}
fn model(id: &str) -> &'static Model {
    MODELS.iter().find(|m| m.id == id).unwrap_or_else(|| fail("unknown_model"))
}
fn installed(m: &Model) -> bool {
    fs::metadata(models_dir().join(m.file)).is_ok_and(|md| md.len() == m.size)
}

fn list() {
    let items: Vec<String> = MODELS
        .iter()
        .map(|m| format!("{{\"id\":{},\"sizeBytes\":{},\"installed\":{}}}", quote(m.id), m.size, installed(m)))
        .collect();
    let recorder = Command::new("pw-record").arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s| s.success());
    emit(format!("{{\"models\":[{}],\"default\":{},\"recorder\":{}}}", items.join(","), quote(DEFAULT_MODEL), recorder));
}

fn sha256(path: &Path) -> io::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hash = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
    }
    Ok(hash.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

fn download(m: &Model) {
    let dir = models_dir();
    fs::create_dir_all(&dir).unwrap_or_else(|_| fail("local_error"));
    let target = dir.join(m.file);
    if installed(m) {
        emit(format!("{{\"type\":\"done\",\"model\":{}}}", quote(m.id)));
        return;
    }
    let part = dir.join(format!("{}.part", m.file));
    let mut curl = Command::new("curl")
        .args(["--fail", "--silent", "--location", "--retry", "2", "--continue-at", "-", "--output"])
        .arg(&part)
        .arg(format!("{SOURCE}{}", m.file))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap_or_else(|_| fail("download_unavailable"));
    RECORDER.store(curl.id() as i32, Ordering::SeqCst);
    let status = loop {
        if let Some(status) = curl.try_wait().unwrap_or(None) {
            break status;
        }
        let bytes = fs::metadata(&part).map(|md| md.len()).unwrap_or(0);
        emit(format!("{{\"type\":\"progress\",\"bytes\":{bytes},\"total\":{}}}", m.size));
        thread::sleep(Duration::from_millis(250));
    };
    if CANCEL.load(Ordering::SeqCst) {
        std::process::exit(130);
    }
    if !status.success() {
        fail("download_failed");
    }
    if fs::metadata(&part).map(|md| md.len()).unwrap_or(0) != m.size || sha256(&part).ok().as_deref() != Some(m.sha256) {
        let _ = fs::remove_file(&part);
        fail("download_corrupt");
    }
    fs::rename(&part, &target).unwrap_or_else(|_| fail("local_error"));
    emit(format!("{{\"type\":\"done\",\"model\":{}}}", quote(m.id)));
}

fn load(m: &Model) -> WhisperContext {
    // OT3_DICTATE_MODEL_FILE swaps in another ggml file, for benchmarks.
    let path = match std::env::var_os("OT3_DICTATE_MODEL_FILE") {
        Some(file) => PathBuf::from(file),
        None if installed(m) => models_dir().join(m.file),
        None => fail("model_missing"),
    };
    WhisperContext::new_with_params(path.to_str().unwrap_or_else(|| fail("local_error")), WhisperContextParameters::default())
        .unwrap_or_else(|_| fail("model_invalid"))
}

fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
}
fn level(samples: &[f32]) -> f32 {
    let db = 20.0 * rms(samples).max(1e-6).log10();
    ((db + 60.0) / 60.0).clamp(0.0, 1.0)
}

/// Cuts leading and trailing silence; None when nothing in the clip rises above
/// the room's noise, so Whisper never sees a silent clip (it invents captions).
fn speech(samples: &[f32]) -> Option<&[f32]> {
    let frame = RATE * 30 / 1000;
    let energies: Vec<f32> = samples.chunks(frame).map(rms).collect();
    if energies.is_empty() {
        return None;
    }
    let mut sorted = energies.clone();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let floor = sorted[sorted.len() / 5];
    let threshold = (floor * 3.0).max(0.01);
    let first = energies.iter().position(|e| *e > threshold)?;
    let last = energies.iter().rposition(|e| *e > threshold)?;
    // A lone click is not speech: ask for about 120 ms above the threshold.
    if energies[first..=last].iter().filter(|e| **e > threshold).count() < 4 {
        return None;
    }
    let pad = RATE / 4;
    let start = (first * frame).saturating_sub(pad);
    let end = ((last + 1) * frame + pad).min(samples.len());
    Some(&samples[start..end])
}

// Captions Whisper is known to invent from noise or silence.
const INVENTED: &[&str] = &[
    "subtítulos realizados por la comunidad de amara.org",
    "subtítulos por la comunidad de amara.org",
    "gracias por ver el video",
    "gracias por ver",
    "¡suscríbete!",
    "suscríbete al canal",
    "thank you for watching",
    "thanks for watching",
    "please subscribe",
    "you",
];
fn clean(text: &str) -> String {
    let mut out = String::new();
    let mut depth = 0;
    for c in text.chars() {
        match c {
            '[' | '(' => depth += 1,
            ']' | ')' if depth > 0 => depth -= 1,
            c if depth == 0 => out.push(c),
            _ => {}
        }
    }
    let out = out.split_whitespace().collect::<Vec<_>>().join(" ");
    let bare = out.trim_matches(|c: char| c.is_ascii_punctuation() || c == ' ').to_lowercase();
    if INVENTED.iter().any(|p| bare == p.trim_end_matches(['.', '!'])) {
        return String::new();
    }
    out
}

struct Options {
    model: &'static Model,
    language: String,
    prompt: String,
    max_seconds: u64,
}
fn options(args: &[String]) -> Options {
    let mut o = Options { model: model(DEFAULT_MODEL), language: "auto".into(), prompt: String::new(), max_seconds: 120 };
    let mut i = 0;
    while i < args.len() {
        let value = args.get(i + 1).cloned().unwrap_or_default();
        match args[i].as_str() {
            "--model" => o.model = model(&value),
            "--language" => o.language = value,
            "--prompt" => o.prompt = value.chars().take(600).collect(),
            "--max-seconds" => o.max_seconds = value.parse().unwrap_or(120).clamp(5, 600),
            _ => fail("bad_arguments"),
        }
        i += 2;
    }
    if !(o.language == "auto" || (o.language.len() <= 3 && o.language.chars().all(|c| c.is_ascii_lowercase()))) {
        fail("bad_language");
    }
    o
}

fn transcribe(ctx: &WhisperContext, samples: &[f32], o: &Options) -> (String, String) {
    let Some(voice) = speech(samples) else {
        return (String::new(), String::new());
    };
    // whisper.cpp ignores clips under a second.
    let mut audio = voice.to_vec();
    if audio.len() < RATE + RATE / 10 {
        audio.resize(RATE + RATE / 10, 0.0);
    }
    let mut state = ctx.create_state().unwrap_or_else(|_| fail("model_invalid"));
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    let threads = std::env::var("OT3_DICTATE_THREADS").ok().and_then(|v| v.parse().ok()).unwrap_or_else(|| thread::available_parallelism().map(|n| n.get()).unwrap_or(4).clamp(1, 8));
    params.set_n_threads(threads as i32);
    params.set_language(Some(if o.language == "auto" { "auto" } else { &o.language }));
    params.set_translate(false);
    params.set_no_context(true);
    params.set_no_timestamps(true);
    params.set_suppress_blank(true);
    params.set_suppress_nst(true);
    params.set_temperature(0.0);
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    if !o.prompt.is_empty() {
        params.set_initial_prompt(&o.prompt);
    }
    // The encoder costs the same for every clip unless told how long it is:
    // one audio context step is 20 ms, and 1500 covers Whisper's 30 s window.
    let ctx_steps = match std::env::var("OT3_DICTATE_AUDIO_CTX").ok().as_deref() {
        Some("full") => 0,
        _ => (audio.len() / 320 + 64).min(1500) as i32,
    };
    if ctx_steps > 0 && ctx_steps < 1500 {
        params.set_audio_ctx(ctx_steps);
    }
    state.full(params, &audio).unwrap_or_else(|_| fail("transcription_failed"));
    let mut text = String::new();
    for segment in state.as_iter() {
        if segment.no_speech_probability() > 0.8 {
            continue;
        }
        if let Ok(s) = segment.to_str_lossy() {
            text.push_str(&s);
        }
    }
    let language = whisper_rs::get_lang_str(state.full_lang_id_from_state()).unwrap_or("").to_string();
    (clean(&text), language)
}

fn result(text: &str, language: &str, seconds: f32, started: Instant) {
    emit(format!(
        "{{\"type\":\"text\",\"text\":{},\"language\":{},\"seconds\":{:.1},\"elapsedMs\":{}}}",
        quote(text),
        quote(language),
        seconds,
        started.elapsed().as_millis()
    ));
}

fn record(max_seconds: u64) -> (Child, Vec<f32>) {
    let mut child = Command::new("pw-record")
        .args(["--container", "raw", "--rate", "16000", "--channels", "1", "--format", "s16", "--media-role", "Communication", "-P", "{ node.name = ot3-dictate, media.name = \"T3 dictation\" }", "-"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap_or_else(|_| fail("recorder_unavailable"));
    RECORDER.store(child.id() as i32, Ordering::SeqCst);
    emit("{\"type\":\"recording\"}".into());
    let mut stdout = child.stdout.take().unwrap();
    let mut samples: Vec<f32> = Vec::with_capacity(RATE * 30);
    let mut buf = [0u8; 2560]; // 80 ms
    let mut carry: Option<u8> = None;
    let mut last_level = Instant::now();
    let limit = max_seconds as usize * RATE;
    while !STOP.load(Ordering::SeqCst) && !CANCEL.load(Ordering::SeqCst) && samples.len() < limit {
        let n = match stdout.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        let mut bytes = &buf[..n];
        let start = samples.len();
        if let Some(lo) = carry.take() {
            samples.push(i16::from_le_bytes([lo, bytes[0]]) as f32 / 32768.0);
            bytes = &bytes[1..];
        }
        let mut pairs = bytes.chunks_exact(2);
        samples.extend(pairs.by_ref().map(|p| i16::from_le_bytes([p[0], p[1]]) as f32 / 32768.0));
        carry = pairs.remainder().first().copied();
        if last_level.elapsed() >= Duration::from_millis(70) {
            emit(format!("{{\"type\":\"level\",\"value\":{:.3}}}", level(&samples[start..])));
            last_level = Instant::now();
        }
    }
    RECORDER.store(0, Ordering::SeqCst);
    let _ = child.kill();
    let _ = child.wait();
    (child, samples)
}

fn listen(o: Options) {
    if !installed(o.model) {
        fail("model_missing");
    }
    let file = o.model;
    let loader = thread::spawn(move || load(file));
    let (_, samples) = record(o.max_seconds);
    if CANCEL.load(Ordering::SeqCst) {
        std::process::exit(130);
    }
    let started = Instant::now();
    emit("{\"type\":\"transcribing\"}".into());
    let ctx = loader.join().unwrap_or_else(|_| fail("model_invalid"));
    if CANCEL.load(Ordering::SeqCst) {
        std::process::exit(130);
    }
    let (text, language) = transcribe(&ctx, &samples, &o);
    result(&text, &language, samples.len() as f32 / RATE as f32, started);
}

fn read_audio(path: &str) -> Vec<f32> {
    let bytes = fs::read(path).unwrap_or_else(|_| fail("local_error"));
    let data = if bytes.starts_with(b"RIFF") && bytes.len() > 44 {
        // Find the "data" chunk of a 16 kHz mono s16 WAV.
        let mut at = 12;
        let mut found = None;
        while at + 8 <= bytes.len() {
            let size = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().unwrap()) as usize;
            if &bytes[at..at + 4] == b"data" {
                found = Some(&bytes[at + 8..(at + 8 + size).min(bytes.len())]);
                break;
            }
            at += 8 + size + (size & 1);
        }
        found.unwrap_or_else(|| fail("bad_audio"))
    } else {
        &bytes[..]
    };
    data.chunks_exact(2).map(|p| i16::from_le_bytes([p[0], p[1]]) as f32 / 32768.0).collect()
}

fn main() {
    whisper_rs::install_logging_hooks();
    unsafe {
        // Never outlive the panel that started us.
        libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
        for sig in [libc::SIGINT, libc::SIGUSR1, libc::SIGTERM, libc::SIGHUP] {
            libc::signal(sig, on_signal as *const () as libc::sighandler_t);
        }
    }
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("models") => list(),
        Some("download") if args.len() == 2 => download(model(&args[1])),
        Some("remove") if args.len() == 2 => {
            let m = model(&args[1]);
            let _ = fs::remove_file(models_dir().join(format!("{}.part", m.file)));
            match fs::remove_file(models_dir().join(m.file)) {
                Ok(()) => emit(format!("{{\"type\":\"removed\",\"model\":{}}}", quote(m.id))),
                Err(e) if e.kind() == io::ErrorKind::NotFound => emit(format!("{{\"type\":\"removed\",\"model\":{}}}", quote(m.id))),
                Err(_) => fail("local_error"),
            }
        }
        Some("listen") => listen(options(&args[1..])),
        Some("transcribe") if args.len() >= 2 => {
            let o = options(&args[2..]);
            let samples = read_audio(&args[1]);
            let started = Instant::now();
            let ctx = load(o.model);
            let loaded = started.elapsed().as_millis();
            let (text, language) = transcribe(&ctx, &samples, &o);
            eprintln!("load {loaded} ms");
            result(&text, &language, samples.len() as f32 / RATE as f32, started);
        }
        _ => fail("bad_arguments"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tone(seconds: f32, amplitude: f32) -> Vec<f32> {
        (0..(seconds * RATE as f32) as usize).map(|i| amplitude * (i as f32 * 0.3).sin()).collect()
    }
    #[test]
    fn silence_and_clicks_are_not_speech() {
        assert!(speech(&vec![0.0; RATE * 2]).is_none());
        let mut click = vec![0.0; RATE * 2];
        click[RATE..RATE + 200].iter_mut().for_each(|s| *s = 0.8);
        assert!(speech(&click).is_none());
    }
    #[test]
    fn speech_is_trimmed_with_padding() {
        let mut clip = vec![0.0; RATE * 2];
        clip.extend(tone(1.0, 0.3));
        clip.extend(vec![0.0; RATE * 2]);
        let voice = speech(&clip).unwrap();
        assert!(voice.len() >= RATE && voice.len() < RATE * 2, "{}", voice.len());
    }
    #[test]
    fn invented_captions_and_annotations_are_dropped() {
        assert_eq!(clean(" Subtítulos realizados por la comunidad de Amara.org"), "");
        assert_eq!(clean("Thank you for watching!"), "");
        assert_eq!(clean("[Música] Abre el   archivo (risas) main.rs"), "Abre el archivo main.rs");
        assert_eq!(clean("Gracias por ver el video que te mandé"), "Gracias por ver el video que te mandé");
    }
    #[test]
    fn output_is_valid_json() {
        assert_eq!(quote("a \"b\"\n\\ \u{1}"), "\"a \\\"b\\\"\\n\\\\ \\u0001\"");
    }
}
