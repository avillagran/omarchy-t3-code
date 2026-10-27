//! Return unused allocator pages after transient snapshots. Never evicts live
//! conversation state or treats allocator RSS as the size of live data.
#[cfg(all(target_os = "linux", target_env = "gnu"))]
pub fn release_unused() {
    unsafe {
        libc::malloc_trim(0);
    }
}
#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
pub fn release_unused() {}

pub struct Sweep;
impl Drop for Sweep {
    fn drop(&mut self) {
        release_unused();
    }
}
