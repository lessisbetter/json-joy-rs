//! JSON Patch operation validator.
//!
//! Mirrors `packages/json-joy/src/json-patch/validate.ts`.
//!
//! Validates raw JSON Patch operations (as `serde_json::Value` maps) against
//! the spec.  This works on the raw JSON representation before decoding,
//! so callers can validate untrusted input early.

use json_joy_json_pointer::validate_json_pointer;
use serde_json::Value;

// ── Error ──────────────────────────────────────────────────────────────────

/// Error returned by validation functions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError(pub String);

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ValidationError {}

fn err(msg: &str) -> ValidationError {
    ValidationError(msg.to_string())
}

// ── Public API ─────────────────────────────────────────────────────────────

/// Validate a list of operations.
///
/// Errors include the index of the failing operation:
/// `"Error in operation [index = N] (reason)."`.
///
/// Mirrors `validateOperations` in the upstream TypeScript.
pub fn validate_operations(ops: &Value, allow_matches_op: bool) -> Result<(), ValidationError> {
    let arr = ops.as_array().ok_or_else(|| err("Not a array."))?;
    if arr.is_empty() {
        return Err(err("Empty operation patch."));
    }
    for (i, op) in arr.iter().enumerate() {
        validate_operation(op, allow_matches_op).map_err(|e| {
            ValidationError(format!("Error in operation [index = {}] ({}).", i, e.0))
        })?;
    }
    Ok(())
}

/// Validate a single operation object.
///
/// Mirrors `validateOperation` in the upstream TypeScript.
pub fn validate_operation(op: &Value, allow_matches_op: bool) -> Result<(), ValidationError> {
    let map = op.as_object().ok_or_else(|| err("OP_INVALID"))?;

    // path must be a string
    let path = map.get("path").ok_or_else(|| err("OP_PATH_INVALID"))?;
    let path_str = path.as_str().ok_or_else(|| err("OP_PATH_INVALID"))?;
    validate_json_pointer_str(path_str)?;

    let op_name = map.get("op").and_then(|v| v.as_str()).unwrap_or("");
    match op_name {
        "add" => validate_op_add(map),
        "remove" => validate_op_remove(map),
        "replace" => validate_op_replace(map),
        "copy" => validate_op_copy(map),
        "move" => validate_op_move(map, path_str),
        "flip" => Ok(()),
        "inc" => validate_op_inc(map),
        "str_ins" => validate_op_str_ins(map),
        "str_del" => validate_op_str_del(map),
        "extend" => validate_op_extend(map),
        "merge" => validate_op_merge(map),
        "split" => validate_op_split(map),
        _ => validate_predicate_operation(op, allow_matches_op),
    }
}

/// Validate a predicate operation.
///
/// Mirrors `validatePredicateOperation` in the upstream TypeScript.
pub fn validate_predicate_operation(
    op: &Value,
    allow_matches_op: bool,
) -> Result<(), ValidationError> {
    let map = op.as_object().ok_or_else(|| err("OP_INVALID"))?;

    let path_str = map
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| err("OP_PATH_INVALID"))?;
    validate_json_pointer_str(path_str)?;

    let op_name = map.get("op").and_then(|v| v.as_str()).unwrap_or("");
    match op_name {
        "test" => validate_op_test(map),
        "test_type" => validate_op_test_type(map),
        "test_string" => validate_op_test_string(map),
        "test_string_len" => validate_op_test_string_len(map),
        "matches" => {
            if !allow_matches_op {
                return Err(err("\"matches\" operation not allowed."));
            }
            validate_predicate_value_and_case(map)
        }
        "contains" | "ends" | "starts" => validate_predicate_value_and_case(map),
        "in" => {
            let val = map.get("value").ok_or_else(|| err("OP_VALUE_MISSING"))?;
            if !val.is_array() {
                return Err(err("\"in\" operation \"value\" must be an array."));
            }
            Ok(())
        }
        "more" | "less" => {
            let val = map.get("value").ok_or_else(|| err("OP_VALUE_MISSING"))?;
            if !val.is_number() {
                return Err(err("Value must be a number."));
            }
            Ok(())
        }
        "type" => {
            let val = map.get("value").ok_or_else(|| err("OP_VALUE_MISSING"))?;
            let type_str = val
                .as_str()
                .ok_or_else(|| err("Expected \"value\" to be string."))?;
            if type_str.len() > 20_000 {
                return Err(err("Value too long."));
            }
            validate_test_type_str(type_str)
        }
        "defined" | "undefined" => Ok(()),
        "and" | "or" | "not" => {
            let apply = map.get("apply").ok_or_else(|| {
                err(&format!(
                    "\"{}\" predicate operators must be an array.",
                    op_name
                ))
            })?;
            let apply_arr = apply.as_array().ok_or_else(|| {
                err(&format!(
                    "\"{}\" predicate operators must be an array.",
                    op_name
                ))
            })?;
            if apply_arr.is_empty() {
                return Err(err("Predicate list is empty."));
            }
            for pred in apply_arr {
                validate_predicate_operation(pred, allow_matches_op)?;
            }
            Ok(())
        }
        _ => Err(err("OP_UNKNOWN")),
    }
}

