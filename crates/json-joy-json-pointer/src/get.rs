use serde_json::Value;

/// Get a value from a JSON document by path.
pub fn get<'a>(val: &'a Value, path: &[String]) -> Option<&'a Value> {
    let path_length = path.len();
    if path_length == 0 {
        return Some(val);
    }

    let mut current = val;
    for path_step in path {
        match current {
            Value::Array(arr) => {
                if path_step == "-" {
                    return None;
                }
                let idx: usize = match path_step.parse() {
                    Ok(i) => i,
                    Err(_) => return None,
                };
                current = arr.get(idx)?;
            }
            Value::Object(map) => {
                current = map.get(path_step)?;
            }
            _ => return None,
        }
    }
    Some(current)
}

/// Get a mutable reference to a value in a JSON document by path.
pub fn get_mut<'a>(val: &'a mut Value, path: &[String]) -> Option<&'a mut Value> {
    let path_length = path.len();
    if path_length == 0 {
        return Some(val);
    }

    let mut current = val;
    for path_step in path {
        match current {
            Value::Array(arr) => {
                if path_step == "-" {
                    return None;
                }
                let idx: usize = path_step.parse().ok()?;
                current = arr.get_mut(idx)?;
            }
            Value::Object(map) => {
                current = map.get_mut(path_step)?;
            }
            _ => return None,
        }
    }
    Some(current)
}
