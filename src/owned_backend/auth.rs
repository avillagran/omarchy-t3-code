//! Process-local rotating bootstrap credentials and single-flight bearer leases.
use crate::*;
use hmac::{Hmac, Mac};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Condvar, Mutex,
};
use std::time::{Duration, Instant};
use zeroize::{Zeroize, Zeroizing};

pub(super) const WINDOW_MS: i64 = 12 * 60 * 60 * 1000;
const RENEW_SKEW: Duration = Duration::from_secs(60);
pub(super) fn token(secret: &str, now_ms: i64) -> Result<Zeroizing<String>> {
    if secret.is_empty() {
        return Err("owned_backend_auth_unavailable".into());
    }
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
        .map_err(|_| "owned_backend_auth_unavailable")?;
    mac.update(format!("t3-desktop-bootstrap:{}", now_ms.div_euclid(WINDOW_MS)).as_bytes());
    Ok(Zeroizing::new(format!("{:x}", mac.finalize().into_bytes())))
}
pub(super) struct Grant {
    pub connection: protocol::Connection,
    pub expires: Instant,
}
struct State {
    current: Option<Grant>,
    snapshot: Option<protocol::Connection>,
    flight: Option<Arc<Flight>>,
}
struct Flight {
    result: Mutex<Option<Result<protocol::Connection>>>,
    ready: Condvar,
}
impl Flight {
    fn complete(&self, result: Result<protocol::Connection>) {
        let mut value = self
            .result
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if value.is_none() {
            *value = Some(result);
            self.ready.notify_all();
        }
    }
    fn wait(&self) -> Result<protocol::Connection> {
        let value = self
            .result
            .lock()
            .map_err(|_| "owned_backend_auth_unavailable")?;
        self.ready
            .wait_while(value, |value| value.is_none())
            .map_err(|_| "owned_backend_auth_unavailable")?
            .as_ref()
            .expect("completed auth flight")
            .clone()
    }
}
type Exchange = Arc<dyn Fn(&str) -> Result<Grant> + Send + Sync>;
pub(super) struct Auth {
    secret: Arc<Mutex<Zeroizing<String>>>,
    state: Mutex<State>,
    closed: AtomicBool,
    exchange: Exchange,
}
impl Auth {
    pub fn new(
        secret: Zeroizing<String>,
        grant: Grant,
        http: reqwest::blocking::Client,
        origin: String,
    ) -> Self {
        Self::with_exchange(
            secret,
            grant,
            Arc::new(move |credential| exchange(&http, &origin, credential)),
        )
    }
    fn with_exchange(secret: Zeroizing<String>, grant: Grant, exchange: Exchange) -> Self {
        let snapshot = Some(grant.connection.clone());
        Self {
            secret: Arc::new(Mutex::new(secret)),
            state: Mutex::new(State {
                current: Some(grant),
                snapshot,
                flight: None,
            }),
            closed: AtomicBool::new(false),
            exchange,
        }
    }
    pub fn credential(&self, now_ms: i64) -> Result<Zeroizing<String>> {
        if self.closed.load(Ordering::Acquire) {
            return Err("owned_backend_stopped".into());
        }
        let secret = self
            .secret
            .lock()
            .map_err(|_| "owned_backend_auth_unavailable")?;
        if self.closed.load(Ordering::Acquire) {
            return Err("owned_backend_stopped".into());
        }
        token(&secret, now_ms)
    }
    /// Immutable last-admitted snapshot. Callers must use fresh() for a new
    /// admission; snapshots already held by an in-flight request are unchanged.
    pub fn snapshot(&self) -> protocol::Connection {
        self.state
            .lock()
            .unwrap()
            .snapshot
            .as_ref()
            .expect("owned backend stopped")
            .clone()
    }
    pub fn fresh(&self) -> Result<protocol::Connection> {
        self.renew(false)
    }
    pub fn refresh(&self) -> Result<protocol::Connection> {
        self.renew(true)
    }
    fn renew(&self, force: bool) -> Result<protocol::Connection> {
        if self.closed.load(Ordering::Acquire) {
            return Err("owned_backend_stopped".into());
        }
        let (flight, leader) = {
            let mut state = self
                .state
                .lock()
                .map_err(|_| "owned_backend_auth_unavailable")?;
            if self.closed.load(Ordering::Acquire) {
                return Err("owned_backend_stopped".into());
            }
            if let Some(flight) = &state.flight {
                (flight.clone(), false)
            } else {
                if !force {
                    if let Some(grant) = &state.current {
                        if grant.expires.saturating_duration_since(Instant::now()) > RENEW_SKEW {
                            return Ok(grant.connection.clone());
                        }
                    }
                }
                let flight = Arc::new(Flight {
                    result: Mutex::new(None),
                    ready: Condvar::new(),
                });
                state.flight = Some(flight.clone());
                (flight, true)
            }
        };
        if leader {
            let result = self
                .credential(Utc::now().timestamp_millis())
                .and_then(|credential| (self.exchange)(&credential));
            let mut state = self
                .state
                .lock()
                .map_err(|_| "owned_backend_auth_unavailable")?;
            let current = state
                .flight
                .as_ref()
                .is_some_and(|active| Arc::ptr_eq(active, &flight));
            let result = if self.closed.load(Ordering::Acquire) || !current {
                Err("owned_backend_stopped".into())
            } else {
                match result {
                    Ok(grant) => {
                        let connection = grant.connection.clone();
                        state.snapshot = Some(connection.clone());
                        state.current = Some(grant);
                        Ok(connection)
                    }
                    Err(error) => {
                        state.current = None;
                        Err(error)
                    }
                }
            };
            if current {
                state.flight = None;
            }
            flight.complete(result);
        }
        let result = flight.wait();
        if self.closed.load(Ordering::Acquire) {
            return Err("owned_backend_stopped".into());
        }
        result
    }
    /// Stop admission and wipe the retained secret without waiting on HTTP.
    pub fn begin_close(&self) {
        self.closed.store(true, Ordering::Release);
        self.secret
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .zeroize();
    }
    pub fn finish_close(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.current = None;
        state.snapshot = None;
        if let Some(flight) = state.flight.take() {
            flight.complete(Err("owned_backend_stopped".into()));
        }
    }
}
impl Drop for Auth {
    fn drop(&mut self) {
        self.begin_close();
        self.finish_close();
    }
}