// ── Operation-specific validators ─────────────────────────────────────────

fn validate_op_add(map: &serde_json::Map<String, Value>) -> Result<(), ValidationError> {
    validate_has_value(map)
}

fn validate_op_remove(map: &serde_json::Map<String, Value>) -> Result<(), ValidationError> {
    // oldValue, if present, must not be undefined (JSON doesn't have undefined,
    // so we just verify that if the key exists the value is not "undefined" —
    // in Rust/JSON this can't happen, but we replicate the check for parity).
    // The upstream checks: hasOwnProp(op, 'oldValue') && op.oldValue === undefined
    // In JSON, `undefined` is not a valid value, so this check always passes.
    Ok(())
}

fn validate_op_replace(map: &serde_json::Map<String, Value>) -> Result<(), ValidationError> {
    // Same reasoning as remove — oldValue can't be JS undefined in JSON.
    Ok(())
}

fn validate_op_copy(map: &serde_json::Map<String, Value>) -> Result<(), ValidationError> {
    validate_from(map)
}

fn validate_op_move(
    map: &serde_json::Map<String, Value>,
    path_str: &str,
) -> Result<(), ValidationError> {
    validate_from(map)?;
    let from_str = map.get("from").and_then(|v| v.as_str()).unwrap_or("");
    // "Cannot move into own children": path must not start with from + "/"
    let prefix = format!("{}/", from_str);
    if path_str.starts_with(&prefix) {
        return Err(err("Cannot move into own children."));
    }
    Ok(())
}

fn validate_op_inc(map: &serde_json::Map<String, Value>) -> Result<(), ValidationError> {
    let inc = map
        .get("inc")
        .ok_or_else(|| err("Invalid \"inc\" value."))?;
    if !inc.is_number() {
        return Err(err("Invalid \"inc\" value."));
    }
    Ok(())
}

fn validate_op_str_ins(map: &serde_json::Map<String, Value>) -> Result<(), ValidationError> {
    validate_non_negative_integer(map, "pos")?;
    let str_val = map
        .get("str")
        .ok_or_else(|| err("Expected a string \"text\" field."))?;
    if !str_val.is_string() {
        return Err(err("Expected a string \"text\" field."));
    }
    Ok(())
}

fn validate_op_str_del(map: &serde_json::Map<String, Value>) -> Result<(), ValidationError> {
    validate_non_negative_integer(map, "pos")?;
    let has_str = map.get("str").is_some();
    let has_len = map.get("len").is_some();
    if !has_str && !has_len {
        return Err(err("Either \"text\" or \"pos\" need to be set."));
    }
    if has_str {
        let str_val = map.get("str").unwrap();
        if !str_val.is_string() {
            return Err(err("Expected a string \"text\" field."));
        }
    } else {
        validate_non_negative_integer(map, "len")?;
    }
    Ok(())
}

