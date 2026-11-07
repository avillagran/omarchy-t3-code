//! Voice transport bound to one original consent identity and its destination.
//! Audio remains binary Rust data; this module exposes no CLI or QML action.
use crate::{
    protocol,
    voice_consent::{AudioGrantGuard, CleanupGrantGuard},
    Environment, Result, Value,
};
use protocol::speech::{
    CancellableSpeech, CloudKeys, CloudProvider, PostProcessRequest, RequestCancellation,
    RequestFailure, RequestResult, TranscriptionOptions,
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
pub struct BoundCleanup {
    audio: Arc<AudioGrantGuard>,
    cleanup: CleanupGrantGuard,
    draft_owner: String,
    draft_environment: String,
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
    /// Capture during preparation, using the draft's environment, which may be
    /// different from the consented audio destination. No cleanup grant is inferred.
    pub fn bind_cleanup(
        &self,
        draft: &Environment,
        draft_owner: &str,
    ) -> Result<Option<BoundCleanup>> {
        if !self.grant.is_current() {
            return Ok(None);
        }
        let Some(cleanup) = self
            .grant
            .cleanup_grant()
            .map_err(|_| "voice_consent_unavailable")?
        else {
            return Ok(None);
        };
        Ok(Some(BoundCleanup {
            audio: self.grant.clone(),
            cleanup,
            draft_owner: draft_owner.into(),
            draft_environment: draft.id.clone(),
            client: draft.connection(true)?.cancellable_speech()?,
        }))
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
impl BoundCleanup {
    fn current(&self, owner: &impl Fn() -> bool) -> bool {
        owner() && self.audio.is_current() && self.cleanup.is_current()
    }
    /// None preserves the raw transcript when any captured/current identity no
    /// longer matches. Failures remain distinct for the original fallback notice.
    pub async fn post_process(
        &self,
        draft_owner: &str,
        draft_environment: &str,
        input: &PostProcessRequest,
        cancel: &RequestCancellation,
        owner: impl Fn() -> bool,
    ) -> RequestResult<Option<String>> {
        if draft_owner != self.draft_owner
            || draft_environment != self.draft_environment
            || !self.current(&owner)
        {
            return Ok(None);
        }
        let session = self.client.session(cancel, || self.current(&owner)).await?;
        if session["authenticated"] != true {
            return Err(RequestFailure::Transport("unauthorized".into()));
        }
        if !session
            .get("permissions")
            .or_else(|| session.get("scopes"))
            .and_then(Value::as_array)
            .is_some_and(|items| items.iter().any(|item| item == "orchestration:operate"))
        {
            return Err(RequestFailure::Transport("permission_denied".into()));
        }
        self.client
            .post_process(input, cancel, || self.current(&owner))
            .await
            .map(Some)
    }
}
