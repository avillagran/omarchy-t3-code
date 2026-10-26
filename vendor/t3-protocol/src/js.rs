//! ECMAScript's rules where the ports of ot3's JavaScript depend on them:
//! truthiness, `String(value)`, `\s` and `Math.round`. JSON in, so `undefined`
//! is a missing member and NaN never occurs.
use serde_json::Value;

/// ToBoolean: `[]` and `{}` are true, unlike `util::truth`.
pub fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::String(s) => !s.is_empty(),
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0),
        Value::Array(_) | Value::Object(_) => true,
    }
}

/// `String(value)`: arrays joined with commas (null inside as empty), objects
/// as `[object Object]`, numbers in JavaScript's shortest form.
pub fn string(v: &Value) -> String {
    match v {
        Value::Null => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::String(s) => s.clone(),
        Value::Number(n) => match (n.as_i64(), n.as_u64()) {
            (Some(i), _) => i.to_string(),
            (_, Some(u)) => number(u as f64),
            _ => number(n.as_f64().unwrap_or(0.0)),
        },
        Value::Array(a) => a.iter().map(|x| if x.is_null() { String::new() } else { string(x) }).collect::<Vec<_>>().join(","),
        Value::Object(_) => "[object Object]".into(),
    }
}

/// Number::toString(10) for a finite double.
pub fn number(f: f64) -> String {
    if f == 0.0 {
        return "0".into();
    }
    if f < 0.0 {
        return format!("-{}", number(-f));
    }
    // Shortest round-trip digits and exponent: f = 0.d1d2…dk × 10^n.
    let sci = format!("{f:e}");
    let (mantissa, exponent) = sci.split_once('e').unwrap();
    let digits = mantissa.replace('.', "");
    let k = digits.len() as i32;
    let n = exponent.parse::<i32>().unwrap() + 1;
    if k <= n && n <= 21 {
        format!("{digits}{}", "0".repeat((n - k) as usize))
    } else if 0 < n && n <= 21 {
        format!("{}.{}", &digits[..n as usize], &digits[n as usize..])
    } else if -6 < n && n <= 0 {
        format!("0.{}{digits}", "0".repeat((-n) as usize))
    } else {
        let e = n - 1;
        let sign = if e < 0 { '-' } else { '+' };
        let head = if k == 1 { digits.clone() } else { format!("{}.{}", &digits[..1], &digits[1..]) };
        format!("{head}e{sign}{}", e.abs())
    }
}

/// `\s` (and what `trim()` removes): WhiteSpace and LineTerminator.
pub fn is_space(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}')
}

/// `String(text || "").replace(/\s+/g, " ").trim()`.
pub fn compact(text: &Value) -> String {
    let s = if truthy(text) { string(text) } else { String::new() };
    s.split(is_space).filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn ecmascript_conversions() {
        assert!(truthy(&json!([])) && truthy(&json!({})) && truthy(&json!("0")) && !truthy(&json!(0)) && !truthy(&json!("")) && !truthy(&json!(null)));
        for (f, s) in [(1.0, "1"), (1.5, "1.5"), (0.1, "0.1"), (1e21, "1e+21"), (1e20, "100000000000000000000"), (1.5e-7, "1.5e-7"), (0.000001, "0.000001"), (123456.789, "123456.789"), (-2.5, "-2.5"), (2e-7, "2e-7")] {
            assert_eq!(number(f), s, "{f}");
        }
        assert_eq!(string(&json!([1, null, "a", [2, 3]])), "1,,a,2,3");
        assert_eq!(string(&json!({"a":1})), "[object Object]");
        assert_eq!(string(&json!(3)), "3");
        assert_eq!(compact(&json!("  a\u{feff}\tb\u{85}c \n")), "a b\u{85}c");
        assert_eq!(compact(&json!(null)), "");
    }
}
