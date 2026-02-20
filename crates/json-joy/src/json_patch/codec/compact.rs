//! Compact array codec for JSON Patch operations.
//!
//! Mirrors `packages/json-joy/src/json-patch/codec/compact/`.
//!
//! The compact format represents each op as a JSON array:
//!   `[opcode, path, ...args]`
//!
//! where `opcode` is either a numeric u8 (matching OPCODE constants) or a
//! string name (when `string_opcode = true`), and `path` is a JSON Pointer
//! string or a path array.
//!
//! The decode side accepts both numeric and string opcodes.

use serde_json::{json, Value};

use crate::json_patch::types::{JsonPatchType, Op, PatchError, Path};

// ── OPCODE constants (mirrors constants.ts) ────────────────────────────────

pub const OPCODE_ADD: u8 = 0;
pub const OPCODE_REMOVE: u8 = 1;
pub const OPCODE_REPLACE: u8 = 2;
pub const OPCODE_COPY: u8 = 3;
pub const OPCODE_MOVE: u8 = 4;
pub const OPCODE_TEST: u8 = 5;
pub const OPCODE_STR_INS: u8 = 6;
pub const OPCODE_STR_DEL: u8 = 7;
pub const OPCODE_FLIP: u8 = 8;
pub const OPCODE_INC: u8 = 9;
pub const OPCODE_SPLIT: u8 = 10;
pub const OPCODE_MERGE: u8 = 11;
pub const OPCODE_EXTEND: u8 = 12;
pub const OPCODE_CONTAINS: u8 = 30;
pub const OPCODE_DEFINED: u8 = 31;
pub const OPCODE_ENDS: u8 = 32;
pub const OPCODE_IN: u8 = 33;
pub const OPCODE_LESS: u8 = 34;
pub const OPCODE_MATCHES: u8 = 35;
pub const OPCODE_MORE: u8 = 36;
pub const OPCODE_STARTS: u8 = 37;
pub const OPCODE_UNDEFINED: u8 = 38;
pub const OPCODE_TEST_TYPE: u8 = 39;
pub const OPCODE_TEST_STRING: u8 = 40;
pub const OPCODE_TEST_STRING_LEN: u8 = 41;
pub const OPCODE_TYPE: u8 = 42;
pub const OPCODE_AND: u8 = 43;
pub const OPCODE_NOT: u8 = 44;
pub const OPCODE_OR: u8 = 45;

// ── Path encoding ──────────────────────────────────────────────────────────

/// Encodes a path as a JSON array of path components (strings/numbers).
/// In the compact format, paths are stored as arrays, not JSON Pointer strings.
fn encode_path_as_array(path: &[String]) -> Value {
    Value::Array(
        path.iter()
            .map(|s| {
                // If the segment is a pure integer, encode as number for compactness.
                if let Ok(n) = s.parse::<u64>() {
                    json!(n)
                } else {
                    json!(s)
                }
            })
            .collect(),
    )
}

/// Decodes a path from a compact value.
///
/// The path may be:
///  - A JSON Pointer string (`"/a/b"`)
///  - An array of path components (`["a", "b"]` or `["a", 1]`)
fn decode_path_from_value(v: &Value) -> Result<Path, PatchError> {
    match v {
        Value::String(s) => Ok(json_joy_json_pointer::parse_json_pointer(s)),
        Value::Array(arr) => arr
            .iter()
            .map(|item| match item {
                Value::String(s) => Ok(s.clone()),
                Value::Number(n) => Ok(n.to_string()),
                _ => Err(PatchError::InvalidOp(
                    "path component must be string or number".into(),
                )),
            })
            .collect(),
        Value::Null => Ok(vec![]),
        _ => Err(PatchError::InvalidOp(
            "path must be a string or array".into(),
        )),
    }
}

/// Merges a parent path with a relative child path array.
fn merge_paths(parent_path: &[String], child_val: &Value) -> Result<Path, PatchError> {
    let child = decode_path_from_value(child_val)?;
    let mut full = parent_path.to_vec();
    full.extend(child);
    Ok(full)
}

// ── Encode ─────────────────────────────────────────────────────────────────

/// Options for the compact encoder.
#[derive(Debug, Clone, Default)]
pub struct EncodeOptions {
    /// If true, encode opcodes as strings (e.g. `"add"`) instead of numbers.
    pub string_opcode: bool,
}

