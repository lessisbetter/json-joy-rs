//! Structural hash — a printable ASCII string representation of a JSON value.
//!
//! Mirrors `packages/json-joy/src/json-hash/structHash.ts`.
//!
//! Unlike the numeric hash, this preserves *spatial information*: each node
//! in the JSON tree produces its own hash token so the structure is visible.
//! Keys are sorted lexicographically so the result is independent of object
//! key insertion order.

use serde_json::Value;

use super::hash::{hash, hash_str};

/// Produce a structural hash string for a JSON value.
///
/// The result contains only printable ASCII characters (no newline).
///
/// - `null` → `"N"`
/// - `true`/`false` → `"T"` / `"F"`
/// - numbers → base-36 representation
/// - strings → 32-bit hash in base-36
/// - arrays → `"[h1;h2;...;]"`
/// - objects → `"{kh1:vh1,kh2:vh2,...,}"` (keys sorted, hashed)
/// - binary (bytes) → 32-bit hash in base-36
pub fn struct_hash(val: &Value) -> String {
    match val {
        Value::String(s) => {
            // String → hash the string value, encode as base-36
            radix_36(hash_str(s) as u64)
        }
        Value::Number(n) => {
            // Numbers encode as base-36; integers exact, floats approximated.
            if let Some(i) = n.as_i64() {
                // Negative numbers: TypeScript uses .toString(36) which gives "-1r"
                // for -1. In Rust, we replicate that for small negative integers.
                if i < 0 {
                    format!("-{}", radix_36((-i) as u64))
                } else {
                    radix_36(i as u64)
                }
            } else if let Some(u) = n.as_u64() {
                radix_36(u)
            } else {
                // Float: format the f64 as base-36 approximation of integer part
                let f = n.as_f64().unwrap_or(0.0);
                if f < 0.0 {
                    format!("-{}", radix_36((-f) as u64))
                } else {
                    radix_36(f as u64)
                }
            }
        }
        Value::Bool(b) => if *b { "T".to_string() } else { "F".to_string() },
        Value::Null => "N".to_string(),
        Value::Array(arr) => {
            let mut res = String::from("[");
            for v in arr {
                res.push_str(&struct_hash(v));
                res.push(';');
            }
            res.push(']');
            res
        }
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let mut res = String::from("{");
            for key in keys {
                // Key hash in base-36, then colon, then value hash, then comma
                res.push_str(&radix_36(hash_str(key) as u64));
                res.push(':');
                res.push_str(&struct_hash(&map[key]));
                res.push(',');
            }
            res.push('}');
            res
        }
    }
}

/// Encode a u64 in base-36 using lowercase letters (matches JS `.toString(36)`).
fn radix_36(mut n: u64) -> String {
    if n == 0 { return "0".to_string(); }
    const DIGITS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut buf = Vec::new();
    while n > 0 {
        buf.push(DIGITS[(n % 36) as usize]);
        n /= 36;
    }
    buf.reverse();
    String::from_utf8(buf).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn struct_hash_null() {
        assert_eq!(struct_hash(&json!(null)), "N");
    }

    #[test]
    fn struct_hash_bool() {
        assert_eq!(struct_hash(&json!(true)), "T");
        assert_eq!(struct_hash(&json!(false)), "F");
    }

    #[test]
    fn struct_hash_zero() {
        assert_eq!(struct_hash(&json!(0)), "0");
    }

    #[test]
    fn struct_hash_array() {
        let h = struct_hash(&json!([null, true]));
        assert!(h.starts_with('['));
        assert!(h.ends_with(']'));
        assert!(h.contains(';'));
    }

    #[test]
    fn struct_hash_object_sorted() {
        // Key order shouldn't matter
        let v1 = json!({"a": 1, "b": 2});
        let v2 = json!({"b": 2, "a": 1});
        assert_eq!(struct_hash(&v1), struct_hash(&v2));
    }

    #[test]
    fn struct_hash_empty_array() {
        assert_eq!(struct_hash(&json!([])), "[]");
    }

    #[test]
    fn struct_hash_empty_object() {
        assert_eq!(struct_hash(&json!({})), "{}");
    }

    #[test]
    fn radix_36_values() {
        assert_eq!(radix_36(0), "0");
        assert_eq!(radix_36(1), "1");
        assert_eq!(radix_36(35), "z");
        assert_eq!(radix_36(36), "10");
        assert_eq!(radix_36(255), "73");
    }
}
