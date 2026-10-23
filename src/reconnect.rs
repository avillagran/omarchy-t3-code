//! Reconnection policy shared by the shell and chat streams.
use crate::protocol;

/// Transport failures leave a held snapshot valid, so the next attempt resumes from
/// its sequence. Anything else (the server ending the stream, auth, malformed data)
/// starts over from a fresh snapshot.
pub fn transient(error: &str) -> bool {
    matches!(
        protocol::error_code(error),
        "disconnected" | "stream_timeout" | "unreachable" | "rpc_connection_failed" | "rpc_outcome_unknown"
    )
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
}
