//! Core hash function for JSON values.
//!
//! Mirrors `packages/json-joy/src/json-hash/hash.ts`.
//!
//! Algorithm: FNV-1a style with 32-bit wrapping arithmetic. Uses the same
//! constant values and per-type discriminators as the upstream TypeScript.
//!
//! Note: string lengths and character codes use UTF-16 semantics to match
//! the upstream (JavaScript strings are UTF-16 internally).

use serde_json::{Map, Value};

// ── Type discriminators ────────────────────────────────────────────────────

pub const START_STATE: i32 = 5381;

pub const NULL_CONST: i32 = 982452847_u32 as i32;
pub const TRUE_CONST: i32 = 982453247_u32 as i32;
pub const FALSE_CONST: i32 = 982454243_u32 as i32;
pub const ARRAY_CONST: i32 = 982452259_u32 as i32;
pub const STRING_CONST: i32 = 982453601_u32 as i32;
pub const OBJECT_CONST: i32 = 982454533_u32 as i32;
pub const BINARY_CONST: i32 = 982454837_u32 as i32;

// ── Hash update functions ─────────────────────────────────────────────────

/// Mix a single integer into the hash state.
///
/// `state = (state << 5) + state + num` with 32-bit wrapping semantics.
pub fn update_num(state: i32, num: i32) -> i32 {
    state.wrapping_shl(5).wrapping_add(state).wrapping_add(num)
}

/// Mix a UTF-8 string into the hash state using UTF-16 code unit iteration
/// (to match JavaScript's `charCodeAt`).
///
/// In JavaScript, string length and character codes are UTF-16. We replicate
/// that here so the hash values match exactly for multi-byte characters.
pub fn update_str(mut state: i32, s: &str) -> i32 {
    let utf16: Vec<u16> = s.encode_utf16().collect();
    let length = utf16.len() as i32;
    state = update_num(state, STRING_CONST);
    state = update_num(state, length);
    // Iterate in reverse (matches: `while (i) state = ... str.charCodeAt(--i)`)
    for &code_unit in utf16.iter().rev() {
        state = update_num(state, code_unit as i32);
    }
    state
}

/// Mix a binary blob into the hash state.
pub fn update_bin(mut state: i32, bin: &[u8]) -> i32 {
    let length = bin.len() as i32;
    state = update_num(state, BINARY_CONST);
    state = update_num(state, length);
    for &b in bin.iter().rev() {
        state = update_num(state, b as i32);
    }
    state
}

/// Mix any JSON value into the hash state.
pub fn update_json(state: i32, json: &Value) -> i32 {
    match json {
        Value::Number(n) => {
            // TypeScript: case 'number': return updateNum(state, json)
            // For integers that fit in i32, this is exact. Large integers
            // or floats are approximated by casting through f64 → i32.
            let num = n.as_f64().unwrap_or(0.0) as i32;
            update_num(state, num)
        }
        Value::String(s) => {
            // TypeScript first adds STRING discriminator, then calls updateStr
            // (which adds STRING discriminator again internally).
            let state = update_num(state, STRING_CONST);
            update_str(state, s)
        }
        Value::Null => update_num(state, NULL_CONST),
        Value::Bool(b) => update_num(state, if *b { TRUE_CONST } else { FALSE_CONST }),
        Value::Array(arr) => {
            let mut state = update_num(state, ARRAY_CONST);
            for v in arr {
                state = update_json(state, v);
            }
            state
        }
        Value::Object(map) => update_json_object(state, map),
    }
}

/// Mix a JSON object into the hash state (with sorted keys, matching
/// the upstream insertion sort on `Object.keys`).
pub fn update_json_object(state: i32, map: &Map<String, Value>) -> i32 {
    let mut state = update_num(state, OBJECT_CONST);
    let mut keys: Vec<&String> = map.keys().collect();
    keys.sort(); // insertion sort and lexicographic sort agree for ASCII keys
    for key in keys {
        state = update_str(state, key);
        state = update_json(state, &map[key]);
    }
    state
}

/// Hash any JSON value, returning a Uint32 result (matching `hash(json) >>> 0`).
pub fn hash(json: &Value) -> u32 {
    update_json(START_STATE, json) as u32
}

/// Hash a string directly.
pub fn hash_str(s: &str) -> u32 {
    // Matches: `hash(str)` in TypeScript where str is a string value
    let state = update_num(START_STATE, STRING_CONST);
    let state = update_str(state, s);
    state as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn hash_null() {
        let h = hash(&json!(null));
        // hash(null) = updateJson(5381, null) = updateNum(5381, NULL_CONST)
        let expected = update_num(START_STATE, NULL_CONST) as u32;
        assert_eq!(h, expected);
    }

    #[test]
    fn hash_bool_true() {
        let h = hash(&json!(true));
        let expected = update_num(START_STATE, TRUE_CONST) as u32;
        assert_eq!(h, expected);
    }

    #[test]
    fn hash_bool_false() {
        let h = hash(&json!(false));
        let expected = update_num(START_STATE, FALSE_CONST) as u32;
        assert_eq!(h, expected);
    }

    #[test]
    fn hash_number_zero() {
        let h = hash(&json!(0));
        let expected = update_num(START_STATE, 0) as u32;
        assert_eq!(h, expected);
    }

    #[test]
    fn hash_empty_string() {
        let h = hash(&json!(""));
        assert_ne!(h, 0); // should produce a non-zero hash
    }

    #[test]
    fn hash_same_object_regardless_of_key_order() {
        // Hash should be order-independent for object keys
        let v1 = json!({"a": 1, "b": 2});
        let v2 = json!({"b": 2, "a": 1});
        // Note: serde_json::json! preserves insertion order, but hash sorts keys
        assert_eq!(hash(&v1), hash(&v2));
    }

    #[test]
    fn hash_empty_array_ne_empty_object() {
        assert_ne!(hash(&json!([])), hash(&json!({})));
    }

    #[test]
    fn hash_different_values_differ() {
        assert_ne!(hash(&json!(1)), hash(&json!(2)));
        assert_ne!(hash(&json!("a")), hash(&json!("b")));
        assert_ne!(hash(&json!(null)), hash(&json!(false)));
    }

    #[test]
    fn update_num_basic() {
        // Verify: (5381 << 5) + 5381 + 982452847 == 987630420
        let result = update_num(START_STATE, NULL_CONST);
        assert_eq!(
            result,
            (5381_i32 << 5).wrapping_add(5381).wrapping_add(NULL_CONST)
        );
    }
}
