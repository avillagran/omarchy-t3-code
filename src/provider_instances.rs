//! Provider instances use T3's atomic mutation contract. The native view is a
//! bounded, non-secret projection of the upstream provider client definitions.
use crate::*;
pub use t3_protocol::provider_instances::{confirm_target, mutation, projection};

pub fn execute(e: &Environment, input: &Value) -> Result<Value> {
    let client=Client::shared()?;
    let session=client.get(e,"/api/auth/session",false)?;
    let scopes=session.get("permissions").or_else(||session.get("scopes")).and_then(Value::as_array);
    let readable=session["authenticated"]==true&&scopes.is_some_and(|v|v.iter().any(|s|s=="orchestration:read"));
    if !readable {return Err("permission_denied".into());}
    let writable=scopes.is_some_and(|v|v.iter().any(|s|s=="providers:manage"));
    if text(input,"action")!="read" {
        if !writable {return Err("permission_denied".into());}
        let settings=crate::write::rpc(e,"server.getSettings",json!({}))?;
        let payload=mutation(input,&settings)?;
        crate::write::rpc(e,"server.updateSettings",payload.clone())?;
        // Read back the exact instance. A successful RPC alone isn't proof it persisted.
        let saved=crate::write::rpc(e,"server.getSettings",json!({}))?;
        let id=text(input,"instanceId");
        if !confirm_target(text(input,"action"),id,&payload,&saved){
            return Err("provider_write_unconfirmed".into());
        }
    }
    let settings=crate::write::rpc(e,"server.getSettings",json!({}))?;
    let config=crate::write::rpc(e,"server.getConfig",json!({}))?;
    Ok(projection(&settings,&config,writable))
}
