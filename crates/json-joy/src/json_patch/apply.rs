//! JSON Patch apply logic.
//!
//! Mirrors `packages/json-joy/src/json-patch/applyPatch/`.

use regex::RegexBuilder;
use serde_json::{Map, Value};

use super::types::{JsonPatchType, Op, OpResult, PatchError, PatchResult};

// ── Path navigation ───────────────────────────────────────────────────────

/// Convert a `Path` (Vec<String>) to a JSON Pointer string (RFC 6901).
fn path_to_pointer(path: &[String]) -> String {
    if path.is_empty() { return String::new(); }
    let mut ptr = String::with_capacity(path.len() * 8);
    for key in path {
        ptr.push('/');
        ptr.push_str(&key.replace('~', "~0").replace('/', "~1"));
    }
    ptr
}

/// Immutable navigation to the value at `path`.
fn get_at<'a>(doc: &'a Value, path: &[String]) -> Option<&'a Value> {
    let ptr = path_to_pointer(path);
    doc.pointer(&ptr)
}

/// Mutable navigation to the value at `path` (must exist).
fn get_mut_at<'a>(doc: &'a mut Value, path: &[String]) -> Result<&'a mut Value, PatchError> {
    let ptr = path_to_pointer(path);
    doc.pointer_mut(&ptr).ok_or(PatchError::NotFound)
}

// ── Individual operation applicators ─────────────────────────────────────

fn apply_add(doc: &mut Value, path: &[String], value: Value) -> Result<Option<Value>, PatchError> {
    if path.is_empty() {
        let old = std::mem::replace(doc, value);
        return Ok(Some(old));
    }
    let (parent_path, key) = path.split_at(path.len() - 1);
    let key = &key[0];
    let parent = get_mut_at(doc, parent_path)?;
    match parent {
        Value::Object(map) => {
            let old = map.insert(key.clone(), value);
            Ok(old)
        }
        Value::Array(arr) => {
            if key == "-" {
                arr.push(value);
                Ok(None)
            } else {
                let idx: usize = key.parse().map_err(|_| PatchError::InvalidIndex)?;
                if idx > arr.len() { return Err(PatchError::InvalidIndex); }
                arr.insert(idx, value);
                Ok(None)
            }
        }
        _ => Err(PatchError::InvalidTarget),
    }
}

fn apply_remove(doc: &mut Value, path: &[String]) -> Result<Option<Value>, PatchError> {
    if path.is_empty() { return Err(PatchError::InvalidTarget); }
    let (parent_path, key) = path.split_at(path.len() - 1);
    let key = &key[0];
    let parent = get_mut_at(doc, parent_path)?;
    match parent {
        Value::Object(map) => {
            map.remove(key).ok_or(PatchError::NotFound).map(Some)
        }
        Value::Array(arr) => {
            let idx: usize = key.parse().map_err(|_| PatchError::InvalidIndex)?;
            if idx >= arr.len() { return Err(PatchError::NotFound); }
            Ok(Some(arr.remove(idx)))
        }
        _ => Err(PatchError::InvalidTarget),
    }
}

fn apply_replace(doc: &mut Value, path: &[String], value: Value) -> Result<Option<Value>, PatchError> {
    if path.is_empty() {
        let old = std::mem::replace(doc, value);
        return Ok(Some(old));
    }
    let (parent_path, key) = path.split_at(path.len() - 1);
    let key = &key[0];
    let parent = get_mut_at(doc, parent_path)?;
    match parent {
        Value::Object(map) => {
            let old = map.insert(key.clone(), value).ok_or(PatchError::NotFound)?;
            Ok(Some(old))
        }
        Value::Array(arr) => {
            let idx: usize = key.parse().map_err(|_| PatchError::InvalidIndex)?;
            if idx >= arr.len() { return Err(PatchError::NotFound); }
            let old = std::mem::replace(&mut arr[idx], value);
            Ok(Some(old))
        }
        _ => Err(PatchError::InvalidTarget),
    }
}

