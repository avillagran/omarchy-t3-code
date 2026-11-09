//! Native local speech/model control and transcript postprocessing HTTP.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalModelAction {
    Download,
    Select,
    CancelDownload,
    Remove,
}
impl LocalModelAction {
    fn path(self) -> &'static str {
        match self {
            Self::Download => "/api/voice/models/download",
            Self::Select => "/api/voice/models/select",
            Self::CancelDownload => "/api/voice/models/cancel",
            Self::Remove => "/api/voice/models/remove",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Acceleration(String);
impl Acceleration {
    pub fn parse(value: &str) -> Result<Self> {
        let gpu = value
            .strip_prefix("gpu:")
            .and_then(|s| s.chars().next())
            .is_some_and(|c| !matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}'));
        if value == "auto" || value == "cpu" || (len(value) <= 500 && gpu) {
            Ok(Self(value.to_owned()))
        } else {
            Err("invalid_speech_acceleration".into())
        }
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnloadTimeout {
    Never,
    Immediately,
    Minutes2,
    Minutes5,
    Minutes10,
    Minutes15,
    Hour1,
}
impl UnloadTimeout {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "never" => Ok(Self::Never),
            "immediately" => Ok(Self::Immediately),
            "min_2" => Ok(Self::Minutes2),
            "min_5" => Ok(Self::Minutes5),
            "min_10" => Ok(Self::Minutes10),
            "min_15" => Ok(Self::Minutes15),
            "hour_1" => Ok(Self::Hour1),
            _ => Err("invalid_speech_unload_timeout".into()),
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Never => "never",
            Self::Immediately => "immediately",
            Self::Minutes2 => "min_2",
            Self::Minutes5 => "min_5",
            Self::Minutes10 => "min_10",
            Self::Minutes15 => "min_15",
            Self::Hour1 => "hour_1",
        }
    }
}

fn model_payload(model: &str) -> Result<Value> {
    if !(1..=160).contains(&len(model)) {
        return Err("invalid_speech_model".into());
    }
    Ok(json!({"modelId":model}))
}
fn bounded_text(value: &Value, max: usize) -> Result<&str> {
    value
        .as_str()
        .filter(|s| len(s) <= max)
        .ok_or_else(|| "invalid_speech_postprocess".into())
}
fn selection_number(value: &Value) -> Result<Value> {
    let number = value
        .as_f64()
        .filter(|v| v.is_finite())
        .ok_or("invalid_speech_postprocess")?;
    // JSON input numbers have JavaScript double semantics, including signed zero
    // and integers beyond 2^53. Do not add integer/range constraints absent upstream.
    serde_json::from_str(&crate::js::number(number))
        .map_err(|_| "invalid_speech_postprocess".into())
}
#[derive(Clone, PartialEq)]
pub struct PostProcessRequest(Value);
impl std::fmt::Debug for PostProcessRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PostProcessRequest").finish_non_exhaustive()
    }
}
impl PostProcessRequest {
    pub fn decode(input: &Value) -> Result<Self> {
        let options = &input["options"];
        let mut recognition = json!({"speechLanguage":"auto","speechTranslateToEnglish":false,"speechRemoveFillerWords":true,"speechCustomWords":options["speechCustomWords"]});
        if let Some(cue) = options.get("speechCorrectionWord") {
            recognition["speechCorrectionWord"] = cue.clone();
        }
        let parsed = TranscriptionOptions::decode(&recognition)?;
        let mut result_options = json!({"speechCustomWords":parsed.words.iter().map(|(term,aliases)|json!({"term":term,"aliases":aliases})).collect::<Vec<_>>()});
        let prompt = &options["speechPostProcessingPrompt"];
        let mode = prompt["mode"]
            .as_str()
            .filter(|v| matches!(*v, "default" | "custom"))
            .ok_or("invalid_speech_postprocess")?;
        result_options["speechPostProcessingPrompt"] = json!({"mode":mode,"customInstructions":bounded_text(&prompt["customInstructions"],10_000)?});
        if let Some(cue) = parsed.correction {
            result_options["speechCorrectionWord"] = json!(cue);
        }
        Ok(Self(
            json!({"options":result_options,"transcript":bounded_text(&input["transcript"],100_000)?,"draft":{"text":bounded_text(&input["draft"]["text"],100_000)?,"selection":{"start":selection_number(&input["draft"]["selection"]["start"])?,"end":selection_number(&input["draft"]["selection"]["end"])?}}}),
        ))
    }
    pub fn as_json(&self) -> &Value {
        &self.0
    }
}