/// Encodes a list of ops as a JSON array of compact arrays.
pub fn encode(ops: &[Op], options: &EncodeOptions) -> Value {
    Value::Array(ops.iter().map(|op| encode_op(op, None, options)).collect())
}

fn opcode_value(numeric: u8, name: &'static str, options: &EncodeOptions) -> Value {
    if options.string_opcode {
        json!(name)
    } else {
        json!(numeric)
    }
}

fn encode_op(op: &Op, parent_path: Option<&[String]>, options: &EncodeOptions) -> Value {
    // For predicate ops that have a parent (inside and/or/not), encode path relative to parent.
    let relative_path = |path: &[String]| -> Value {
        if let Some(pp) = parent_path {
            encode_path_as_array(&path[pp.len()..])
        } else {
            encode_path_as_array(path)
        }
    };

    match op {
        Op::Add { path, value } => json!([
            opcode_value(OPCODE_ADD, "add", options),
            encode_path_as_array(path),
            value
        ]),
        Op::Remove { path, old_value } => {
            let opcode = opcode_value(OPCODE_REMOVE, "remove", options);
            if let Some(ov) = old_value {
                json!([opcode, encode_path_as_array(path), ov])
            } else {
                json!([opcode, encode_path_as_array(path)])
            }
        }
        Op::Replace {
            path,
            value,
            old_value,
        } => {
            let opcode = opcode_value(OPCODE_REPLACE, "replace", options);
            if let Some(ov) = old_value {
                json!([opcode, encode_path_as_array(path), value, ov])
            } else {
                json!([opcode, encode_path_as_array(path), value])
            }
        }
        Op::Copy { path, from } => json!([
            opcode_value(OPCODE_COPY, "copy", options),
            encode_path_as_array(path),
            encode_path_as_array(from)
        ]),
        Op::Move { path, from } => json!([
            opcode_value(OPCODE_MOVE, "move", options),
            encode_path_as_array(path),
            encode_path_as_array(from)
        ]),
        Op::Test { path, value, not } => {
            let opcode = opcode_value(OPCODE_TEST, "test", options);
            let rp = relative_path(path);
            if *not {
                json!([opcode, rp, value, 1])
            } else {
                json!([opcode, rp, value])
            }
        }
        Op::StrIns { path, pos, str_val } => json!([
            opcode_value(OPCODE_STR_INS, "str_ins", options),
            encode_path_as_array(path),
            pos,
            str_val
        ]),
        Op::StrDel {
            path,
            pos,
            str_val,
            len,
        } => {
            let opcode = opcode_value(OPCODE_STR_DEL, "str_del", options);
            if let Some(s) = str_val {
                json!([opcode, encode_path_as_array(path), pos, s])
            } else {
                json!([opcode, encode_path_as_array(path), pos, 0, len])
            }
        }
        Op::Flip { path } => json!([
            opcode_value(OPCODE_FLIP, "flip", options),
            encode_path_as_array(path)
        ]),
        Op::Inc { path, inc } => json!([
            opcode_value(OPCODE_INC, "inc", options),
            encode_path_as_array(path),
            inc
        ]),
        Op::Split { path, pos, props } => {
            let opcode = opcode_value(OPCODE_SPLIT, "split", options);
            if let Some(p) = props {
                json!([opcode, encode_path_as_array(path), pos, p])
            } else {
                json!([opcode, encode_path_as_array(path), pos])
            }
        }
        Op::Merge { path, pos, props } => {
            let opcode = opcode_value(OPCODE_MERGE, "merge", options);
            if let Some(p) = props {
                json!([opcode, encode_path_as_array(path), pos, p])
            } else {
                json!([opcode, encode_path_as_array(path), pos])
            }
        }
        Op::Extend {
            path,
            props,
            delete_null,
        } => {
            let opcode = opcode_value(OPCODE_EXTEND, "extend", options);
            if *delete_null {
                json!([
                    opcode,
                    encode_path_as_array(path),
                    Value::Object(props.clone()),
                    1
                ])
            } else {
                json!([
                    opcode,
                    encode_path_as_array(path),
                    Value::Object(props.clone())
                ])
            }
        }
        Op::Defined { path } => json!([
            opcode_value(OPCODE_DEFINED, "defined", options),
            relative_path(path)
        ]),
        Op::Undefined { path } => json!([
            opcode_value(OPCODE_UNDEFINED, "undefined", options),
            relative_path(path)
        ]),
        Op::Contains {
            path,
            value,
            ignore_case,
        } => {
            let opcode = opcode_value(OPCODE_CONTAINS, "contains", options);
            let rp = relative_path(path);
            if *ignore_case {
                json!([opcode, rp, value, 1])
            } else {
                json!([opcode, rp, value])
            }
        }
        Op::Ends {
            path,
            value,
            ignore_case,
        } => {
            let opcode = opcode_value(OPCODE_ENDS, "ends", options);
            let rp = relative_path(path);
            if *ignore_case {
                json!([opcode, rp, value, 1])
            } else {
                json!([opcode, rp, value])
            }
        }
        Op::Starts {
            path,
            value,
            ignore_case,
        } => {
            let opcode = opcode_value(OPCODE_STARTS, "starts", options);
            let rp = relative_path(path);
            if *ignore_case {
                json!([opcode, rp, value, 1])
            } else {
                json!([opcode, rp, value])
            }
        }
        Op::In { path, value } => json!([
            opcode_value(OPCODE_IN, "in", options),
            relative_path(path),
            Value::Array(value.clone())
        ]),
        Op::Less { path, value } => json!([
            opcode_value(OPCODE_LESS, "less", options),
            relative_path(path),
            value
        ]),
        Op::More { path, value } => json!([
            opcode_value(OPCODE_MORE, "more", options),
            relative_path(path),
            value
        ]),
        Op::Matches {
            path,
            value,
            ignore_case,
        } => {
            let opcode = opcode_value(OPCODE_MATCHES, "matches", options);
            let rp = relative_path(path);
            if *ignore_case {
                json!([opcode, rp, value, 1])
            } else {
                json!([opcode, rp, value])
            }
        }
        Op::TestType { path, type_vals } => {
            let types: Vec<Value> = type_vals.iter().map(|t| json!(t.as_str())).collect();
            json!([
                opcode_value(OPCODE_TEST_TYPE, "test_type", options),
                relative_path(path),
                Value::Array(types)
            ])
        }
        Op::TestString {
            path,
            pos,
            str_val,
            not,
        } => {
            let opcode = opcode_value(OPCODE_TEST_STRING, "test_string", options);
            let rp = relative_path(path);
            if *not {
                json!([opcode, rp, pos, str_val, 1])
            } else {
                json!([opcode, rp, pos, str_val])
            }
        }
        Op::TestStringLen { path, len, not } => {
            let opcode = opcode_value(OPCODE_TEST_STRING_LEN, "test_string_len", options);
            let rp = relative_path(path);
            if *not {
                json!([opcode, rp, len, 1])
            } else {
                json!([opcode, rp, len])
            }
        }
        Op::Type { path, value } => json!([
            opcode_value(OPCODE_TYPE, "type", options),
            relative_path(path),
            value.as_str()
        ]),
        Op::And { path, ops } => {
            let opcode = opcode_value(OPCODE_AND, "and", options);
            let rp = relative_path(path);
            let sub_ops: Vec<Value> = ops
                .iter()
                .map(|op| encode_op(op, Some(path), options))
                .collect();
            json!([opcode, rp, Value::Array(sub_ops)])
        }
        Op::Not { path, ops } => {
            let opcode = opcode_value(OPCODE_NOT, "not", options);
            let rp = relative_path(path);
            let sub_ops: Vec<Value> = ops
                .iter()
                .map(|op| encode_op(op, Some(path), options))
                .collect();
            json!([opcode, rp, Value::Array(sub_ops)])
        }
        Op::Or { path, ops } => {
            let opcode = opcode_value(OPCODE_OR, "or", options);
            let rp = relative_path(path);
            let sub_ops: Vec<Value> = ops
                .iter()
                .map(|op| encode_op(op, Some(path), options))
                .collect();
            json!([opcode, rp, Value::Array(sub_ops)])
        }
    }
}