fn apply_copy(doc: &mut Value, path: &[String], from: &[String]) -> Result<Option<Value>, PatchError> {
    let src = get_at(doc, from).ok_or(PatchError::NotFound)?.clone();
    apply_add(doc, path, src)
}

fn apply_move(doc: &mut Value, path: &[String], from: &[String]) -> Result<Option<Value>, PatchError> {
    // Validate: path must not be a child of from
    if path.len() >= from.len() && path[..from.len()] == from[..] {
        return Err(PatchError::InvalidTarget);
    }
    let value = apply_remove(doc, from)?.ok_or(PatchError::NotFound)?;
    apply_add(doc, path, value)
}

fn apply_test_op(doc: &Value, path: &[String], value: &Value, not: bool) -> Result<(), PatchError> {
    let actual = get_at(doc, path).ok_or(PatchError::NotFound)?;
    let equal = actual == value;
    if equal == not { Err(PatchError::Test) } else { Ok(()) }
}

fn apply_str_ins(doc: &mut Value, path: &[String], pos: usize, str_val: &str) -> Result<(), PatchError> {
    let target = get_mut_at(doc, path)?;
    match target {
        Value::String(s) => {
            // pos is a char-based position
            let byte_pos = s.char_indices().nth(pos).map(|(i, _)| i).unwrap_or(s.len());
            s.insert_str(byte_pos, str_val);
            Ok(())
        }
        _ => Err(PatchError::NotAString),
    }
}

fn apply_str_del(
    doc: &mut Value,
    path: &[String],
    pos: usize,
    str_val: Option<&str>,
    len: Option<usize>,
) -> Result<(), PatchError> {
    let target = get_mut_at(doc, path)?;
    match target {
        Value::String(s) => {
            let delete_len = str_val.map(|sv| sv.chars().count()).or(len).unwrap_or(0);
            let chars: Vec<char> = s.chars().collect();
            if pos + delete_len > chars.len() { return Err(PatchError::InvalidIndex); }
            let new_str: String = chars[..pos].iter().chain(chars[pos + delete_len..].iter()).collect();
            *s = new_str;
            Ok(())
        }
        _ => Err(PatchError::NotAString),
    }
}

fn apply_flip(doc: &mut Value, path: &[String]) -> Result<(), PatchError> {
    let target = get_mut_at(doc, path)?;
    match target {
        Value::Bool(b) => { *b = !*b; Ok(()) }
        _ => Err(PatchError::InvalidTarget),
    }
}

fn apply_inc(doc: &mut Value, path: &[String], inc: f64) -> Result<(), PatchError> {
    let target = get_mut_at(doc, path)?;
    match target {
        Value::Number(n) => {
            let current = n.as_f64().ok_or(PatchError::InvalidTarget)?;
            let new_val = current + inc;
            *target = serde_json::Number::from_f64(new_val)
                .map(Value::Number)
                .ok_or(PatchError::InvalidTarget)?;
            Ok(())
        }
        _ => Err(PatchError::InvalidTarget),
    }
}

fn apply_extend(doc: &mut Value, path: &[String], props: &Map<String, Value>, delete_null: bool) -> Result<(), PatchError> {
    let target = get_mut_at(doc, path)?;
    match target {
        Value::Object(map) => {
            for (k, v) in props {
                if delete_null && v.is_null() {
                    map.remove(k);
                } else {
                    map.insert(k.clone(), v.clone());
                }
            }
            Ok(())
        }
        _ => Err(PatchError::InvalidTarget),
    }
}

