//! JSON codec for JSON Patch operations.
//!
//! Converts operations to/from `serde_json::Value` in RFC 6902 + extensions format.
//!
//! Mirrors `packages/json-joy/src/json-patch/codec/json/`.

use serde_json::{json, Map, Value};

use crate::json_patch::types::{JsonPatchType, Op, PatchError};

// ── Path helpers ──────────────────────────────────────────────────────────

fn encode_path(path: &[String]) -> Value {
    Value::String(if path.is_empty() {
        String::new()
    } else {
        format!(
            "/{}",
            path.iter()
                .map(|s| s.replace('~', "~0").replace('/', "~1"))
                .collect::<Vec<_>>()
                .join("/")
        )
    })
}

fn decode_path(v: &Value) -> Result<Vec<String>, PatchError> {
    let s = v
        .as_str()
        .ok_or_else(|| PatchError::InvalidOp("path must be a string".into()))?;
    Ok(json_joy_json_pointer::parse_json_pointer(s))
}

fn decode_type(v: &Value) -> Result<JsonPatchType, PatchError> {
    let s = v
        .as_str()
        .ok_or_else(|| PatchError::InvalidOp("type must be a string".into()))?;
    JsonPatchType::from_str(s)
}

fn encode_type(t: &JsonPatchType) -> Value {
    Value::String(t.as_str().to_string())
}

fn decode_ops(arr: &Value) -> Result<Vec<Op>, PatchError> {
    let arr = arr
        .as_array()
        .ok_or_else(|| PatchError::InvalidOp("ops must be array".into()))?;
    arr.iter().map(from_json).collect()
}

// ── Serialization ─────────────────────────────────────────────────────────

