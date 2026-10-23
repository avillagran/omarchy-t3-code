//! Reconnection policy shared by the shell and chat streams, after T3's connection
//! supervisor (client-runtime `connection/supervisor.ts`, `wakeups.ts`):
//!
//! - retries back off from 1 s to 300 s with jitter between half the ceiling and
//!   the ceiling, and the backoff resets once a connection stayed up for 30 s;
//! - resuming from suspend (logind `PrepareForSleep(false)`) or a NetworkManager
//!   connectivity change wakes every stream: a waiting one resets its backoff and
//!   reconnects after a quick probe, a connected one checks its socket at once.
//!
//! D-Bus signals are read with `gdbus monitor` (glib2, part of every Omarchy
//! install) rather than a new D-Bus crate. Without it, resume is detected from the
//! gap between the boot-time and monotonic clocks.
use crate::protocol;
use std::{
    io::{BufRead, BufReader},
    os::unix::process::CommandExt,
    process::{Command, Stdio},
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
        protocol::error_code(error),
        "disconnected" | "stream_timeout" | "unreachable" | "rpc_connection_failed" | "rpc_outcome_unknown"
    )
}

/// Quick check that a server answers before a wakeup reconnects to it.
pub fn reachable(e: &crate::Environment) -> bool {
    crate::Client::new().and_then(|c| c.get(e, "/.well-known/t3/environment", false)).is_ok()
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

static WAKE: Mutex<u64> = Mutex::new(0);
static WAKE_SIGNAL: Condvar = Condvar::new();

/// Wakes every stream (resume, network change).
pub fn wake(reason: &str) {
    *WAKE.lock().unwrap_or_else(|e| e.into_inner()) += 1;
    WAKE_SIGNAL.notify_all();
    eprintln!("ot3: reconnecting streams after {reason}");
}
fn wake_count() -> u64 {
    *WAKE.lock().unwrap_or_else(|e| e.into_inner())
}

/// One stream's view of wakeups.
pub struct Waiter {
    seen: u64,
}
impl Default for Waiter {
    fn default() -> Self {
        Self { seen: wake_count() }
    }
}
impl Waiter {
    /// True once per wakeup that happened since the last call.
    pub fn take(&mut self) -> bool {
        let now = wake_count();
        let woke = now != self.seen;
        self.seen = now;
        woke
    }
    /// Sleeps up to `delay` or until a wakeup (cut short by `stop`); true when woken.
    pub fn sleep(&mut self, delay: Duration, stop: impl Fn() -> bool) -> bool {
        let until = Instant::now() + delay;
        let mut guard = WAKE.lock().unwrap_or_else(|e| e.into_inner());
        while *guard == self.seen && !stop() {
            let Some(left) = until.checked_duration_since(Instant::now()) else { return false };
            guard = WAKE_SIGNAL.wait_timeout(guard, left.min(Duration::from_millis(250))).unwrap_or_else(|e| e.into_inner()).0;
        }
        let woke = *guard != self.seen;
        self.seen = *guard;
        woke
    }
}

/// What a `gdbus monitor` line means for the streams.
#[derive(Debug, PartialEq)]
pub enum Signal {
    Resumed,
    Connectivity(String),
    PrimaryConnection(String),
}
fn after<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let rest = &line[line.find(key)? + key.len()..];
    Some(rest[..rest.find(|c| c == ',' || c == '}').unwrap_or(rest.len())].trim())
}
pub fn parse_signal(line: &str) -> Option<Signal> {
    if line.contains("org.freedesktop.login1.Manager.PrepareForSleep") {
        return line.contains("(false,)").then_some(Signal::Resumed);
    }
    if line.contains("org.freedesktop.DBus.Properties.PropertiesChanged") && line.contains("'org.freedesktop.NetworkManager'") {
        if let Some(value) = after(line, "'Connectivity': ") {
            return Some(Signal::Connectivity(value.to_string()));
        }
        if let Some(value) = after(line, "'PrimaryConnection': ") {
            return Some(Signal::PrimaryConnection(value.to_string()));
        }
    }
    None
}

