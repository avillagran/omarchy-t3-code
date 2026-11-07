//! Mounted metadata queries on actual admitted SDK sockets, with native cache policy.
use crate::{
    file_metadata_admission as admission,
    file_metadata_cache::{self as model, Cache, Completion, Metadata, ThreadRef},
    protocol, Result,
};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc, Arc, Mutex, OnceLock, Weak,
    },
    time::{SystemTime, UNIX_EPOCH},
};
type Clock = Arc<dyn Fn() -> u64 + Send + Sync>;
#[derive(Clone, Eq, PartialEq, Hash)]
struct Key {
    session: String,
    reference: Option<ThreadRef>,
    path: String,
}
#[derive(Default)]
struct State {
    cache: Cache,
    leases: HashMap<String, ThreadRef>,
    memo: HashMap<Key, Weak<Job>>,
    waiters: HashMap<u64, Weak<Waiter>>,
    revisions: HashMap<(String, String), u64>,
}
struct Shared {
    state: Mutex<State>,
    clock: Clock,
    events: Mutex<Option<mpsc::Sender<Value>>>,
}
struct Waiter {
    id: u64,
    owner: String,
    lease: Option<String>,
    cancelled: AtomicBool,
    done: AtomicBool,
    reply: mpsc::SyncSender<Result<Answer>>,
    job: Mutex<Weak<Job>>,
}
struct Job {
    key: Key,
    environment: String,
    session: protocol::RpcSession,
    request: Mutex<Option<model::Request>>,
    waiters: Mutex<Vec<Weak<Waiter>>>,
    cancelled: AtomicBool,
    finished: AtomicBool,
    batch: Mutex<Weak<BatchCancellation>>,
    shared: Weak<Shared>,
}
struct BatchCancellation {
    jobs: Vec<Weak<Job>>,
    cancel: protocol::RpcCancellation,
}
impl BatchCancellation {
    fn cancel_if_unused(&self) {
        if !self
            .jobs
            .iter()
            .any(|weak| weak.upgrade().is_some_and(|job| job.active()))
        {
            self.cancel.cancel();
        }
    }
}
impl Waiter {
    fn active(&self) -> bool {
        !self.cancelled.load(Ordering::Acquire) && !self.done.load(Ordering::Acquire)
    }
    fn cancel(&self) {
        if self.done.load(Ordering::Acquire) || self.cancelled.swap(true, Ordering::AcqRel) {
            return;
        }
        let _ = self.reply.try_send(Err("metadata_cancelled".into()));
        if let Some(job) = self.job.lock().unwrap().upgrade() {
            job.cancel_if_unused();
        }
    }
}
impl Job {
    fn active(&self) -> bool {
        !self.finished.load(Ordering::Acquire)
            && !self.cancelled.load(Ordering::Acquire)
            && self
                .waiters
                .lock()
                .unwrap()
                .iter()
                .any(|w| w.upgrade().is_some_and(|w| w.active()))
    }
    fn cancel_if_unused(&self) {
        if self.active() || self.finished.load(Ordering::Acquire) {
            return;
        }
        self.cancelled.store(true, Ordering::Release);
        let request = self.request.lock().unwrap().take();
        if let Some(shared) = self.shared.upgrade() {
            let mut state = shared.state.lock().unwrap();
            if let Some(request) = request {
                state.cache.abandon(request);
            }
            if state
                .memo
                .get(&self.key)
                .and_then(Weak::upgrade)
                .is_some_and(|current| std::ptr::eq(current.as_ref(), self))
            {
                state.memo.remove(&self.key);
            }
            for waiter in self
                .waiters
                .lock()
                .unwrap()
                .iter()
                .filter_map(Weak::upgrade)
            {
                if !waiter.active() {
                    state.waiters.remove(&waiter.id);
                }
            }
        }
        if let Some(batch) = self.batch.lock().unwrap().upgrade() {
            batch.cancel_if_unused();
        }
    }
    fn finish_locked(&self, state: &mut State, result: Result<Answer>) {
        if self.finished.swap(true, Ordering::AcqRel) {
            return;
        }
        if state
            .memo
            .get(&self.key)
            .and_then(Weak::upgrade)
            .is_some_and(|current| std::ptr::eq(current.as_ref(), self))
        {
            state.memo.remove(&self.key);
        }
        for waiter in std::mem::take(&mut *self.waiters.lock().unwrap())
            .into_iter()
            .filter_map(|w| w.upgrade())
        {
            state.waiters.remove(&waiter.id);
            if waiter.active() {
                waiter.done.store(true, Ordering::Release);
                let _ = waiter.reply.try_send(result.clone());
            }
        }
    }
    fn finish(&self, result: Result<Answer>) {
        if let Some(shared) = self.shared.upgrade() {
            self.finish_locked(&mut shared.state.lock().unwrap(), result);
        }
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        if !self.finished.load(Ordering::Acquire) {
            for waiter in self
                .waiters
                .get_mut()
                .unwrap()
                .iter()
                .filter_map(Weak::upgrade)
            {
                if waiter.active() {
                    waiter.done.store(true, Ordering::Release);
                    let _ = waiter.reply.try_send(Err("metadata_unavailable".into()));
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct Answer {
    pub environment: String,
    pub session: String,
    pub path: String,
    pub value: Option<Metadata>,
    pub revision: u64,
}
impl Answer {
    pub fn json(&self) -> Value {
        json!({"environment":self.environment,"sessionId":self.session,"path":self.path,"metadata":self.value.as_ref().map(|m|json!({"kind":m.kind,"byteLength":m.byte_length,"mimeType":m.mime_type})),"revision":self.revision})
    }
}
pub struct Query {
    waiter: Arc<Waiter>,
    reply: mpsc::Receiver<Result<Answer>>,
}
impl Query {
    pub fn wait(self) -> Result<Answer> {
        self.reply
            .recv()
            .map_err(|_| "metadata_unavailable".to_owned())?
    }
    pub fn cancel(&self) {
        self.waiter.cancel();
    }
}
impl Drop for Query {
    fn drop(&mut self) {
        self.waiter.cancel();
    }
}
pub struct Runtime {
    shared: Arc<Shared>,
    queue: mpsc::Sender<Vec<Arc<Job>>>,
    next: AtomicU64,
}
impl Runtime {
    pub fn new(clock: Clock) -> Self {
        let shared = Arc::new(Shared {
            state: Mutex::new(State::default()),
            clock,
            events: Mutex::new(None),
        });
        let (tx, rx) = mpsc::channel::<Vec<Arc<Job>>>();
        let worker = shared.clone();
        std::thread::Builder::new()
            .name("o3-file-metadata".into())
            .spawn(move || {
                while let Ok(mut jobs) = rx.recv() {
                    std::thread::yield_now();
                    for more in rx.try_iter() {
                        jobs.extend(more);
                    }
                    dispatch(&worker, jobs);
                }
            })
            .expect("metadata worker");
        Self {
            shared,
            queue: tx,
            next: AtomicU64::new(1),
        }
    }
    pub fn retain(&self, lease: &str, reference: ThreadRef) -> Result<()> {
        if lease.is_empty()
            || !crate::valid_id(&reference.thread)
            || reference.environment.is_empty()
        {
            return Err("invalid_metadata_scope".into());
        }
        let mut state = self.shared.state.lock().unwrap();
        if let Some(old) = state.leases.get(lease) {
            return if old == &reference {
                Ok(())
            } else {
                Err("metadata_lease_changed".into())
            };
        }
        state.cache.retain_thread(reference.clone());
        state.leases.insert(lease.into(), reference);
        Ok(())
    }
    pub fn release(&self, lease: &str) {
        let waiters = {
            let mut state = self.shared.state.lock().unwrap();
            if let Some(reference) = state.leases.remove(lease) {
                state.cache.release_thread(&reference);
            }
            state
                .waiters
                .values()
                .filter_map(Weak::upgrade)
                .filter(|w| w.lease.as_deref() == Some(lease))
                .collect::<Vec<_>>()
        };
        for waiter in waiters {
            waiter.cancel();
        }
    }
    pub fn cancel_owner(&self, owner: &str) {
        let waiters = self
            .shared
            .state
            .lock()
            .unwrap()
            .waiters
            .values()
            .filter_map(Weak::upgrade)
            .filter(|w| w.owner == owner)
            .collect::<Vec<_>>();
        for waiter in waiters {
            waiter.cancel();
        }
    }
    pub fn session_open(&self, environment: &str, id: &str) {
        self.shared
            .state
            .lock()
            .unwrap()
            .cache
            .open_session(id, environment);
    }
    pub fn session_closed(&self, id: &str) {
        let waiters = {
            let mut state = self.shared.state.lock().unwrap();
            state.cache.close_session(id);
            state
                .memo
                .iter()
                .filter(|(key, _)| key.session == id)
                .filter_map(|(_, job)| job.upgrade())
                .flat_map(|job| {
                    job.waiters
                        .lock()
                        .unwrap()
                        .iter()
                        .filter_map(Weak::upgrade)
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>()
        };
        for waiter in waiters {
            waiter.cancel();
        }
    }
    pub fn query(
        &self,
        environment: &str,
        paths: &[String],
        lease: Option<&str>,
        owner: &str,
    ) -> Result<Vec<Query>> {
        let normalized = paths
            .iter()
            .map(|p| admission::admitted(p).ok_or_else(|| "invalid_metadata_path".to_owned()))
            .collect::<Result<Vec<_>>>()?;
        let (connection, session) = crate::rpc_sessions::admitted(environment, false)?;
        authorize(&connection, "filesystem:read")?;
        if !crate::rpc_sessions::is_current(environment, false, session.id())? || !session.is_open()
        {
            return Err("metadata_scope_changed".into());
        }
        let mut state = self.shared.state.lock().unwrap();
        state.cache.open_session(session.id(), environment);
        let reference = match lease {
            Some(lease) => Some(
                state
                    .leases
                    .get(lease)
                    .filter(|r| r.environment == environment)
                    .cloned()
                    .ok_or("metadata_lease_closed")?,
            ),
            None => None,
        };
        let now = (self.shared.clock)();
        let mut queries = Vec::new();
        let mut jobs = Vec::new();
        for path in normalized {
            let id = self
                .next
                .try_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_add(1))
                .map_err(|_| "metadata_unavailable")?;
            let (tx, rx) = mpsc::sync_channel(1);
            let waiter = Arc::new(Waiter {
                id,
                owner: owner.into(),
                lease: lease.map(str::to_owned),
                cancelled: AtomicBool::new(false),
                done: AtomicBool::new(false),
                reply: tx,
                job: Mutex::new(Weak::new()),
            });
            if let Some(value) = state
                .cache
                .known(session.id(), reference.as_ref(), &path, now)
            {
                let revision = *state
                    .revisions
                    .get(&(environment.into(), path.clone()))
                    .unwrap_or(&0);
                waiter.done.store(true, Ordering::Release);
                let _ = waiter.reply.try_send(Ok(Answer {
                    environment: environment.into(),
                    session: session.id().into(),
                    path,
                    value,
                    revision,
                }));
            } else {
                let key = Key {
                    session: session.id().into(),
                    reference: reference.clone(),
                    path: path.clone(),
                };
                let existing = state
                    .memo
                    .get(&key)
                    .and_then(Weak::upgrade)
                    .filter(|job| job.active());
                let job = if let Some(job) = existing {
                    job
                } else {
                    let request = state
                        .cache
                        .begin(session.id(), reference.as_ref(), &path)
                        .ok_or("metadata_scope_changed")?;
                    let job = Arc::new(Job {
                        key: key.clone(),
                        environment: environment.into(),
                        session: session.clone(),
                        request: Mutex::new(Some(request)),
                        waiters: Mutex::new(Vec::new()),
                        cancelled: AtomicBool::new(false),
                        finished: AtomicBool::new(false),
                        batch: Mutex::new(Weak::new()),
                        shared: Arc::downgrade(&self.shared),
                    });
                    state.memo.insert(key, Arc::downgrade(&job));
                    jobs.push(job.clone());
                    job
                };
                *waiter.job.lock().unwrap() = Arc::downgrade(&job);
                job.waiters.lock().unwrap().push(Arc::downgrade(&waiter));
                state.waiters.insert(id, Arc::downgrade(&waiter));
            }
            queries.push(Query { waiter, reply: rx });
        }
        drop(state);
        if !jobs.is_empty() {
            if let Err(failed) = self.queue.send(jobs) {
                for job in failed.0 {
                    if let Some(request) = job.request.lock().unwrap().take() {
                        self.shared.state.lock().unwrap().cache.abandon(request);
                    }
                    job.finish(Err("metadata_unavailable".into()));
                }
                return Err("metadata_unavailable".into());
            }
        }
        Ok(queries)
    }
    pub fn invalidate(&self, environment: &str, paths: &[String]) -> Result<()> {
        let session = crate::rpc_sessions::existing(environment, false)?;
        let mut events = Vec::new();
        {
            let mut state = self.shared.state.lock().unwrap();
            if let Some(session) = session.as_ref() {
                state.cache.open_session(session.id(), environment);
            }
            let mut seen = std::collections::HashSet::new();
            for path in paths.iter().filter(|p| seen.insert((*p).clone())) {
                let path = model::normalize(path);
                if let Some(session) = session.as_ref() {
                    state.cache.invalidate(session.id(), &path);
                }
                let revision = state
                    .revisions
                    .entry((environment.into(), path.clone()))
                    .or_default();
                *revision = revision.saturating_add(1);
                events.push(json!({"op":"file-metadata-invalidated","environment":environment,"sessionId":session.as_ref().map(|s|s.id()),"path":path,"revision":*revision}));
            }
        }
        if let Some(sink) = self.shared.events.lock().unwrap().as_ref() {
            for event in events {
                let _ = sink.send(event);
            }
        }
        Ok(())
    }
    pub fn seeded_read(
        &self,
        environment: &str,
        function: &protocol::api::Function,
        args: &Value,
        cwd: &str,
        file: bool,
    ) -> Result<Value> {
        let tag = function.tag.ok_or("invalid_metadata_read")?;
        if !matches!(
            (tag, file),
            ("projects.readFile", true)
                | ("projects.searchEntries" | "projects.listEntries", false)
        ) {
            return Err("invalid_metadata_read".into());
        }
        let payload = function.payload(args)?;
        let (connection, session) = crate::rpc_sessions::admitted(environment, false)?;
        authorize(&connection, "filesystem:read")?;
        if !crate::rpc_sessions::is_current(environment, false, session.id())? {
            return Err("metadata_scope_changed".into());
        }
        let guard = {
            let mut state = self.shared.state.lock().unwrap();
            state.cache.open_session(session.id(), environment);
            state
                .cache
                .seed_guard(session.id())
                .ok_or("metadata_scope_changed")?
        };
        let result = session.rpc(tag, payload, function.timeout())?;
        let result = crate::file_metadata_wire::decode_project(&result, file)?;
        if session.is_open() && crate::rpc_sessions::is_current(environment, false, session.id())? {
            let mut state = self.shared.state.lock().unwrap();
            let now = (self.shared.clock)();
            if file {
                if result["exists"] != false {
                    if let (Some(path), Some(bytes)) = (
                        result["relativePath"].as_str(),
                        result["byteLength"].as_u64(),
                    ) {
                        state
                            .cache
                            .seed_file(&guard, &admission::workspace(path, cwd), bytes, now);
                    }
                }
            } else {
                let entries = result["entries"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|entry| {
                        Some((
                            admission::workspace(entry["path"].as_str()?, cwd),
                            entry["kind"].as_str()?.to_owned(),
                        ))
                    })
                    .collect::<Vec<_>>();
                state.cache.seed_entries(&guard, &entries, now);
            }
        }
        Ok(result)
    }
    pub fn set_events(&self, events: mpsc::Sender<Value>) {
        *self.shared.events.lock().unwrap() = Some(events);
    }
}
fn dispatch(shared: &Arc<Shared>, jobs: Vec<Arc<Job>>) {
    let mut live = Vec::new();
    let mut requests = Vec::new();
    for job in jobs {
        let request = job.request.lock().unwrap().take();
        if let Some(request) = request {
            if job.active() && job.session.is_open() {
                live.push(job);
                requests.push(request);
            } else {
                shared.state.lock().unwrap().cache.abandon(request);
                job.finish(Err("metadata_cancelled".into()));
            }
        }
    }
    let batches = model::batches(&requests);
    let mut requests = requests.into_iter().map(Some).collect::<Vec<_>>();
    for batch in batches {
        let pending = batch
            .requests
            .iter()
            .map(|index| (live[*index].clone(), requests[*index].take().unwrap()))
            .collect::<Vec<_>>();
        let worker = shared.clone();
        // Keep requests available if the OS cannot create a worker. Dropping a
        // closure must not leave cache request tokens pending indefinitely.
        let pending = Arc::new(Mutex::new(Some(pending)));
        let task = pending.clone();
        if std::thread::Builder::new()
            .name("o3-metadata-batch".into())
            .spawn(move || {
                let jobs = task.lock().unwrap().take().unwrap();
                resolve(&worker, jobs);
            })
            .is_err()
        {
            if let Some(jobs) = pending.lock().unwrap().take() {
                for (job, request) in jobs {
                    shared.state.lock().unwrap().cache.abandon(request);
                    job.finish(Err("metadata_unavailable".into()));
                }
            }
        }
    }
}
fn resolve(shared: &Arc<Shared>, mut jobs: Vec<(Arc<Job>, model::Request)>) {
    while !jobs.is_empty() {
        let mut live = Vec::new();
        for (job, request) in jobs {
            if job.active() && job.session.is_open() {
                live.push((job, request));
            } else {
                shared.state.lock().unwrap().cache.abandon(request);
                job.finish(Err("metadata_cancelled".into()));
            }
        }
        jobs = live;
        if jobs.is_empty() {
            break;
        }
        let session = jobs[0].0.session.clone();
        let mut paths = Vec::new();
        for (job, _) in &jobs {
            if !paths.contains(&job.key.path) {
                paths.push(job.key.path.clone());
            }
        }
        let function = &protocol::api::filesystem::GET_METADATA;
        let result = session
            .start(
                function.tag.unwrap(),
                json!({"paths":paths}),
                function.timeout(),
            )
            .and_then(|pending| {
                let cancellation = Arc::new(BatchCancellation {
                    jobs: jobs.iter().map(|(job, _)| Arc::downgrade(job)).collect(),
                    cancel: pending.cancellation(),
                });
                for (job, _) in &jobs {
                    *job.batch.lock().unwrap() = Arc::downgrade(&cancellation);
                }
                cancellation.cancel_if_unused();
                pending.wait()
            })
            .and_then(|value| decode(&value));
        let current = crate::rpc_sessions::is_current(&jobs[0].0.environment, false, session.id())
            .unwrap_or(false);
        let mut retry = Vec::new();
        for (job, request) in jobs {
            if !job.active() || !session.is_open() || !current {
                shared.state.lock().unwrap().cache.abandon(request);
                job.finish(Err("metadata_cancelled".into()));
                continue;
            }
            match &result {
                Err(_) => {
                    shared.state.lock().unwrap().cache.abandon(request);
                    job.finish(Err("metadata_unavailable".into()));
                }
                Ok(values) => {
                    let index = paths.iter().position(|p| p == &job.key.path).unwrap();
                    let value = values.get(index).cloned().unwrap_or(None);
                    let mut state = shared.state.lock().unwrap();
                    let completion = state
                        .cache
                        .complete(request, value.clone(), (shared.clock)());
                    match completion {
                        Completion::Remembered => {
                            let environment = job.environment.clone();
                            let revision = *state
                                .revisions
                                .get(&(environment.clone(), job.key.path.clone()))
                                .unwrap_or(&0);
                            let answer = Answer {
                                environment,
                                session: job.key.session.clone(),
                                path: job.key.path.clone(),
                                value,
                                revision,
                            };
                            job.finish_locked(&mut state, Ok(answer));
                        }
                        Completion::Retry => {
                            if let Some(request) = state.cache.begin(
                                &job.key.session,
                                job.key.reference.as_ref(),
                                &job.key.path,
                            ) {
                                retry.push((job, request));
                            } else {
                                drop(state);
                                job.finish(Err("metadata_scope_changed".into()));
                            }
                        }
                        Completion::SessionClosed => {
                            drop(state);
                            job.finish(Err("metadata_scope_changed".into()));
                        }
                    }
                }
            }
        }
        jobs = retry;
    }
}

use crate::file_metadata_wire::decode;

pub fn authorize(connection: &protocol::Connection, scope: &str) -> Result<()> {
    let value = connection
        .request_json("GET", "/api/auth/session", None, false)
        .map_err(|_| "metadata_unavailable")?;
    if value["authenticated"] == true
        && value
            .get("permissions")
            .or_else(|| value.get("scopes"))
            .and_then(Value::as_array)
            .is_some_and(|scopes| scopes.iter().any(|s| s == scope))
    {
        Ok(())
    } else {
        Err("permission_denied".into())
    }
}
static RUNTIME: OnceLock<Runtime> = OnceLock::new();
static EVENTS: Mutex<Option<mpsc::Sender<Value>>> = Mutex::new(None);
pub fn global() -> &'static Runtime {
    RUNTIME.get_or_init(|| {
        let runtime = Runtime::new(Arc::new(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
                .min(u64::MAX as u128) as u64
        }));
        if let Some(events) = EVENTS.lock().unwrap().clone() {
            runtime.set_events(events);
        }
        runtime
    })
}
pub fn set_events(events: mpsc::Sender<Value>) {
    *EVENTS.lock().unwrap() = Some(events.clone());
    if let Some(runtime) = RUNTIME.get() {
        runtime.set_events(events);
    }
}
pub fn session_event(environment: &str, write: bool, id: &str, state: &str) {
    if write {
        return;
    }
    if let Some(runtime) = RUNTIME.get() {
        match state {
            "open" => runtime.session_open(environment, id),
            "closed" => runtime.session_closed(id),
            _ => {}
        }
    }
}

/// Handle lease/cancellation controls on the input thread before spawning any
/// query worker. Release remains valid after an environment has been removed.
pub fn control(input: &Value) -> Option<Result<Value>> {
    let op = crate::text(input, "op");
    if !matches!(
        op,
        "file-metadata-retain"
            | "file-metadata-release"
            | "file-metadata-cancel"
            | "file-metadata-invalidate"
    ) {
        return None;
    }
    let runtime = global();
    match op {
        "file-metadata-retain" => Some(
            runtime
                .retain(
                    crate::text(input, "leaseId"),
                    ThreadRef {
                        environment: crate::text(input, "environment").into(),
                        thread: crate::text(input, "thread").into(),
                    },
                )
                .map(|()| json!({"retained":true})),
        ),
        "file-metadata-release" => {
            runtime.release(crate::text(input, "leaseId"));
            Some(Ok(json!({"released":true})))
        }
        "file-metadata-cancel" => {
            runtime.cancel_owner(crate::text(input, "owner"));
            Some(Ok(json!({"cancelled":true})))
        }
        "file-metadata-invalidate" => Some((|| {
            let paths = input["paths"]
                .as_array()
                .ok_or("invalid_metadata_paths")?
                .iter()
                .map(|value| {
                    value
                        .as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| "invalid_metadata_paths".to_owned())
                })
                .collect::<Result<Vec<_>>>()?;
            runtime.invalidate(crate::text(input, "environment"), &paths)?;
            Ok(json!({"invalidated":true}))
        })()),
        _ => None,
    }
}
pub fn query_command(input: &Value) -> Result<Value> {
    let paths = input["paths"]
        .as_array()
        .ok_or("invalid_metadata_paths")?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| "invalid_metadata_paths".to_owned())
        })
        .collect::<Result<Vec<_>>>()?;
    let queries = global().query(
        crate::text(input, "environment"),
        &paths,
        input["leaseId"].as_str().filter(|value| !value.is_empty()),
        crate::text(input, "owner"),
    )?;
    let entries = queries
        .into_iter()
        .map(|query| query.wait().map(|answer| answer.json()))
        .collect::<Result<Vec<_>>>()?;
    Ok(json!({"entries":entries}))
}
