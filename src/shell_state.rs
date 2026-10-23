//! Per-environment shell kept current by `orchestration.subscribeShell` deltas
//! (the bin's shell watcher). The sidebar reads a live shell from here instead of
//! fetching `/api/orchestration/shell` on every change; an environment whose
//! stream is down falls back to HTTP.
//!
//! `reduce` follows T3's client reducer (client-runtime `state/shellReducer.ts`):
//! an authoritative snapshot replaces the shell, a snapshot carrying
//! `resolvedRepositoryIdentityRoots` only refreshes project identities, and
//! deltas at or below the shell's `snapshotSequence` are ignored.
use serde_json::{json, Value};
use std::{collections::HashMap, sync::Mutex};

#[derive(Default)]
struct Entry {
    shell: Option<Value>,
    live: bool,
}
static SHELLS: Mutex<Option<HashMap<String, Entry>>> = Mutex::new(None);

fn with<R>(env: &str, f: impl FnOnce(&mut Entry) -> R) -> R {
    let mut guard = SHELLS.lock().unwrap_or_else(|e| e.into_inner());
    f(guard.get_or_insert_with(HashMap::new).entry(env.to_string()).or_default())
}

/// The shell of an environment whose stream is connected and caught up.
pub fn live(env: &str) -> Option<Value> {
    with(env, |e| if e.live { e.shell.clone() } else { None })
}
/// Sequence to resume from after a reconnect, if a shell is held.
pub fn cursor(env: &str) -> Option<u64> {
    with(env, |e| e.shell.as_ref().and_then(|s| s["snapshotSequence"].as_u64()))
}
pub fn has_shell(env: &str) -> bool {
    with(env, |e| e.shell.is_some())
}
pub fn set_live(env: &str, live: bool) {
    with(env, |e| e.live = live && e.shell.is_some())
}
/// Applies one stream item; true when the visible shell changed.
pub fn apply(env: &str, item: &Value) -> bool {
    with(env, |e| reduce(&mut e.shell, item))
}
/// Drops the held shell so the next subscription starts from a full snapshot.
pub fn reset(env: &str) {
    with(env, |e| *e = Entry::default())
}
pub fn forget(env: &str) {
    let mut guard = SHELLS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(map) = guard.as_mut() {
        map.remove(env);
    }
}

fn same_id(a: &Value, b: &Value) -> bool {
    !a["id"].is_null() && a["id"] == b["id"]
}
fn upsert(list: &mut Vec<Value>, item: Value) {
    match list.iter_mut().find(|x| same_id(x, &item)) {
        Some(slot) => *slot = item,
        None => list.push(item),
    }
}
fn array(shell: &mut Value, key: &str) -> Vec<Value> {
    shell[key].as_array_mut().map(std::mem::take).unwrap_or_default()
}
/// Enrichment is asynchronous: keep a resolved identity for the same workspace root
/// when a later project body still has none.
fn retain_identity(previous: Option<&Value>, mut next: Value) -> Value {
    if let Some(prior) = previous {
        if next["repositoryIdentity"].is_null()
            && !prior["repositoryIdentity"].is_null()
            && prior["workspaceRoot"] == next["workspaceRoot"]
        {
            next["repositoryIdentity"] = prior["repositoryIdentity"].clone();
        }
    }
    next
}