fn apply_split(doc: &mut Value, path: &[String], pos: usize, props: Option<&Value>) -> Result<(), PatchError> {
    if path.is_empty() { return Err(PatchError::InvalidTarget); }
    let (parent_path, key) = path.split_at(path.len() - 1);
    let key = &key[0];
    let parent = get_mut_at(doc, parent_path)?;
    match parent {
        Value::Array(arr) => {
            let idx: usize = key.parse().map_err(|_| PatchError::InvalidIndex)?;
            if idx >= arr.len() { return Err(PatchError::NotFound); }
            let node = arr[idx].clone();
            // Handle string split
            if let Value::String(s) = &node {
                let chars: Vec<char> = s.chars().collect();
                let left: String = chars[..pos.min(chars.len())].iter().collect();
                let right: String = chars[pos.min(chars.len())..].iter().collect();
                arr[idx] = Value::String(left);
                // Apply extra props to the new right node if it's an object
                let mut right_val = Value::String(right);
                if let (Some(Value::Object(extra)), Value::String(_)) = (props, &right_val) {
                    // If props are provided and right is still a string, wrap in object
                    let mut map = serde_json::Map::new();
                    map.insert("text".to_string(), right_val);
                    for (k, v) in extra { map.insert(k.clone(), v.clone()); }
                    right_val = Value::Object(map);
                }
                arr.insert(idx + 1, right_val);
                return Ok(());
            }
            // Handle object/array split
            if let Value::Object(_) = &node {
                let mut right = node.clone();
                if let (Value::Object(r), Some(Value::Object(extra))) = (&mut right, props) {
                    for (k, v) in extra { r.insert(k.clone(), v.clone()); }
                }
                arr.insert(idx + 1, right);
                return Ok(());
            }
            Err(PatchError::InvalidTarget)
        }
        _ => Err(PatchError::InvalidTarget),
    }
}

fn apply_merge(doc: &mut Value, path: &[String], pos: usize, _props: Option<&Value>) -> Result<(), PatchError> {
    if path.is_empty() { return Err(PatchError::InvalidTarget); }
    let (parent_path, key) = path.split_at(path.len() - 1);
    let key = &key[0];
    let parent = get_mut_at(doc, parent_path)?;
    match parent {
        Value::Array(arr) => {
            let idx: usize = key.parse().map_err(|_| PatchError::InvalidIndex)?;
            if idx + 1 >= arr.len() { return Err(PatchError::NotFound); }
            let right = arr.remove(idx + 1);
            // Merge based on node types
            match (&mut arr[idx], right) {
                (Value::String(left_str), Value::String(right_str)) => {
                    left_str.push_str(&right_str);
                }
                (Value::Object(left_obj), Value::Object(right_obj)) => {
                    for (k, v) in right_obj { left_obj.insert(k, v); }
                }
                _ => return Err(PatchError::InvalidTarget),
            }
            Ok(())
        }
        _ => Err(PatchError::InvalidTarget),
    }
}

// ── Predicate test functions ──────────────────────────────────────────────

