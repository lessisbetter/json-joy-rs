use serde_json::Value;

use super::ModelApiError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathStep {
    Key(String),
    Index(usize),
    Append,
}

pub fn get_path_mut<'a>(value: &'a mut Value, path: &[PathStep]) -> Option<&'a mut Value> {
    let mut cur = value;
    for step in path {
        match (step, cur) {
            (PathStep::Key(key), Value::Object(map)) => {
                cur = map.get_mut(key)?;
            }
            (PathStep::Index(idx), Value::Array(arr)) => {
                cur = arr.get_mut(*idx)?;
            }
            _ => return None,
        }
    }
    Some(cur)
}

pub fn value_at_path<'a>(value: &'a Value, path: &[PathStep]) -> Option<&'a Value> {
    let mut cur = value;
    for step in path {
        cur = match (step, cur) {
            (PathStep::Key(key), Value::Object(map)) => map.get(key)?,
            (PathStep::Index(idx), Value::Array(arr)) => arr.get(*idx)?,
            (PathStep::Append, _) => return None,
            _ => return None,
        };
    }
    Some(cur)
}

pub fn split_parent(path: &[PathStep]) -> Result<(&[PathStep], &PathStep), ModelApiError> {
    if path.is_empty() {
        return Err(ModelApiError::InvalidPathOp);
    }
    let (parent, leaf) = path.split_at(path.len() - 1);
    Ok((parent, &leaf[0]))
}

pub fn parse_json_pointer(path: &str) -> Result<Vec<PathStep>, ModelApiError> {
    if path.is_empty() || path == "/" {
        return Ok(Vec::new());
    }
    let normalized = if path.starts_with('/') {
        path
    } else {
        // Match upstream convenience behavior: allow relative pointer strings.
        // Example: "doc/items/0" => "/doc/items/0".
        return parse_json_pointer(&format!("/{path}"));
    };

    let mut out = Vec::new();
    for raw in normalized.split('/').skip(1) {
        let token = raw.replace("~1", "/").replace("~0", "~");
        if token == "-" {
            out.push(PathStep::Append);
            continue;
        }
        if let Ok(idx) = token.parse::<usize>() {
            out.push(PathStep::Index(idx));
        } else {
            out.push(PathStep::Key(token));
        }
    }
    Ok(out)
}
