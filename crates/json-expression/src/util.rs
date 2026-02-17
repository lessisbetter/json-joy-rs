//! Utility helpers — mirrors upstream `util.ts`.

use crate::error::JsError;
use crate::types::JsValue;
use crate::vars::Vars;
use json_joy_json_pointer::{get, parse_json_pointer};
use json_joy_util::deep_equal;
use serde_json::Value;

// ----------------------------------------------------------------- Input helpers

/// Resolves a variable path string to a value.
///
/// Mirrors upstream `get(path, data)` (but note: upstream passes `data` then `path` — util.rs
/// wraps this). Here we expose the same logic used by input operators.
pub fn get_path(path: &str, data: &JsValue) -> Option<JsValue> {
    match data {
        JsValue::Json(v) => {
            let parsed = parse_json_pointer(path);
            get(v, &parsed).map(|v| JsValue::Json(v.clone()))
        }
        _ => None,
    }
}

/// Throws `NOT_FOUND` if value is `undefined` and no default was given.
pub fn throw_on_undef(value: JsValue, def: Option<JsValue>) -> Result<JsValue, JsError> {
    if value != JsValue::Undefined {
        return Ok(value);
    }
    match def {
        Some(d) if d != JsValue::Undefined => Ok(d),
        _ => Err(JsError::NotFound),
    }
}

// ----------------------------------------------------------------- Type helpers

/// Returns the JavaScript type name of a value.
///
/// Mirrors upstream `type(value)`.
pub fn js_type(value: &JsValue) -> &'static str {
    match value {
        JsValue::Undefined => "undefined",
        JsValue::Binary(_) => "binary",
        JsValue::Json(v) => match v {
            Value::Null => "null",
            Value::Bool(_) => "boolean",
            Value::Number(_) => "number",
            Value::String(_) => "string",
            Value::Array(_) => "array",
            Value::Object(_) => "object",
        },
    }
}

/// Converts a value to a number. Returns 0.0 for NaN/undefined.
///
/// Mirrors upstream `num(value) = +(value as number) || 0`.
pub fn num(value: &JsValue) -> f64 {
    let n = match value {
        JsValue::Undefined | JsValue::Binary(_) => f64::NAN,
        JsValue::Json(v) => match v {
            Value::Null => 0.0,
            Value::Bool(b) => {
                if *b {
                    1.0
                } else {
                    0.0
                }
            }
            Value::Number(n) => n.as_f64().unwrap_or(f64::NAN),
            Value::String(s) => s.trim().parse::<f64>().unwrap_or(f64::NAN),
            Value::Array(_) | Value::Object(_) => f64::NAN,
        },
    };
    if n.is_nan() { 0.0 } else { n }
}

/// Truncates a value to an i32. Mirrors JS `~~value` (ToInt32: wrapping, not saturating).
pub fn int(value: &JsValue) -> i32 {
    (num(value) as i64 as u32) as i32
}

/// Converts a value to a string. Mirrors upstream `str(value)`.
pub fn str_val(value: &JsValue) -> String {
    match value {
        JsValue::Undefined => "undefined".to_string(),
        JsValue::Binary(_) => "[object Uint8Array]".to_string(),
        JsValue::Json(v) => match v {
            Value::Null => "null".to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Number(n) => n.to_string(),
            Value::String(s) => s.clone(),
            // Objects/arrays → JSON
            _ => v.to_string(),
        },
    }
}

// -------------------------------------------------------------- Comparison helpers

pub fn cmp(a: &JsValue, b: &JsValue) -> i64 {
    // Mirrors upstream: a > b ? 1 : a < b ? -1 : 0
    match (a, b) {
        (JsValue::Json(Value::Number(na)), JsValue::Json(Value::Number(nb))) => {
            let fa = na.as_f64().unwrap_or(0.0);
            let fb = nb.as_f64().unwrap_or(0.0);
            if fa > fb { 1 } else if fa < fb { -1 } else { 0 }
        }
        _ => {
            let sa = str_val(a);
            let sb = str_val(b);
            if sa > sb { 1 } else if sa < sb { -1 } else { 0 }
        }
    }
}