// ── Decode ─────────────────────────────────────────────────────────────────

/// Decodes a JSON array of compact ops into a list of `Op` values.
pub fn decode(data: &Value) -> Result<Vec<Op>, PatchError> {
    let arr = data
        .as_array()
        .ok_or_else(|| PatchError::InvalidOp("compact patch must be an array".into()))?;
    arr.iter().map(|v| decode_op(v, None)).collect()
}

fn get_u8_or_str_opcode(v: &Value) -> Result<OpCodeKey<'_>, PatchError> {
    match v {
        Value::Number(n) => {
            let code = n
                .as_u64()
                .ok_or_else(|| PatchError::InvalidOp("opcode must be u8".into()))?
                as u8;
            Ok(OpCodeKey::Numeric(code))
        }
        Value::String(s) => Ok(OpCodeKey::Name(s.as_str())),
        _ => Err(PatchError::InvalidOp(
            "opcode must be number or string".into(),
        )),
    }
}

enum OpCodeKey<'a> {
    Numeric(u8),
    Name(&'a str),
}

impl<'a> OpCodeKey<'a> {
    fn matches(&self, numeric: u8, name: &str) -> bool {
        match self {
            OpCodeKey::Numeric(n) => *n == numeric,
            OpCodeKey::Name(s) => *s == name,
        }
    }
}

