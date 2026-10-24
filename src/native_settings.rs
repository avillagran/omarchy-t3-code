//! Native server preferences exposed to the QML settings views. Credentials and
//! arbitrary provider configuration are never projected into these descriptors.
use crate::*;
pub use t3_protocol::native_settings::{description, idle_compaction, idle_compaction_patch, model_selection, patch, resolve_idle_compaction, snapshot, thread_defaults, MODEL_KEYS};
pub fn execute(e:&Environment,input:&Value)->Result<Value>{
    if text(input,"action")=="update"&&text(input,"key")=="enableAgentDeviceAccess" {
        return Err("device_configuration_required".into());
    }
    match text(input,"action") {
        "read"=>{},
        "update"=>{
            let config=if crate::storage_settings::is_key(text(input,"key"))||MODEL_KEYS.iter().any(|(key,_)|*key==text(input,"key"))&&!input["value"].is_null(){Some(crate::write::rpc(e,"server.getConfig",json!({}))?)}else{None};
            let mut next=patch(input,config.as_ref())?;
            if text(input,"key")=="idleCompaction"{next=idle_compaction_patch(&crate::write::rpc(e,"server.getSettings",json!({}))?,&next)?;}
            crate::write::rpc(e,"server.updateSettings",next)?;
        },
        _=>return Err("invalid_settings".into()),
    }
    crate::write::rpc(e,"server.getSettings",json!({})).map(|v|snapshot(&v))
}