pub fn js_gt(a: &JsValue, b: &JsValue) -> bool {
    match (a, b) {
        (JsValue::Json(Value::Number(na)), JsValue::Json(Value::Number(nb))) => {
            na.as_f64().unwrap_or(0.0) > nb.as_f64().unwrap_or(0.0)
        }
        _ => str_val(a) > str_val(b),
    }
}

pub fn js_gte(a: &JsValue, b: &JsValue) -> bool {
    match (a, b) {
        (JsValue::Json(Value::Number(na)), JsValue::Json(Value::Number(nb))) => {
            na.as_f64().unwrap_or(0.0) >= nb.as_f64().unwrap_or(0.0)
        }
        _ => str_val(a) >= str_val(b),
    }
}

pub fn js_lt(a: &JsValue, b: &JsValue) -> bool {
    match (a, b) {
        (JsValue::Json(Value::Number(na)), JsValue::Json(Value::Number(nb))) => {
            na.as_f64().unwrap_or(0.0) < nb.as_f64().unwrap_or(0.0)
        }
        _ => str_val(a) < str_val(b),
    }
}

pub fn js_lte(a: &JsValue, b: &JsValue) -> bool {
    match (a, b) {
        (JsValue::Json(Value::Number(na)), JsValue::Json(Value::Number(nb))) => {
            na.as_f64().unwrap_or(0.0) <= nb.as_f64().unwrap_or(0.0)
        }
        _ => str_val(a) <= str_val(b),
    }
}

pub fn between_ne_ne(val: &JsValue, min: &JsValue, max: &JsValue) -> bool {
    js_gt(val, min) && js_lt(val, max)
}

pub fn between_ne_eq(val: &JsValue, min: &JsValue, max: &JsValue) -> bool {
    js_gt(val, min) && js_lte(val, max)
}

pub fn between_eq_ne(val: &JsValue, min: &JsValue, max: &JsValue) -> bool {
    js_gte(val, min) && js_lt(val, max)
}

pub fn between_eq_eq(val: &JsValue, min: &JsValue, max: &JsValue) -> bool {
    js_gte(val, min) && js_lte(val, max)
}

// -------------------------------------------------------------- Arithmetic helpers

pub fn slash(a: &JsValue, b: &JsValue) -> Result<JsValue, JsError> {
    let divisor = num(b);
    if divisor == 0.0 {
        return Err(JsError::DivisionByZero);
    }
    let res = num(a) / divisor;
    Ok(f64_to_jsval(if res.is_finite() { res } else { 0.0 }))
}

pub fn modulo(a: &JsValue, b: &JsValue) -> Result<JsValue, JsError> {
    let divisor = num(b);
    if divisor == 0.0 {
        return Err(JsError::DivisionByZero);
    }
    let res = num(a) % divisor;
    Ok(f64_to_jsval(if res.is_finite() { res } else { 0.0 }))
}

pub fn f64_to_jsval(n: f64) -> JsValue {
    match serde_json::Number::from_f64(n) {
        Some(num) => JsValue::Json(Value::Number(num)),
        None => JsValue::Json(Value::Null), // NaN/Infinity → null (matches JS JSON.stringify)
    }
}

pub fn i32_to_jsval(n: i32) -> JsValue {
    JsValue::Json(Value::Number(serde_json::Number::from(n)))
}

pub fn i64_to_jsval(n: i64) -> JsValue {
    JsValue::Json(Value::Number(serde_json::Number::from(n)))
}

// ------------------------------------------------------------ Container helpers

pub fn len(value: &JsValue) -> JsValue {
    let n: usize = match value {
        JsValue::Json(v) => match v {
            Value::String(s) => s.chars().count(),
            Value::Array(a) => a.len(),
            Value::Object(o) => o.len(),
            Value::Null => 0,
            _ => 0,
        },
        JsValue::Binary(b) => b.len(),
        JsValue::Undefined => 0,
    };
    i64_to_jsval(n as i64)
}

