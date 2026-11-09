//! Ordinary voice preferences use the original metadata codec. They cannot
//! restore either consent grant, which lives in a separate owned namespace.
use crate::{connection_preferences, json, voice_consent::ConsentOwner, Result, Value};
use std::path::Path;

pub use connection_preferences::VOICE_KEYS as KEYS;

pub fn update_at(
    consent: &ConsentOwner,
    path: &Path,
    expected: &str,
    patch: &Value,
) -> Result<Value> {
    let fields = patch
        .as_object()
        .filter(|fields| {
            !fields.is_empty() && fields.keys().all(|key| KEYS.contains(&key.as_str()))
        })
        .ok_or("invalid_client_preference")?;
    let raw = connection_preferences::client_settings_at(path)?;
    let mut candidate = json!({});
    for key in KEYS {
        if let Some(value) = raw.get(*key) {
            candidate[*key] = value.clone();
        }
    }
    for (key, value) in fields {
        candidate[key] = value.clone();
    }
    let decoded = consent
        .decode_speech("preferences", &candidate.to_string())
        .map_err(|_| "voice_preferences_unavailable")?
        .ok_or("invalid_client_preference")?;
    let decoded: Value =
        serde_json::from_str(&decoded).map_err(|_| "voice_preferences_unrepresentable")?;
    connection_preferences::update_validated_voice_at(path, expected, patch, &decoded)
}

pub fn update(consent: &ConsentOwner, expected: &str, patch: &Value) -> Result<Value> {
    update_at(
        consent,
        &crate::t3_home().join("userdata/client-settings.json"),
        expected,
        patch,
    )
}