/// Existing cloud/PCM error types remain unchanged. These new control calls
/// preserve native invalid-request reasons and trace identity separately.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ControlFailure {
    Transport(String),
    InvalidRequest { reason: String, trace_id: String },
}
impl From<String> for ControlFailure {
    fn from(v: String) -> Self {
        Self::Transport(v)
    }
}
impl From<&str> for ControlFailure {
    fn from(v: &str) -> Self {
        Self::Transport(v.into())
    }
}
pub type ControlResult<T> = std::result::Result<T, ControlFailure>;
fn invalid_request(mut response: reqwest::blocking::Response) -> ControlFailure {
    use std::io::Read;
    if crate::media_type(&response) != "application/json" {
        return "http_400".into();
    }
    if response
        .headers()
        .get("content-encoding")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| !v.is_empty() && !v.eq_ignore_ascii_case("identity"))
    {
        return "unsupported_encoding".into();
    }
    let mut bytes = zeroize::Zeroizing::new(Vec::new());
    if response
        .by_ref()
        .take(65537)
        .read_to_end(&mut bytes)
        .is_err()
    {
        return "unreachable".into();
    }
    if bytes.len() > 65536 {
        return "too_large".into();
    }
    invalid_request_payload(&bytes)
}
pub(super) fn invalid_request_payload(bytes: &[u8]) -> ControlFailure {
    let Ok(v) = serde_json::from_slice::<Value>(bytes) else {
        return "malformed".into();
    };
    let Some(reason) = v["reason"].as_str().filter(|r| {
        matches!(
            *r,
            "invalid_scope"
                | "scope_not_granted"
                | "invalid_command"
                | "invalid_history_cursor"
                | "thread_transcript_too_large"
                | "invalid_audio"
                | "speech_unavailable"
                | "speech_busy"
        )
    }) else {
        return "http_400".into();
    };
    let Some(trace) = v["traceId"].as_str().map(trim).filter(|s| !s.is_empty()) else {
        return "http_400".into();
    };
    if v["_tag"] != "EnvironmentRequestInvalidError" || v["code"] != "invalid_request" {
        return "http_400".into();
    }
    ControlFailure::InvalidRequest {
        reason: reason.to_owned(),
        trace_id: trace.to_owned(),
    }
}
fn response_json(response: reqwest::blocking::Response) -> ControlResult<Value> {
    if response.status().as_u16() == 400 {
        return Err(invalid_request(response));
    }
    crate::decode_with_limit(response, 8 * 1024 * 1024).map_err(Into::into)
}
impl Connection {
    fn speech_control(&self, path: &str, body: &Value) -> ControlResult<Value> {
        response_json(self.speech_request(
            "POST",
            path,
            Some(body.to_string().into_bytes()),
            false,
        )?)
    }
    /// Forward-compatible native status, including supported:false platforms.
    pub fn speech_status(&self) -> ControlResult<Value> {
        response_json(self.speech_request("GET", "/api/voice/status", None, false)?)
    }
    pub fn speech_models(&self, benchmarks: bool) -> ControlResult<Value> {
        response_json(self.speech_request(
            "GET",
            if benchmarks {
                "/api/voice/models?benchmarks=1"
            } else {
                "/api/voice/models"
            },
            None,
            false,
        )?)
    }
    /// Preparation is an explicit operation, never a side effect of reading status.
    pub fn speech_prepare_model(&self) -> ControlResult<()> {
        let response = self.speech_request("POST", "/api/voice/models/prepare", None, false)?;
        if response.status().as_u16() == 400 {
            return Err(invalid_request(response));
        }
        check_status(&response).map_err(Into::into)
    }
    pub fn speech_model_action(
        &self,
        action: LocalModelAction,
        model_id: &str,
    ) -> ControlResult<Value> {
        self.speech_control(action.path(), &model_payload(model_id)?)
    }
    pub fn speech_set_acceleration(&self, acceleration: &Acceleration) -> ControlResult<Value> {
        self.speech_control(
            "/api/voice/acceleration",
            &json!({"acceleration":acceleration.as_str()}),
        )
    }
    pub fn speech_set_unload_timeout(&self, timeout: UnloadTimeout) -> ControlResult<Value> {
        self.speech_control(
            "/api/voice/model-unload-timeout",
            &json!({"timeout":timeout.as_str()}),
        )
    }
    /// Requires separate host consent to send the transcript for cleanup.
    /// Audio and microphone state never enter this request.
    pub fn speech_postprocess(&self, request: &PostProcessRequest) -> ControlResult<String> {
        let v = self.speech_control("/api/voice/post-process", request.as_json())?;
        Ok(v["text"].as_str().ok_or("malformed")?.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn whole_native_client_helpers_determine_per_operation_deadlines() {
        assert!(std::env::var_os("ANDROID_SANDBOX").is_some());
        let fixture: Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/speech-deadlines-4314.json"
        ))
        .unwrap();
        assert_eq!(fixture["cases"].as_array().unwrap().len(), 16);
        for case in fixture["cases"].as_array().unwrap() {
            assert_eq!(
                super::super::request_timeout(case["path"].as_str().unwrap()).as_millis(),
                case["timeoutMs"].as_u64().unwrap() as u128,
                "{case}"
            );
            assert_eq!(case["redirect"], "error");
        }
    }
    #[test]
    fn pinned_native_controls_schemas_match_boundaries_and_json_double_numbers() {
        assert!(std::env::var_os("ANDROID_SANDBOX").is_some());
        let fixture: Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/speech-controls-4314.json"
        ))
        .unwrap();
        assert_eq!(
            fixture["revision"],
            "83a3674ffd84ffb33de768210c806cb1ac53fd80"
        );
        for case in fixture["cases"].as_array().unwrap() {
            let input = case
                .get("inputJson")
                .and_then(Value::as_str)
                .map(|raw| serde_json::from_str::<Value>(raw).unwrap())
                .unwrap_or_else(|| case["input"].clone());
            let result = match case["family"].as_str().unwrap() {
                "model" => input["modelId"]
                    .as_str()
                    .ok_or_else(|| String::from("invalid"))
                    .and_then(model_payload),
                "acceleration" => input["acceleration"]
                    .as_str()
                    .ok_or_else(|| String::from("invalid"))
                    .and_then(Acceleration::parse)
                    .map(|v| json!({"acceleration":v.as_str()})),
                "timeout" => input["timeout"]
                    .as_str()
                    .ok_or_else(|| String::from("invalid"))
                    .and_then(UnloadTimeout::parse)
                    .map(|v| json!({"timeout":v.as_str()})),
                "postprocess" => PostProcessRequest::decode(&input).map(|v| v.as_json().clone()),
                _ => panic!("Unknown native case family"),
            };
            assert_eq!(result.is_ok(), case["valid"].as_bool().unwrap(), "{case}");
            if let Ok(encoded) = result {
                assert_eq!(encoded, case["encoded"], "{case}");
            }
        }
    }
}