pub fn member(container: &JsValue, index: &JsValue) -> Result<JsValue, JsError> {
    match container {
        JsValue::Json(Value::String(s)) => {
            let i = int(index);
            let char_len = s.chars().count();
            if i < 0 || i as usize >= char_len {
                return Ok(JsValue::Undefined);
            }
            let ch = s.chars().nth(i as usize).unwrap(); // safe: bounds checked above
            Ok(JsValue::Json(Value::String(ch.to_string())))
        }
        JsValue::Json(Value::Array(arr)) => {
            let i = int(index);
            if i < 0 || i as usize >= arr.len() {
                return Ok(JsValue::Undefined);
            }
            Ok(JsValue::Json(arr[i as usize].clone()))
        }
        JsValue::Binary(b) => {
            let i = int(index);
            if i < 0 || i as usize >= b.len() {
                return Ok(JsValue::Undefined);
            }
            Ok(i64_to_jsval(b[i as usize] as i64))
        }
        JsValue::Json(Value::Object(obj)) => match index {
            JsValue::Json(Value::String(k)) => {
                Ok(obj.get(k.as_str()).map(|v| JsValue::Json(v.clone())).unwrap_or(JsValue::Undefined))
            }
            JsValue::Json(Value::Number(n)) => {
                let k = n.to_string();
                Ok(obj.get(k.as_str()).map(|v| JsValue::Json(v.clone())).unwrap_or(JsValue::Undefined))
            }
            _ => Err(JsError::NotStringIndex),
        },
        JsValue::Json(Value::Null) | JsValue::Undefined => Err(JsError::NotContainer),
        _ => Err(JsError::NotContainer),
    }
}

pub fn as_bin(value: &JsValue) -> Result<&Vec<u8>, JsError> {
    match value {
        JsValue::Binary(b) => Ok(b),
        _ => Err(JsError::NotBinary),
    }
}

// ---------------------------------------------------------- String helpers

pub fn as_str(value: &JsValue) -> Result<&str, JsError> {
    match value {
        JsValue::Json(Value::String(s)) => Ok(s.as_str()),
        _ => Err(JsError::NotString),
    }
}

pub fn starts(outer: &JsValue, inner: &JsValue) -> bool {
    str_val(outer).starts_with(&str_val(inner))
}

pub fn contains(outer: &JsValue, inner: &JsValue) -> bool {
    str_val(outer).contains(&str_val(inner) as &str)
}

pub fn ends(outer: &JsValue, inner: &JsValue) -> bool {
    str_val(outer).ends_with(&str_val(inner))
}

pub fn substr(s: &JsValue, from: &JsValue, to: &JsValue) -> JsValue {
    let string = str_val(s);
    let chars: Vec<char> = string.chars().collect();
    let len = chars.len() as i32;
    let from_i = int(from);
    let to_i = int(to);

    // Mirror JS str.slice(): negative indices count from end
    let start = if from_i < 0 { (len + from_i).max(0) as usize } else { from_i.min(len) as usize };
    let end = if to_i < 0 { (len + to_i).max(0) as usize } else { to_i.min(len) as usize };

    let result: String = chars[start..end].iter().collect();
    JsValue::Json(Value::String(result))
}

// Format validators (mirrors upstream regexes in util.ts)

fn email_regex() -> &'static regex::Regex {
    use std::sync::OnceLock;
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(
            r"(?i)^[a-z0-9.!#$%&'*+/=?^_`{|}~-]+@[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?)*$"
        ).unwrap()
    })
}

fn hostname_regex() -> &'static regex::Regex {
    use std::sync::OnceLock;
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(
            r"(?i)^(?=.{1,253}\.?$)[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?(?:\.[a-z0-9](?:[-0-9a-z]{0,61}[0-9a-z])?)*\.?$"
        ).unwrap()
    })
}

fn ip4_regex() -> &'static regex::Regex {
    use std::sync::OnceLock;
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(
            r"^(?:(?:25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)\.){3}(?:25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)$"
        ).unwrap()
    })
}

fn ip6_regex() -> &'static regex::Regex {
    use std::sync::OnceLock;
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(
            r"(?i)^((([0-9a-f]{1,4}:){7}([0-9a-f]{1,4}|:))|(([0-9a-f]{1,4}:){6}(:[0-9a-f]{1,4}|((25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)(\.(25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)){3})|:))|(([0-9a-f]{1,4}:){5}(((:[0-9a-f]{1,4}){1,2})|:((25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)(\.(25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)){3})|:))|(([0-9a-f]{1,4}:){4}(((:[0-9a-f]{1,4}){1,3})|((:[0-9a-f]{1,4})?:((25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)(\.(25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)){3}))|:))|(([0-9a-f]{1,4}:){3}(((:[0-9a-f]{1,4}){1,4})|((:[0-9a-f]{1,4}){0,2}:((25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)(\.(25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)){3}))|:))|(([0-9a-f]{1,4}:){2}(((:[0-9a-f]{1,4}){1,5})|((:[0-9a-f]{1,4}){0,3}:((25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)(\.(25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)){3}))|:))|(([0-9a-f]{1,4}:){1}(((:[0-9a-f]{1,4}){1,6})|((:[0-9a-f]{1,4}){0,4}:((25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)(\.(25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)){3}))|:))|(:(((:[0-9a-f]{1,4}){1,7})|((:[0-9a-f]{1,4}){0,5}:((25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)(\.(25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)){3}))|:)))$"
        ).unwrap()
    })
}