fn validate_op_extend(map: &serde_json::Map<String, Value>) -> Result<(), ValidationError> {
    let props = map
        .get("props")
        .ok_or_else(|| err("Invalid \"props\" field."))?;
    if !props.is_object() {
        return Err(err("Invalid \"props\" field."));
    }
    if let Some(delete_null) = map.get("deleteNull") {
        if !delete_null.is_boolean() {
            return Err(err("Expected \"deleteNull\" field to be boolean."));
        }
    }
    Ok(())
}

fn validate_op_merge(map: &serde_json::Map<String, Value>) -> Result<(), ValidationError> {
    validate_integer_field(map, "pos")?;
    let pos = map.get("pos").and_then(|v| v.as_i64()).unwrap_or(0);
    if pos < 1 {
        return Err(err("Expected \"pos\" field to be greater than 0."));
    }
    if let Some(props) = map.get("props") {
        if !props.is_object() {
            return Err(err("Invalid \"props\" field."));
        }
    }
    Ok(())
}

fn validate_op_split(map: &serde_json::Map<String, Value>) -> Result<(), ValidationError> {
    validate_integer_field(map, "pos")?;
    if let Some(props) = map.get("props") {
        if !props.is_object() {
            return Err(err("Invalid \"props\" field."));
        }
    }
    Ok(())
}

fn validate_op_test(map: &serde_json::Map<String, Value>) -> Result<(), ValidationError> {
    validate_has_value(map)?;
    validate_not_field(map)
}

fn validate_op_test_type(map: &serde_json::Map<String, Value>) -> Result<(), ValidationError> {
    let type_val = map
        .get("type")
        .ok_or_else(|| err("Invalid \"type\" field."))?;
    let type_arr = type_val
        .as_array()
        .ok_or_else(|| err("Invalid \"type\" field."))?;
    if type_arr.is_empty() {
        return Err(err("Empty type list."));
    }
    for t in type_arr {
        let s = t.as_str().ok_or_else(|| err("Invalid type."))?;
        validate_test_type_str(s)?;
    }
    Ok(())
}

fn validate_op_test_string(map: &serde_json::Map<String, Value>) -> Result<(), ValidationError> {
    validate_not_field(map)?;
    validate_non_negative_integer(map, "pos")?;
    let str_val = map
        .get("str")
        .ok_or_else(|| err("Value must be a string."))?;
    if !str_val.is_string() {
        return Err(err("Value must be a string."));
    }
    Ok(())
}

fn validate_op_test_string_len(
    map: &serde_json::Map<String, Value>,
) -> Result<(), ValidationError> {
    validate_not_field(map)?;
    validate_non_negative_integer(map, "len")
}

fn validate_predicate_value_and_case(
    map: &serde_json::Map<String, Value>,
) -> Result<(), ValidationError> {
    let val = map
        .get("value")
        .ok_or_else(|| err("Expected \"value\" to be string."))?;
    let s = val
        .as_str()
        .ok_or_else(|| err("Expected \"value\" to be string."))?;
    if s.len() > 20_000 {
        return Err(err("Value too long."));
    }
    if let Some(ic) = map.get("ignore_case") {
        if !ic.is_boolean() {
            return Err(err("Expected \"ignore_case\" to be a boolean."));
        }
    }
    Ok(())
}

// ── Field validators ───────────────────────────────────────────────────────

fn validate_has_value(map: &serde_json::Map<String, Value>) -> Result<(), ValidationError> {
    if !map.contains_key("value") {
        return Err(err("OP_VALUE_MISSING"));
    }
    Ok(())
}

fn validate_from(map: &serde_json::Map<String, Value>) -> Result<(), ValidationError> {
    let from = map.get("from").ok_or_else(|| err("OP_FROM_INVALID"))?;
    let from_str = from.as_str().ok_or_else(|| err("OP_FROM_INVALID"))?;
    validate_json_pointer_str(from_str)
}

fn validate_not_field(map: &serde_json::Map<String, Value>) -> Result<(), ValidationError> {
    if let Some(not_val) = map.get("not") {
        if !not_val.is_boolean() {
            return Err(err("Invalid \"not\" modifier."));
        }
    }
    Ok(())
}