fn test_predicate(doc: &Value, op: &Op) -> bool {
    match op {
        Op::Test { path, value, not } => {
            let actual = match get_at(doc, path) { Some(v) => v, None => return false };
            (actual == value) != *not
        }
        Op::Defined { path } => get_at(doc, path).is_some(),
        Op::Undefined { path } => get_at(doc, path).is_none(),
        Op::Contains { path, value, ignore_case } => {
            let actual = match get_at(doc, path).and_then(|v| v.as_str()) { Some(s) => s, None => return false };
            if *ignore_case {
                actual.to_lowercase().contains(&value.to_lowercase())
            } else {
                actual.contains(value.as_str())
            }
        }
        Op::Ends { path, value, ignore_case } => {
            let actual = match get_at(doc, path).and_then(|v| v.as_str()) { Some(s) => s, None => return false };
            if *ignore_case {
                actual.to_lowercase().ends_with(&value.to_lowercase())
            } else {
                actual.ends_with(value.as_str())
            }
        }
        Op::Starts { path, value, ignore_case } => {
            let actual = match get_at(doc, path).and_then(|v| v.as_str()) { Some(s) => s, None => return false };
            if *ignore_case {
                actual.to_lowercase().starts_with(&value.to_lowercase())
            } else {
                actual.starts_with(value.as_str())
            }
        }
        Op::In { path, value } => {
            let actual = match get_at(doc, path) { Some(v) => v, None => return false };
            value.iter().any(|v| v == actual)
        }
        Op::Less { path, value } => {
            let actual = match get_at(doc, path).and_then(|v| v.as_f64()) { Some(n) => n, None => return false };
            actual < *value
        }
        Op::More { path, value } => {
            let actual = match get_at(doc, path).and_then(|v| v.as_f64()) { Some(n) => n, None => return false };
            actual > *value
        }
        Op::Matches { path, value, ignore_case } => {
            let actual = match get_at(doc, path).and_then(|v| v.as_str()) { Some(s) => s, None => return false };
            match RegexBuilder::new(value).case_insensitive(*ignore_case).build() {
                Ok(re) => re.is_match(actual),
                // If the pattern is invalid, fall back to contains-based matching
                Err(_) => if *ignore_case {
                    actual.to_lowercase().contains(&value.to_lowercase())
                } else {
                    actual.contains(value.as_str())
                },
            }
        }
        Op::TestType { path, type_vals } => {
            let actual = match get_at(doc, path) { Some(v) => v, None => return false };
            type_vals.iter().any(|t| t.matches_value(actual))
        }
        Op::TestString { path, pos, str_val, not } => {
            let actual = match get_at(doc, path).and_then(|v| v.as_str()) { Some(s) => s, None => return false };
            let chars: Vec<char> = actual.chars().collect();
            let needle: Vec<char> = str_val.chars().collect();
            let matched = chars.len() >= *pos + needle.len()
                && chars[*pos..*pos + needle.len()] == needle[..];
            matched != *not
        }
        Op::TestStringLen { path, len, not } => {
            let actual = match get_at(doc, path).and_then(|v| v.as_str()) { Some(s) => s, None => return false };
            let char_len = actual.chars().count();
            (char_len >= *len) != *not
        }
        Op::Type { path, value } => {
            let actual = match get_at(doc, path) { Some(v) => v, None => return false };
            value.matches_value(actual)
        }
        Op::And { path, ops } => {
            // Ops are relative to path; for simplicity, pass the sub-document
            let sub = match get_at(doc, path) { Some(v) => v, None => return false };
            ops.iter().all(|op| test_predicate_relative(sub, doc, op, path))
        }
        Op::Or { path, ops } => {
            let sub = match get_at(doc, path) { Some(v) => v, None => return false };
            ops.iter().any(|op| test_predicate_relative(sub, doc, op, path))
        }
        Op::Not { path, ops } => {
            let sub = match get_at(doc, path) { Some(v) => v, None => return false };
            ops.iter().all(|op| !test_predicate_relative(sub, doc, op, path))
        }
        _ => false,
    }
}

/// Test a predicate op where paths are relative to the parent path.
///
/// Child ops inside `And`/`Or`/`Not` use paths relative to the parent's value
/// (`sub`), not the full document root.
fn test_predicate_relative(sub: &Value, _doc: &Value, op: &Op, _base_path: &[String]) -> bool {
    test_predicate(sub, op)
}

// ── Main apply function ───────────────────────────────────────────────────

