//! Narrow metadata projection for the original recording preference callback.
//! No credentials or unrelated provider settings enter the GUI.
use crate::{json, protocol, voice_consent::ConsentOwner, Environment, Result};

pub fn project_words(
    consent: &ConsentOwner,
    environment: &Environment,
    project: Option<&str>,
) -> Result<String> {
    // The original useEnvironmentSettings hook selects against the native
    // defaults while its environment settings are unavailable. Vocabulary is
    // optional context, not authority to capture or upload audio.
    let settings = environment
        .connection(true)
        .and_then(|connection| protocol::api::server::settings(&connection))
        .unwrap_or_else(|_| json!({}));
    let mut projected = json!({});
    if let Some(words) = settings.get("speechProjectCustomWords") {
        projected["speechProjectCustomWords"] = words.clone();
    }
    // This setting is not file-backed and has no legacy project aggregate.
    // A present empty project dictionary is authoritative over the environment.
    if let Some(words) = project
        .and_then(|id| settings["projectSettingsOverrides"].get(id))
        .and_then(|overrides| overrides.get("speechProjectCustomWords"))
    {
        projected["speechProjectCustomWords"] = words.clone();
    }
    consent
        .decode_speech("project-context", &projected.to_string())
        .map_err(|_| "voice_project_context_unavailable".to_owned())?
        .ok_or_else(|| "voice_project_context_invalid".into())
}