/// Serialize an `Op` to a `serde_json::Value` in the JSON Patch format.
pub fn to_json(op: &Op) -> Value {
    match op {
        Op::Add { path, value } => json!({
            "op": "add",
            "path": encode_path(path),
            "value": value
        }),
        Op::Remove { path, old_value } => {
            let mut m = serde_json::Map::new();
            m.insert("op".into(), json!("remove"));
            m.insert("path".into(), encode_path(path));
            if let Some(ov) = old_value {
                m.insert("oldValue".into(), ov.clone());
            }
            Value::Object(m)
        }
        Op::Replace {
            path,
            value,
            old_value,
        } => {
            let mut m = serde_json::Map::new();
            m.insert("op".into(), json!("replace"));
            m.insert("path".into(), encode_path(path));
            m.insert("value".into(), value.clone());
            if let Some(ov) = old_value {
                m.insert("oldValue".into(), ov.clone());
            }
            Value::Object(m)
        }
        Op::Copy { path, from } => json!({
            "op": "copy",
            "path": encode_path(path),
            "from": encode_path(from)
        }),
        Op::Move { path, from } => json!({
            "op": "move",
            "path": encode_path(path),
            "from": encode_path(from)
        }),
        Op::Test { path, value, not } => {
            let mut m = serde_json::Map::new();
            m.insert("op".into(), json!("test"));
            m.insert("path".into(), encode_path(path));
            m.insert("value".into(), value.clone());
            if *not {
                m.insert("not".into(), json!(true));
            }
            Value::Object(m)
        }
        Op::StrIns { path, pos, str_val } => json!({
            "op": "str_ins",
            "path": encode_path(path),
            "pos": pos,
            "str": str_val
        }),
        Op::StrDel {
            path,
            pos,
            str_val,
            len,
        } => {
            let mut m = serde_json::Map::new();
            m.insert("op".into(), json!("str_del"));
            m.insert("path".into(), encode_path(path));
            m.insert("pos".into(), json!(pos));
            if let Some(s) = str_val {
                m.insert("str".into(), json!(s));
            }
            if let Some(l) = len {
                m.insert("len".into(), json!(l));
            }
            Value::Object(m)
        }
        Op::Flip { path } => json!({ "op": "flip", "path": encode_path(path) }),
        Op::Inc { path, inc } => json!({
            "op": "inc",
            "path": encode_path(path),
            "inc": inc
        }),
        Op::Split { path, pos, props } => {
            let mut m = serde_json::Map::new();
            m.insert("op".into(), json!("split"));
            m.insert("path".into(), encode_path(path));
            m.insert("pos".into(), json!(pos));
            if let Some(p) = props {
                m.insert("props".into(), p.clone());
            }
            Value::Object(m)
        }
        Op::Merge { path, pos, props } => {
            let mut m = serde_json::Map::new();
            m.insert("op".into(), json!("merge"));
            m.insert("path".into(), encode_path(path));
            m.insert("pos".into(), json!(pos));
            if let Some(p) = props {
                m.insert("props".into(), p.clone());
            }
            Value::Object(m)
        }
        Op::Extend {
            path,
            props,
            delete_null,
        } => {
            let mut m = serde_json::Map::new();
            m.insert("op".into(), json!("extend"));
            m.insert("path".into(), encode_path(path));
            m.insert("props".into(), Value::Object(props.clone()));
            if *delete_null {
                m.insert("deleteNull".into(), json!(true));
            }
            Value::Object(m)
        }
        Op::Defined { path } => json!({ "op": "defined",   "path": encode_path(path) }),
        Op::Undefined { path } => json!({ "op": "undefined", "path": encode_path(path) }),
        Op::Contains {
            path,
            value,
            ignore_case,
        } => {
            let mut m = serde_json::Map::new();
            m.insert("op".into(), json!("contains"));
            m.insert("path".into(), encode_path(path));
            m.insert("value".into(), json!(value));
            if *ignore_case {
                m.insert("ignore_case".into(), json!(true));
            }
            Value::Object(m)
        }
        Op::Ends {
            path,
            value,
            ignore_case,
        } => {
            let mut m = serde_json::Map::new();
            m.insert("op".into(), json!("ends"));
            m.insert("path".into(), encode_path(path));
            m.insert("value".into(), json!(value));
            if *ignore_case {
                m.insert("ignore_case".into(), json!(true));
            }
            Value::Object(m)
        }
        Op::Starts {
            path,
            value,
            ignore_case,
        } => {
            let mut m = serde_json::Map::new();
            m.insert("op".into(), json!("starts"));
            m.insert("path".into(), encode_path(path));
            m.insert("value".into(), json!(value));
            if *ignore_case {
                m.insert("ignore_case".into(), json!(true));
            }
            Value::Object(m)
        }
        Op::In { path, value } => json!({
            "op": "in",
            "path": encode_path(path),
            "value": value
        }),
        Op::Less { path, value } => json!({
            "op": "less",
            "path": encode_path(path),
            "value": value
        }),
        Op::More { path, value } => json!({
            "op": "more",
            "path": encode_path(path),
            "value": value
        }),
        Op::Matches {
            path,
            value,
            ignore_case,
        } => {
            let mut m = serde_json::Map::new();
            m.insert("op".into(), json!("matches"));
            m.insert("path".into(), encode_path(path));
            m.insert("value".into(), json!(value));
            if *ignore_case {
                m.insert("ignore_case".into(), json!(true));
            }
            Value::Object(m)
        }
        Op::TestType { path, type_vals } => json!({
            "op": "test_type",
            "path": encode_path(path),
            "type": type_vals.iter().map(encode_type).collect::<Vec<_>>()
        }),
        Op::TestString {
            path,
            pos,
            str_val,
            not,
        } => {
            let mut m = serde_json::Map::new();
            m.insert("op".into(), json!("test_string"));
            m.insert("path".into(), encode_path(path));
            m.insert("pos".into(), json!(pos));
            m.insert("str".into(), json!(str_val));
            if *not {
                m.insert("not".into(), json!(true));
            }
            Value::Object(m)
        }
        Op::TestStringLen { path, len, not } => {
            let mut m = serde_json::Map::new();
            m.insert("op".into(), json!("test_string_len"));
            m.insert("path".into(), encode_path(path));
            m.insert("len".into(), json!(len));
            if *not {
                m.insert("not".into(), json!(true));
            }
            Value::Object(m)
        }
        Op::Type { path, value } => json!({
            "op": "type",
            "path": encode_path(path),
            "value": encode_type(value)
        }),
        Op::And { path, ops } => json!({
            "op": "and",
            "path": encode_path(path),
            "apply": ops.iter().map(to_json).collect::<Vec<_>>()
        }),
        Op::Not { path, ops } => json!({
            "op": "not",
            "path": encode_path(path),
            "apply": ops.iter().map(to_json).collect::<Vec<_>>()
        }),
        Op::Or { path, ops } => json!({
            "op": "or",
            "path": encode_path(path),
            "apply": ops.iter().map(to_json).collect::<Vec<_>>()
        }),
    }
}