fn arr_get(arr: &[Value], idx: usize) -> Result<&Value, PatchError> {
    arr.get(idx).ok_or_else(|| {
        PatchError::InvalidOp(format!("compact op array too short, missing index {idx}"))
    })
}

fn decode_op(v: &Value, parent_path: Option<&[String]>) -> Result<Op, PatchError> {
    let arr = v
        .as_array()
        .ok_or_else(|| PatchError::InvalidOp("compact op must be an array".into()))?;
    if arr.is_empty() {
        return Err(PatchError::InvalidOp("compact op array is empty".into()));
    }
    let key = get_u8_or_str_opcode(&arr[0])?;
    let len = arr.len();

    if key.matches(OPCODE_ADD, "add") {
        let path = decode_path_from_value(arr_get(arr, 1)?)?;
        let value = arr_get(arr, 2)?.clone();
        return Ok(Op::Add { path, value });
    }
    if key.matches(OPCODE_REMOVE, "remove") {
        let path = decode_path_from_value(arr_get(arr, 1)?)?;
        let old_value = arr.get(2).cloned();
        return Ok(Op::Remove { path, old_value });
    }
    if key.matches(OPCODE_REPLACE, "replace") {
        let path = decode_path_from_value(arr_get(arr, 1)?)?;
        let value = arr_get(arr, 2)?.clone();
        let old_value = arr.get(3).cloned();
        return Ok(Op::Replace {
            path,
            value,
            old_value,
        });
    }
    if key.matches(OPCODE_COPY, "copy") {
        let path = decode_path_from_value(arr_get(arr, 1)?)?;
        let from = decode_path_from_value(arr_get(arr, 2)?)?;
        return Ok(Op::Copy { path, from });
    }
    if key.matches(OPCODE_MOVE, "move") {
        let path = decode_path_from_value(arr_get(arr, 1)?)?;
        let from = decode_path_from_value(arr_get(arr, 2)?)?;
        return Ok(Op::Move { path, from });
    }
    if key.matches(OPCODE_TEST, "test") {
        let path = decode_relative_path(arr_get(arr, 1)?, parent_path)?;
        let value = arr_get(arr, 2)?.clone();
        let not = arr
            .get(3)
            .and_then(|v| v.as_u64())
            .map(|n| n != 0)
            .unwrap_or(false);
        return Ok(Op::Test { path, value, not });
    }
    if key.matches(OPCODE_STR_INS, "str_ins") {
        let path = decode_path_from_value(arr_get(arr, 1)?)?;
        let pos = arr_get(arr, 2)?
            .as_u64()
            .ok_or_else(|| PatchError::InvalidOp("str_ins: pos must be number".into()))?
            as usize;
        let str_val = arr_get(arr, 3)?
            .as_str()
            .ok_or_else(|| PatchError::InvalidOp("str_ins: str must be string".into()))?
            .to_string();
        return Ok(Op::StrIns { path, pos, str_val });
    }
    if key.matches(OPCODE_STR_DEL, "str_del") {
        let path = decode_path_from_value(arr_get(arr, 1)?)?;
        let pos = arr_get(arr, 2)?
            .as_u64()
            .ok_or_else(|| PatchError::InvalidOp("str_del: pos must be number".into()))?
            as usize;
        // len == 4 means str form; len == 5 means numeric-length form (arr[3] == 0, arr[4] == len)
        if len < 5 {
            let str_val = arr_get(arr, 3)?
                .as_str()
                .ok_or_else(|| PatchError::InvalidOp("str_del: str must be string".into()))?
                .to_string();
            return Ok(Op::StrDel {
                path,
                pos,
                str_val: Some(str_val),
                len: None,
            });
        } else {
            // arr[3] is 0 (sentinel), arr[4] is len
            let del_len = arr_get(arr, 4)?
                .as_u64()
                .ok_or_else(|| PatchError::InvalidOp("str_del: len must be number".into()))?
                as usize;
            return Ok(Op::StrDel {
                path,
                pos,
                str_val: None,
                len: Some(del_len),
            });
        }
    }
    if key.matches(OPCODE_FLIP, "flip") {
        let path = decode_path_from_value(arr_get(arr, 1)?)?;
        return Ok(Op::Flip { path });
    }
    if key.matches(OPCODE_INC, "inc") {
        let path = decode_path_from_value(arr_get(arr, 1)?)?;
        let inc = arr_get(arr, 2)?
            .as_f64()
            .ok_or_else(|| PatchError::InvalidOp("inc: inc must be number".into()))?;
        return Ok(Op::Inc { path, inc });
    }
    if key.matches(OPCODE_SPLIT, "split") {
        let path = decode_path_from_value(arr_get(arr, 1)?)?;
        let pos = arr_get(arr, 2)?
            .as_u64()
            .ok_or_else(|| PatchError::InvalidOp("split: pos must be number".into()))?
            as usize;
        let props = arr.get(3).filter(|v| !v.is_null()).cloned();
        return Ok(Op::Split { path, pos, props });
    }
    if key.matches(OPCODE_MERGE, "merge") {
        let path = decode_path_from_value(arr_get(arr, 1)?)?;
        let pos = arr_get(arr, 2)?
            .as_u64()
            .ok_or_else(|| PatchError::InvalidOp("merge: pos must be number".into()))?
            as usize;
        let props = arr.get(3).filter(|v| !v.is_null()).cloned();
        return Ok(Op::Merge { path, pos, props });
    }
    if key.matches(OPCODE_EXTEND, "extend") {
        let path = decode_path_from_value(arr_get(arr, 1)?)?;
        let props = arr_get(arr, 2)?
            .as_object()
            .ok_or_else(|| PatchError::InvalidOp("extend: props must be object".into()))?
            .clone();
        let delete_null = arr
            .get(3)
            .and_then(|v| v.as_u64())
            .map(|n| n != 0)
            .unwrap_or(false);
        return Ok(Op::Extend {
            path,
            props,
            delete_null,
        });
    }
    if key.matches(OPCODE_DEFINED, "defined") {
        let path = decode_relative_path(arr_get(arr, 1)?, parent_path)?;
        return Ok(Op::Defined { path });
    }
    if key.matches(OPCODE_UNDEFINED, "undefined") {
        let path = decode_relative_path(arr_get(arr, 1)?, parent_path)?;
        return Ok(Op::Undefined { path });
    }
    if key.matches(OPCODE_CONTAINS, "contains") {
        let path = decode_relative_path(arr_get(arr, 1)?, parent_path)?;
        let value = arr_get(arr, 2)?
            .as_str()
            .ok_or_else(|| PatchError::InvalidOp("contains: value must be string".into()))?
            .to_string();
        let ignore_case = arr
            .get(3)
            .and_then(|v| v.as_u64())
            .map(|n| n != 0)
            .unwrap_or(false);
        return Ok(Op::Contains {
            path,
            value,
            ignore_case,
        });
    }
    if key.matches(OPCODE_ENDS, "ends") {
        let path = decode_relative_path(arr_get(arr, 1)?, parent_path)?;
        let value = arr_get(arr, 2)?
            .as_str()
            .ok_or_else(|| PatchError::InvalidOp("ends: value must be string".into()))?
            .to_string();
        let ignore_case = arr
            .get(3)
            .and_then(|v| v.as_u64())
            .map(|n| n != 0)
            .unwrap_or(false);
        return Ok(Op::Ends {
            path,
            value,
            ignore_case,
        });
    }
    if key.matches(OPCODE_STARTS, "starts") {
        let path = decode_relative_path(arr_get(arr, 1)?, parent_path)?;
        let value = arr_get(arr, 2)?
            .as_str()
            .ok_or_else(|| PatchError::InvalidOp("starts: value must be string".into()))?
            .to_string();
        let ignore_case = arr
            .get(3)
            .and_then(|v| v.as_u64())
            .map(|n| n != 0)
            .unwrap_or(false);
        return Ok(Op::Starts {
            path,
            value,
            ignore_case,
        });
    }
    if key.matches(OPCODE_IN, "in") {
        let path = decode_relative_path(arr_get(arr, 1)?, parent_path)?;
        let value = arr_get(arr, 2)?
            .as_array()
            .ok_or_else(|| PatchError::InvalidOp("in: value must be array".into()))?
            .clone();
        return Ok(Op::In { path, value });
    }
    if key.matches(OPCODE_LESS, "less") {
        let path = decode_relative_path(arr_get(arr, 1)?, parent_path)?;
        let value = arr_get(arr, 2)?
            .as_f64()
            .ok_or_else(|| PatchError::InvalidOp("less: value must be number".into()))?;
        return Ok(Op::Less { path, value });
    }
    if key.matches(OPCODE_MORE, "more") {
        let path = decode_relative_path(arr_get(arr, 1)?, parent_path)?;
        let value = arr_get(arr, 2)?
            .as_f64()
            .ok_or_else(|| PatchError::InvalidOp("more: value must be number".into()))?;
        return Ok(Op::More { path, value });
    }
    if key.matches(OPCODE_MATCHES, "matches") {
        let path = decode_relative_path(arr_get(arr, 1)?, parent_path)?;
        let value = arr_get(arr, 2)?
            .as_str()
            .ok_or_else(|| PatchError::InvalidOp("matches: value must be string".into()))?
            .to_string();
        let ignore_case = arr
            .get(3)
            .and_then(|v| v.as_u64())
            .map(|n| n != 0)
            .unwrap_or(false);
        return Ok(Op::Matches {
            path,
            value,
            ignore_case,
        });
    }
    if key.matches(OPCODE_TEST_TYPE, "test_type") {
        let path = decode_relative_path(arr_get(arr, 1)?, parent_path)?;
        let types_arr = arr_get(arr, 2)?
            .as_array()
            .ok_or_else(|| PatchError::InvalidOp("test_type: type must be array".into()))?;
        let type_vals: Result<Vec<JsonPatchType>, PatchError> = types_arr
            .iter()
            .map(|v| {
                let s = v.as_str().ok_or_else(|| {
                    PatchError::InvalidOp("test_type: type element must be string".into())
                })?;
                JsonPatchType::parse_str(s)
            })
            .collect();
        return Ok(Op::TestType {
            path,
            type_vals: type_vals?,
        });
    }
    if key.matches(OPCODE_TEST_STRING, "test_string") {
        let path = decode_relative_path(arr_get(arr, 1)?, parent_path)?;
        let pos = arr_get(arr, 2)?
            .as_u64()
            .ok_or_else(|| PatchError::InvalidOp("test_string: pos must be number".into()))?
            as usize;
        let str_val = arr_get(arr, 3)?
            .as_str()
            .ok_or_else(|| PatchError::InvalidOp("test_string: str must be string".into()))?
            .to_string();
        let not = arr
            .get(4)
            .and_then(|v| v.as_u64())
            .map(|n| n != 0)
            .unwrap_or(false);
        return Ok(Op::TestString {
            path,
            pos,
            str_val,
            not,
        });
    }
    if key.matches(OPCODE_TEST_STRING_LEN, "test_string_len") {
        let path = decode_relative_path(arr_get(arr, 1)?, parent_path)?;
        let the_len = arr_get(arr, 2)?
            .as_u64()
            .ok_or_else(|| PatchError::InvalidOp("test_string_len: len must be number".into()))?
            as usize;
        let not = arr
            .get(3)
            .and_then(|v| v.as_u64())
            .map(|n| n != 0)
            .unwrap_or(false);
        return Ok(Op::TestStringLen {
            path,
            len: the_len,
            not,
        });
    }
    if key.matches(OPCODE_TYPE, "type") {
        let path = decode_relative_path(arr_get(arr, 1)?, parent_path)?;
        let type_str = arr_get(arr, 2)?
            .as_str()
            .ok_or_else(|| PatchError::InvalidOp("type: value must be string".into()))?;
        let value = JsonPatchType::parse_str(type_str)?;
        return Ok(Op::Type { path, value });
    }
    if key.matches(OPCODE_AND, "and") {
        let path = decode_relative_path(arr_get(arr, 1)?, parent_path)?;
        let sub_arr = arr_get(arr, 2)?
            .as_array()
            .ok_or_else(|| PatchError::InvalidOp("and: ops must be array".into()))?;
        let ops: Result<Vec<Op>, PatchError> = sub_arr
            .iter()
            .map(|v| decode_op_relative(v, &path))
            .collect();
        return Ok(Op::And { path, ops: ops? });
    }
    if key.matches(OPCODE_NOT, "not") {
        let path = decode_relative_path(arr_get(arr, 1)?, parent_path)?;
        let sub_arr = arr_get(arr, 2)?
            .as_array()
            .ok_or_else(|| PatchError::InvalidOp("not: ops must be array".into()))?;
        let ops: Result<Vec<Op>, PatchError> = sub_arr
            .iter()
            .map(|v| decode_op_relative(v, &path))
            .collect();
        return Ok(Op::Not { path, ops: ops? });
    }
    if key.matches(OPCODE_OR, "or") {
        let path = decode_relative_path(arr_get(arr, 1)?, parent_path)?;
        let sub_arr = arr_get(arr, 2)?
            .as_array()
            .ok_or_else(|| PatchError::InvalidOp("or: ops must be array".into()))?;
        let ops: Result<Vec<Op>, PatchError> = sub_arr
            .iter()
            .map(|v| decode_op_relative(v, &path))
            .collect();
        return Ok(Op::Or { path, ops: ops? });
    }

    Err(PatchError::InvalidOp("OP_UNKNOWN".into()))
}

