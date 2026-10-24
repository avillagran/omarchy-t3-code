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
use std::{
    io::{BufRead, BufReader},
    os::unix::process::CommandExt,
    process::{Command, Stdio},
    time::Duration,
};
pub use t3_protocol::reconnect::{transient, Backoff, Waiter, BASE_DELAY, MAX_DELAY, PROBE_TIMEOUT, STABLE_AFTER};

/// Quick check that a server answers before a wakeup reconnects to it.
pub fn reachable(e: &crate::Environment) -> bool {
    crate::Client::shared().and_then(|c| c.get(e, "/.well-known/t3/environment", false)).is_ok()
}

/// Wakes every stream (resume, network change): `Waiter::default()` follows the
/// library's process-wide counter.
pub fn wake(reason: &str) {
    t3_protocol::reconnect::notify();
    eprintln!("ot3: reconnecting streams after {reason}");
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