fn uuid_regex() -> &'static regex::Regex {
    use std::sync::OnceLock;
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(
            r"(?i)^(?:urn:uuid:)?[0-9a-f]{8}-(?:[0-9a-f]{4}-){3}[0-9a-f]{12}$"
        ).unwrap()
    })
}

fn uri_regex() -> &'static regex::Regex {
    use std::sync::OnceLock;
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(
            r#"(?i)^(?:[a-z][a-z0-9+\-.]*:)(?:\/?\/(?:(?:[a-z0-9\-._~!$&'()*+,;=:]|%[0-9a-f]{2})*@)?(?:\[(?:(?:(?:(?:[0-9a-f]{1,4}:){6}|::(?:[0-9a-f]{1,4}:){5}|(?:[0-9a-f]{1,4})?::(?:[0-9a-f]{1,4}:){4}|(?:(?:[0-9a-f]{1,4}:){0,1}[0-9a-f]{1,4})?::(?:[0-9a-f]{1,4}:){3}|(?:(?:[0-9a-f]{1,4}:){0,2}[0-9a-f]{1,4})?::(?:[0-9a-f]{1,4}:){2}|(?:(?:[0-9a-f]{1,4}:){0,3}[0-9a-f]{1,4})?::[0-9a-f]{1,4}:|(?:(?:[0-9a-f]{1,4}:){0,4}[0-9a-f]{1,4})?::)(?:[0-9a-f]{1,4}:[0-9a-f]{1,4}|(?:(?:25[0-5]|2[0-4]\d|[01]?\d\d?)\.){3}(?:25[0-5]|2[0-4]\d|[01]?\d\d?))|(?:(?:[0-9a-f]{1,4}:){0,5}[0-9a-f]{1,4})?::[0-9a-f]{1,4}|(?:(?:[0-9a-f]{1,4}:){0,6}[0-9a-f]{1,4})?::)|[Vv][0-9a-f]+\.[a-z0-9\-._~!$&'()*+,;=:]+)\]|(?:(?:25[0-5]|2[0-4]\d|[01]?\d\d?)\.){3}(?:25[0-5]|2[0-4]\d|[01]?\d\d?)|(?:[a-z0-9\-._~!$&'()*+,;=]|%[0-9a-f]{2})*)(?::\d*)?(?:\/(?:[a-z0-9\-._~!$&'()*+,;=:@]|%[0-9a-f]{2})*)*|\/(?:(?:[a-z0-9\-._~!$&'()*+,;=:@]|%[0-9a-f]{2})+(?:\/(?:[a-z0-9\-._~!$&'()*+,;=:@]|%[0-9a-f]{2})*)*)?|(?:[a-z0-9\-._~!$&'()*+,;=:@]|%[0-9a-f]{2})+(?:\/(?:[a-z0-9\-._~!$&'()*+,;=:@]|%[0-9a-f]{2})*)*)(?:\?(?:[a-z0-9\-._~!$&'()*+,;=:@/?]|%[0-9a-f]{2})*)?(?:#(?:[a-z0-9\-._~!$&'()*+,;=:@/?]|%[0-9a-f]{2})*)?$"#
        ).unwrap()
    })
}

fn not_uri_fragment_regex() -> &'static regex::Regex {
    use std::sync::OnceLock;
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"[/:]").unwrap())
}

fn duration_regex() -> &'static regex::Regex {
    use std::sync::OnceLock;
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(r"^P(?!$)((\d+Y)?(\d+M)?(\d+D)?(T(?=\d)(\d+H)?(\d+M)?(\d+S)?)?|(\d+W)?)$").unwrap()
    })
}