// ── Deserialization ───────────────────────────────────────────────────────

/// Deserialize a `serde_json::Value` into an `Op`.
pub fn from_json(v: &Value) -> Result<Op, PatchError> {
    let obj = v
        .as_object()
        .ok_or_else(|| PatchError::InvalidOp("operation must be an object".into()))?;
    let op_str = obj
        .get("op")
        .and_then(|v| v.as_str())
        .ok_or_else(|| PatchError::InvalidOp("missing 'op' field".into()))?;

    let get_path = |key: &str| -> Result<Vec<String>, PatchError> {
        obj.get(key)
            .map(decode_path)
            .transpose()?
            .unwrap_or_default()
            .pipe_ok()
    };

    let path = decode_path(obj.get("path").unwrap_or(&Value::String(String::new())))?;

    match op_str {
        "add" => {
            let value = obj
                .get("value")
                .ok_or_else(|| PatchError::InvalidOp("add requires 'value'".into()))?
                .clone();
            Ok(Op::Add { path, value })
        }
        "remove" => {
            let old_value = obj.get("oldValue").cloned();
            Ok(Op::Remove { path, old_value })
        }
        "replace" => {
            let value = obj
                .get("value")
                .ok_or_else(|| PatchError::InvalidOp("replace requires 'value'".into()))?
                .clone();
            let old_value = obj.get("oldValue").cloned();
            Ok(Op::Replace {
                path,
                value,
                old_value,
            })
        }
        "copy" => {
            let from = decode_path(
                obj.get("from")
                    .ok_or_else(|| PatchError::InvalidOp("copy requires 'from'".into()))?,
            )?;
            Ok(Op::Copy { path, from })
        }
        "move" => {
            let from = decode_path(
                obj.get("from")
                    .ok_or_else(|| PatchError::InvalidOp("move requires 'from'".into()))?,
            )?;
            Ok(Op::Move { path, from })
        }
        "test" => {
            let value = obj
                .get("value")
                .ok_or_else(|| PatchError::InvalidOp("test requires 'value'".into()))?
                .clone();
            let not = obj.get("not").and_then(|v| v.as_bool()).unwrap_or(false);
            Ok(Op::Test { path, value, not })
        }
        "str_ins" => {
            let pos = obj
                .get("pos")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| PatchError::InvalidOp("str_ins requires 'pos'".into()))?
                as usize;
            let str_val = obj
                .get("str")
                .and_then(|v| v.as_str())
                .ok_or_else(|| PatchError::InvalidOp("str_ins requires 'str'".into()))?
                .to_string();
            Ok(Op::StrIns { path, pos, str_val })
        }
        "str_del" => {
            let pos = obj
                .get("pos")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| PatchError::InvalidOp("str_del requires 'pos'".into()))?
                as usize;
            let str_val = obj
                .get("str")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let len = obj.get("len").and_then(|v| v.as_u64()).map(|l| l as usize);
            Ok(Op::StrDel {
                path,
                pos,
                str_val,
                len,
            })
        }
        "flip" => Ok(Op::Flip { path }),
        "inc" => {
            let inc = obj
                .get("inc")
                .and_then(|v| v.as_f64())
                .ok_or_else(|| PatchError::InvalidOp("inc requires 'inc'".into()))?;
            Ok(Op::Inc { path, inc })
        }
        "split" => {
            let pos = obj
                .get("pos")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| PatchError::InvalidOp("split requires 'pos'".into()))?
                as usize;
            let props = obj.get("props").cloned();
            Ok(Op::Split { path, pos, props })
        }
        "merge" => {
            let pos = obj
                .get("pos")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| PatchError::InvalidOp("merge requires 'pos'".into()))?
                as usize;
            let props = obj.get("props").cloned();
            Ok(Op::Merge { path, pos, props })
        }
        "extend" => {
            let props = obj
                .get("props")
                .and_then(|v| v.as_object())
                .ok_or_else(|| PatchError::InvalidOp("extend requires 'props'".into()))?
                .clone();
            let delete_null = obj
                .get("deleteNull")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            Ok(Op::Extend {
                path,
                props,
                delete_null,
            })
        }
        "defined" => Ok(Op::Defined { path }),
        "undefined" => Ok(Op::Undefined { path }),
        "contains" => {
            let value = obj
                .get("value")
                .and_then(|v| v.as_str())
                .ok_or_else(|| PatchError::InvalidOp("contains requires 'value'".into()))?
                .to_string();
            let ignore_case = obj
                .get("ignore_case")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            Ok(Op::Contains {
                path,
                value,
                ignore_case,
            })
        }
        "ends" => {
            let value = obj
                .get("value")
                .and_then(|v| v.as_str())
                .ok_or_else(|| PatchError::InvalidOp("ends requires 'value'".into()))?
                .to_string();
            let ignore_case = obj
                .get("ignore_case")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            Ok(Op::Ends {
                path,
                value,
                ignore_case,
            })
        }
        "starts" => {
            let value = obj
                .get("value")
                .and_then(|v| v.as_str())
                .ok_or_else(|| PatchError::InvalidOp("starts requires 'value'".into()))?
                .to_string();
            let ignore_case = obj
                .get("ignore_case")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            Ok(Op::Starts {
                path,
                value,
                ignore_case,
            })
        }
        "in" => {
            let value = obj
                .get("value")
                .and_then(|v| v.as_array())
                .ok_or_else(|| PatchError::InvalidOp("in requires 'value' array".into()))?
                .clone();
            Ok(Op::In { path, value })
        }
        "less" => {
            let value = obj
                .get("value")
                .and_then(|v| v.as_f64())
                .ok_or_else(|| PatchError::InvalidOp("less requires 'value'".into()))?;
            Ok(Op::Less { path, value })
        }
        "more" => {
            let value = obj
                .get("value")
                .and_then(|v| v.as_f64())
                .ok_or_else(|| PatchError::InvalidOp("more requires 'value'".into()))?;
            Ok(Op::More { path, value })
        }
        "matches" => {
            let value = obj
                .get("value")
                .and_then(|v| v.as_str())
                .ok_or_else(|| PatchError::InvalidOp("matches requires 'value'".into()))?
                .to_string();
            let ignore_case = obj
                .get("ignore_case")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            Ok(Op::Matches {
                path,
                value,
                ignore_case,
            })
        }
        "test_type" => {
            let types = obj
                .get("type")
                .and_then(|v| v.as_array())
                .ok_or_else(|| PatchError::InvalidOp("test_type requires 'type' array".into()))?;
            let type_vals: Result<Vec<_>, _> = types.iter().map(decode_type).collect();
            Ok(Op::TestType {
                path,
                type_vals: type_vals?,
            })
        }
        "test_string" => {
            let pos = obj
                .get("pos")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| PatchError::InvalidOp("test_string requires 'pos'".into()))?
                as usize;
            let str_val = obj
                .get("str")
                .and_then(|v| v.as_str())
                .ok_or_else(|| PatchError::InvalidOp("test_string requires 'str'".into()))?
                .to_string();
            let not = obj.get("not").and_then(|v| v.as_bool()).unwrap_or(false);
            Ok(Op::TestString {
                path,
                pos,
                str_val,
                not,
            })
        }
        "test_string_len" => {
            let len = obj
                .get("len")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| PatchError::InvalidOp("test_string_len requires 'len'".into()))?
                as usize;
            let not = obj.get("not").and_then(|v| v.as_bool()).unwrap_or(false);
            Ok(Op::TestStringLen { path, len, not })
        }
        "type" => {
            let value = decode_type(
                obj.get("value")
                    .ok_or_else(|| PatchError::InvalidOp("type requires 'value'".into()))?,
            )?;
            Ok(Op::Type { path, value })
        }
        "and" => {
            let ops = decode_ops(
                obj.get("apply")
                    .ok_or_else(|| PatchError::InvalidOp("and requires 'apply'".into()))?,
            )?;
            Ok(Op::And { path, ops })
        }
        "not" => {
            let ops = decode_ops(
                obj.get("apply")
                    .ok_or_else(|| PatchError::InvalidOp("not requires 'apply'".into()))?,
            )?;
            Ok(Op::Not { path, ops })
        }
        "or" => {
            let ops = decode_ops(
                obj.get("apply")
                    .ok_or_else(|| PatchError::InvalidOp("or requires 'apply'".into()))?,
            )?;
            Ok(Op::Or { path, ops })
        }
        other => Err(PatchError::InvalidOp(format!("unknown op: {other}"))),
    }
}

