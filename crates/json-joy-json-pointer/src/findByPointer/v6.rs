use serde_json::Value;

use crate::util::{is_valid_index, unescape_component};
use crate::JsonPointerError;

/// Upstream parity note:
/// - Upstream has multiple optimized `findByPointer` variants.
/// - This Rust implementation keeps one canonical traversal algorithm.
#[allow(unused_assignments)]
pub fn find_by_pointer_v6(
    pointer: &str,
    val: &Value,
) -> Result<(Option<Value>, String), JsonPointerError> {
    if pointer.is_empty() {
        return Ok((Some(val.clone()), String::new()));
    }

    let mut current: &Value = val;
    let mut obj: Option<Value> = None;
    let mut key = String::new();

    let mut start = 1;
    for (i, c) in pointer.char_indices() {
        if c == '/' && i > 0 {
            let component = &pointer[start..i];
            key = unescape_component(component);
            obj = Some(current.clone());

            match current {
                Value::Array(arr) => {
                    let idx: usize = if key == "-" {
                        arr.len()
                    } else {
                        if !is_valid_index(&key) {
                            return Err(JsonPointerError::InvalidIndex);
                        }
                        key.parse().map_err(|_| JsonPointerError::InvalidIndex)?
                    };
                    current = arr.get(idx).unwrap_or(&Value::Null);
                }
                Value::Object(map) => {
                    current = map.get(&key).unwrap_or(&Value::Null);
                }
                _ => return Err(JsonPointerError::NotFound),
            }
            start = i + 1;
        }
    }

    if start <= pointer.len() {
        let component = &pointer[start..];
        key = unescape_component(component);
        obj = Some(current.clone());

        match current {
            Value::Array(arr) => {
                let idx: usize = if key == "-" {
                    arr.len()
                } else {
                    if !is_valid_index(&key) {
                        return Err(JsonPointerError::InvalidIndex);
                    }
                    key.parse().map_err(|_| JsonPointerError::InvalidIndex)?
                };
                current = arr.get(idx).unwrap_or(&Value::Null);
            }
            Value::Object(map) => {
                current = map.get(&key).unwrap_or(&Value::Null);
            }
            _ => return Err(JsonPointerError::NotFound),
        }
    }

    Ok((obj, key))
}
