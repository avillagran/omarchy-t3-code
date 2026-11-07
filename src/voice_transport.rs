//! Voice transport bound to one original consent identity and its destination.
//! Audio remains binary Rust data; this module exposes no CLI or QML action.
use crate::{protocol, voice_consent::AudioGrantGuard, Environment, Result, Value};
use protocol::speech::{
    CancellableSpeech, CloudKeys, CloudProvider, RequestCancellation, RequestFailure,
    RequestResult, TranscriptionOptions,
};
use std::sync::Arc;

/// An immutable transport selection. The caller also owns recording/draft
/// cancellation and must cancel its token when that lifetime ends.
pub struct BoundSpeech {
    grant: Arc<AudioGrantGuard>,
    environment_id: String,
    cloud: Option<(CloudProvider, Option<String>)>,
    client: CancellableSpeech,
}
impl BoundSpeech {
    /// The original consent realm already decoded this destination. UTF16 that
    /// cannot name a native environment is unavailable, never a fallback route.
    /// The trusted host resolver must return exactly the requested environment.
    pub fn bind(
        grant: AudioGrantGuard,
        resolve: impl FnOnce(&str) -> Result<Environment>,
    ) -> Result<Self> {
        if !grant.is_current() {
            return Err("voice_consent_revoked".into());
        }
        let destination: Value = serde_json::from_str(grant.destination_json())
            .map_err(|_| "voice_destination_unavailable")?;
        let environment_id = destination["environmentId"]
            .as_str()
            .ok_or("voice_destination_unavailable")?;
        let cloud = match destination["kind"].as_str() {
            Some("environment") => None,
            Some("cloud") => {
                let provider = match destination["provider"].as_str() {
                    Some("openai") => CloudProvider::OpenAi,
                    Some("xai") => CloudProvider::Xai,
                    Some("openrouter") => CloudProvider::OpenRouter,
                    _ => return Err("voice_destination_unavailable".into()),
                };
                let model = if provider == CloudProvider::OpenRouter {
                    Some(
                        destination["model"]
                            .as_str()
                            .ok_or("voice_destination_unavailable")?
                            .to_owned(),
                    )
                } else {
                    None
                };
                Some((provider, model))
            }
            _ => return Err("voice_destination_unavailable".into()),
        };
        let environment = resolve(environment_id)?;
        if environment.id != environment_id || !grant.is_current() {
            return Err("voice_destination_unavailable".into());
        }
        // Freeze one physical write credential for both authority and voice.
        // Drop the blocking Connection here, outside any caller async runtime.
        let client = environment.connection(true)?.cancellable_speech()?;
        Ok(Self {
            grant: Arc::new(grant),
            environment_id: environment_id.to_owned(),
            cloud,
            client,
        })
    }
    pub fn environment_id(&self) -> &str {
        &self.environment_id
    }
    pub fn is_cloud(&self) -> bool {
        self.cloud.is_some()
    }
    /// Current draft/recording ownership and the original grant are independent
    /// requirements. No stored boolean or selected UI environment grants audio.
    fn current(&self, owner: &impl Fn() -> bool) -> bool {
        owner() && self.grant.is_current()
    }
    async fn authorize(
        &self,
        cancel: &RequestCancellation,
        owner: &impl Fn() -> bool,
    ) -> RequestResult<()> {
        let session = self.client.session(cancel, || self.current(owner)).await?;
        if session["authenticated"] != true {
            return Err(RequestFailure::Transport("unauthorized".into()));
        }
        let scopes = session.get("permissions").or_else(|| session.get("scopes"));
        if !scopes
            .and_then(Value::as_array)
            .is_some_and(|items| items.iter().any(|item| item == "orchestration:operate"))
        {
            return Err(RequestFailure::Transport("permission_denied".into()));
        }
        Ok(())
    }
    /// The caller uses the original platform's status/language/stream selection.
    /// This returns the native status unchanged and does not start capture.
    pub async fn status(
        &self,
        cancel: &RequestCancellation,
        owner: impl Fn() -> bool,
    ) -> RequestResult<Value> {
        self.authorize(cancel, &owner).await?;
        self.client.status(cancel, || self.current(&owner)).await
    }
    pub async fn cloud_keys(
        &self,
        cancel: &RequestCancellation,
        owner: impl Fn() -> bool,
    ) -> RequestResult<CloudKeys> {
        self.authorize(cancel, &owner).await?;
        self.client
            .cloud_keys(cancel, || self.current(&owner))
            .await
    }
    pub async fn transcribe(
        &self,
        pcm: &[u8],
        options: &TranscriptionOptions,
        cancel: &RequestCancellation,
        owner: impl Fn() -> bool,
    ) -> RequestResult<String> {
        self.authorize(cancel, &owner).await?;
        self.client
            .transcribe(
                pcm,
                options,
                self.cloud
                    .as_ref()
                    .map(|(provider, model)| (*provider, model.as_deref())),
                cancel,
                || self.current(&owner),
            )
            .await
    }
}
