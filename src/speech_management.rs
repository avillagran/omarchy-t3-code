//! Native voice controls use the selected environment's current write credential.
use crate::{json, protocol, text, Environment, Result, Value};
use protocol::speech::{
    Acceleration, CloudProvider, ControlFailure, LocalModelAction, SpeechFailure, UnloadTimeout,
};

fn granted(session: &Value, scope: &str) -> bool {
    session["authenticated"] == true
        && session
            .get("permissions")
            .or_else(|| session.get("scopes"))
            .and_then(Value::as_array)
            .is_some_and(|scopes| scopes.iter().any(|value| value == scope))
}
fn success(value: Value) -> Value {
    json!({"ok":true,"value":value})
}
fn control(result: protocol::speech::ControlResult<Value>) -> Result<Value> {
    match result {
        Ok(value) => Ok(success(value)),
        Err(ControlFailure::Transport(error)) => Err(error),
        Err(ControlFailure::InvalidRequest { reason, trace_id }) => Ok(
            json!({"ok":false,"failure":{"code":"invalid_request","reason":reason,"traceId":trace_id}}),
        ),
    }
}
fn cloud(result: protocol::speech::SpeechResult<Value>) -> Result<Value> {
    match result {
        Ok(value) => Ok(success(value)),
        Err(SpeechFailure::Transport(error)) => Err(error),
        Err(SpeechFailure::Cloud { provider, reason }) => Ok(
            json!({"ok":false,"failure":{"code":"cloud_speech","provider":provider.as_str(),"reason":reason}}),
        ),
    }
}
fn provider(value: &str) -> Result<CloudProvider> {
    match value {
        "openai" => Ok(CloudProvider::OpenAi),
        "xai" => Ok(CloudProvider::Xai),
        "openrouter" => Ok(CloudProvider::OpenRouter),
        _ => Err("invalid_speech_provider".into()),
    }
}

/// Preparation and reads require operate; model/config/key mutations require
/// settings authority. This adapter never records, uploads or invokes inference.
pub fn execute(environment: &Environment, input: &Value) -> Result<Value> {
    let action = text(input, "action");
    let scope = match action {
        "access" => None,
        "status" | "models" | "prepare" | "cloud-keys" | "cloud-models" => {
            Some("orchestration:operate")
        }
        "download" | "select" | "cancel-download" | "remove" | "acceleration"
        | "unload-timeout" | "cloud-key" => Some("settings:write"),
        _ => return Err("invalid_speech_action".into()),
    };
    // Select the same physical credential for preflight and the operation. A
    // read credential's grants can never authorize a different write identity.
    let connection = environment.connection(true)?;
    let session = connection.request_json("GET", "/api/auth/session", None, false)?;
    if session["authenticated"] != true {
        return Err("unauthorized".into());
    }
    if let Some(scope) = scope {
        if !granted(&session, scope) {
            return Err("permission_denied".into());
        }
    }
    match action {
        "access"=>Ok(success(json!({"canOperate":granted(&session,"orchestration:operate"),"canConfigure":granted(&session,"settings:write")}))),
        "status"=>control(connection.speech_status()),
        "models"=>{let benchmarks=match input.get("benchmarks"){None=>false,Some(value)=>value.as_bool().ok_or("invalid_speech_input")?};control(connection.speech_models(benchmarks))},
        "prepare"=>control(connection.speech_prepare_model().map(|()|Value::Null)),
        "download"|"select"|"cancel-download"|"remove"=>{
            let action=match action {"download"=>LocalModelAction::Download,"select"=>LocalModelAction::Select,"cancel-download"=>LocalModelAction::CancelDownload,_=>LocalModelAction::Remove};
            control(connection.speech_model_action(action,input["modelId"].as_str().ok_or("invalid_speech_input")?))
        },
        "acceleration"=>{let value=Acceleration::parse(input["value"].as_str().ok_or("invalid_speech_input")?)?;control(connection.speech_set_acceleration(&value))},
        "unload-timeout"=>{let value=UnloadTimeout::parse(input["value"].as_str().ok_or("invalid_speech_input")?)?;control(connection.speech_set_unload_timeout(value))},
        "cloud-keys"=>cloud(connection.speech_cloud_keys().map(|keys|{let mut out=json!({"openai":keys.openai,"xai":keys.xai});if let Some(value)=keys.openrouter{out["openrouter"]=json!(value);}out})),
        "cloud-models"=>cloud(connection.speech_cloud_models().map(|models|json!({"models":models.into_iter().map(|model|json!({"id":model.id,"name":model.name})).collect::<Vec<_>>()}))),
        "cloud-key"=>{connection.speech_set_cloud_key(provider(text(input,"provider"))?,input.get("key").ok_or("invalid_speech_input")?)?;Ok(success(Value::Null))},
        _=>Err("invalid_speech_action".into()),
    }
}
