//! Shared profile reservation boundary between cleanup/import orchestration and the
//! local Qt owner. Only confirmed, preflighted profile actions acquire a lease.
use crate::Result;
use std::path::Path;
#[derive(Clone)]
pub struct Scope {
    pub environment: String,
    pub origin_key: String,
}
pub trait Lease: Send {
    fn clear(&mut self, environment: &str) -> Result<()>;
    /// Provisional store receipt. Final success requires flush_import.
    fn write_cookie(
        &mut self,
        _environment: &str,
        _cookie: crate::browser_cookies::ImportedCookie,
    ) -> Result<bool> {
        Err("browser_profile_runtime_unavailable".into())
    }
    /// Flush and verify persisted metadata while retaining the exclusive lease.
    fn flush_import(&mut self, _environment: &str) -> Result<()> {
        Err("browser_profile_runtime_unavailable".into())
    }
}
pub trait Runtime: Send + Sync {
    fn available(&self) -> bool;
    fn can_import(&self) -> bool {
        self.available()
    }
    fn begin(&self, profile: &str, client_store: &Path, scopes: &[Scope])
        -> Result<Box<dyn Lease>>;
}
