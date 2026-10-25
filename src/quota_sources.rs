//! Public subscription observations only. Raw provider configuration and credentials
//! never cross this boundary. Native drivers retain ownership of credential reads.
use crate::{json, protocol::{self, api::streams}, text, Environment, Result};
use serde_json::Value;
pub use t3_protocol::quota_sources::{apply, limits, providers, sources};

pub fn authorize(c: &protocol::Connection) -> Result<()> {
    let session = c.request_json("GET", "/api/auth/session", None, false)?;
    if session["authenticated"] == true
        && session
            .get("permissions")
            .or_else(|| session.get("scopes"))
            .and_then(Value::as_array)
            .is_some_and(|s| s.iter().any(|s| s == "orchestration:read"))
    {
        Ok(())
    } else {
        Err("permission_denied".into())
    }
}
pub fn subscribe(
    e: &Environment,
    cancel: impl Fn() -> bool,
    mut receive: impl FnMut(Option<Value>) -> Result<()>,
) -> Result<()> {
    let c = e.connection(false)?;
    authorize(&c)?;
    let sub = streams::ConfigSub { usage_limit_sources: Some(true), ..Default::default() };
    streams::subscribe_server_config(&c,&sub,cancel,||false,|event| {
        // Unknown native events (including settings) never enter the observation.
        if let Some(event)=event {
            if event["version"]==1 && matches!(text(&event,"type"),"snapshot"|"providerStatuses"|"usageLimitSourcesUpdated") {
                let mut projected=json!({});
                apply(&mut projected,&event);
                let kind=text(&event,"type");
                let public=match kind {"snapshot"=>json!({"version":1,"type":kind,"config":{"providers":projected["providers"]}}),"providerStatuses"=>json!({"version":1,"type":kind,"payload":{"providers":projected["providers"]}}),_=>json!({"version":1,"type":kind,"payload":{"sources":projected["usageLimitSources"]}})};
                receive(Some(public))?;
            }
        } else {receive(None)?;}
        Ok(())
    })
}
