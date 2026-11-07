//! Buffered speech transport. Recording and consent belong to the host; nothing
//! records, downloads a model, or uploads audio until explicitly requested.
use crate::{check_status, decode_with_limit, endpoint, Connection, Result};
use serde_json::{json, Value};
use std::{collections::HashSet, time::Duration};
use unicode_normalization::UnicodeNormalization;

mod controls;
mod live;
mod request;
pub use controls::{
    Acceleration, ControlFailure, ControlResult, LocalModelAction, PostProcessRequest,
    UnloadTimeout,
};
pub use live::{
    AsyncSpeechStream, LiveEvent, LiveFailure, LiveText, SpeechCancellation, SpeechStream,
    SpeechStreamOpening,
};
pub use request::{CancellableSpeech, RequestCancellation, RequestFailure, RequestResult};

pub const SAMPLE_RATE: usize = 16_000;
pub const MAX_PCM_BYTES: usize = SAMPLE_RATE * 4 * 5 * 60;
pub const MAX_OPTIONS_BYTES: usize = 128_000;
fn request_timeout(path: &str) -> Duration {
    let path = path.split('?').next().unwrap_or(path);
    if path.starts_with("/api/voice/cloud/") && path.ends_with("/transcribe") {
        return Duration::from_secs(130);
    }
    let inference = matches!(
        path,
        "/api/voice/models/download" | "/api/voice/transcribe" | "/api/voice/post-process"
    );
    Duration::from_secs(if inference { 30 * 60 } else { 30 })
}
fn len(s: &str) -> usize {
    s.encode_utf16().count()
}
fn trim(s: &str) -> &str {
    s.trim_matches(crate::js::is_space)
}
fn string(v: &Value, min: usize, max: usize) -> Result<&str> {
    v.as_str()
        .filter(|s| (min..=max).contains(&len(s)))
        .ok_or_else(|| "invalid_speech_options".into())
}
fn boolean(v: &Value) -> Result<bool> {
    v.as_bool().ok_or_else(|| "invalid_speech_options".into())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloudProvider {
    OpenAi,
    Xai,
    OpenRouter,
}
impl CloudProvider {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::Xai => "xai",
            Self::OpenRouter => "openrouter",
        }
    }
}
/// Owns validated, schema-shaped options. Unknown input fields are discarded.
#[derive(Clone, Debug, PartialEq)]
pub struct TranscriptionOptions {
    language: String,
    translate: bool,
    words: Vec<(String, Vec<String>)>,
    remove_filler: bool,
    correction: Option<String>,
    silence_filtering: bool,
}
impl Default for TranscriptionOptions {
    fn default() -> Self {
        Self {
            language: "auto".into(),
            translate: false,
            words: vec![],
            remove_filler: true,
            correction: None,
            silence_filtering: true,
        }
    }
}
impl TranscriptionOptions {
    pub fn decode(v: &Value) -> Result<Self> {
        let language = string(&v["speechLanguage"], 2, 16)?.to_owned();
        let translate = boolean(&v["speechTranslateToEnglish"])?;
        let remove_filler = boolean(&v["speechRemoveFillerWords"])?;
        let silence_filtering = match v.get("speechSilenceFiltering") {
            None => true,
            Some(v) => boolean(v)?,
        };
        let correction = v
            .get("speechCorrectionWord")
            .map(|v| string(v, 0, 50).map(str::to_owned))
            .transpose()?;
        let entries = v["speechCustomWords"]
            .as_array()
            .filter(|a| a.len() <= 100)
            .ok_or("invalid_speech_options")?;
        let mut seen = HashSet::new();
        let mut words = Vec::new();
        for entry in entries {
            let term = string(&entry["term"], 1, 50)?.to_owned();
            let aliases = entry["aliases"]
                .as_array()
                .filter(|a| a.len() <= 8)
                .ok_or("invalid_speech_options")?
                .iter()
                .map(|v| string(v, 1, 50).map(str::to_owned))
                .collect::<Result<Vec<_>>>()?;
            for value in std::iter::once(&term).chain(aliases.iter()) {
                let key = trim(value).nfc().collect::<String>().to_lowercase();
                if key.is_empty() || !seen.insert(key) {
                    return Err("invalid_speech_options".into());
                }
            }
            words.push((term, aliases));
        }
        Ok(Self {
            language,
            translate,
            words,
            remove_filler,
            correction,
            silence_filtering,
        })
    }
    /// Native Schema encoding preserves declared field order and includes the
    /// decoding-default silence flag. Vocabulary spelling is never normalized.
    pub fn encode_json(&self) -> String {
        let words = self
            .words
            .iter()
            .map(|(term, aliases)| {
                format!(
                    "{{\"term\":{},\"aliases\":{}}}",
                    json!(term),
                    json!(aliases)
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let correction = self
            .correction
            .as_ref()
            .map(|s| format!(",\"speechCorrectionWord\":{}", json!(s)))
            .unwrap_or_default();
        format!("{{\"speechLanguage\":{},\"speechTranslateToEnglish\":{},\"speechCustomWords\":[{}],\"speechRemoveFillerWords\":{}{},\"speechSilenceFiltering\":{}}}",json!(self.language),self.translate,words,self.remove_filler,correction,self.silence_filtering)
    }
}
/// Validates the exact mono16kHz Float32LE payload; rejects empty, oversized,
/// incomplete, nonfinite and out-of-range samples. Signed zero is retained.
pub fn validate_pcm(pcm: &[u8]) -> Result<()> {
    if pcm.is_empty() || pcm.len() > MAX_PCM_BYTES || !pcm.len().is_multiple_of(4) {
        return Err("invalid_speech_audio".into());
    }
    for bytes in pcm.as_chunks::<4>().0 {
        let sample = f32::from_le_bytes(*bytes);
        if !sample.is_finite() || !(-1.0..=1.0).contains(&sample) {
            return Err("invalid_speech_audio".into());
        }
    }
    Ok(())
}
pub fn encode_pcm_request(pcm: &[u8], options: &TranscriptionOptions) -> Result<Vec<u8>> {
    validate_pcm(pcm)?;
    let metadata = options.encode_json();
    if metadata.len() > MAX_OPTIONS_BYTES {
        return Err("speech_options_too_large".into());
    }
    let mut out = Vec::with_capacity(4 + metadata.len() + pcm.len());
    out.extend_from_slice(&(metadata.len() as u32).to_le_bytes());
    out.extend_from_slice(metadata.as_bytes());
    out.extend_from_slice(pcm);
    Ok(out)
}
pub fn decode_pcm_request(body: &[u8]) -> Result<(TranscriptionOptions, &[u8])> {
    if body.len() < 4 {
        return Err("missing_speech_options".into());
    }
    let n = u32::from_le_bytes(body[..4].try_into().unwrap()) as usize;
    if n > MAX_OPTIONS_BYTES || 4 + n >= body.len() {
        return Err("invalid_speech_options_length".into());
    }
    let text = std::str::from_utf8(&body[4..4 + n])
        .map_err(|_| "invalid_speech_options")?
        .strip_prefix('\u{feff}')
        .unwrap_or(std::str::from_utf8(&body[4..4 + n]).unwrap());
    let value = serde_json::from_str(text).map_err(|_| "invalid_speech_options")?;
    Ok((TranscriptionOptions::decode(&value)?, &body[4 + n..]))
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CloudKeys {
    pub openai: bool,
    pub xai: bool,
    pub openrouter: Option<bool>,
}
fn decode_cloud_keys(value: &Value) -> SpeechResult<CloudKeys> {
    Ok(CloudKeys {
        openai: value["openai"].as_bool().ok_or("malformed")?,
        xai: value["xai"].as_bool().ok_or("malformed")?,
        openrouter: value
            .get("openrouter")
            .map(|value| value.as_bool().ok_or_else(|| String::from("malformed")))
            .transpose()?,
    })
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CloudModel {
    pub id: String,
    pub name: String,
}
fn model_id(id: &str) -> bool {
    if !(3..=160).contains(&len(id)) || id.to_ascii_lowercase().starts_with("openrouter/") {
        return false;
    }
    let Some((author, model)) = id.split_once('/') else {
        return false;
    };
    let first = |s: &str| s.as_bytes().first().is_some_and(u8::is_ascii_alphanumeric);
    first(author)
        && first(model)
        && author
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
        && model
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-')
}
/// Provider failures preserve their native tagged reason without retaining keys.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpeechFailure {
    Transport(String),
    Cloud {
        provider: CloudProvider,
        reason: String,
    },
}
impl From<String> for SpeechFailure {
    fn from(value: String) -> Self {
        Self::Transport(value)
    }
}
impl From<&str> for SpeechFailure {
    fn from(value: &str) -> Self {
        Self::Transport(value.into())
    }
}
pub type SpeechResult<T> = std::result::Result<T, SpeechFailure>;
fn speech_json(mut response: reqwest::blocking::Response) -> SpeechResult<Value> {
    use std::io::Read;
    if response.status().as_u16() != 422 {
        return decode_with_limit(response, 8 * 1024 * 1024).map_err(Into::into);
    }
    let mut bytes = Vec::new();
    response
        .by_ref()
        .take(65537)
        .read_to_end(&mut bytes)
        .map_err(|_| SpeechFailure::from("unreachable"))?;
    if bytes.len() > 65536 {
        return Err("too_large".into());
    }
    cloud_error(&bytes)
}
fn cloud_error(bytes: &[u8]) -> SpeechResult<Value> {
    let value: Value =
        serde_json::from_slice(bytes).map_err(|_| SpeechFailure::from("malformed"))?;
    let provider = match value["provider"].as_str() {
        Some("openai") => CloudProvider::OpenAi,
        Some("xai") => CloudProvider::Xai,
        Some("openrouter") => CloudProvider::OpenRouter,
        _ => return Err("http_422".into()),
    };
    let reason = value["reason"]
        .as_str()
        .filter(|v| {
            matches!(
                *v,
                "missing-key"
                    | "key-changed"
                    | "unreadable-key"
                    | "rejected-key"
                    | "rate-limit"
                    | "model-unavailable"
                    | "unavailable"
            )
        })
        .ok_or_else(|| SpeechFailure::from("http_422"))?;
    if value["_tag"] != "CloudSpeechError" {
        return Err("http_422".into());
    }
    Err(SpeechFailure::Cloud {
        provider,
        reason: reason.into(),
    })
}
impl Connection {
    /// Independent no-proxy/no-redirect HTTP client, same admitted origin and
    /// credential. Native client deadlines, bounded JSON and no retry apply.
    fn speech_request(
        &self,
        method: &str,
        path: &str,
        body: Option<Vec<u8>>,
        binary: bool,
    ) -> Result<reqwest::blocking::Response> {
        let client = reqwest::blocking::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .timeout(request_timeout(path))
            .build()
            .map_err(|_| "local_error")?;
        let url = endpoint(&self.base, path)?;
        let method =
            reqwest::Method::from_bytes(method.as_bytes()).map_err(|_| "invalid_method")?;
        let mut request = client
            .request(method.clone(), url.clone())
            .header("Accept", "application/json")
            .header("Accept-Encoding", "identity");
        for (name, value) in self.credential.headers(method.as_str(), url.as_str())? {
            request = request.header(name, value);
        }
        if let Some(body) = body {
            request = request
                .header(
                    "Content-Type",
                    if binary {
                        "application/octet-stream"
                    } else {
                        "application/json"
                    },
                )
                .body(body);
        }
        request.send().map_err(|_| "unreachable".into())
    }
    pub fn speech_cloud_keys(&self) -> SpeechResult<CloudKeys> {
        let v = speech_json(self.speech_request("GET", "/api/voice/cloud/keys", None, false)?)?;
        decode_cloud_keys(&v)
    }
    pub fn speech_cloud_models(&self) -> SpeechResult<Vec<CloudModel>> {
        let v = speech_json(self.speech_request(
            "GET",
            "/api/voice/cloud/openrouter/models",
            None,
            false,
        )?)?;
        v["models"]
            .as_array()
            .ok_or("malformed")?
            .iter()
            .map(|m| {
                let id = m["id"]
                    .as_str()
                    .filter(|s| model_id(s))
                    .ok_or("malformed")?;
                Ok(CloudModel {
                    id: id.into(),
                    name: m["name"].as_str().ok_or("malformed")?.into(),
                })
            })
            .collect()
    }
    /// Sends audio only when explicitly called. Hosts must obtain recording and
    /// cloud-upload consent before calling; no retry occurs after transmission.
    pub fn speech_transcribe(
        &self,
        pcm: &[u8],
        options: &TranscriptionOptions,
        cloud: Option<(CloudProvider, Option<&str>)>,
    ) -> SpeechResult<String> {
        let body = encode_pcm_request(pcm, options)?;
        let mut path = match cloud {
            None => "/api/voice/transcribe".to_owned(),
            Some((p, _)) => format!("/api/voice/cloud/{}/transcribe", p.as_str()),
        };
        if let Some((_, Some(id))) = cloud {
            if !model_id(id) {
                return Err("invalid_speech_model".into());
            }
            let mut url = endpoint(&self.base, &path)?;
            url.query_pairs_mut().append_pair("model", id);
            path = format!("{}?{}", url.path(), url.query().unwrap());
        }
        let v = speech_json(self.speech_request("POST", &path, Some(body), true)?)?;
        Ok(v["text"].as_str().ok_or("malformed")?.into())
    }
    /// Stores a literal, a configured secret-provider reference, or clears it.
    /// This never reads back the key. The value is redacted by the server.
    pub fn speech_set_cloud_key(&self, provider: CloudProvider, key: &Value) -> Result<()> {
        let normalized = normalize_cloud_key(key)?;
        check_status(&self.speech_request(
            "POST",
            &format!("/api/voice/cloud/{}/key", provider.as_str()),
            Some(json!({"key":normalized}).to_string().into_bytes()),
            false,
        )?)
    }
}

fn normalize_cloud_key(key: &Value) -> Result<Value> {
    let normalized = match key {
        Value::Null => Value::Null,
        Value::String(s) => {
            let s = trim(s);
            if s.is_empty()
                || len(s) > 4096
                || !s.bytes().all(|b| (33..=126).contains(&b))
                || s.split_once("://").is_some_and(|(prefix, _)| {
                    let mut b = prefix.bytes();
                    b.next().is_some_and(|c| c.is_ascii_alphabetic())
                        && b.all(|c| c.is_ascii_alphanumeric() || matches!(c, b'+' | b'.' | b'-'))
                })
            {
                return Err("invalid_speech_key".into());
            }
            json!(s)
        }
        Value::Object(m) => {
            let provider = trim(
                m.get("provider")
                    .and_then(Value::as_str)
                    .ok_or("invalid_speech_key")?,
            );
            let reference = trim(
                m.get("reference")
                    .and_then(Value::as_str)
                    .ok_or("invalid_speech_key")?,
            );
            if provider.is_empty()
                || len(provider) > 64
                || !provider.as_bytes()[0].is_ascii_alphanumeric()
                || !provider
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-'))
                || reference.is_empty()
                || len(reference) > 1024
                || reference.contains(['\r', '\n'])
            {
                return Err("invalid_speech_key".into());
            }
            json!({"provider":provider,"reference":reference})
        }
        _ => return Err("invalid_speech_key".into()),
    };
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actual_native_schema_and_framing_corpus() {
        assert_eq!(unicode_normalization::UNICODE_VERSION, (17, 0, 0));
        assert_eq!(std::char::UNICODE_VERSION, (17, 0, 0));
        let fixture: Value =
            serde_json::from_str(include_str!("../tests/fixtures/speech-4314.json")).unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let value = &case["input"];
            let valid = case["valid"].as_bool().unwrap();
            match case["family"].as_str().unwrap() {
                "options" => {
                    let result = TranscriptionOptions::decode(value);
                    assert_eq!(result.is_ok(), valid, "{case}");
                    if let Ok(options) = result {
                        assert_eq!(
                            options.encode_json(),
                            case["json"].as_str().unwrap(),
                            "{case}"
                        );
                        let body = encode_pcm_request(&[0; 4], &options).unwrap();
                        let hex = body.iter().map(|b| format!("{b:02x}")).collect::<String>();
                        assert_eq!(hex, case["framing"].as_str().unwrap());
                        let (decoded, pcm) = decode_pcm_request(&body).unwrap();
                        assert_eq!(decoded, options);
                        assert_eq!(pcm, &[0; 4]);
                    }
                }
                "key" => {
                    let result = normalize_cloud_key(value);
                    assert_eq!(result.is_ok(), valid, "{case}");
                    if let Ok(v) = result {
                        assert_eq!(v, case["encoded"]);
                    }
                }
                "reference" => {
                    let result = normalize_cloud_key(value);
                    assert_eq!(result.is_ok(), valid, "{case}");
                    if let Ok(v) = result {
                        assert_eq!(v, case["encoded"]);
                    }
                }
                "model" => assert_eq!(value.as_str().is_some_and(model_id), valid, "{case}"),
                _ => panic!("Unknown oracle family"),
            }
        }
    }
    #[test]
    fn pcm_boundaries_and_framing_validation() {
        for sample in [-1f32, -0.0, 0.0, 1.0] {
            assert!(validate_pcm(&sample.to_le_bytes()).is_ok());
        }
        for sample in [
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
            1.00001,
            -1.00001,
        ] {
            assert!(validate_pcm(&sample.to_le_bytes()).is_err());
        }
        assert!(validate_pcm(&[]).is_err());
        assert!(validate_pcm(&[0; 3]).is_err());
        assert!(validate_pcm(&vec![0; MAX_PCM_BYTES]).is_ok());
        assert!(validate_pcm(&vec![0; MAX_PCM_BYTES + 4]).is_err());
        assert!(decode_pcm_request(&[0; 3]).is_err());
        assert!(decode_pcm_request(&[0; 4]).is_err());
        assert!(decode_pcm_request(&u32::MAX.to_le_bytes()).is_err());
        let mut invalid = 1u32.to_le_bytes().to_vec();
        invalid.extend_from_slice(&[255, 0, 0, 0, 0]);
        assert!(decode_pcm_request(&invalid).is_err());
    }
}
