//! Reconnection policy shared by the shell and chat streams, after T3's connection
//! supervisor (client-runtime `connection/supervisor.ts`, `wakeups.ts`):
//!
//! - retries back off from 1 s to 300 s with jitter between half the ceiling and
//!   the ceiling, and the backoff resets once a connection stayed up for 30 s;
//! - a wakeup (resume from suspend, connectivity change) is broadcast through
//!   [`Wakeups`]: a waiting stream resets its backoff and reconnects after a quick
//!   probe, a connected one checks its socket at once.
//!
//! Detecting resume and network changes is the host's job (D-Bus on Linux,
//! `ConnectivityManager` on Android); it reports them with [`notify`].
use std::{
    sync::{Condvar, Mutex},
    time::{Duration, Instant},
};

pub const BASE_DELAY: Duration = Duration::from_secs(1);
pub const MAX_DELAY: Duration = Duration::from_secs(300);
pub const STABLE_AFTER: Duration = Duration::from_secs(30);
/// Budget for the liveness check of a connected socket after a wakeup.
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(3);

/// Transport failures leave a held snapshot valid, so the next attempt resumes from
/// its sequence. Anything else (the server ending the stream, auth, malformed data)
/// starts over from a fresh snapshot.
pub fn transient(error: &str) -> bool {
    matches!(
        crate::error_code(error),
        "disconnected" | "stream_timeout" | "unreachable" | "rpc_connection_failed" | "rpc_outcome_unknown"
    )
}

pub struct Backoff {
    failures: u32,
    connected_at: Option<Instant>,
    seed: u64,
}
impl Default for Backoff {
    fn default() -> Self {
        let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1);
        Self { failures: 0, connected_at: None, seed: seed | 1 }
    }
}
impl Backoff {
    /// The stream is up; a failure 30 s or more from now starts again at the base delay.
    pub fn connected(&mut self) {
        self.connected_at.get_or_insert_with(Instant::now);
    }
    pub fn reset(&mut self) {
        self.failures = 0;
        self.connected_at = None;
    }
    fn random(&mut self) -> f64 {
        // xorshift64: enough to spread reconnects of several streams apart.
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        (self.seed >> 11) as f64 / (1u64 << 53) as f64
    }
    pub fn ceiling(failures: u32) -> Duration {
        BASE_DELAY.saturating_mul(2u32.saturating_pow(failures.saturating_add(1).min(20))).min(MAX_DELAY)
    }
    /// Delay before the next attempt after a failure.
    pub fn next_delay(&mut self) -> Duration {
        if self.connected_at.take().is_some_and(|at| at.elapsed() >= STABLE_AFTER) {
            self.failures = 0;
        }
        let ceiling = Self::ceiling(self.failures);
        self.failures = self.failures.saturating_add(1);
        ceiling / 2 + ceiling.mul_f64(self.random() / 2.0)
    }
}

/// A wakeup counter streams wait on. Hosts normally share [`WAKEUPS`].
pub struct Wakeups {
    count: Mutex<u64>,
    signal: Condvar,
}
impl Default for Wakeups {
    fn default() -> Self {
        Self::new()
    }
}
impl Wakeups {
    pub const fn new() -> Self {
        Self { count: Mutex::new(0), signal: Condvar::new() }
    }
    /// Wakes every [`Waiter`] following this counter.
    pub fn notify(&self) {
        *self.count.lock().unwrap_or_else(|e| e.into_inner()) += 1;
        self.signal.notify_all();
    }
    fn count(&self) -> u64 {
        *self.count.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// The process-wide wakeups that `Waiter::default()` follows.
pub static WAKEUPS: Wakeups = Wakeups::new();

/// Wakes every stream (resume, network change). Logging is the host's.
pub fn notify() {
    WAKEUPS.notify();
}

/// One stream's view of wakeups.
pub struct Waiter {
    wakeups: &'static Wakeups,
    seen: u64,
}
impl Default for Waiter {
    /// Follows the process-wide [`WAKEUPS`].
    fn default() -> Self {
        Self::new(&WAKEUPS)
    }
}
impl Waiter {
    pub fn new(wakeups: &'static Wakeups) -> Self {
        Self { wakeups, seen: wakeups.count() }
    }
    /// True once per wakeup that happened since the last call.
    pub fn take(&mut self) -> bool {
        let now = self.wakeups.count();
        let woke = now != self.seen;
        self.seen = now;
        woke
    }
    /// Sleeps up to `delay` or until a wakeup (cut short by `stop`); true when woken.
    pub fn sleep(&mut self, delay: Duration, stop: impl Fn() -> bool) -> bool {
        let until = Instant::now() + delay;
        let mut guard = self.wakeups.count.lock().unwrap_or_else(|e| e.into_inner());
        while *guard == self.seen && !stop() {
            let Some(left) = until.checked_duration_since(Instant::now()) else { return false };
            guard = self.wakeups.signal.wait_timeout(guard, left.min(Duration::from_millis(250))).unwrap_or_else(|e| e.into_inner()).0;
        }
        let woke = *guard != self.seen;
        self.seen = *guard;
        woke
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_transport_failures_keep_the_cursor() {
        for e in ["disconnected", "stream_timeout", "unreachable", "rpc_connection_failed: tls handshake eof"] {
            assert!(transient(e), "{e}");
        }
        for e in ["stream_closed", "unauthorized", "malformed_stream", "unknown_environment", "rpc_rejected: no"] {
            assert!(!transient(e), "{e}");
        }
    }
    #[test]
    fn backoff_doubles_with_jitter_up_to_five_minutes() {
        let mut b = Backoff::default();
        for failures in 0..12 {
            let ceiling = Backoff::ceiling(failures);
            let d = b.next_delay();
            assert!(d >= ceiling / 2 && d <= ceiling, "{failures}: {d:?} outside {ceiling:?}");
        }
        assert_eq!(Backoff::ceiling(0), Duration::from_secs(2));
        assert_eq!(Backoff::ceiling(3), Duration::from_secs(16));
        assert_eq!(Backoff::ceiling(11), MAX_DELAY);
        assert_eq!(Backoff::ceiling(u32::MAX), MAX_DELAY);
    }
    #[test]
    fn backoff_resets_only_after_a_stable_connection() {
        let mut b = Backoff::default();
        for _ in 0..5 {
            b.next_delay();
        }
        b.connected();
        assert!(b.next_delay() >= Backoff::ceiling(5) / 2, "a short-lived connection keeps backing off");
        b.connected_at = Some(Instant::now() - STABLE_AFTER);
        assert!(b.next_delay() <= Backoff::ceiling(0), "30 s up starts over");
        b.reset();
        assert!(b.next_delay() <= Backoff::ceiling(0));
    }
    #[test]
    fn waiter_sees_each_wakeup_once() {
        let mut w = Waiter::default();
        assert!(!w.take());
        assert!(!w.sleep(Duration::from_millis(20), || false));
        notify();
        assert!(w.sleep(Duration::from_secs(5), || false));
        assert!(!w.take());
        notify();
        assert!(w.take());
    }
}