/// Apply a single operation to the document (in-place mutation).
///
/// Returns the old value at the path for mutating ops, or `None` for predicates.
pub fn apply_op(doc: &mut Value, op: &Op) -> Result<Option<Value>, PatchError> {
    match op {
        Op::Add { path, value } =>
            apply_add(doc, path, value.clone()),
        Op::Remove { path, .. } =>
            apply_remove(doc, path),
        Op::Replace { path, value, .. } =>
            apply_replace(doc, path, value.clone()),
        Op::Copy { path, from } =>
            apply_copy(doc, path, from),
        Op::Move { path, from } =>
            apply_move(doc, path, from),
        Op::Test { path, value, not } => {
            apply_test_op(doc, path, value, *not)?;
            Ok(None)
        }
        Op::StrIns { path, pos, str_val } => {
            apply_str_ins(doc, path, *pos, str_val)?;
            Ok(None)
        }
        Op::StrDel { path, pos, str_val, len } => {
            apply_str_del(doc, path, *pos, str_val.as_deref(), *len)?;
            Ok(None)
        }
        Op::Flip { path } => {
            apply_flip(doc, path)?;
            Ok(None)
        }
        Op::Inc { path, inc } => {
            apply_inc(doc, path, *inc)?;
            Ok(None)
        }
        Op::Extend { path, props, delete_null } => {
            apply_extend(doc, path, props, *delete_null)?;
            Ok(None)
        }
        Op::Split { path, pos, props } => {
            apply_split(doc, path, *pos, props.as_ref())?;
            Ok(None)
        }
        Op::Merge { path, pos, props } => {
            apply_merge(doc, path, *pos, props.as_ref())?;
            Ok(None)
        }
        // Predicate operations: test and throw if failed
        pred => {
            if !test_predicate(doc, pred) {
                Err(PatchError::Test)
            } else {
                Ok(None)
            }
        }
    }
}

/// Apply a sequence of operations, returning the final document and per-op results.
pub fn apply_ops(mut doc: Value, ops: &[Op]) -> Result<PatchResult, PatchError> {
    let mut results = Vec::with_capacity(ops.len());
    for op in ops {
        let old = apply_op(&mut doc, op)?;
        results.push(OpResult { doc: doc.clone(), old });
    }
    Ok(PatchResult { doc, res: results })
}

