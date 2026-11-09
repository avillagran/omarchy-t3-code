//! Lossless JavaScript strings for JSON boundaries. No replacement characters.
use crate::Result;
use serde_json::value::RawValue;

/// JavaScript strings are UTF-16 code units, including isolated surrogates.
/// Rust `String` and `serde_json::Value::String` cannot represent that domain.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Wtf16String(Vec<u16>);
impl Wtf16String {
    pub fn from_units(units: impl Into<Vec<u16>>) -> Self {
        Self(units.into())
    }
    pub fn units(&self) -> &[u16] {
        &self.0
    }
    pub fn into_units(self) -> Vec<u16> {
        self.0
    }
    /// Validate complete JSON syntax first, then decode each escape as a unit.
    /// A high/low pair stays two units, and a lone unit is never repaired.
    pub fn from_json(value: &RawValue) -> Result<Self> {
        let text = value.get();
        if !text.starts_with('"') || !text.ends_with('"') {
            return Err("invalid_utf16_json_string".into());
        }
        let mut chars = text[1..text.len() - 1].chars();
        let mut units = Vec::new();
        while let Some(ch) = chars.next() {
            if ch != '\\' {
                let mut pair = [0; 2];
                units.extend_from_slice(ch.encode_utf16(&mut pair));
                continue;
            }
            let escape = chars.next().ok_or("invalid_utf16_json_string")?;
            let unit = match escape {
                '"' => 0x22,
                '\\' => 0x5c,
                '/' => 0x2f,
                'b' => 8,
                'f' => 12,
                'n' => 10,
                'r' => 13,
                't' => 9,
                'u' => {
                    let mut value = 0u16;
                    for _ in 0..4 {
                        let digit = chars
                            .next()
                            .and_then(|ch| ch.to_digit(16))
                            .ok_or("invalid_utf16_json_string")?;
                        value = (value << 4) | digit as u16;
                    }
                    value
                }
                _ => return Err("invalid_utf16_json_string".into()),
            };
            units.push(unit);
        }
        Ok(Self(units))
    }
    /// Escaping all units keeps the output valid UTF-8 JSON without changing
    /// any JavaScript character, including NUL, lone high and lone low units.
    pub fn to_json(&self) -> Box<RawValue> {
        use std::fmt::Write;
        let mut output = String::with_capacity(self.0.len().saturating_mul(6).saturating_add(2));
        output.push('"');
        for unit in &self.0 {
            write!(&mut output, "\\u{unit:04x}").expect("String writing cannot fail");
        }
        output.push('"');
        RawValue::from_string(output).expect("Escaped UTF-16 units are valid JSON")
    }
    pub fn to_utf8(&self) -> Result<String> {
        String::from_utf16(&self.0).map_err(|_| "unpaired_utf16_surrogate".into())
    }
    /// ECMA-262 WhiteSpace and LineTerminator, exactly String.prototype.trim.
    /// This deliberately differs from Rust's Unicode White_Space predicate.
    pub fn trim_ecmascript(&self) -> Self {
        fn whitespace(unit: u16) -> bool {
            matches!(unit,0x0009..=0x000d|0x0020|0x00a0|0x1680|0x2000..=0x200a|0x2028|0x2029|0x202f|0x205f|0x3000|0xfeff)
        }
        let start = self
            .0
            .iter()
            .position(|u| !whitespace(*u))
            .unwrap_or(self.0.len());
        let end = self
            .0
            .iter()
            .rposition(|u| !whitespace(*u))
            .map_or(start, |i| i + 1);
        Self(self.0[start..end].to_vec())
    }
}
impl From<&str> for Wtf16String {
    fn from(value: &str) -> Self {
        Self(value.encode_utf16().collect())
    }
}