pub fn reduce(state: &mut Option<Value>, item: &Value) -> bool {
    match item["kind"].as_str().unwrap_or("") {
        "snapshot" => {
            let Some(snapshot) = item["snapshot"].as_object() else { return false };
            if let Some(roots) = item["resolvedRepositoryIdentityRoots"].as_array() {
                // Metadata-only refresh: structure and sequence never change.
                let Some(shell) = state.as_mut() else { return false };
                let candidates = snapshot.get("projects").and_then(Value::as_array).cloned().unwrap_or_default();
                let mut changed = false;
                for project in shell["projects"].as_array_mut().into_iter().flatten() {
                    let Some(candidate) = candidates.iter().find(|c| same_id(c, project)) else { continue };
                    if candidate["workspaceRoot"] != project["workspaceRoot"] {
                        continue;
                    }
                    let resolved = roots.contains(&project["workspaceRoot"]);
                    let identity = &candidate["repositoryIdentity"];
                    if (resolved || (project["repositoryIdentity"].is_null() && !identity.is_null()))
                        && project["repositoryIdentity"] != *identity
                    {
                        project["repositoryIdentity"] = identity.clone();
                        changed = true;
                    }
                }
                return changed;
            }
            let mut next = Value::Object(snapshot.clone());
            // The archive has its own bounded query; the sidebar never shows it.
            next["archivedThreads"] = json!([]);
            if let Some(previous) = state.as_ref() {
                let prior = previous["projects"].as_array().cloned().unwrap_or_default();
                let projects = array(&mut next, "projects")
                    .into_iter()
                    .map(|p| retain_identity(prior.iter().find(|q| same_id(q, &p)), p))
                    .collect::<Vec<_>>();
                next["projects"] = json!(projects);
            }
            let changed = state.as_ref() != Some(&next);
            *state = Some(next);
            changed
        }
        kind @ ("project.updated" | "project.removed" | "thread.updated" | "thread.removed") => {
            let Some(shell) = state.as_mut() else { return false };
            let Some(sequence) = item["sequence"].as_u64() else { return false };
            if sequence <= shell["snapshotSequence"].as_u64().unwrap_or(0) {
                return false;
            }
            let before = shell.clone();
            match kind {
                "project.updated" => {
                    let mut projects = array(shell, "projects");
                    let project = retain_identity(projects.iter().find(|p| same_id(p, &item["project"])), item["project"].clone());
                    upsert(&mut projects, project);
                    shell["projects"] = json!(projects);
                }
                "project.removed" => {
                    let mut projects = array(shell, "projects");
                    projects.retain(|p| p["id"] != item["projectId"]);
                    shell["projects"] = json!(projects);
                }
                "thread.updated" => {
                    let mut threads = array(shell, "threads");
                    if item["location"] == "active" {
                        upsert(&mut threads, item["thread"].clone());
                    } else {
                        // Older servers still send archive-located deltas on this stream.
                        threads.retain(|t| !same_id(t, &item["thread"]));
                    }
                    shell["threads"] = json!(threads);
                }
                _ => {
                    let mut threads = array(shell, "threads");
                    threads.retain(|t| t["id"] != item["threadId"]);
                    shell["threads"] = json!(threads);
                }
            }
            let changed = before["projects"] != shell["projects"] || before["threads"] != shell["threads"];
            shell["snapshotSequence"] = json!(sequence);
            changed
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn snapshot(sequence: u64) -> Value {
        json!({"kind":"snapshot","snapshot":{"schemaVersion":1,"snapshotSequence":sequence,
            "projects":[{"id":"p1","workspaceRoot":"/w","repositoryIdentity":{"key":"r"}}],
            "threads":[{"id":"t1","title":"One","updatedAt":"a"}],
            "archivedThreads":[{"id":"old"}]}})
    }
    #[test]
    fn authoritative_snapshot_replaces_and_drops_the_archive() {
        let mut state = None;
        assert!(reduce(&mut state, &snapshot(5)));
        let shell = state.as_ref().unwrap();
        assert_eq!(shell["snapshotSequence"], 5);
        assert_eq!(shell["archivedThreads"], json!([]));
        assert!(!reduce(&mut state, &snapshot(5)), "an identical snapshot changes nothing");
    }
    #[test]
    fn deltas_apply_once_in_sequence_order() {
        let mut state = None;
        reduce(&mut state, &snapshot(5));
        let add = json!({"kind":"thread.updated","sequence":6,"location":"active","thread":{"id":"t2","title":"Two"}});
        assert!(reduce(&mut state, &add));
        assert!(!reduce(&mut state, &add), "replayed sequence is ignored");
        let stale = json!({"kind":"thread.removed","sequence":4,"location":"active","threadId":"t1"});
        assert!(!reduce(&mut state, &stale));
        let rename = json!({"kind":"thread.updated","sequence":7,"location":"active","thread":{"id":"t1","title":"Renamed"}});
        assert!(reduce(&mut state, &rename));
        let shell = state.as_ref().unwrap();
        assert_eq!(shell["snapshotSequence"], 7);
        assert_eq!(shell["threads"].as_array().unwrap().len(), 2);
        assert_eq!(shell["threads"][0]["title"], "Renamed", "upsert keeps the position");
        let archived = json!({"kind":"thread.updated","sequence":8,"location":"archive","thread":{"id":"t2","title":"Two"}});
        assert!(reduce(&mut state, &archived));
        let removed = json!({"kind":"project.removed","sequence":9,"projectId":"p1"});
        assert!(reduce(&mut state, &removed));
        let shell = state.unwrap();
        assert_eq!(shell["threads"].as_array().unwrap().len(), 1);
        assert_eq!(shell["projects"], json!([]));
        assert_eq!(shell["snapshotSequence"], 9);
    }
    #[test]
    fn unchanged_thread_body_only_advances_the_cursor() {
        let mut state = None;
        reduce(&mut state, &snapshot(5));
        let same = json!({"kind":"thread.updated","sequence":6,"location":"active","thread":{"id":"t1","title":"One","updatedAt":"a"}});
        assert!(!reduce(&mut state, &same));
        assert_eq!(state.unwrap()["snapshotSequence"], 6);
    }
    #[test]
    fn deltas_without_a_shell_wait_for_the_snapshot() {
        let mut state = None;
        assert!(!reduce(&mut state, &json!({"kind":"thread.removed","sequence":1,"location":"active","threadId":"t1"})));
        assert!(state.is_none());
    }
    #[test]
    fn metadata_refresh_merges_identities_without_moving_the_cursor() {
        let mut state = None;
        reduce(&mut state, &snapshot(5));
        let refresh = json!({"kind":"snapshot","resolvedRepositoryIdentityRoots":["/w"],"snapshot":{"schemaVersion":1,"snapshotSequence":40,
            "projects":[{"id":"p1","workspaceRoot":"/w","repositoryIdentity":{"key":"new"}}],"threads":[],"archivedThreads":[]}});
        assert!(reduce(&mut state, &refresh));
        let shell = state.as_ref().unwrap();
        assert_eq!(shell["snapshotSequence"], 5, "a resume prefix must not skip replayed events");
        assert_eq!(shell["threads"].as_array().unwrap().len(), 1, "metadata frames carry no threads");
        assert_eq!(shell["projects"][0]["repositoryIdentity"]["key"], "new");
        assert!(!reduce(&mut None, &refresh), "nothing to merge into");
    }
    #[test]
    fn project_update_keeps_a_resolved_identity() {
        let mut state = None;
        reduce(&mut state, &snapshot(5));
        let update = json!({"kind":"project.updated","sequence":6,"project":{"id":"p1","workspaceRoot":"/w","repositoryIdentity":null,"title":"P"}});
        assert!(reduce(&mut state, &update));
        let shell = state.unwrap();
        assert_eq!(shell["projects"][0]["repositoryIdentity"]["key"], "r");
        assert_eq!(shell["projects"][0]["title"], "P");
    }
}