pub(super) fn exchange(
    http: &reqwest::blocking::Client,
    origin: &str,
    credential: &str,
) -> Result<Grant> {
    use std::io::Read;
    let response = http
        .post(format!("{origin}/oauth/token"))
        .form(&[("subject_token", credential)])
        .send()
        .map_err(|_| "owned_backend_auth_failed")?;
    if !response.status().is_success() {
        return Err("owned_backend_auth_failed".into());
    }
    let mut bytes = Zeroizing::new(Vec::new());
    response
        .take(65537)
        .read_to_end(&mut bytes)
        .map_err(|_| "owned_backend_auth_failed")?;
    if bytes.len() > 65536 {
        return Err("owned_backend_auth_failed".into());
    }
    let mut value: Value =
        serde_json::from_slice(&bytes).map_err(|_| "owned_backend_auth_failed")?;
    let token = match value
        .as_object_mut()
        .and_then(|object| object.remove("access_token"))
    {
        Some(Value::String(token)) => Zeroizing::new(token),
        _ => return Err("owned_backend_auth_failed".into()),
    };
    let ttl = value["expires_in"]
        .as_u64()
        .filter(|seconds| *seconds > 0)
        .ok_or("owned_backend_auth_failed")?;
    if text(&value, "token_type") != "Bearer" || text(&value, "scope").is_empty() {
        return Err("owned_backend_auth_failed".into());
    }
    let expires = Instant::now()
        .checked_add(Duration::from_secs(ttl))
        .ok_or("owned_backend_auth_failed")?;
    let connection = t3_connection(origin, protocol::Credential::bearer(&token)?)?;
    // Publish only an actual authenticated RPC admission, not token-shaped JSON.
    protocol::api::call(
        &connection,
        "server.getConfig",
        &json!({}),
        Some(Duration::from_secs(2)),
    )
    .map_err(|_| "owned_backend_rpc_not_ready")?;
    Ok(Grant {
        connection,
        expires,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Barrier,
    };
    fn grant(token: &str, ttl: Duration) -> Grant {
        Grant {
            connection: protocol::Connection::new(
                "http://127.0.0.1:1",
                None,
                protocol::Credential::bearer(token).unwrap(),
            )
            .unwrap(),
            expires: Instant::now() + ttl,
        }
    }
    #[test]
    fn actual_native_rotating_token_window_fixtures() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/owned-bootstrap-tokens-4288.json"
        ))
        .unwrap();
        let cases = fixture["tokens"].as_array().unwrap();
        assert_eq!(cases.len(), 44);
        for case in cases {
            let actual = token(
                case["secret"].as_str().unwrap(),
                case["nowMs"].as_i64().unwrap(),
            );
            if let Some(expected) = case["expected"]["value"].as_str() {
                assert_eq!(actual.unwrap().as_str(), expected);
            } else {
                assert!(actual.is_err());
            }
        }
    }
    #[test]
    fn unexpired_grant_reuses_admitted_snapshot_without_exchange() {
        let old = grant("synthetic-original-token", Duration::from_secs(3600));
        let frozen = old.connection.clone();
        let auth = Auth::with_exchange(
            Zeroizing::new("synthetic-secret".into()),
            old,
            Arc::new(|_| panic!("unexpired snapshot must not exchange")),
        );
        assert!(auth.fresh().unwrap().same_session_admission(&frozen));
        assert!(auth.snapshot().same_session_admission(&frozen));
    }
    #[test]
    fn expiry_renewal_is_single_flight_and_keeps_existing_snapshots_immutable() {
        let count = Arc::new(AtomicUsize::new(0));
        let calls = count.clone();
        let old = grant("synthetic-original-token", Duration::from_secs(30));
        let frozen = old.connection.clone();
        let auth = Arc::new(Auth::with_exchange(
            Zeroizing::new("synthetic-secret".into()),
            old,
            Arc::new(move |_| {
                calls.fetch_add(1, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(30));
                Ok(grant("synthetic-renewed-token", Duration::from_secs(3600)))
            }),
        ));
        let barrier = Arc::new(Barrier::new(8));
        let mut threads = vec![];
        for _ in 0..8 {
            let auth = auth.clone();
            let barrier = barrier.clone();
            threads.push(std::thread::spawn(move || {
                barrier.wait();
                auth.fresh().unwrap()
            }));
        }
        let results = threads
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(count.load(Ordering::SeqCst), 1);
        for connection in &results {
            assert!(connection.same_session_admission(&results[0]));
            assert!(!connection.same_session_admission(&frozen));
        }
        assert!(frozen.same_session_admission(&frozen.clone()));
    }
    #[test]
    fn failed_renewal_invalidates_current_admission_without_rewriting_old_snapshot() {
        let old = grant("synthetic-original-token", Duration::from_secs(30));
        let frozen = old.connection.clone();
        let auth = Auth::with_exchange(
            Zeroizing::new("synthetic-secret".into()),
            old,
            Arc::new(|_| Err("synthetic_exchange_failed".into())),
        );
        assert!(matches!(auth.fresh(),Err(error) if error=="synthetic_exchange_failed"));
        assert!(auth.state.lock().unwrap().current.is_none());
        assert!(auth.snapshot().same_session_admission(&frozen));
    }
    #[test]
    fn shutdown_zeroizes_retained_secret_and_rejects_late_renewal_success() {
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let release = Mutex::new(release_rx);
        let auth = Arc::new(Auth::with_exchange(
            Zeroizing::new("synthetic-retained-secret".into()),
            grant("synthetic-original-token", Duration::from_secs(30)),
            Arc::new(move |_| {
                entered_tx.send(()).unwrap();
                release.lock().unwrap().recv().unwrap();
                Ok(grant("synthetic-late-token", Duration::from_secs(3600)))
            }),
        ));
        let retained = auth.secret.clone();
        let worker_auth = auth.clone();
        let worker = std::thread::spawn(move || worker_auth.fresh());
        entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        auth.begin_close();
        auth.finish_close();
        assert!(retained.lock().unwrap().is_empty());
        assert!(auth.credential(0).is_err());
        release_tx.send(()).unwrap();
        assert!(matches!(worker.join().unwrap(),Err(error) if error=="owned_backend_stopped"));
        let state = auth.state.lock().unwrap();
        assert!(state.current.is_none());
        assert!(state.snapshot.is_none());
        assert!(state.flight.is_none());
    }
}

