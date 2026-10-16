//! Read-only by default. --metadata-test creates and deletes an empty test thread
//! per environment; it never dispatches a message or starts a provider run.
use omarchy_t3_client::*;
use serde_json::{json, Value};
use std::{cell::Cell, time::{Duration, Instant}};

fn connection(e: &Environment) -> Result<protocol::Connection> {
    if let Some(c) = &e.connection { return Ok(c.as_ref().clone()) }
    protocol::Connection::new(&e.base_url, None, protocol::Credential::bearer(&environment_token(e, true)?)?)
}
fn check(e: &Environment, metadata: bool) -> Result<Value> {
    if let Some(error) = &e.connection_error { return Err(error.clone()) }
    let c = connection(e)?;
    let catalog = c.model_catalog()?;
    let client = Client::new()?;
    let shell = client.get(e, "/api/orchestration/shell", true)?;
    let project = shell["projects"].as_array().into_iter().flatten().next();
    let mut created_project = None;
    let mut created_thread = None;
    let result = (|| -> Result<Value> {
        let thread = if metadata {
            let project_id = if let Some(p) = project { text(p, "id").to_owned() } else {
                if e.id != "local" { return Err("no_project_for_metadata_test".into()) }
                let id = format!("omarchy-probe-{}", uuid::Uuid::new_v4());
                c.rpc("projects.mutate", json!({"type":"project.create","commandId":uuid::Uuid::new_v4().to_string(),"projectId":id,"title":"Temporary connection validation","workspaceRoot":std::env::current_dir().map_err(|_| "local_error")?,"createWorkspaceRootIfMissing":false}))?;
                created_project = Some(id.clone()); id
            };
            let m = catalog["models"].as_array().into_iter().flatten().find(|m| m["available"] == true).ok_or("no_available_model")?;
            let response = write::manage(e, &json!({"action":"new-thread","requestId":uuid::Uuid::new_v4().to_string(),"projectId":project_id,"title":"Temporary headless connection validation","modelSelection":{"instanceId":m["instanceId"],"model":m["model"]}}))?;
            let id = text(&response, "threadId").to_owned(); created_thread = Some(id.clone()); id
        } else {
            shell["threads"].as_array().into_iter().flatten().next().map(|t|text(t,"id").to_owned()).ok_or("no_thread_for_stream_test")?
        };
        let envelope = client.get(e, &format!("/api/orchestration/threads/{thread}/bounded"), true)?;
        let received = Cell::new(false);
        let started = Instant::now();
        c.subscribe("orchestration.subscribeThread", json!({"threadId":thread,"afterSequence":envelope["snapshotSequence"],"requestCompletionMarker":true,"acceptBoundedSnapshot":true}), || received.get() || started.elapsed() > Duration::from_secs(5), |update| { if update.is_some() { received.set(true) } Ok(()) })?;
        if !received.get() { return Err("no_stream_response".into()) }
        Ok(json!({"id":e.id,"name":e.name,"read":true,"stream":true,"metadataWrite":metadata,"models":catalog["models"].as_array().map(Vec::len)}))
    })();
    // Only IDs created by this invocation are eligible for cleanup.
    let mut cleanup = None;
    if let Some(id) = created_thread { if let Err(error)=c.rpc("orchestration.dispatchCommand", json!({"type":"thread.delete","commandId":uuid::Uuid::new_v4().to_string(),"threadId":id})) { cleanup=Some(error); } }
    if let Some(id) = created_project { if let Err(error)=c.rpc("projects.mutate", json!({"type":"project.delete","commandId":uuid::Uuid::new_v4().to_string(),"projectId":id})) { cleanup=Some(error); } }
    if let Some(error) = cleanup { return Err(format!("probe_cleanup_failed:{error}")) }
    result
}
fn main() {
    let metadata = std::env::args().any(|a| a == "--metadata-test");
    let environments = match environments() { Ok(e) => e, Err(error) => { println!("{}",json!({"error":error}));std::process::exit(1) } };
    let mut failed = environments.is_empty();
    for e in environments { match check(&e, metadata) { Ok(result) => println!("{result}"), Err(error) => { failed=true;println!("{}",json!({"id":e.id,"name":e.name,"error":error})); } } }
    if failed { std::process::exit(1) }
}
