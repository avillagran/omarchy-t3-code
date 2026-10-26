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
    generation:u64,
    snapshot_sequence:u64,
    older_counts:Option<(String,Value)>,
    supplemental: HashMap<String, (u64, Value)>,
    versions: HashMap<String, u64>,
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
pub fn generation(env:&str)->u64{with(env,|e|e.generation)}
pub fn is_live(env: &str) -> bool {
    with(env, |e| e.live)
}
pub fn has_shell(env: &str) -> bool {
    with(env, |e| e.shell.is_some())
}
pub fn set_live(env: &str, live: bool) {
    with(env, |e| e.live = live && e.shell.is_some())
}
/// Applies one stream item; true when the visible shell changed.
pub fn apply(env: &str, item: &Value) -> bool {
    with(env, |e| {
        let sequence=crate::protocol::stream::cursor_of(crate::protocol::api::streams::SUBSCRIBE_SHELL.name,item).unwrap_or(0);
        if item["kind"]=="snapshot" && item.get("resolvedRepositoryIdentityRoots").is_none() { e.snapshot_sequence=sequence;e.generation=e.generation.wrapping_add(1);e.supplemental.clear(); e.versions.clear();e.older_counts=None; }
        let id=item["thread"]["id"].as_str().or(item["threadId"].as_str());
        if let Some(id)=id {
            e.versions.entry(id.to_string()).and_modify(|n|*n=(*n).max(sequence)).or_insert(sequence);
            if e.supplemental.get(id).is_some_and(|(at,_)|*at<=sequence){e.supplemental.remove(id);}
        }
        reduce(&mut e.shell,item)
    })
}
/// Merge a read page without advancing the stream cursor or overwriting a newer delta.
pub fn supplement(env: &str, page: &Value) -> crate::Result<()> {
    let sequence=page["snapshotSequence"].as_u64().ok_or("missing_sequence")?;
    let rows=page["threads"].as_array().ok_or("malformed_shell_page")?;
    if rows.iter().any(|r|!r["id"].as_str().is_some_and(crate::valid_id)){return Err("malformed_shell_page".into())}
    with(env,|e| {
        let floor=e.snapshot_sequence;
        if sequence<floor{return Err("stale_shell_page".into())}
        for row in rows {
            let id=row["id"].as_str().unwrap();
            if sequence<floor || e.versions.get(id).is_some_and(|n|*n>sequence) {continue}
            if e.supplemental.get(id).is_none_or(|(n,_)|*n<=sequence){e.supplemental.insert(id.into(),(sequence,row.clone()));}
        }
        Ok(())
    })
}
pub fn update_older_counts(env:&str,before:&str,page:&Value)->crate::Result<()> {
    let sequence=page["snapshotSequence"].as_u64().ok_or("missing_sequence")?;
    let counts=page["counts"].as_array().ok_or("malformed_shell_page")?;
    if counts.iter().any(|c|!c["projectId"].as_str().is_some_and(crate::valid_id)||c["count"].as_u64().is_none()){return Err("malformed_shell_page".into())}
    with(env,|e|{
        if e.snapshot_sequence>sequence{return Err("stale_shell_page".into())}
        e.older_counts=Some((before.to_string(),json!({"before":before,"asOfSequence":sequence,"counts":counts})));Ok(())
    })
}
pub fn enriched(env: &str, mut shell: Value) -> Value {
    with(env,|e| {
        if let Some((before,counts))=&e.older_counts{if shell["olderSettled"]["before"]==*before&&shell["olderSettled"]["asOfSequence"].as_u64().unwrap_or(0)<=counts["asOfSequence"].as_u64().unwrap_or(0){shell["olderSettled"]=counts.clone();}}
        if let Some(rows)=shell["threads"].as_array_mut(){for (_,row) in e.supplemental.values(){upsert(rows,row.clone());}}
    });shell
}
/// Drops the held shell so the next subscription starts from a full snapshot.
pub fn reset(env: &str) {
    with(env, |e| {let generation=e.generation.wrapping_add(1);*e = Entry{generation,..Default::default()}})
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
            let Some(sequence) = crate::protocol::stream::cursor_of(crate::protocol::api::streams::SUBSCRIBE_SHELL.name, item) else { return false };
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

#[cfg(test)]
mod nightly_tests {
 use super::*;
 #[test]
 fn loaded_pages_never_resurrect_newer_removed_or_archived_threads() {
  let env="nightly-page-test";
  reset(env);
  apply(env,&json!({"kind":"snapshot","snapshot":{"snapshotSequence":10,"threads":[{"id":"t","title":"base"}],"projects":[]}}));
  supplement(env,&json!({"snapshotSequence":12,"threads":[{"id":"t","title":"page"},{"id":"other","title":"other"}]})).unwrap();
  apply(env,&json!({"kind":"thread.updated","sequence":11,"location":"active","thread":{"id":"t","title":"older delta"}}));
  let base=with(env,|e|e.shell.clone().unwrap());assert_eq!(enriched(env,base)["threads"][0]["title"],"page");
  apply(env,&json!({"kind":"thread.removed","sequence":13,"threadId":"t"}));
  supplement(env,&json!({"snapshotSequence":12,"threads":[{"id":"t","title":"stale"}]})).unwrap();
  assert_eq!(supplement(env,&json!({"snapshotSequence":9,"threads":[{"id":"t","title":"before snapshot"}]})).unwrap_err(),"stale_shell_page");
  let base=with(env,|e|e.shell.clone().unwrap());assert!(enriched(env,base)["threads"].as_array().unwrap().iter().all(|t|t["id"]!="t"));
  forget(env);
 }
}