fn date_regex() -> &'static regex::Regex {
    use std::sync::OnceLock;
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"^(\d{4})-(\d{2})-(\d{2})$").unwrap())
}

fn time_regex() -> &'static regex::Regex {
    use std::sync::OnceLock;
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(r"(?i)^(\d{2}):(\d{2}):(\d{2}(?:\.\d+)?)(z|([+-])(\d{2})(?::?(\d{2}))?)?$").unwrap()
    })
}

pub fn is_email(value: &JsValue) -> bool {
    match value {
        JsValue::Json(Value::String(s)) => email_regex().is_match(s),
        _ => false,
    }
}

pub fn is_hostname(value: &JsValue) -> bool {
    match value {
        JsValue::Json(Value::String(s)) => hostname_regex().is_match(s),
        _ => false,
    }
}

pub fn is_ip4(value: &JsValue) -> bool {
    match value {
        JsValue::Json(Value::String(s)) => ip4_regex().is_match(s),
        _ => false,
    }
}

pub fn is_ip6(value: &JsValue) -> bool {
    match value {
        JsValue::Json(Value::String(s)) => ip6_regex().is_match(s),
        _ => false,
    }
}

pub fn is_uuid(value: &JsValue) -> bool {
    match value {
        JsValue::Json(Value::String(s)) => uuid_regex().is_match(s),
        _ => false,
    }
}

pub fn is_uri(value: &JsValue) -> bool {
    match value {
        JsValue::Json(Value::String(s)) => {
            not_uri_fragment_regex().is_match(s) && uri_regex().is_match(s)
        }
        _ => false,
    }
}

pub fn is_duration(value: &JsValue) -> bool {
    match value {
        JsValue::Json(Value::String(s)) => duration_regex().is_match(s),
        _ => false,
    }
}

