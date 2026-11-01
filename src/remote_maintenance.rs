//! Bounded, single-use maintenance confirmations. No backend credential or path authority.
use crate::*;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    path::Path,
    time::{Duration, Instant},
};
type Reply = std::result::Result<Value, &'static str>;
pub const SURFACES: &[&str] = &[
    "memory",
    "vault",
    "host-sync",
    "compaction",
    "idle-release",
    "build-cleanup",
];
pub type Backend = fn(&Value, &Value, &str) -> Result<Value>;
fn hash(v: &Value) -> String {
    format!("{:x}", Sha256::digest(v.to_string().as_bytes()))
}
pub fn epoch(settings: &Value) -> Option<&str> {
    (settings["remoteMaintenanceEnabled"] == true)
        .then(|| settings["remoteMaintenanceConsent"].as_str())
        .flatten()
        .filter(|s| valid_id(s))
}
pub fn operate_allowed(auth: &Value) -> bool {
    super::remote_surfaces::read_allowed(auth)
        && auth
            .get("permissions")
            .unwrap_or(&auth["scopes"])
            .as_array()
            .is_some_and(|a| {
                a.iter()
                    .any(|p| p == "orchestration:operate" || p == "environment:maintain")
            })
}
pub fn report(backend: Backend, surface: &str, settings: &Value, bearer: &str) -> Reply {
    if !SURFACES.contains(&surface) {
        return Err("not_found");
    }
    let value = backend(&json!({"op":"report","surface":surface}), settings, bearer)
        .map_err(|_| "surface_unavailable")?;
    if value.to_string().len() > 64 * 1024 {
        return Err("report_limit");
    }
    Ok(json!({"surface":surface,"reportId":hash(&value),"report":value}))
}
#[derive(Clone)]
struct Pending {
    session: String,
    epoch: String,
    read_epoch: String,
    report: Value,
    request: Value,
    at: Instant,
}
#[derive(Default)]
pub struct Confirmations {
    pending: HashMap<String, Pending>,
}
impl Confirmations {
    pub fn prepare(
        &mut self,
        backend: Backend,
        settings: &Value,
        session: &str,
        bearer: &str,
        input: &Value,
    ) -> Reply {
        let epoch = epoch(settings).ok_or("consent_required")?;
        self.pending
            .retain(|_, p| p.at.elapsed() < Duration::from_secs(60));
        if self.pending.len() >= 32 {
            return Err("confirmation_limit");
        }
        let surface = text(input, "surface");
        let action = text(input, "action");
        let valid = match (surface, action) {
            ("compaction" | "idle-release", "settings") => valid_patch(surface, &input["settings"]),
            ("vault", "rollout") | ("build-cleanup", "run") => input.get("settings").is_none(),
            _ => false,
        };
        if !valid
            || input.as_object().is_none_or(|o| {
                o.keys()
                    .any(|k| !["surface", "action", "settings", "reportId"].contains(&k.as_str()))
            })
        {
            return Err("invalid_command");
        }
        let report = report(backend, surface, settings, bearer)?;
        if input["reportId"] != report["reportId"] {
            return Err("stale_report");
        }
        let token = uuid::Uuid::new_v4().to_string();
        self.pending.insert(
            token.clone(),
            Pending {
                session: session.into(),
                epoch: epoch.into(),
                read_epoch: text(settings, "remoteDocumentsConsent").into(),
                report: report.clone(),
                request: input.clone(),
                at: Instant::now(),
            },
        );
        Ok(
            json!({"confirmationToken":token,"report":report,"request":input,"expiresIn":60,"retry":false}),
        )
    }
    // Remove before validation: failed, revoked and uncertain attempts cannot be replayed.
    #[cfg(test)]
    pub fn confirm(
        &mut self,
        dir: &Path,
        backend: Backend,
        settings: &Value,
        session: &str,
        bearer: &str,
        input: &Value,
    ) -> Reply {
        self.confirm_checked(dir, backend, settings, session, bearer, input, || Ok(()))
    }
    #[allow(clippy::too_many_arguments)]
    pub fn confirm_checked(
        &mut self,
        dir: &Path,
        backend: Backend,
        settings: &Value,
        session: &str,
        bearer: &str,
        input: &Value,
        mut reauthorize: impl FnMut() -> std::result::Result<(), &'static str>,
    ) -> Reply {
        let pending = self
            .pending
            .remove(text(input, "confirmationToken"))
            .ok_or("confirmation_required")?;
        if pending.session != session
            || Some(pending.epoch.as_str()) != epoch(settings)
            || pending.read_epoch != text(settings, "remoteDocumentsConsent")
        {
            return Err("consent_required");
        }
        if pending.at.elapsed() >= Duration::from_secs(60)
            || input["reportId"] != pending.report["reportId"]
        {
            return Err("stale_report");
        }
        let _operation = super::remote_surfaces::operation_lock(dir)?;
        let fresh = report(backend, text(&pending.request, "surface"), settings, bearer)?;
        if fresh != pending.report {
            return Err("stale_report");
        }
        reauthorize()?;
        if pending.at.elapsed() >= Duration::from_secs(60) {
            return Err("stale_report");
        }
        let claim = hash(&json!([
            session,
            pending.epoch,
            pending.read_epoch,
            pending.request,
            pending.report
        ]));
        // Persist the claim before invoking a mutating backend. Re-preparing the same
        // report after a lost reply/restart cannot run it again. Store is bounded.
        super::remote_surfaces::claim(dir, &claim)?;
        if pending.at.elapsed() >= Duration::from_secs(60) {
            return Ok(json!({"outcome":"unknown","retry":false}));
        }
        let mut request = pending.request;
        request["op"] = json!("apply");
        request["report"] = fresh["report"].clone();
        match backend(&request, settings, bearer) {
            Ok(result) if result.to_string().len() <= 64 * 1024 => {
                Ok(json!({"outcome":"completed","result":result,"retry":false}))
            }
            _ => Ok(json!({"outcome":"unknown","retry":false})),
        }
    }
}
pub fn valid_patch(surface: &str, patch: &Value) -> bool {
    let Some(o) = patch.as_object().filter(|o| !o.is_empty() && o.len() <= 4) else {
        return false;
    };
    o.iter().all(|(k, v)| match (surface, k.as_str()) {
        ("compaction", "autoCompactEnabled") | ("idle-release", "idleReleaseEnabled") => {
            v.is_boolean()
        }
        ("compaction", "autoCompactFallbackMinutes") | ("idle-release", "idleReleaseMinutes") => {
            v.as_u64().is_some_and(|n| (1..=1440).contains(&n))
        }
        ("compaction", "autoCompactMinTokens") => {
            v.as_u64().is_some_and(|n| (1000..=1_000_000).contains(&n))
        }
        _ => false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn settings() -> Value {
        json!({"remoteDocumentsEnabled":true,"remoteDocumentsConsent":"read-epoch","remoteMaintenanceEnabled":true,"remoteMaintenanceConsent":"write-epoch","autoCompactEnabled":false})
    }
    fn backend(input: &Value, s: &Value, _: &str) -> Result<Value> {
        if input["op"] == "report" {
            Ok(json!({"enabled":s["autoCompactEnabled"]}))
        } else {
            Err("lost_reply".into())
        }
    }
    fn request(s: &Value) -> Value {
        let report = report(backend, "compaction", s, "bearer").unwrap();
        json!({"surface":"compaction","action":"settings","settings":{"autoCompactEnabled":true},"reportId":report["reportId"]})
    }
    fn prepared(c: &mut Confirmations, s: &Value) -> Value {
        let p = c
            .prepare(backend, s, "session", "bearer", &request(s))
            .unwrap();
        json!({"confirmationToken":p["confirmationToken"],"reportId":p["report"]["reportId"]})
    }
    fn dir() -> std::path::PathBuf {
        std::path::PathBuf::from("/var/tmp").join(format!("remote-memory-{}", uuid::Uuid::new_v4()))
    }
    #[test]
    fn operate_requires_read_and_explicit_permission_not_wildcard() {
        let mut auth = json!({"authenticated":true,"auth":{"policy":"remote-reachable"},"sessionMethod":"bearer-access-token","scopes":["orchestration:read","orchestration:operate"]});
        assert!(operate_allowed(&auth));
        for p in [
            json!([]),
            json!(["*"]),
            json!(["orchestration:operate"]),
            json!(["orchestration:read"]),
            Value::Null,
        ] {
            auth["permissions"] = p;
            assert!(!operate_allowed(&auth));
        }
        auth["permissions"] = json!(["orchestration:read", "environment:maintain"]);
        assert!(operate_allowed(&auth));
        auth["sessionMethod"] = json!("browser-session-cookie");
        assert!(!operate_allowed(&auth));
    }
    #[test]
    fn default_off_and_bad_consent_deny_preparation() {
        for s in [
            json!({}),
            json!({"remoteMaintenanceEnabled":true}),
            json!({"remoteMaintenanceEnabled":true,"remoteMaintenanceConsent":"../escape"}),
        ] {
            assert_eq!(
                Confirmations::default().prepare(backend, &s, "s", "b", &request(&s)),
                Err("consent_required")
            );
        }
    }
    #[test]
    fn revoked_foreign_expired_stale_and_rotated_confirmations_are_consumed() {
        let path = dir();
        for mode in 0..7 {
            let mut c = Confirmations::default();
            let mut s = settings();
            let mut token = prepared(&mut c, &s);
            let mut session = "session";
            match mode {
                0 => s["remoteMaintenanceEnabled"] = json!(false),
                1 => s["remoteMaintenanceConsent"] = json!("new-epoch"),
                2 => s["remoteDocumentsConsent"] = json!("new-read-epoch"),
                3 => session = "foreign",
                4 => {
                    c.pending.values_mut().next().unwrap().at =
                        Instant::now() - Duration::from_secs(61)
                }
                5 => s["autoCompactEnabled"] = json!(true),
                _ => token["reportId"] = json!("foreign-report"),
            }
            assert!(c.confirm(&path, backend, &s, session, "b", &token).is_err());
            assert_eq!(
                c.confirm(&path, backend, &s, session, "b", &token),
                Err("confirmation_required")
            );
            assert!(!path.join("maintenance-claims.json").exists());
        }
        let _ = fs::remove_dir_all(path);
    }
    #[test]
    fn lost_reply_is_unknown_and_restart_cannot_repeat_exact_report() {
        let path = dir();
        let s = settings();
        let mut c = Confirmations::default();
        let token = prepared(&mut c, &s);
        assert_eq!(
            c.confirm(&path, backend, &s, "session", "b", &token)
                .unwrap(),
            json!({"outcome":"unknown","retry":false})
        );
        assert_eq!(
            c.confirm(&path, backend, &s, "session", "b", &token),
            Err("confirmation_required")
        );
        let mut restarted = Confirmations::default();
        let next = prepared(&mut restarted, &s);
        assert_eq!(
            restarted.confirm(&path, backend, &s, "session", "b", &next),
            Err("already_claimed")
        );
        fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn pending_reports_and_claims_are_bounded() {
        let mut c = Confirmations::default();
        let s = settings();
        for _ in 0..32 {
            prepared(&mut c, &s);
        }
        assert_eq!(
            c.prepare(backend, &s, "session", "b", &request(&s)),
            Err("confirmation_limit")
        );
        let path = dir();
        for i in 0..128 {
            super::super::remote_surfaces::claim(&path, &hash(&json!(i))).unwrap();
        }
        assert_eq!(
            super::super::remote_surfaces::claim(&path, &hash(&json!(129))),
            Err("store_limit")
        );
        fs::remove_dir_all(path).unwrap();
        fn large(_: &Value, _: &Value, _: &str) -> Result<Value> {
            Ok(json!({"data":"x".repeat(65536)}))
        }
        assert_eq!(report(large, "memory", &s, "b"), Err("report_limit"));
    }
    #[test]
    fn prepare_stale_and_unlisted_actions_and_settings_fail_closed() {
        let s = settings();
        let mut c = Confirmations::default();
        let mut r = request(&s);
        r["reportId"] = json!("old");
        assert_eq!(c.prepare(backend, &s, "s", "b", &r), Err("stale_report"));
        for patch in [
            json!({"autoCompactEnabled":"true"}),
            json!({"autoCompactMinTokens":999}),
            json!({"autoCompactFallbackMinutes":1441}),
            json!({"path":"/etc"}),
            json!({}),
        ] {
            r = request(&s);
            r["settings"] = patch;
            assert_eq!(c.prepare(backend, &s, "s", "b", &r), Err("invalid_command"));
        }
        for surface in ["memory", "host-sync", "browser", "../vault"] {
            r = request(&s);
            r["surface"] = json!(surface);
            assert_eq!(c.prepare(backend, &s, "s", "b", &r), Err("invalid_command"));
        }
    }
    #[test]
    fn revocation_after_fresh_report_prevents_durable_claim_and_mutation() {
        let path = dir();
        let s = settings();
        let mut c = Confirmations::default();
        let token = prepared(&mut c, &s);
        assert_eq!(
            c.confirm_checked(&path, backend, &s, "session", "b", &token, || Err(
                "unauthorized"
            )),
            Err("unauthorized")
        );
        assert!(!path.join("maintenance-claims.json").exists());
        assert_eq!(
            c.confirm(&path, backend, &s, "session", "b", &token),
            Err("confirmation_required")
        );
    }
    #[test]
    fn concurrent_broker_mutations_are_rejected_without_claim_or_retry() {
        let path = dir();
        let s = settings();
        let mut c = Confirmations::default();
        let token = prepared(&mut c, &s);
        let lock = super::super::remote_surfaces::operation_lock(&path).unwrap();
        assert_eq!(
            c.confirm(&path, backend, &s, "session", "b", &token),
            Err("store_busy")
        );
        assert!(!path.join("maintenance-claims.json").exists());
        drop(lock);
        assert_eq!(
            c.confirm(&path, backend, &s, "session", "b", &token),
            Err("confirmation_required")
        );
        fs::remove_dir_all(path).unwrap();
    }
}