/// Apply a sequence of operations with options (mutate vs. clone).
///
/// When `mutate: true`, ops are applied without capturing per-op intermediate
/// snapshots (efficient in-place semantics). When `mutate: false`, the full
/// `apply_ops` path is used, which captures the doc state after each op.
pub fn apply_patch(doc: Value, ops: &[Op], options: &super::types::ApplyPatchOptions) -> Result<PatchResult, PatchError> {
    if options.mutate {
        let mut working = doc;
        for op in ops {
            apply_op(&mut working, op)?;
        }
        Ok(PatchResult { doc: working, res: vec![] })
    } else {
        apply_ops(doc, ops)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use crate::json_patch::types::Op;

    fn path(s: &str) -> Vec<String> {
        if s.is_empty() { return vec![]; }
        s.split('/').filter(|p| !p.is_empty()).map(|s| s.to_string()).collect()
    }

    #[test]
    fn add_to_object() {
        let mut doc = json!({"a": 1});
        apply_op(&mut doc, &Op::Add { path: path("b"), value: json!(2) }).unwrap();
        assert_eq!(doc, json!({"a": 1, "b": 2}));
    }

    #[test]
    fn add_to_array() {
        let mut doc = json!([1, 2, 3]);
        apply_op(&mut doc, &Op::Add { path: path("1"), value: json!(99) }).unwrap();
        assert_eq!(doc, json!([1, 99, 2, 3]));
    }

    #[test]
    fn add_append_array() {
        let mut doc = json!([1, 2]);
        apply_op(&mut doc, &Op::Add { path: path("-"), value: json!(3) }).unwrap();
        assert_eq!(doc, json!([1, 2, 3]));
    }

    #[test]
    fn remove_from_object() {
        let mut doc = json!({"a": 1, "b": 2});
        let old = apply_op(&mut doc, &Op::Remove { path: path("a"), old_value: None }).unwrap();
        assert_eq!(doc, json!({"b": 2}));
        assert_eq!(old, Some(json!(1)));
    }

    #[test]
    fn replace_value() {
        let mut doc = json!({"a": 1});
        apply_op(&mut doc, &Op::Replace { path: path("a"), value: json!(99), old_value: None }).unwrap();
        assert_eq!(doc, json!({"a": 99}));
    }

    #[test]
    fn copy_op() {
        let mut doc = json!({"a": {"x": 1}, "b": {}});
        apply_op(&mut doc, &Op::Copy { path: path("b/x"), from: path("a/x") }).unwrap();
        assert_eq!(doc["b"]["x"], json!(1));
    }

    #[test]
    fn move_op() {
        let mut doc = json!({"a": 1, "b": 2});
        apply_op(&mut doc, &Op::Move { path: path("c"), from: path("a") }).unwrap();
        assert_eq!(doc, json!({"b": 2, "c": 1}));
    }

    #[test]
    fn test_pass() {
        let mut doc = json!({"a": 42});
        apply_op(&mut doc, &Op::Test { path: path("a"), value: json!(42), not: false }).unwrap();
    }

    #[test]
    fn test_fail() {
        let mut doc = json!({"a": 42});
        let result = apply_op(&mut doc, &Op::Test { path: path("a"), value: json!(99), not: false });
        assert_eq!(result, Err(PatchError::Test));
    }

    #[test]
    fn str_ins_op() {
        let mut doc = json!({"s": "helo"});
        apply_op(&mut doc, &Op::StrIns { path: path("s"), pos: 3, str_val: "l".to_string() }).unwrap();
        assert_eq!(doc["s"], json!("hello"));
    }

    #[test]
    fn str_del_op() {
        let mut doc = json!({"s": "hello world"});
        apply_op(&mut doc, &Op::StrDel { path: path("s"), pos: 5, str_val: Some(" world".to_string()), len: None }).unwrap();
        assert_eq!(doc["s"], json!("hello"));
    }

    #[test]
    fn flip_op() {
        let mut doc = json!({"b": true});
        apply_op(&mut doc, &Op::Flip { path: path("b") }).unwrap();
        assert_eq!(doc["b"], json!(false));
    }

    #[test]
    fn inc_op() {
        let mut doc = json!({"n": 10});
        apply_op(&mut doc, &Op::Inc { path: path("n"), inc: 5.0 }).unwrap();
        assert_eq!(doc["n"], json!(15.0));
    }

    #[test]
    fn extend_op() {
        let mut doc = json!({"a": 1});
        let mut props = serde_json::Map::new();
        props.insert("b".to_string(), json!(2));
        apply_op(&mut doc, &Op::Extend { path: path(""), props, delete_null: false }).unwrap();
        assert_eq!(doc["b"], json!(2));
    }

    #[test]
    fn predicate_defined() {
        let mut doc = json!({"a": 1});
        apply_op(&mut doc, &Op::Defined { path: path("a") }).unwrap();
        let r = apply_op(&mut doc, &Op::Defined { path: path("z") });
        assert_eq!(r, Err(PatchError::Test));
    }

    #[test]
    fn predicate_less_more() {
        let mut doc = json!({"n": 5});
        apply_op(&mut doc, &Op::Less { path: path("n"), value: 10.0 }).unwrap();
        apply_op(&mut doc, &Op::More { path: path("n"), value: 2.0 }).unwrap();
    }

    #[test]
    fn predicate_in() {
        let mut doc = json!({"x": "b"});
        apply_op(&mut doc, &Op::In { path: path("x"), value: vec![json!("a"), json!("b"), json!("c")] }).unwrap();
    }

    #[test]
    fn predicate_test_type() {
        let mut doc = json!({"n": 42});
        apply_op(&mut doc, &Op::TestType { path: path("n"), type_vals: vec![JsonPatchType::Number] }).unwrap();
        let r = apply_op(&mut doc, &Op::TestType { path: path("n"), type_vals: vec![JsonPatchType::String] });
        assert_eq!(r, Err(PatchError::Test));
    }

    #[test]
    fn apply_ops_sequence() {
        let doc = json!({"a": 1});
        let ops = vec![
            Op::Add { path: vec!["b".to_string()], value: json!(2) },
            Op::Replace { path: vec!["a".to_string()], value: json!(10), old_value: None },
        ];
        let result = apply_ops(doc, &ops).unwrap();
        assert_eq!(result.doc["a"], json!(10));
        assert_eq!(result.doc["b"], json!(2));
    }
}
