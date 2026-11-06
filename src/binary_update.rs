//! An installed update replaces `bin/omarchy-t3-client` while the app's `watch` worker keeps
//! running the old executable. The worker notices the replacement and exits once the UI has been
//! quiet for a while; the app restarts it within seconds on the new binary, so an update (a new
//! local-server registry rule, say) takes effect without restarting the app.
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Long enough for any request the UI started, a server start included, to have finished.
pub const QUIET_MS: u64 = 60_000;
static LAST_INPUT_MS: AtomicU64 = AtomicU64::new(0);

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Records UI activity; called for every request line the worker reads.
pub fn touch() {
    LAST_INPUT_MS.store(now_ms(), Ordering::Relaxed);
}

/// Linux names a running executable whose file was replaced or removed `<path> (deleted)`.
pub fn replaced(exe: &str) -> bool {
    exe.ends_with(" (deleted)")
}

/// Exit only for a replaced binary, and only after `QUIET_MS` without requests.
pub fn should_exit(replaced: bool, last_input_ms: u64, now_ms: u64) -> bool {
    replaced && now_ms.saturating_sub(last_input_ms) >= QUIET_MS
}

/// Starts the check beside the `watch` loop.
pub fn spawn() {
    touch();
    let _ = std::thread::Builder::new()
        .name("binary-update".into())
        .stack_size(64 * 1024)
        .spawn(|| loop {
            std::thread::sleep(Duration::from_secs(5));
            let exe = std::fs::read_link("/proc/self/exe")
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            if should_exit(
                replaced(&exe),
                LAST_INPUT_MS.load(Ordering::Relaxed),
                now_ms(),
            ) {
                eprintln!("omarchy-t3-client: binary updated; restarting the worker");
                std::process::exit(0);
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exits_only_for_a_replaced_binary_after_a_quiet_minute() {
        assert!(replaced("/x/bin/omarchy-t3-client (deleted)"));
        assert!(!replaced("/x/bin/omarchy-t3-client"));
        assert!(!should_exit(false, 0, QUIET_MS * 10));
        assert!(!should_exit(true, 1_000, 1_000 + QUIET_MS - 1));
        assert!(should_exit(true, 1_000, 1_000 + QUIET_MS));
    }
}