/// Decode a sub-op whose path is relative to `parent_path`.
fn decode_op_relative(v: &Value, parent_path: &[String]) -> Result<Op, PatchError> {
    decode_op(v, Some(parent_path))
}

/// Decodes a path that may be relative to a parent path.
///
/// When `parent_path` is `Some`, the decoded path is prepended with the parent path.
fn decode_relative_path(v: &Value, parent_path: Option<&[String]>) -> Result<Path, PatchError> {
    if let Some(pp) = parent_path {
        merge_paths(pp, v)
    } else {
        decode_path_from_value(v)
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_patch::codec::json::to_json;
    use serde_json::json;

    fn opts() -> EncodeOptions {
        EncodeOptions {
            string_opcode: false,
        }
    }
    fn opts_str() -> EncodeOptions {
        EncodeOptions {
            string_opcode: true,
        }
    }

    /// Helper: encode via JSON codec then roundtrip through compact codec, compare JSON output.
    fn json_roundtrip(op: Op) {
        let json_val = to_json(&op);
        let ops = vec![op];
        // numeric opcode roundtrip
        let compact = encode(&ops, &opts());
        let decoded = decode(&compact).expect("decode failed");
        assert_eq!(
            to_json(&decoded[0]),
            json_val,
            "numeric opcode roundtrip failed"
        );
        // string opcode roundtrip
        let compact_str = encode(&ops, &opts_str());
        let decoded_str = decode(&compact_str).expect("decode str failed");
        assert_eq!(
            to_json(&decoded_str[0]),
            json_val,
            "string opcode roundtrip failed"
        );
    }

    #[test]
    fn roundtrip_add() {
        let op = Op::Add {
            path: vec!["a".to_string()],
            value: json!(42),
        };
        json_roundtrip(op);
    }

    #[test]
    fn roundtrip_remove() {
        let op = Op::Remove {
            path: vec!["a".to_string()],
            old_value: None,
        };
        json_roundtrip(op);
    }

    #[test]
    fn roundtrip_remove_with_old_value() {
        let op = Op::Remove {
            path: vec!["x".to_string()],
            old_value: Some(json!("prev")),
        };
        json_roundtrip(op);
    }

    #[test]
    fn roundtrip_replace() {
        let op = Op::Replace {
            path: vec!["x".to_string()],
            value: json!("new"),
            old_value: Some(json!("old")),
        };
        json_roundtrip(op);
    }

    #[test]
    fn roundtrip_copy() {
        let op = Op::Copy {
            path: vec!["b".to_string()],
            from: vec!["a".to_string()],
        };
        json_roundtrip(op);
    }

    #[test]
    fn roundtrip_move() {
        let op = Op::Move {
            path: vec!["b".to_string()],
            from: vec!["a".to_string()],
        };
        json_roundtrip(op);
    }

    #[test]
    fn roundtrip_test() {
        let op = Op::Test {
            path: vec!["a".to_string()],
            value: json!(1),
            not: false,
        };
        json_roundtrip(op);
    }

    #[test]
    fn roundtrip_test_not() {
        let op = Op::Test {
            path: vec!["a".to_string()],
            value: json!(null),
            not: true,
        };
        json_roundtrip(op);
    }

    #[test]
    fn roundtrip_str_ins() {
        let op = Op::StrIns {
            path: vec!["s".to_string()],
            pos: 3,
            str_val: "hello".to_string(),
        };
        json_roundtrip(op);
    }

    #[test]
    fn roundtrip_str_del_with_str() {
        let op = Op::StrDel {
            path: vec!["s".to_string()],
            pos: 2,
            str_val: Some("ell".to_string()),
            len: None,
        };
        json_roundtrip(op);
    }

    #[test]
    fn roundtrip_str_del_with_len() {
        let op = Op::StrDel {
            path: vec!["s".to_string()],
            pos: 2,
            str_val: None,
            len: Some(3),
        };
        // Note: JSON codec only serializes str or len, so check both survive compact roundtrip
        let ops = vec![op];
        let compact = encode(&ops, &opts());
        let decoded = decode(&compact).expect("decode failed");
        match &decoded[0] {
            Op::StrDel {
                pos, str_val, len, ..
            } => {
                assert_eq!(*pos, 2);
                assert!(str_val.is_none());
                assert_eq!(*len, Some(3));
            }
            _ => panic!("wrong op type"),
        }
    }

    #[test]
    fn roundtrip_flip() {
        let op = Op::Flip {
            path: vec!["b".to_string()],
        };
        json_roundtrip(op);
    }

    #[test]
    fn roundtrip_inc() {
        let op = Op::Inc {
            path: vec!["n".to_string()],
            inc: 5.0,
        };
        json_roundtrip(op);
    }

    #[test]
    fn roundtrip_and() {
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
        json_roundtrip(op);
    }

    #[test]
    fn roundtrip_or() {
        let op = Op::Or {
            path: vec!["x".to_string()],
            ops: vec![Op::Defined {
                path: vec!["x".to_string()],
            }],
        };
        json_roundtrip(op);
    }

    #[test]
    fn roundtrip_not() {
        let op = Op::Not {
            path: vec![],
            ops: vec![Op::Undefined {
                path: vec!["x".to_string()],
            }],
        };
        json_roundtrip(op);
    }

    #[test]
    fn encode_format_numeric_opcode() {
        let op = Op::Add {
            path: vec!["foo".to_string()],
            value: json!(1),
        };
        let compact = encode(&[op], &opts());
        let arr = compact.as_array().unwrap();
        // Outer array has one op
        let op_arr = arr[0].as_array().unwrap();
        assert_eq!(op_arr[0], json!(OPCODE_ADD));
        // path is an array
        assert_eq!(op_arr[1], json!(["foo"]));
        assert_eq!(op_arr[2], json!(1));
    }

    #[test]
    fn encode_format_string_opcode() {
        let op = Op::Remove {
            path: vec!["a".to_string()],
            old_value: None,
        };
        let compact = encode(&[op], &opts_str());
        let arr = compact.as_array().unwrap();
        let op_arr = arr[0].as_array().unwrap();
        assert_eq!(op_arr[0], json!("remove"));
    }

    #[test]
    fn decode_json_pointer_path() {
        // The compact decoder should accept JSON pointer strings too
        let compact = json!([[0, "/foo/bar", 42]]);
        let decoded = decode(&compact).expect("decode failed");
        match &decoded[0] {
            Op::Add { path, value } => {
                assert_eq!(path, &["foo".to_string(), "bar".to_string()]);
                assert_eq!(value, &json!(42));
            }
            _ => panic!("wrong op"),
        }
    }

    #[test]
    fn encode_empty_path_as_empty_array() {
        let op = Op::Add {
            path: vec![],
            value: json!("x"),
        };
        let compact = encode(&[op], &opts());
        let op_arr = compact.as_array().unwrap()[0].as_array().unwrap();
        assert_eq!(op_arr[1], json!([]));
    }

    #[test]
    fn roundtrip_extend() {
        let mut props = serde_json::Map::new();
        props.insert("k".to_string(), json!("v"));
        let op = Op::Extend {
            path: vec![],
            props,
            delete_null: true,
        };
        json_roundtrip(op);
    }
}
