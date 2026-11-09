//! Admission for actual guest/inspector instances. These IDs describe owned
//! process/object lifetimes; they are not credentials, thread epochs or RPC IDs.
use std::collections::HashMap;

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Source {
    pub environment: String,
    pub profile: String,
    /// The existing browser_guest Session ID allocated for the spawned child.
    pub guest: String,
}
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Inspector {
    pub source: Source,
    pub thread: String,
    pub tab: String,
    /// Allocated for the actual trusted inspector object, before async probing.
    pub instance: String,
}
type GuestKey = (String, String);
type Key = (String, String, String, String);
fn guest_key(source: &Source) -> GuestKey {
    (source.environment.clone(), source.profile.clone())
}
fn key(inspector: &Inspector) -> Key {
    (
        inspector.source.environment.clone(),
        inspector.source.profile.clone(),
        inspector.thread.clone(),
        inspector.tab.clone(),
    )
}

pub struct TargetProof {
    inspector: Inspector,
    target: String,
}
impl TargetProof {
    /// The producer supplies WebEngineView.devToolsId and the actual response to
    /// its private Target.getTargetInfo call on the same captured inspector.
    pub fn verify(
        inspector: Inspector,
        view_target: &str,
        cdp_target: &str,
    ) -> Result<Self, &'static str> {
        if view_target.is_empty() || view_target != cdp_target {
            return Err("browser_target_mismatch");
        }
        Ok(Self {
            inspector,
            target: cdp_target.into(),
        })
    }
}
#[derive(Clone, PartialEq, Eq)]
pub struct Lease {
    inspector: Inspector,
    target: String,
    id: String,
}
impl Lease {
    pub fn inspector(&self) -> &Inspector {
        &self.inspector
    }
    pub fn target(&self) -> &str {
        &self.target
    }
    pub fn id(&self) -> &str {
        &self.id
    }
}
#[derive(Default)]
pub struct Registry {
    guests: HashMap<GuestKey, String>,
    inspectors: HashMap<Key, Inspector>,
    leases: HashMap<Key, Lease>,
}
impl Registry {
    /// Called from the actual spawn/replacement owner, never from a page event.
    /// Return retired work for cancellation outside the registry/global lock.
    pub fn register_guest(&mut self, source: &Source) -> Vec<Lease> {
        let scope = guest_key(source);
        if self.guests.get(&scope) == Some(&source.guest) {
            return vec![];
        }
        let retired = self.retire_profile(&scope);
        self.guests.insert(scope, source.guest.clone());
        retired
    }
    pub fn expect_inspector(
        &mut self,
        inspector: &Inspector,
    ) -> Result<Option<Lease>, &'static str> {
        if !self.source_is_current(&inspector.source) {
            return Err("browser_guest_replaced");
        }
        let key = key(inspector);
        if self.inspectors.get(&key) == Some(inspector) {
            return Ok(None);
        }
        self.inspectors.insert(key.clone(), inspector.clone());
        Ok(self.leases.remove(&key))
    }
    pub fn admit(&mut self, proof: TargetProof) -> Result<Lease, &'static str> {
        let key = key(&proof.inspector);
        if !self.source_is_current(&proof.inspector.source)
            || self.inspectors.get(&key) != Some(&proof.inspector)
        {
            return Err("browser_inspector_replaced");
        }
        if self.leases.iter().any(|(other, lease)| {
            other != &key
                && lease.inspector.source == proof.inspector.source
                && lease.target == proof.target
        }) {
            return Err("browser_target_already_owned");
        }
        if let Some(current) = self.leases.get(&key) {
            // An attachment instance cannot silently move to another physical
            // target. Retargeting must retire and replace the inspector first.
            if current.inspector == proof.inspector {
                if current.target != proof.target {
                    return Err("browser_target_mismatch");
                }
                return Ok(current.clone());
            }
        }
        let lease = Lease {
            inspector: proof.inspector,
            target: proof.target,
            id: uuid::Uuid::new_v4().to_string(),
        };
        self.leases.insert(key, lease.clone());
        Ok(lease)
    }
    pub fn accepts(&self, lease: &Lease) -> bool {
        self.source_is_current(&lease.inspector.source)
            && self.inspectors.get(&key(&lease.inspector)) == Some(&lease.inspector)
            && self.leases.get(&key(&lease.inspector)) == Some(lease)
    }
    pub fn current(&self, source: &Source, thread: &str, tab: &str) -> Option<Lease> {
        if !self.source_is_current(source) {
            return None;
        }
        self.leases
            .get(&(
                source.environment.clone(),
                source.profile.clone(),
                thread.into(),
                tab.into(),
            ))
            .filter(|lease| self.accepts(lease))
            .cloned()
    }
    /// A late close/error from an older inspector must not retire its successor.
    pub fn retire_inspector(&mut self, inspector: &Inspector) -> Option<Lease> {
        let key = key(inspector);
        if self.inspectors.get(&key) != Some(inspector) {
            return None;
        }
        self.inspectors.remove(&key);
        self.leases.remove(&key)
    }
    pub fn retire_lease(&mut self, lease: &Lease) -> Option<Lease> {
        if !self.accepts(lease) {
            return None;
        }
        self.inspectors.remove(&key(&lease.inspector));
        self.leases.remove(&key(&lease.inspector))
    }
    pub fn retire_guest(&mut self, source: &Source) -> Vec<Lease> {
        if !self.source_is_current(source) {
            return vec![];
        }
        self.retire_profile(&guest_key(source))
    }
    fn source_is_current(&self, source: &Source) -> bool {
        self.guests.get(&guest_key(source)) == Some(&source.guest)
    }
    fn retire_profile(&mut self, scope: &GuestKey) -> Vec<Lease> {
        self.guests.remove(scope);
        self.inspectors
            .retain(|_, value| guest_key(&value.source) != *scope);
        let keys = self
            .leases
            .iter()
            .filter(|(_, value)| guest_key(&value.inspector.source) == *scope)
            .map(|(key, _)| key.clone())
            .collect::<Vec<_>>();
        keys.into_iter()
            .filter_map(|key| self.leases.remove(&key))
            .collect()
    }
}