trait PipeOk {
    fn pipe_ok(self) -> Result<Self, PatchError>
    where
        Self: Sized;
}
impl PipeOk for Vec<String> {
    fn pipe_ok(self) -> Result<Self, PatchError> {
        Ok(self)
    }
}

/// Serialize a list of operations to a JSON array.
pub fn to_json_patch(ops: &[Op]) -> Value {
    Value::Array(ops.iter().map(to_json).collect())
}

/// Deserialize a JSON array into a list of operations.
pub fn from_json_patch(v: &Value) -> Result<Vec<Op>, PatchError> {
    let arr = v
        .as_array()
        .ok_or_else(|| PatchError::InvalidOp("patch must be an array".into()))?;
    arr.iter().map(from_json).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn roundtrip(op: Op) -> Op {
        let v = to_json(&op);
        from_json(&v).expect("roundtrip failed")
    }

    #[test]
    fn roundtrip_add() {
        let op = Op::Add {
            path: vec!["a".to_string()],
            value: json!(42),
        };
        let rt = roundtrip(op);
        assert_eq!(rt.op_name(), "add");
    }

    #[test]
    fn roundtrip_remove() {
        let op = Op::Remove {
            path: vec!["a".to_string()],
            old_value: None,
        };
        let rt = roundtrip(op);
        assert_eq!(rt.op_name(), "remove");
    }

    #[test]
    fn roundtrip_replace() {
        let op = Op::Replace {
            path: vec!["x".to_string()],
            value: json!("new"),
            old_value: Some(json!("old")),
        };
        let v = to_json(&op);
        assert_eq!(v["op"], "replace");
        assert_eq!(v["oldValue"], "old");
    }

    #[test]
    fn roundtrip_test_type() {
        let op = Op::TestType {
            path: vec!["n".to_string()],
            type_vals: vec![JsonPatchType::Number, JsonPatchType::Integer],
        };
        let rt = roundtrip(op);
        assert_eq!(rt.op_name(), "test_type");
    }

    #[test]
    fn decode_rfc6902_patch() {
        let patch_json = json!([
            {"op": "add", "path": "/foo", "value": 1},
            {"op": "remove", "path": "/bar"},
            {"op": "replace", "path": "/baz", "value": "new"},
        ]);
        let ops = from_json_patch(&patch_json).unwrap();
        assert_eq!(ops.len(), 3);
        assert_eq!(ops[0].op_name(), "add");
        assert_eq!(ops[1].op_name(), "remove");
        assert_eq!(ops[2].op_name(), "replace");
    }

    #[test]
    fn roundtrip_and_predicate() {
        let op = Op::And {
            path: vec![],
            ops: vec![
                Op::Defined {
                    path: vec!["a".to_string()],
                },
                Op::Less {
                    path: vec!["a".to_string()],
                    value: 100.0,
                },
            ],
        };
        let rt = roundtrip(op);
        assert_eq!(rt.op_name(), "and");
    }
}
