//! Worktree setup progress of the open thread (subscribeWorktreeSetup): fetch, checkout,
//! submodules and setup script stages while a new worktree is being prepared.
use omarchy_t3_client::*;
use serde_json::json;
use std::sync::{atomic::{AtomicU64, Ordering}, Arc};

#[derive(Clone, Default)]
pub struct Watch {
    generation: Arc<AtomicU64>,
}
impl Watch {
    pub fn stop(&self) {
        self.generation.fetch_add(1, Ordering::Relaxed);
    }
    pub fn start(&self, env: String, thread: String) {
        let generation = self.generation.fetch_add(1, Ordering::Relaxed) + 1;
        let current = self.generation.clone();
        std::thread::spawn(move || {
            let Ok(e) = environments().and_then(|all| all.into_iter().find(|e| e.id == env).ok_or("unknown_environment".into())) else { return };
            let Ok(c) = integration_devices::connection(&e) else { return };
            let cancel = || current.load(Ordering::Relaxed) != generation;
            let _ = c.subscribe("subscribeWorktreeSetup", json!({"threadId":thread}), cancel, |item| {
                if let Some(snapshot) = item {
                    if current.load(Ordering::Relaxed) == generation {
                        crate::emit("worktree-setup", json!({"environment":env,"thread":thread,"snapshot":snapshot}));
                    }
                }
                Ok(())
            });
        });
    }
}
