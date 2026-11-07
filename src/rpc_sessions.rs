//! Physical unary RPC session owners. Private route/auth admission determines
//! replacement; only the SDK's random socket ID is a cache lifecycle identity.
use crate::*;
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc, Mutex, OnceLock,
};

type Key = (String, bool);
struct Entry {
    connection: protocol::Connection,
    session: protocol::RpcSession,
}
#[derive(Default)]
struct Slot {
    retired: AtomicBool,
    entry: Mutex<Option<Entry>>,
}
impl Slot {
    fn retire(&self) {
        self.retired.store(true, Ordering::Release);
        // An opening handshake may hold the slot. Its post-connect admission
        // check closes the new socket instead of publishing a retired lease.
        if let Ok(mut entry) = self.entry.try_lock() {
            if let Some(entry) = entry.take() {
                entry.session.close()
            }
        }
    }
}
#[derive(Default)]
pub struct Owners {
    slots: Mutex<HashMap<Key, Arc<Slot>>>,
}
impl Drop for Owners {
    fn drop(&mut self) {
        if let Ok(slots) = self.slots.get_mut() {
            for slot in slots.values() {
                slot.retire()
            }
        }
    }
}
fn current(environment: &str, write: bool) -> Result<protocol::Connection> {
    environments()?
        .into_iter()
        .find(|e| e.id == environment)
        .ok_or("unknown_environment")?
        .connection(write)
}
impl Owners {
    /// Obtain a current admitted socket. Opening sockets are serialized only
    /// within one environment/access mode; unrelated environments stay free.
    pub fn session(&self, environment: &str, write: bool) -> Result<protocol::RpcSession> {
        self.admitted(environment, write)
            .map(|(_, session)| session)
    }
    pub(crate) fn admitted(
        &self,
        environment: &str,
        write: bool,
    ) -> Result<(protocol::Connection, protocol::RpcSession)> {
        let connection = match current(environment, write) {
            Ok(c) => c,
            Err(error) => {
                self.close_environment(environment);
                return Err(error);
            }
        };
        let key = (environment.to_owned(), write);
        let slot = {
            let mut slots = self.slots.lock().map_err(|_| "local_error")?;
            slots.entry(key).or_default().clone()
        };
        let mut entry = slot.entry.lock().map_err(|_| "local_error")?;
        if slot.retired.load(Ordering::Acquire) {
            return Err("connection_changed".into());
        }
        if let Some(old) = entry.as_ref() {
            if old.session.is_open() && old.connection.same_session_admission(&connection) {
                return Ok((old.connection.clone(), old.session.clone()));
            }
        }
        if let Some(old) = entry.take() {
            old.session.close()
        }
        let session = connection.rpc_session(protocol::SessionOptions::default())?;
        // A route/account update during the handshake must not resurrect its
        // previous socket, even if the server accepted that old admission.
        let admitted = current(environment, write);
        if slot.retired.load(Ordering::Acquire)
            || !admitted
                .as_ref()
                .is_ok_and(|c| connection.same_session_admission(c))
        {
            session.close();
            return Err("connection_changed".into());
        }
        *entry = Some(Entry {
            connection: connection.clone(),
            session: session.clone(),
        });
        publish(environment, write, session.id(), "open");
        let closed = session.closed();
        let observed_environment = environment.to_owned();
        let id = session.id().to_owned();
        let observed_id = id.clone();
        std::thread::Builder::new()
            .name("o3-rpc-lifecycle".into())
            .spawn(move || {
                let _ = closed.wait();
                publish(&observed_environment, write, &observed_id, "closed");
            })
            .map_err(|_| {
                session.close();
                publish(environment, write, &id, "closed");
                "local_error"
            })?;
        Ok((connection, session))
    }
    pub(crate) fn existing(
        &self,
        environment: &str,
        write: bool,
    ) -> Result<Option<protocol::RpcSession>> {
        let wanted = match current(environment, write) {
            Ok(c) => c,
            Err(_) => return Ok(None),
        };
        let slot = self
            .slots
            .lock()
            .map_err(|_| "local_error")?
            .get(&(environment.into(), write))
            .cloned();
        let Some(slot) = slot else { return Ok(None) };
        let entry = slot.entry.lock().map_err(|_| "local_error")?;
        Ok(entry
            .as_ref()
            .filter(|old| {
                !slot.retired.load(Ordering::Acquire)
                    && old.session.is_open()
                    && old.connection.same_session_admission(&wanted)
            })
            .map(|old| old.session.clone()))
    }
    /// Reconcile only actual changed/removed private admissions. Opening slots
    /// are not blocked: their final current-admission check rejects stale opens.
    pub fn reconcile(&self, environments: &[Environment]) {
        let slots = {
            let Ok(slots) = self.slots.lock() else { return };
            slots
                .iter()
                .map(|(key, slot)| (key.clone(), slot.clone()))
                .collect::<Vec<_>>()
        };
        for (key, slot) in slots {
            let current = environments
                .iter()
                .find(|e| e.id == key.0)
                .map(|e| e.connection(key.1));
            let Ok(mut entry) = slot.entry.try_lock() else {
                continue;
            };
            let changed = match (entry.as_ref(), current.as_ref()) {
                (Some(old), Some(Ok(connection))) => {
                    !old.connection.same_session_admission(connection)
                }
                (_, Some(Ok(_))) => false,
                _ => true,
            };
            if !changed {
                continue;
            }
            slot.retired.store(true, Ordering::Release);
            if let Some(old) = entry.take() {
                old.session.close()
            }
            drop(entry);
            if let Ok(mut slots) = self.slots.lock() {
                if slots
                    .get(&key)
                    .is_some_and(|current| Arc::ptr_eq(current, &slot))
                {
                    slots.remove(&key);
                }
            }
        }
    }
    pub fn close_environment(&self, environment: &str) {
        let retired = {
            let Ok(mut slots) = self.slots.lock() else {
                return;
            };
            let keys = slots
                .keys()
                .filter(|(id, _)| id == environment)
                .cloned()
                .collect::<Vec<_>>();
            keys.into_iter()
                .filter_map(|key| slots.remove(&key))
                .collect::<Vec<_>>()
        };
        for slot in retired {
            slot.retire()
        }
    }
    pub fn close_all(&self) {
        let retired = {
            let Ok(mut slots) = self.slots.lock() else {
                return;
            };
            slots.drain().map(|(_, slot)| slot).collect::<Vec<_>>()
        };
        for slot in retired {
            slot.retire()
        }
    }
}
static OWNERS: OnceLock<Owners> = OnceLock::new();
static EVENTS: Mutex<Option<mpsc::Sender<Value>>> = Mutex::new(None);
fn publish(environment: &str, write: bool, id: &str, state: &str) {
    crate::file_metadata_runtime::session_event(environment, write, id, state);
    if let Ok(events) = EVENTS.lock() {
        if let Some(events) = events.as_ref() {
            let _=events.send(json!({"op":"rpc-session-state","environment":environment,"write":write,"sessionId":id,"state":state}));
        }
    }
}
/// Register the watch process event sink before acquiring its first session.
pub fn set_events(events: mpsc::Sender<Value>) {
    if let Ok(mut sink) = EVENTS.lock() {
        *sink = Some(events)
    }
}
pub fn session(environment: &str, write: bool) -> Result<protocol::RpcSession> {
    OWNERS
        .get_or_init(Owners::default)
        .session(environment, write)
}
pub fn close_environment(environment: &str) {
    if let Some(owners) = OWNERS.get() {
        owners.close_environment(environment)
    }
}
pub fn close_all() {
    if let Some(owners) = OWNERS.get() {
        owners.close_all()
    }
}

pub fn reconcile(environments: &[Environment]) {
    if let Some(owners) = OWNERS.get() {
        owners.reconcile(environments)
    }
}

pub(crate) fn admitted(
    environment: &str,
    write: bool,
) -> Result<(protocol::Connection, protocol::RpcSession)> {
    OWNERS
        .get_or_init(Owners::default)
        .admitted(environment, write)
}
pub(crate) fn existing(environment: &str, write: bool) -> Result<Option<protocol::RpcSession>> {
    match OWNERS.get() {
        Some(owners) => owners.existing(environment, write),
        None => Ok(None),
    }
}
pub(crate) fn is_current(environment: &str, write: bool, id: &str) -> Result<bool> {
    Ok(existing(environment, write)?.is_some_and(|s| s.id() == id))
}
