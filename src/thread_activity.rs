//! Native sidebar clock inputs, separate from reply sorting and elapsed work.
use serde_json::{json, Value};

fn date(value: Option<&Value>) -> Option<Value> {
    match value {
        None | Some(Value::Null) => Some(Value::Null),
        Some(Value::String(value)) => Some(Value::String(value.clone())),
        _ => None,
    }
}
/// Present the timestamp fields of a native shell using the same absent/null
/// run-completion boundary as models.presentThreadShell. Legacy numeric shells
/// keep the existing display fallback instead of guessing their date encoding.
pub fn shell_clock(thread: &Value) -> Option<Value> {
    let id = thread.get("latestRunId")?;
    let updated = thread.get("updatedAt")?.as_str()?;
    let user = date(Some(thread.get("latestUserMessageAt")?))?;
    let settled = date(Some(thread.get("settledAt")?))?;
    let run = if id.is_null() {
        Value::Null
    } else {
        let id = id.as_str()?;
        let status = thread.get("status")?.as_str()?;
        let completed = match thread.get("latestRunCompletedAt") {
            None if matches!(
                status,
                "idle" | "completed" | "interrupted" | "failed" | "cancelled" | "rolled_back"
            ) =>
            {
                json!(updated)
            }
            value => date(value)?,
        };
        json!({"runId":id,"status":if status=="idle"{"completed"}else{status},
            "requestedAt":date(thread.get("latestRunRequestedAt"))?,
            "startedAt":date(thread.get("latestRunStartedAt"))?,
            "completedAt":completed,"assistantMessageId":null})
    };
    Some(
        json!({"latestUserMessageAt":user,"latestRun":run,"updatedAt":updated,"settledAt":settled}),
    )
}