#[cfg(test)]
mod explicit_refresh_tests {
    use super::*;
    #[test]
    fn simultaneous_forced_renewal_shares_one_exchange() {
        use std::sync::{
            atomic::{AtomicUsize, Ordering},
            Barrier,
        };
        let calls = Arc::new(AtomicUsize::new(0));
        let count = calls.clone();
        let connection = protocol::Connection::new(
            "http://127.0.0.1:1",
            None,
            protocol::Credential::bearer("synthetic-original-token").unwrap(),
        )
        .unwrap();
        let next = protocol::Connection::new(
            "http://127.0.0.1:1",
            None,
            protocol::Credential::bearer("synthetic-renewed-token").unwrap(),
        )
        .unwrap();
        let auth = Arc::new(Auth::with_exchange(
            Zeroizing::new("synthetic-secret".into()),
            Grant {
                connection,
                expires: Instant::now() + Duration::from_secs(3600),
            },
            Arc::new(move |_| {
                count.fetch_add(1, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(30));
                Ok(Grant {
                    connection: next.clone(),
                    expires: Instant::now() + Duration::from_secs(3600),
                })
            }),
        ));
        let barrier = Arc::new(Barrier::new(8));
        let mut threads = vec![];
        for _ in 0..8 {
            let auth = auth.clone();
            let barrier = barrier.clone();
            threads.push(std::thread::spawn(move || {
                barrier.wait();
                auth.refresh().unwrap()
            }));
        }
        for thread in threads {
            thread.join().unwrap();
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}
