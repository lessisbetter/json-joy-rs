use serde_json::Value;

use crate::types::{Reference, ReferenceKey};
use crate::util::is_valid_index;
use crate::JsonPointerError;

/// Find a value in a JSON document by path.
pub fn find(val: &Value, path: &[String]) -> Result<Reference, JsonPointerError> {
    if path.is_empty() {
        return Ok(Reference {
            val: Some(val.clone()),
            obj: None,
            key: None,
        });
    }

    let path_len = path.len();
    let mut current: &Value = val;
    let mut obj: Option<Value> = None;
    let mut key: Option<ReferenceKey> = None;

    for (step_idx, path_step) in path.iter().enumerate() {
        let is_last = step_idx == path_len - 1;
        obj = Some(current.clone());

        match current {
            Value::Array(arr) => {
                let idx: usize = if path_step == "-" {
                    arr.len()
                } else {
                    if !is_valid_index(path_step) {
                        return Err(JsonPointerError::InvalidIndex);
                    }
                    path_step
                        .parse()
                        .map_err(|_| JsonPointerError::InvalidIndex)?
                };
                key = Some(ReferenceKey::Index(idx));
                match arr.get(idx) {
                    Some(v) => current = v,
                    None => {
                        if !is_last {
                            return Err(JsonPointerError::NotFound);
                        }
                        return Ok(Reference {
                            val: None,
                            obj,
                            key,
                        });
                    }
                }
            }
            Value::Object(map) => {
                let step_key = path_step.clone();
                key = Some(ReferenceKey::String(step_key.clone()));
                match map.get(&step_key) {
                    Some(v) => current = v,
                    None => {
                        if !is_last {
                            return Err(JsonPointerError::NotFound);
                        }
                        return Ok(Reference {
                            val: None,
                            obj,
                            key,
                        });
                    }
                }
            }
            _ => return Err(JsonPointerError::NotFound),
        }
    }

    Ok(Reference {
        val: Some(current.clone()),
        obj,
        key,
    })
}