fn monitor(args: &[&str], mut on_line: impl FnMut(&str)) -> bool {
    let child = unsafe {
        Command::new("gdbus")
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            // The monitor must not outlive this client.
            .pre_exec(|| {
                libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
                Ok(())
            })
            .spawn()
    };
    let Ok(mut child) = child else { return false };
    if let Some(out) = child.stdout.take() {
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            on_line(&line);
        }
    }
    let _ = child.wait();
    true
}

/// Resume detection without D-Bus: the boot-time clock keeps counting during suspend,
/// the monotonic clock does not.
fn clock_gap_watch() {
    let read = |clock| {
        let mut t = libc::timespec { tv_sec: 0, tv_nsec: 0 };
        unsafe { libc::clock_gettime(clock, &mut t) };
        Duration::new(t.tv_sec as u64, t.tv_nsec as u32)
    };
    let gap = || read(libc::CLOCK_BOOTTIME).saturating_sub(read(libc::CLOCK_MONOTONIC));
    let mut last = gap();
    loop {
        std::thread::sleep(Duration::from_secs(5));
        let now = gap();
        if now.saturating_sub(last) > Duration::from_secs(5) {
            wake("resume");
        }
        last = now;
    }
}

/// Starts the logind and NetworkManager watchers; `on_wake` runs after each wakeup.
pub fn watch_system(on_wake: impl Fn() + Send + Sync + 'static) {
    let on_wake = std::sync::Arc::new(on_wake);
    let resume = on_wake.clone();
    std::thread::spawn(move || {
        let args = ["monitor", "--system", "--dest", "org.freedesktop.login1", "--object-path", "/org/freedesktop/login1"];
        loop {
            let started = monitor(&args, |line| {
                if parse_signal(line) == Some(Signal::Resumed) {
                    wake("resume");
                    resume();
                }
            });
            if !started {
                return clock_gap_watch();
            }
            // logind or the bus restarted; watch again.
            std::thread::sleep(Duration::from_secs(30));
        }
    });
    std::thread::spawn(move || {
        let args = ["monitor", "--system", "--dest", "org.freedesktop.NetworkManager", "--object-path", "/org/freedesktop/NetworkManager"];
        let (mut connectivity, mut primary) = (None::<String>, None::<String>);
        loop {
            let started = monitor(&args, |line| {
                // NetworkManager only signals a property when it changes; the guard
                // drops repeats from a restarted monitor.
                let changed = match parse_signal(line) {
                    Some(Signal::Connectivity(v)) => connectivity.replace(v.clone()) != Some(v),
                    Some(Signal::PrimaryConnection(v)) => primary.replace(v.clone()) != Some(v),
                    _ => false,
                };
                if changed {
                    wake("network change");
                    on_wake();
                }
            });
            if !started {
                return;
            }
            std::thread::sleep(Duration::from_secs(30));
        }
    });
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
        wake("test");
        assert!(w.sleep(Duration::from_secs(5), || false));
        assert!(!w.take());
        wake("test");
        assert!(w.take());
    }
    #[test]
    fn gdbus_monitor_lines() {
        assert_eq!(parse_signal("/org/freedesktop/login1: org.freedesktop.login1.Manager.PrepareForSleep (false,)"), Some(Signal::Resumed));
        assert_eq!(parse_signal("/org/freedesktop/login1: org.freedesktop.login1.Manager.PrepareForSleep (true,)"), None);
        let nm = "/org/freedesktop/NetworkManager: org.freedesktop.DBus.Properties.PropertiesChanged ('org.freedesktop.NetworkManager', {'Connectivity': <uint32 4>, 'State': <uint32 70>}, @as [])";
        assert_eq!(parse_signal(nm), Some(Signal::Connectivity("<uint32 4>".into())));
        let primary = "/org/freedesktop/NetworkManager: org.freedesktop.DBus.Properties.PropertiesChanged ('org.freedesktop.NetworkManager', {'PrimaryConnection': <objectpath '/org/freedesktop/NetworkManager/ActiveConnection/7'>}, @as [])";
        assert_eq!(parse_signal(primary), Some(Signal::PrimaryConnection("<objectpath '/org/freedesktop/NetworkManager/ActiveConnection/7'>".into())));
        assert_eq!(parse_signal("/org/freedesktop/NetworkManager: org.freedesktop.DBus.Properties.PropertiesChanged ('org.freedesktop.NetworkManager', {'Metered': <uint32 4>}, @as [])"), None);
    }
}