fn validate_test_type_str(s: &str) -> Result<(), ValidationError> {
    match s {
        "string" | "number" | "boolean" | "object" | "integer" | "array" | "null" => Ok(()),
        _ => Err(err("Invalid type.")),
    }
}

fn validate_integer_field(
    map: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<(), ValidationError> {
    let val = map.get(field).ok_or_else(|| err("Not an integer."))?;
    match val {
        Value::Number(n) => {
            // Must be an integer: f64 with no fractional part
            let f = n.as_f64().ok_or_else(|| err("Not an integer."))?;
            if f.fract() != 0.0 {
                return Err(err("Not an integer."));
            }
            Ok(())
        }
        _ => Err(err("Not an integer.")),
    }
}

fn validate_non_negative_integer(
    map: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<(), ValidationError> {
    validate_integer_field(map, field)?;
    let val = map.get(field).unwrap();
    let n = val.as_f64().unwrap();
    if n < 0.0 {
        return Err(err("Number is negative."));
    }
    Ok(())
}

fn validate_json_pointer_str(s: &str) -> Result<(), ValidationError> {
    validate_json_pointer(s).map_err(|e| err(&e.to_string()))
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ── validate_operations ──────────────────────────────────────────────

    #[test]
    fn ops_throws_not_array() {
        let result = validate_operations(&json!(123), false);
        assert_eq!(result, Err(ValidationError("Not a array.".into())));
    }

    #[test]
    fn ops_throws_empty_array() {
        let result = validate_operations(&json!([]), false);
        assert_eq!(
            result,
            Err(ValidationError("Empty operation patch.".into()))
        );
    }

    #[test]
    fn ops_throws_invalid_operation_type() {
        let result = validate_operations(&json!([123]), false);
        assert_eq!(
            result,
            Err(ValidationError(
                "Error in operation [index = 0] (OP_INVALID).".into()
            ))
        );
    }

    #[test]
    fn ops_throws_no_path() {
        let result = validate_operations(&json!([{}]), false);
        assert_eq!(
            result,
            Err(ValidationError(
                "Error in operation [index = 0] (OP_PATH_INVALID).".into()
            ))
        );
    }

    #[test]
    fn ops_throws_no_op_code() {
        let result = validate_operations(&json!([{"path": ""}]), false);
        assert_eq!(
            result,
            Err(ValidationError(
                "Error in operation [index = 0] (OP_UNKNOWN).".into()
            ))
        );
    }

    #[test]
    fn ops_throws_invalid_op_code() {
        let result = validate_operations(&json!([{"path": "", "op": "123"}]), false);
        assert_eq!(
            result,
            Err(ValidationError(
                "Error in operation [index = 0] (OP_UNKNOWN).".into()
            ))
        );
    }

    #[test]
    fn ops_succeeds_valid_add() {
        let result = validate_operations(
            &json!([{"op": "add", "path": "/adsf", "value": 123}]),
            false,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn ops_throws_second_invalid_op() {
        let result = validate_operations(
            &json!([
                {"op": "add", "path": "/adsf", "value": 123},
                {"op": "test", "path": "/adsf"}
            ]),
            false,
        );
        assert_eq!(
            result,
            Err(ValidationError(
                "Error in operation [index = 1] (OP_VALUE_MISSING).".into()
            ))
        );
    }

    #[test]
    fn ops_throws_if_pointer_no_slash() {
        let result = validate_operations(
            &json!([
                {"op": "add", "path": "/adsf", "value": 123},
                {"op": "test", "path": "adsf", "value": 1}
            ]),
            false,
        );
        assert_eq!(
            result,
            Err(ValidationError(
                "Error in operation [index = 1] (POINTER_INVALID).".into()
            ))
        );
    }

    // ── add ───────────────────────────────────────────────────────────────

    #[test]
    fn add_throws_no_path() {
        let result = validate_operation(&json!({"op": "add"}), false);
        assert_eq!(result, Err(ValidationError("OP_PATH_INVALID".into())));
    }

    #[test]
    fn add_throws_invalid_path_type() {
        let result = validate_operation(&json!({"op": "add", "path": 123}), false);
        assert_eq!(result, Err(ValidationError("OP_PATH_INVALID".into())));
    }

    #[test]
    fn add_throws_missing_value() {
        let result = validate_operation(&json!({"op": "add", "path": ""}), false);
        assert_eq!(result, Err(ValidationError("OP_VALUE_MISSING".into())));
    }

    #[test]
    fn add_succeeds_valid() {
        let result = validate_operation(&json!({"op": "add", "path": "", "value": 123}), false);
        assert!(result.is_ok());
    }

    // ── remove ────────────────────────────────────────────────────────────

    #[test]
    fn remove_succeeds_valid() {
        let result = validate_operation(&json!({"op": "remove", "path": ""}), false);
        assert!(result.is_ok());
    }

    #[test]
    fn remove_throws_invalid_path() {
        let result = validate_operation(&json!({"op": "remove", "path": "asdf"}), false);
        assert_eq!(result, Err(ValidationError("POINTER_INVALID".into())));
    }

    // ── copy ──────────────────────────────────────────────────────────────

    #[test]
    fn copy_succeeds_valid() {
        let result = validate_operation(&json!({"op": "copy", "from": "", "path": ""}), false);
        assert!(result.is_ok());
    }

    // ── move ──────────────────────────────────────────────────────────────

    #[test]
    fn move_succeeds_valid() {
        let result = validate_operation(
            &json!({"op": "move", "from": "/", "path": "/foo/bar"}),
            false,
        );
        assert!(result.is_ok());

        let result = validate_operation(
            &json!({"op": "move", "from": "/foo/bar", "path": "/foo"}),
            false,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn move_cannot_move_into_own_children() {
        let result = validate_operation(
            &json!({"op": "move", "from": "/foo", "path": "/foo/bar"}),
            false,
        );
        assert_eq!(
            result,
            Err(ValidationError("Cannot move into own children.".into()))
        );
    }

    // ── test ──────────────────────────────────────────────────────────────

    #[test]
    fn test_succeeds_valid() {
        let result = validate_operation(
            &json!({"op": "test", "path": "/foo/bar", "value": null}),
            false,
        );
        assert!(result.is_ok());
    }

    // ── defined ───────────────────────────────────────────────────────────

    #[test]
    fn defined_succeeds() {
        let result = validate_operation(&json!({"op": "defined", "path": ""}), false);
        assert!(result.is_ok());
        let result = validate_operation(&json!({"op": "defined", "path": "/"}), false);
        assert!(result.is_ok());
        let result = validate_operation(&json!({"op": "defined", "path": "/foo/bar"}), false);
        assert!(result.is_ok());
    }

    // ── test_type ─────────────────────────────────────────────────────────

    #[test]
    fn test_type_succeeds_valid() {
        let types = [
            "number", "array", "string", "boolean", "integer", "null", "object",
        ];
        for t in &types {
            let result = validate_operation(
                &json!({"op": "test_type", "path": "/foo", "type": [t]}),
                false,
            );
            assert!(result.is_ok(), "Expected ok for type {t}");
        }
    }

    #[test]
    fn test_type_throws_empty_list() {
        let result = validate_operation(
            &json!({"op": "test_type", "path": "/foo", "type": []}),
            false,
        );
        assert_eq!(result, Err(ValidationError("Empty type list.".into())));
    }

    #[test]
    fn test_type_throws_invalid_type() {
        let result = validate_operation(
            &json!({"op": "test_type", "path": "/foo", "type": ["monkey"]}),
            false,
        );
        assert_eq!(result, Err(ValidationError("Invalid type.".into())));
    }

    // ── test_string ───────────────────────────────────────────────────────

    #[test]
    fn test_string_succeeds_valid() {
        let result = validate_operation(
            &json!({"op": "test_string", "path": "/foo", "pos": 0, "str": "asdf"}),
            false,
        );
        assert!(result.is_ok());
        let result = validate_operation(
            &json!({"op": "test_string", "path": "/foo", "pos": 123, "str": "", "not": true}),
            false,
        );
        assert!(result.is_ok());
    }

    // ── test_string_len ───────────────────────────────────────────────────

    #[test]
    fn test_string_len_succeeds_valid() {
        let result = validate_operation(
            &json!({"op": "test_string_len", "path": "/foo", "len": 1}),
            false,
        );
        assert!(result.is_ok());
        let result = validate_operation(
            &json!({"op": "test_string_len", "path": "/foo", "len": 0, "not": true}),
            false,
        );
        assert!(result.is_ok());
    }

    // ── flip ──────────────────────────────────────────────────────────────

    #[test]
    fn flip_succeeds_valid() {
        let paths = ["", "/", "/foo", "/foo/bar", "/foo/123/bar"];
        for path in &paths {
            let result = validate_operation(&json!({"op": "flip", "path": path}), false);
            assert!(result.is_ok(), "Expected ok for path {path}");
        }
    }

    // ── inc ───────────────────────────────────────────────────────────────

    #[test]
    fn inc_succeeds_valid() {
        for inc_val in [0, 1, -1] {
            let result = validate_operation(
                &json!({"op": "inc", "path": "/foo/bar", "inc": inc_val}),
                false,
            );
            assert!(result.is_ok());
        }
        let result =
            validate_operation(&json!({"op": "inc", "path": "/foo/bar", "inc": 1.5}), false);
        assert!(result.is_ok());
    }

    // ── str_ins ───────────────────────────────────────────────────────────

    #[test]
    fn str_ins_succeeds_valid() {
        let result = validate_operation(
            &json!({"op": "str_ins", "path": "/foo/bar", "pos": 0, "str": ""}),
            false,
        );
        assert!(result.is_ok());
        let result = validate_operation(
            &json!({"op": "str_ins", "path": "/foo/bar", "pos": 1, "str": "asdf"}),
            false,
        );
        assert!(result.is_ok());
    }

    // ── str_del ───────────────────────────────────────────────────────────

    #[test]
    fn str_del_succeeds_valid() {
        let result = validate_operation(
            &json!({"op": "str_del", "path": "/foo/bar", "pos": 0, "str": ""}),
            false,
        );
        assert!(result.is_ok());
        let result = validate_operation(
            &json!({"op": "str_del", "path": "/foo/bar", "pos": 0, "len": 4}),
            false,
        );
        assert!(result.is_ok());
    }

    // ── extend ────────────────────────────────────────────────────────────

    #[test]
    fn extend_succeeds_valid() {
        let result = validate_operation(
            &json!({"op": "extend", "path": "/foo/bar", "props": {}, "deleteNull": true}),
            false,
        );
        assert!(result.is_ok());
        let result = validate_operation(
            &json!({"op": "extend", "path": "/foo/bar", "props": {"foo": "bar"}}),
            false,
        );
        assert!(result.is_ok());
    }

    // ── merge ─────────────────────────────────────────────────────────────

    #[test]
    fn merge_succeeds_valid() {
        let result =
            validate_operation(&json!({"op": "merge", "path": "/foo/bar", "pos": 1}), false);
        assert!(result.is_ok());
        let result = validate_operation(
            &json!({"op": "merge", "path": "/foo/bar", "pos": 2, "props": {}}),
            false,
        );
        assert!(result.is_ok());
    }

    // ── split ─────────────────────────────────────────────────────────────

    #[test]
    fn split_succeeds_valid() {
        let result =
            validate_operation(&json!({"op": "split", "path": "/foo/bar", "pos": 0}), false);
        assert!(result.is_ok());
        let result = validate_operation(
            &json!({"op": "split", "path": "/foo/bar", "pos": 2, "props": {}}),
            false,
        );
        assert!(result.is_ok());
    }

    // ── contains / ends / starts ──────────────────────────────────────────

    #[test]
    fn contains_succeeds_valid() {
        let result = validate_operations(
            &json!([{"op": "contains", "path": "/foo/bar", "value": "asdf"}]),
            false,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn contains_throws_non_string_value() {
        let result = validate_operations(
            &json!([{"op": "contains", "path": "/foo/bar", "value": 123}]),
            false,
        );
        assert!(result.is_err());
    }

    #[test]
    fn contains_throws_invalid_ignore_case() {
        let result = validate_operations(
            &json!([{"op": "contains", "path": "/foo/bar", "value": "asdf", "ignore_case": 1}]),
            false,
        );
        assert!(result.is_err());
    }

    // ── matches ───────────────────────────────────────────────────────────

    #[test]
    fn matches_succeeds_when_allowed() {
        let result = validate_operations(
            &json!([{"op": "matches", "path": "/foo/bar", "value": "asdf"}]),
            true,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn matches_throws_when_not_allowed() {
        let result = validate_operations(
            &json!([{"op": "matches", "path": "/foo/bar", "value": "asdf"}]),
            false,
        );
        assert!(result.is_err());
    }

    // ── defined / undefined ───────────────────────────────────────────────

    #[test]
    fn defined_ops_succeed() {
        assert!(
            validate_operations(&json!([{"op": "defined", "path": "/foo/bar"}]), false).is_ok()
        );
        assert!(
            validate_operations(&json!([{"op": "undefined", "path": "/foo/bar"}]), false).is_ok()
        );
    }

    // ── in ────────────────────────────────────────────────────────────────

    #[test]
    fn in_succeeds_valid() {
        let result = validate_operations(
            &json!([{"op": "in", "path": "/foo/bar", "value": ["asdf"]}]),
            false,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn in_throws_non_array_value() {
        let result = validate_operations(
            &json!([{"op": "in", "path": "/foo/bar", "value": 123}]),
            false,
        );
        assert!(result.is_err());
    }

    // ── more / less ───────────────────────────────────────────────────────

    #[test]
    fn more_less_succeed_valid() {
        assert!(validate_operations(
            &json!([{"op": "more", "path": "/foo/bar", "value": 5}]),
            false
        )
        .is_ok());
        assert!(validate_operations(
            &json!([{"op": "less", "path": "/foo/bar", "value": 5}]),
            false
        )
        .is_ok());
    }

    #[test]
    fn more_less_throw_string_value() {
        assert!(validate_operations(
            &json!([{"op": "more", "path": "/foo/bar", "value": "abc"}]),
            false
        )
        .is_err());
        assert!(validate_operations(
            &json!([{"op": "less", "path": "/foo/bar", "value": "abc"}]),
            false
        )
        .is_err());
    }

    // ── type ──────────────────────────────────────────────────────────────

    #[test]
    fn type_op_succeeds_valid() {
        let result = validate_operations(
            &json!([{"op": "type", "path": "/foo/bar", "value": "number"}]),
            false,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn type_op_throws_non_string() {
        let result = validate_operations(
            &json!([{"op": "type", "path": "/foo/bar", "value": 123}]),
            false,
        );
        assert!(result.is_err());
    }

    // ── and / or / not ────────────────────────────────────────────────────

    #[test]
    fn and_succeeds_valid() {
        let result = validate_operations(
            &json!([{"op": "and", "path": "/foo/bar", "apply": [{"op": "test", "path": "/foo", "value": 123}]}]),
            false,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn and_throws_empty_apply() {
        let result = validate_operations(
            &json!([{"op": "and", "path": "/foo/bar", "apply": []}]),
            false,
        );
        assert!(result.is_err());
    }

    #[test]
    fn and_throws_non_predicate_in_apply() {
        let result = validate_operations(
            &json!([{"op": "and", "path": "/foo/bar", "apply": [{"op": "replace", "path": "/foo", "value": 123}]}]),
            false,
        );
        assert!(result.is_err());
    }

    #[test]
    fn not_succeeds_nested() {
        let result = validate_operations(
            &json!([{
                "op": "not", "path": "/foo/bar",
                "apply": [{
                    "op": "not", "path": "",
                    "apply": [{"op": "test", "path": "/foo", "value": 123}]
                }]
            }]),
            false,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn or_succeeds_valid() {
        let result = validate_operations(
            &json!([{"op": "or", "path": "/foo/bar", "apply": [{"op": "test", "path": "/foo", "value": 123}]}]),
            false,
        );
        assert!(result.is_ok());
    }
}
