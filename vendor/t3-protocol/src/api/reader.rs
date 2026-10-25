//! Shared input helpers of the typed API: a strict member reader for nested
//! objects (`Reader`) and the small checks every module repeats. They only
//! read JSON; none touches the network.
use super::ids;
use crate::Result;
use serde_json::{Map, Value};

/// Strict member reader: each read marks the member used; `done` rejects the rest.
pub(crate) struct Reader<'a> {
    map: &'a Map<String, Value>,
    used: Vec<&'static str>,
}
impl<'a> Reader<'a> {
    pub(crate) fn new(v: &'a Value) -> Result<Self> {
        Ok(Self { map: v.as_object().ok_or("invalid_input")?, used: vec![] })
    }
    /// A reader whose non-object input is `invalid_<field>` instead of `invalid_input`.
    pub(crate) fn of(field: &str, v: &'a Value) -> Result<Self> {
        Self::new(v).map_err(|_| ids::invalid(field))
    }
    pub(crate) fn raw(&mut self, k: &'static str) -> Option<&'a Value> {
        self.used.push(k);
        self.map.get(k)
    }
    pub(crate) fn req_str(&mut self, k: &'static str) -> Result<String> {
        self.opt_str(k)?.ok_or_else(|| ids::invalid(k))
    }
    pub(crate) fn opt_str(&mut self, k: &'static str) -> Result<Option<String>> {
        match self.raw(k) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::String(s)) => Ok(Some(s.clone())),
            Some(_) => Err(ids::invalid(k)),
        }
    }
    /// Present-and-null → `Some(None)`; absent → `None`.
    pub(crate) fn nullable_str(&mut self, k: &'static str) -> Result<Option<Option<String>>> {
        match self.raw(k) {
            None => Ok(None),
            Some(Value::Null) => Ok(Some(None)),
            Some(Value::String(s)) => Ok(Some(Some(s.clone()))),
            Some(_) => Err(ids::invalid(k)),
        }
    }
    pub(crate) fn nullable_value(&mut self, k: &'static str) -> Option<Value> {
        self.raw(k).cloned()
    }
    pub(crate) fn opt_bool(&mut self, k: &'static str) -> Result<Option<bool>> {
        match self.raw(k) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::Bool(b)) => Ok(Some(*b)),
            Some(_) => Err(ids::invalid(k)),
        }
    }
    pub(crate) fn req_bool(&mut self, k: &'static str) -> Result<bool> {
        self.opt_bool(k)?.ok_or_else(|| ids::invalid(k))
    }
    pub(crate) fn opt_u64(&mut self, k: &'static str) -> Result<Option<u64>> {
        match self.raw(k) {
            None | Some(Value::Null) => Ok(None),
            Some(v) => v.as_u64().map(Some).ok_or_else(|| ids::invalid(k)),
        }
    }
    pub(crate) fn req_u64(&mut self, k: &'static str) -> Result<u64> {
        self.opt_u64(k)?.ok_or_else(|| ids::invalid(k))
    }
    pub(crate) fn opt_f64(&mut self, k: &'static str) -> Result<Option<f64>> {
        match self.raw(k) {
            None | Some(Value::Null) => Ok(None),
            Some(v) => v.as_f64().map(Some).ok_or_else(|| ids::invalid(k)),
        }
    }
    pub(crate) fn req_f64(&mut self, k: &'static str) -> Result<f64> {
        self.opt_f64(k)?.ok_or_else(|| ids::invalid(k))
    }
    pub(crate) fn opt_value(&mut self, k: &'static str) -> Option<Value> {
        self.raw(k).filter(|v| !v.is_null()).cloned()
    }
    pub(crate) fn req_value(&mut self, k: &'static str) -> Result<Value> {
        self.opt_value(k).ok_or_else(|| ids::invalid(k))
    }
    pub(crate) fn opt_object(&mut self, k: &'static str) -> Result<Option<Value>> {
        self.opt_value(k).map(|v| object(k, &v)).transpose()
    }
    pub(crate) fn req_object(&mut self, k: &'static str) -> Result<Value> {
        self.opt_object(k)?.ok_or_else(|| ids::invalid(k))
    }
    pub(crate) fn req_array(&mut self, k: &'static str) -> Result<Vec<Value>> {
        self.raw(k).and_then(Value::as_array).cloned().ok_or_else(|| ids::invalid(k))
    }
    pub(crate) fn opt_array(&mut self, k: &'static str) -> Result<Option<Vec<Value>>> {
        match self.raw(k) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::Array(a)) => Ok(Some(a.clone())),
            Some(_) => Err(ids::invalid(k)),
        }
    }
    pub(crate) fn literal(&mut self, k: &'static str, expected: &str) -> Result<()> {
        if self.raw(k).and_then(Value::as_str) == Some(expected) {
            Ok(())
        } else {
            Err(ids::invalid(k))
        }
    }
    pub(crate) fn done(self) -> Result<()> {
        if self.map.keys().all(|k| self.used.contains(&k.as_str())) {
            Ok(())
        } else {
            Err("invalid_input".into())
        }
    }
}

pub(crate) fn strings(field: &str, items: &[Value]) -> Result<Vec<String>> {
    items.iter().map(|v| v.as_str().map(str::to_string).ok_or_else(|| ids::invalid(field))).collect()
}
/// Set `key` when `value` is present.
pub(crate) fn put<T: Into<Value>>(v: &mut Value, key: &str, value: Option<T>) {
    if let Some(value) = value {
        v[key] = value.into();
    }
}
pub(crate) fn object(field: &str, v: &Value) -> Result<Value> {
    Some(v).filter(|v| v.is_object()).cloned().ok_or_else(|| ids::invalid(field))
}
pub(crate) fn one_of<'a>(field: &str, value: &'a str, allowed: &[&str]) -> Result<&'a str> {
    if allowed.contains(&value) {
        Ok(value)
    } else {
        Err(ids::invalid(field))
    }
}
pub(crate) fn timestamp<'a>(field: &str, value: &'a str) -> Result<&'a str> {
    chrono::DateTime::parse_from_rfc3339(value).map(|_| value).map_err(|_| ids::invalid(field))
}
/// An integer in `min..=max`.
pub(crate) fn int(field: &str, value: u64, min: u64, max: u64) -> Result<u64> {
    if (min..=max).contains(&value) {
        Ok(value)
    } else {
        Err(ids::invalid(field))
    }
}
/// A finite number in `min..=max`.
pub(crate) fn number(field: &str, value: f64, min: f64, max: f64) -> Result<f64> {
    if value.is_finite() && (min..=max).contains(&value) {
        Ok(value)
    } else {
        Err(ids::invalid(field))
    }
}
/// Text that may be empty but has a cap and no NUL (bodies, terminal data…).
pub(crate) fn bounded<'a>(field: &str, value: &'a str, max: usize) -> Result<&'a str> {
    if value.chars().count() > max || value.contains('\0') {
        Err(ids::invalid(field))
    } else {
        Ok(value)
    }
}
