//! Native file-metadata cache state, separate from transport and mounted UI leases.
//! Reference: nightly 4288 fileMetadata.ts at 32a0531748c44b25072d7495ce0b9bd7e9450a87.
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Weak,
    },
};
pub const STALE_MS: u64 = 60_000;
pub const CACHE_LIMIT: usize = 1024;
pub const BATCH_LIMIT: usize = 64;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Metadata {
    pub kind: String,
    pub byte_length: Option<u64>,
    pub mime_type: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ThreadRef {
    pub environment: String,
    pub thread: String,
}
#[derive(Clone)]
struct Entry {
    value: Option<Metadata>,
    expires: u64,
}
#[derive(Default)]
struct Session {
    environment: String,
    entries: HashMap<String, Entry>,
    order: VecDeque<String>,
    version: u64,
    in_flight: HashMap<String, Vec<Weak<AtomicBool>>>,
}
#[derive(Default)]
struct Thread {
    readers: usize,
    identity: Arc<()>,
    sessions: HashMap<String, HashMap<String, Entry>>,
}
#[derive(Default)]
pub struct Cache {
    sessions: HashMap<String, Session>,
    threads: HashMap<ThreadRef, Thread>,
}
/// One in-flight path response. Invalidation only expires tokens for that path.
pub struct Request {
    session: String,
    path: String,
    valid: Arc<AtomicBool>,
    retained: Option<(ThreadRef, Arc<()>)>,
}
#[derive(Debug, PartialEq, Eq)]
pub enum Completion {
    Remembered,
    Retry,
    SessionClosed,
}
#[derive(Clone)]
pub struct SeedGuard {
    session: String,
    version: u64,
}
#[derive(Debug, PartialEq, Eq)]
pub struct Batch {
    pub session: String,
    pub paths: Vec<String>,
    pub requests: Vec<usize>,
}
/// Schedule already-created query requests by actual socket, preserving path
/// order and deduplicating each native-size batch's wire paths.
pub fn batches(requests: &[Request]) -> Vec<Batch> {
    let mut groups = Vec::<(String, Vec<usize>)>::new();
    for (index, request) in requests.iter().enumerate() {
        if let Some((_, indices)) = groups.iter_mut().find(|(id, _)| id == &request.session) {
            indices.push(index)
        } else {
            groups.push((request.session.clone(), vec![index]))
        }
    }
    groups
        .into_iter()
        .flat_map(|(session, indices)| {
            indices
                .chunks(BATCH_LIMIT)
                .map(|chunk| {
                    let mut seen = HashSet::new();
                    let paths = chunk
                        .iter()
                        .filter_map(|index| {
                            let path = &requests[*index].path;
                            seen.insert(path.clone()).then(|| path.clone())
                        })
                        .collect();
                    Batch {
                        session: session.clone(),
                        paths,
                        requests: chunk.to_vec(),
                    }
                })
                .collect::<Vec<_>>()
        })
        .collect()
}
fn space(c: char) -> bool {
    matches!(c,'\u{9}'..='\u{d}'|' '|'\u{a0}'|'\u{1680}'|'\u{2000}'..='\u{200a}'|'\u{2028}'|'\u{2029}'|'\u{202f}'|'\u{205f}'|'\u{3000}'|'\u{feff}')
}
fn drive(path: &str) -> bool {
    let b = path.as_bytes();
    b.len() >= 2
        && b[0].is_ascii_alphabetic()
        && b[1] == b':'
        && (b.len() == 2 || matches!(b[2], b'/' | b'\\'))
}
/// Native comparison normalization only: numeric suffixes remain literal here.
/// Position parsing and UNC/absolute/length admission belong to the query caller.
pub fn normalize(value: &str) -> String {
    let raw = value.trim_matches(space);
    let root = raw == "/" || raw == "\\" || (raw.len() == 3 && drive(raw));
    let trimmed = if root {
        raw
    } else if raw.starts_with('/') {
        raw.trim_end_matches('/')
    } else {
        raw.trim_end_matches(['/', '\\'])
    };
    let path = if trimmed.is_empty() {
        raw.to_owned()
    } else if trimmed.len() == 2 && drive(trimmed) {
        format!("{trimmed}\\")
    } else {
        trimmed.to_owned()
    };
    if drive(&path) || path.starts_with("\\\\") {
        path.replace('/', "\\").to_lowercase()
    } else {
        path
    }
}
fn merge(previous: &Metadata, next: &Metadata) -> Metadata {
    Metadata {
        kind: next.kind.clone(),
        byte_length: next.byte_length.or(previous.byte_length),
        mime_type: next
            .mime_type
            .clone()
            .or_else(|| previous.mime_type.clone()),
    }
}
impl Cache {
    /// Register an actual SDK socket identity. Re-registering it does not reset data.
    pub fn open_session(&mut self, id: &str, environment: &str) {
        self.sessions.entry(id.into()).or_insert_with(|| Session {
            environment: environment.into(),
            ..Session::default()
        });
    }
    pub fn close_session(&mut self, id: &str) {
        self.sessions.remove(id);
        for thread in self.threads.values_mut() {
            thread.sessions.remove(id);
        }
    }
    pub fn retain_thread(&mut self, reference: ThreadRef) {
        self.threads.entry(reference).or_default().readers += 1;
    }
    pub fn release_thread(&mut self, reference: &ThreadRef) {
        if let Some(thread) = self.threads.get_mut(reference) {
            thread.readers -= 1;
            if thread.readers == 0 {
                self.threads.remove(reference);
            }
        }
    }
    /// Outer None means fetch; Some(None) means a fresh known miss.
    pub fn known(
        &mut self,
        id: &str,
        reference: Option<&ThreadRef>,
        path: &str,
        now: u64,
    ) -> Option<Option<Metadata>> {
        let path = normalize(path);
        let session = self.sessions.get(id)?;
        let reference = reference.filter(|r| r.environment == session.environment);
        let retained = reference
            .and_then(|r| self.threads.get_mut(r))
            .map(|thread| thread.sessions.entry(id.into()).or_default());
        if let Some(entry) = retained.as_ref().and_then(|entries| entries.get(&path)) {
            if entry.value.is_some() || entry.expires > now {
                return Some(entry.value.clone());
            }
        }
        let entry = session
            .entries
            .get(&path)
            .filter(|entry| entry.expires > now)?;
        if let Some(entries) = retained {
            entries.insert(path, entry.clone());
        }
        Some(entry.value.clone())
    }
    pub fn begin(
        &mut self,
        id: &str,
        reference: Option<&ThreadRef>,
        path: &str,
    ) -> Option<Request> {
        let environment = &self.sessions.get(id)?.environment;
        let retained = reference
            .filter(|r| &r.environment == environment)
            .and_then(|r| {
                self.threads
                    .get(r)
                    .map(|thread| (r.clone(), thread.identity.clone()))
            });
        let session = self.sessions.get_mut(id)?;
        let path = normalize(path);
        let valid = Arc::new(AtomicBool::new(true));
        let tokens = session.in_flight.entry(path.clone()).or_default();
        tokens.retain(|token| token.strong_count() > 0);
        tokens.push(Arc::downgrade(&valid));
        Some(Request {
            session: id.into(),
            path,
            valid,
            retained,
        })
    }
    pub fn complete(&mut self, request: Request, value: Option<Metadata>, now: u64) -> Completion {
        let Some(session) = self.sessions.get_mut(&request.session) else {
            return Completion::SessionClosed;
        };
        if let Some(tokens) = session.in_flight.get_mut(&request.path) {
            tokens.retain(|token| {
                token
                    .upgrade()
                    .is_some_and(|token| !Arc::ptr_eq(&token, &request.valid))
            });
            if tokens.is_empty() {
                session.in_flight.remove(&request.path);
            }
        }
        if !request.valid.load(Ordering::Acquire) {
            return Completion::Retry;
        }
        self.remember(&request.session, &request.path, value.clone(), now, false);
        if let Some(thread) = request.retained.as_ref().and_then(|(reference, identity)| {
            self.threads
                .get_mut(reference)
                .filter(|thread| Arc::ptr_eq(&thread.identity, identity))
        }) {
            thread.sessions.entry(request.session).or_default().insert(
                request.path,
                Entry {
                    value,
                    expires: now.saturating_add(STALE_MS),
                },
            );
        }
        Completion::Remembered
    }
    pub fn abandon(&mut self, request: Request) {
        if let Some(session) = self.sessions.get_mut(&request.session) {
            if let Some(tokens) = session.in_flight.get_mut(&request.path) {
                tokens.retain(|token| {
                    token
                        .upgrade()
                        .is_some_and(|token| !Arc::ptr_eq(&token, &request.valid))
                });
                if tokens.is_empty() {
                    session.in_flight.remove(&request.path);
                }
            }
        }
    }
    fn remember(&mut self, id: &str, path: &str, value: Option<Metadata>, now: u64, partial: bool) {
        let Some(session) = self.sessions.get_mut(id) else {
            return;
        };
        let path = normalize(path);
        let entry = Entry {
            value: value.clone(),
            expires: now.saturating_add(STALE_MS),
        };
        session.order.retain(|key| key != &path);
        session.order.push_back(path.clone());
        session.entries.insert(path.clone(), entry.clone());
        if session.order.len() > CACHE_LIMIT {
            if let Some(oldest) = session.order.pop_front() {
                session.entries.remove(&oldest);
            }
        }
        for thread in self.threads.values_mut() {
            if let Some(previous) = thread
                .sessions
                .get_mut(id)
                .and_then(|entries| entries.get_mut(&path))
            {
                let retained = match (&previous.value, &value) {
                    (Some(old), Some(next)) if partial && old.kind == next.kind => {
                        Some(merge(old, next))
                    }
                    _ => value.clone(),
                };
                *previous = Entry {
                    value: retained,
                    ..entry.clone()
                };
            }
        }
    }
    pub fn invalidate(&mut self, id: &str, path: &str) {
        let Some(session) = self.sessions.get_mut(id) else {
            return;
        };
        let path = normalize(path);
        session.version = session.version.saturating_add(1);
        session.entries.remove(&path);
        session.order.retain(|key| key != &path);
        if let Some(tokens) = session.in_flight.get_mut(&path) {
            tokens.retain(|weak| {
                if let Some(token) = weak.upgrade() {
                    token.store(false, Ordering::Release);
                    true
                } else {
                    false
                }
            });
        }
        for thread in self.threads.values_mut() {
            if let Some(entries) = thread.sessions.get_mut(id) {
                entries.remove(&path);
            }
        }
    }
    pub fn seed_guard(&self, id: &str) -> Option<SeedGuard> {
        self.sessions.get(id).map(|session| SeedGuard {
            session: id.into(),
            version: session.version,
        })
    }
    fn can_seed(&self, guard: &SeedGuard) -> bool {
        self.sessions
            .get(&guard.session)
            .is_some_and(|session| session.version == guard.version)
    }
    pub fn seed_entries(&mut self, guard: &SeedGuard, entries: &[(String, String)], now: u64) {
        if !self.can_seed(guard) {
            return;
        }
        for (path, kind) in entries {
            let path = normalize(path);
            let same = self.sessions[&guard.session]
                .entries
                .get(&path)
                .is_some_and(|entry| {
                    entry.expires > now
                        && entry
                            .value
                            .as_ref()
                            .is_some_and(|value| value.kind == *kind)
                });
            if !same {
                self.remember(
                    &guard.session,
                    &path,
                    Some(Metadata {
                        kind: kind.clone(),
                        byte_length: None,
                        mime_type: None,
                    }),
                    now,
                    true,
                );
            }
        }
    }
    pub fn seed_file(&mut self, guard: &SeedGuard, path: &str, byte_length: u64, now: u64) {
        if !self.can_seed(guard) {
            return;
        }
        let path = normalize(path);
        let previous = self.sessions[&guard.session]
            .entries
            .get(&path)
            .filter(|entry| entry.expires > now)
            .and_then(|entry| entry.value.as_ref());
        let next = Metadata {
            kind: "file".into(),
            byte_length: Some(byte_length),
            mime_type: previous.and_then(|value| value.mime_type.clone()),
        };
        self.remember(&guard.session, &path, Some(next), now, true);
    }
    pub fn entry_count(&self, id: &str) -> usize {
        self.sessions
            .get(id)
            .map_or(0, |session| session.entries.len())
    }
}
