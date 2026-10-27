//! A bounded-age metadata revision check, never a replacement for pre-mutation
//! validation. Failed reads are not remembered, so recovery remains responsive.
use omarchy_t3_client::text;
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};
#[derive(Default)]
pub struct Cache(HashMap<String, (Value, Instant)>);
const TTL: Duration = Duration::from_secs(300);
fn revision(t: &Value) -> Value {
    json!([
        t["updatedAt"],
        t["compactActivityAt"],
        t["compactRunId"],
        t["modelSelection"],
        t["working"],
        t["backgroundWaiting"],
        t["canSnooze"]
    ])
}
impl Cache {
    pub fn due(&self, t: &Value, now: Instant) -> bool {
        self.0
            .get(text(t, "key"))
            .is_none_or(|(r, at)| *r != revision(t) || now.saturating_duration_since(*at) >= TTL)
    }
    pub fn checked(&mut self, t: &Value, now: Instant) {
        self.0
            .insert(text(t, "key").to_string(), (revision(t), now));
    }
    pub fn retain(&mut self, threads: &[Value]) {
        let keys: HashSet<&str> = threads.iter().map(|t| text(t, "key")).collect();
        self.0.retain(|k, _| keys.contains(k.as_str()));
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unchanged_metadata_avoids_reads_but_revision_and_expiry_force_validation() {
        let mut c = Cache::default();
        let now = Instant::now();
        let mut t = json!({"key":"node/a","updatedAt":1,"compactRunId":"r1"});
        assert!(c.due(&t, now));
        c.checked(&t, now);
        assert!(!c.due(&t, now + Duration::from_secs(299)));
        t["updatedAt"] = json!(2);
        assert!(c.due(&t, now));
        c.checked(&t, now);
        t["compactRunId"] = json!("r2");
        assert!(c.due(&t, now));
        c.checked(&t, now);
        assert!(c.due(&t, now + TTL));
        c.retain(&[]);
        assert!(c.due(&t, now));
    }
}
