use super::{failed, Microphone, Phase};
use serde_json::Value;
use std::io;

/// Per-capture metadata ordering and bounds, independent of the PCM channel.
#[derive(Default)]
pub(super) struct Updates {
    revision: u64,
    exhausted: bool,
    initial_done: bool,
}
impl Updates {
    pub(super) fn initial_received(&self) -> bool {
        self.initial_done || self.exhausted
    }
    pub(super) fn available(&self) -> bool {
        !self.exhausted
    }
    pub(super) fn unavailable(
        &mut self,
        value: &Value,
        phase: Phase,
        closing: bool,
    ) -> io::Result<bool> {
        if closing || matches!(phase, Phase::Stopping | Phase::Stopped | Phase::Failed) {
            return Ok(false);
        }
        if phase != Phase::Preparing
            || self.initial_received()
            || value.as_object().is_none_or(|object| {
                object.len() != 2
                    || !object.contains_key("type")
                    || !object.contains_key("captureId")
            })
        {
            return Err(failed());
        }
        self.initial_done = true;
        Ok(true)
    }
    pub(super) fn accept(
        &mut self,
        value: &Value,
        phase: Phase,
        closing: bool,
    ) -> io::Result<Option<Vec<Microphone>>> {
        if closing
            || self.exhausted
            || matches!(phase, Phase::Stopping | Phase::Stopped | Phase::Failed)
        {
            return Ok(None);
        }
        let revision = value["revision"].as_u64().ok_or_else(failed)?;
        if revision > 9_007_199_254_740_991 {
            self.exhausted = true;
            return Ok(None);
        }
        if value.as_object().is_none_or(|object| {
            object.len() != 4
                || !["type", "captureId", "revision", "devices"]
                    .iter()
                    .all(|key| object.contains_key(*key))
        }) || revision != self.revision + 1
            || phase == Phase::Preparing && self.initial_received()
            || phase != Phase::Preparing && !self.initial_received()
        {
            return Err(failed());
        }
        let devices: Vec<Microphone> =
            serde_json::from_value(value["devices"].clone()).map_err(|_| failed())?;
        if devices.len() > 256
            || devices
                .iter()
                .any(|device| device.device_id.len() > 1024 || device.label.len() > 4096)
        {
            return Err(failed());
        }
        self.revision = revision;
        self.initial_done = true;
        self.exhausted = revision == 9_007_199_254_740_991;
        Ok(Some(devices))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn event(revision: Value) -> Value {
        json!({"type":"microphones","captureId":"owned-capture","revision":revision,
            "devices":[{"deviceId":"opaque-device","label":"<b>Literal label</b>"}]})
    }
    #[test]
    fn initial_then_repeated_active_phases_keep_exact_ids_and_labels() {
        let mut updates = Updates::default();
        let initial = updates
            .accept(&event(json!(1)), Phase::Preparing, false)
            .unwrap()
            .unwrap();
        assert_eq!(initial[0].device_id, "opaque-device");
        assert_eq!(initial[0].label, "<b>Literal label</b>");
        assert!(updates
            .accept(&event(json!(2)), Phase::Prepared, false)
            .unwrap()
            .is_some());
        assert!(updates
            .accept(&event(json!(3)), Phase::Recording, false)
            .unwrap()
            .is_some());
    }
    #[test]
    fn replay_gap_or_second_preparing_event_does_not_advance_order() {
        let mut updates = Updates::default();
        updates
            .accept(&event(json!(1)), Phase::Preparing, false)
            .unwrap();
        for (revision, phase) in [
            (1, Phase::Prepared),
            (3, Phase::Prepared),
            (2, Phase::Preparing),
        ] {
            assert!(updates
                .accept(&event(json!(revision)), phase, false)
                .is_err());
        }
        assert!(updates
            .accept(&event(json!(2)), Phase::Recording, false)
            .is_ok());
    }
    #[test]
    fn unprepared_capture_cannot_invent_a_later_metadata_event() {
        assert!(Updates::default()
            .accept(&event(json!(1)), Phase::Prepared, false)
            .is_err());
        assert!(Updates::default()
            .accept(&event(json!(2)), Phase::Preparing, false)
            .is_err());
    }
    #[test]
    fn stopped_failed_and_closing_metadata_is_ignored() {
        for phase in [Phase::Stopping, Phase::Stopped, Phase::Failed] {
            assert!(Updates::default()
                .accept(&Value::Null, phase, false)
                .unwrap()
                .is_none());
        }
        assert!(Updates::default()
            .accept(&Value::Null, Phase::Recording, true)
            .unwrap()
            .is_none());
    }
    #[test]
    fn malformed_revisions_and_unknown_fields_are_rejected() {
        for revision in [json!(null), json!(-1), json!(0), json!(1.5), json!("1")] {
            assert!(Updates::default()
                .accept(&event(revision), Phase::Preparing, false)
                .is_err());
        }
        let mut extra = event(json!(1));
        extra["unexpected"] = json!(true);
        assert!(Updates::default()
            .accept(&extra, Phase::Preparing, false)
            .is_err());
        let mut extra_device = event(json!(1));
        extra_device["devices"][0]["path"] = json!("not-a-device-path");
        assert!(Updates::default()
            .accept(&extra_device, Phase::Preparing, false)
            .is_err());
    }
    #[test]
    fn device_count_and_utf8_byte_limits_are_enforced() {
        for devices in [
            json!([{"deviceId":"x","label":"é".repeat(2049)}]),
            json!([{"deviceId":"x".repeat(1025),"label":"name"}]),
            json!(vec![json!({"deviceId":"x","label":"name"}); 257]),
        ] {
            let mut value = event(json!(1));
            value["devices"] = devices;
            assert!(Updates::default()
                .accept(&value, Phase::Preparing, false)
                .is_err());
        }
    }
    #[test]
    fn safe_integer_exhaustion_closes_only_metadata() {
        let maximum = 9_007_199_254_740_991_u64;
        let mut updates = Updates {
            revision: maximum - 1,
            exhausted: false,
            initial_done: true,
        };
        assert!(updates
            .accept(&event(json!(maximum)), Phase::Recording, false)
            .unwrap()
            .is_some());
        assert!(!updates.available());
        assert!(updates
            .accept(&event(json!(maximum + 1)), Phase::Recording, false)
            .unwrap()
            .is_none());
        let mut overflow = Updates::default();
        assert!(overflow
            .accept(&event(json!(u64::MAX)), Phase::Preparing, false)
            .unwrap()
            .is_none());
        assert!(!overflow.available());
        assert!(overflow.initial_received());
        assert!(overflow
            .accept(&Value::Null, Phase::Recording, false)
            .unwrap()
            .is_none());
    }
    #[test]
    fn unavailable_initial_metadata_is_terminal_without_a_replacement_list() {
        let status = json!({"type":"microphones-unavailable","captureId":"owned-capture"});
        let mut updates = Updates::default();
        assert!(updates
            .unavailable(&status, Phase::Preparing, false)
            .unwrap());
        assert!(updates.initial_received());
        assert!(updates.available());
        let mut empty = event(json!(1));
        empty["devices"] = json!([]);
        assert!(updates
            .accept(&empty, Phase::Prepared, false)
            .unwrap()
            .unwrap()
            .is_empty());
        assert!(updates
            .unavailable(&status, Phase::Preparing, false)
            .is_err());
        assert!(!updates
            .unavailable(&status, Phase::Stopping, false)
            .unwrap());
    }
}