const DAYS: [u32; 13] = [0, 31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

fn is_leap_year(year: u32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

pub fn is_date(value: &JsValue) -> bool {
    let s = match value {
        JsValue::Json(Value::String(s)) => s,
        _ => return false,
    };
    let caps = match date_regex().captures(s) {
        Some(c) => c,
        None => return false,
    };
    let year: u32 = caps[1].parse().unwrap_or(0);
    let month: u32 = caps[2].parse().unwrap_or(0);
    let day: u32 = caps[3].parse().unwrap_or(0);
    if month < 1 || month > 12 {
        return false;
    }
    let max_day = if month == 2 && is_leap_year(year) {
        29
    } else {
        DAYS[month as usize]
    };
    day >= 1 && day <= max_day
}

pub fn is_time(value: &JsValue) -> bool {
    let s = match value {
        JsValue::Json(Value::String(s)) => s,
        _ => return false,
    };
    let caps = match time_regex().captures(s) {
        Some(c) => c,
        None => return false,
    };
    let hr: i32 = caps[1].parse().unwrap_or(99);
    let min: i32 = caps[2].parse().unwrap_or(99);
    let sec: f64 = caps[3].parse().unwrap_or(99.0);
    let tz = caps.get(4).map(|m| m.as_str());
    let tz_sign: i32 = if caps.get(5).map(|m| m.as_str()) == Some("-") { -1 } else { 1 };
    let tz_h: i32 = caps.get(6).and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
    let tz_m: i32 = caps.get(7).and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
    if tz_h > 23 || tz_m > 59 || tz.is_none() {
        return false;
    }
    if hr <= 23 && min <= 59 && sec < 60.0 {
        return true;
    }
    let utc_min = min - tz_m * tz_sign;
    let utc_hr = hr - tz_h * tz_sign - if utc_min < 0 { 1 } else { 0 };
    (utc_hr == 23 || utc_hr == -1) && (utc_min == 59 || utc_min == -1) && sec < 61.0
}

pub fn is_datetime(value: &JsValue) -> bool {
    let s = match value {
        JsValue::Json(Value::String(s)) => s,
        _ => return false,
    };
    // Split on 't', 'T', ' ' or '\t'
    let parts: Vec<&str> = s.splitn(2, |c: char| c == 't' || c == 'T' || c == ' ').collect();
    if parts.len() != 2 {
        return false;
    }
    is_date(&JsValue::Json(Value::String(parts[0].to_string())))
        && is_time(&JsValue::Json(Value::String(parts[1].to_string())))
}

// --------------------------------------------------------- Binary helpers

pub fn u8_val(bin: &JsValue, pos: &JsValue) -> Result<JsValue, JsError> {
    let buf = as_bin(bin)?;
    let index = int(pos);
    if index < 0 || index as usize >= buf.len() {
        return Err(JsError::OutOfBounds);
    }
    Ok(i64_to_jsval(buf[index as usize] as i64))
}

// ---------------------------------------------------------- Array helpers

pub fn as_arr(value: &JsValue) -> Result<&Vec<Value>, JsError> {
    match value {
        JsValue::Json(Value::Array(a)) => Ok(a),
        _ => Err(JsError::NotArray),
    }
}

pub fn head(operand1: &JsValue, operand2: &JsValue) -> Result<JsValue, JsError> {
    let arr = as_arr(operand1)?;
    let count = int(operand2);
    let result = if count >= 0 {
        arr[..count.min(arr.len() as i32) as usize].to_vec()
    } else {
        let start = (arr.len() as i32 + count).max(0) as usize;
        arr[start..].to_vec()
    };
    Ok(JsValue::Json(Value::Array(result)))
}

pub fn concat_arrays(arrays: &[JsValue]) -> Result<JsValue, JsError> {
    let mut result = Vec::new();
    for array in arrays {
        let arr = as_arr(array)?;
        result.extend(arr.iter().cloned());
    }
    Ok(JsValue::Json(Value::Array(result)))
}

pub fn is_in_arr(arr: &JsValue, what: &JsValue) -> Result<bool, JsError> {
    let arr2 = as_arr(arr)?;
    let what_val = match what {
        JsValue::Json(v) => v,
        _ => return Ok(false),
    };
    for item in arr2 {
        if deep_equal(item, what_val) {
            return Ok(true);
        }
    }
    Ok(false)
}

pub fn from_entries(maybe_entries: &JsValue) -> Result<JsValue, JsError> {
    let entries = as_arr(maybe_entries)?;
    let mut result = serde_json::Map::new();
    for maybe_entry in entries {
        let entry = match maybe_entry {
            Value::Array(a) => a,
            _ => return Err(JsError::NotArray),
        };
        if entry.len() != 2 {
            return Err(JsError::NotPair);
        }
        let key = str_val(&JsValue::Json(entry[0].clone()));
        result.insert(key, entry[1].clone());
    }
    Ok(JsValue::Json(Value::Object(result)))
}

pub fn index_of(container: &JsValue, item: &JsValue) -> Result<JsValue, JsError> {
    let arr = as_arr(container)?;
    let item_val = match item {
        JsValue::Json(v) => v,
        _ => return Ok(i64_to_jsval(-1)),
    };
    for (i, element) in arr.iter().enumerate() {
        if deep_equal(element, item_val) {
            return Ok(i64_to_jsval(i as i64));
        }
    }
    Ok(i64_to_jsval(-1))
}

pub fn zip(maybe_arr1: &JsValue, maybe_arr2: &JsValue) -> Result<JsValue, JsError> {
    let arr1 = as_arr(maybe_arr1)?;
    let arr2 = as_arr(maybe_arr2)?;
    let length = arr1.len().min(arr2.len());
    let mut result = Vec::with_capacity(length);
    for i in 0..length {
        result.push(Value::Array(vec![arr1[i].clone(), arr2[i].clone()]));
    }
    Ok(JsValue::Json(Value::Array(result)))
}

pub fn filter_arr(
    arr: &[Value],
    varname: &str,
    vars: &mut Vars,
    run: &mut dyn FnMut(&mut Vars) -> Result<JsValue, JsError>,
) -> Result<JsValue, JsError> {
    let mut result = Vec::new();
    for item in arr {
        vars.set(varname, JsValue::Json(item.clone()))?;
        let keep = run(vars)?;
        if is_truthy(&keep) {
            result.push(item.clone());
        }
    }
    vars.del(varname);
    Ok(JsValue::Json(Value::Array(result)))
}

pub fn map_arr(
    arr: &[Value],
    varname: &str,
    vars: &mut Vars,
    run: &mut dyn FnMut(&mut Vars) -> Result<JsValue, JsError>,
) -> Result<JsValue, JsError> {
    let mut result = Vec::with_capacity(arr.len());
    for item in arr {
        vars.set(varname, JsValue::Json(item.clone()))?;
        let mapped = run(vars)?;
        result.push(jsvalue_to_json(mapped));
    }
    vars.del(varname);
    Ok(JsValue::Json(Value::Array(result)))
}

pub fn reduce_arr(
    arr: &[Value],
    initial_value: JsValue,
    accname: &str,
    varname: &str,
    vars: &mut Vars,
    run: &mut dyn FnMut(&mut Vars) -> Result<JsValue, JsError>,
) -> Result<JsValue, JsError> {
    vars.set(accname, initial_value)?;
    for item in arr {
        vars.set(varname, JsValue::Json(item.clone()))?;
        let res = run(vars)?;
        vars.set(accname, res)?;
    }
    let result = vars.get(accname);
    vars.del(accname);
    vars.del(varname);
    Ok(result)
}

// ---------------------------------------------------------- Object helpers

pub fn as_obj(value: &JsValue) -> Result<&serde_json::Map<String, Value>, JsError> {
    match value {
        JsValue::Json(Value::Object(o)) => Ok(o),
        _ => {
            if js_type(value) == "object" {
                // only object type passes
                unreachable!()
            }
            Err(JsError::NotObject)
        }
    }
}

pub fn keys(value: &JsValue) -> Result<JsValue, JsError> {
    let obj = as_obj(value)?;
    let ks: Vec<Value> = obj.keys().map(|k| Value::String(k.clone())).collect();
    Ok(JsValue::Json(Value::Array(ks)))
}

pub fn values(value: &JsValue) -> Result<JsValue, JsError> {
    let obj = as_obj(value)?;
    let vs: Vec<Value> = obj.values().cloned().collect();
    Ok(JsValue::Json(Value::Array(vs)))
}

pub fn entries(value: &JsValue) -> Result<JsValue, JsError> {
    let obj = as_obj(value)?;
    let es: Vec<Value> = obj
        .iter()
        .map(|(k, v)| Value::Array(vec![Value::String(k.clone()), v.clone()]))
        .collect();
    Ok(JsValue::Json(Value::Array(es)))
}

pub fn obj_set_raw(
    obj: &mut serde_json::Map<String, Value>,
    key: &str,
    value: Value,
) -> Result<(), JsError> {
    if key == "__proto__" {
        return Err(JsError::ProtoKey);
    }
    obj.insert(key.to_string(), value);
    Ok(())
}

// ------------------------------------------------------------ Various

pub fn is_literal(value: &Value) -> bool {
    match value {
        Value::Array(a) => a.len() == 1,
        _ => true,
    }
}

pub fn as_literal(value: &Value) -> Result<&Value, JsError> {
    match value {
        Value::Array(a) => {
            if a.len() != 1 {
                return Err(JsError::InvalidLiteral);
            }
            Ok(&a[0])
        }
        other => Ok(other),
    }
}

/// Parses a variable path like `"varname/path/to/field"` into `(varname, pointer)`.
///
/// Mirrors upstream `parseVar(name)`.
pub fn parse_var(name: &str) -> (&str, &str) {
    if name.starts_with('/') {
        return ("", name);
    }
    match name.find('/') {
        None => (name, ""),
        Some(idx) => (&name[..idx], &name[idx..]),
    }
}

/// Deep clones a JSON-like value (mirrors upstream `clone` in array.ts).
pub fn deep_clone(v: &Value) -> Value {
    v.clone()
}

/// Returns true if a JsValue is truthy (like JS truthiness).
pub fn is_truthy(value: &JsValue) -> bool {
    match value {
        JsValue::Undefined => false,
        JsValue::Binary(b) => !b.is_empty(),
        JsValue::Json(v) => match v {
            Value::Null => false,
            Value::Bool(b) => *b,
            Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(false),
            Value::String(s) => !s.is_empty(),
            Value::Array(_) | Value::Object(_) => true,
        },
    }
}

/// Converts a JsValue to a serde_json::Value (undefined/binary → null).
pub fn jsvalue_to_json(v: JsValue) -> Value {
    match v {
        JsValue::Undefined => Value::Null,
        JsValue::Binary(_) => Value::Null,
        JsValue::Json(v) => v,
    }
}
