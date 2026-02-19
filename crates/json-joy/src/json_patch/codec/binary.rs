//! MsgPack binary codec for JSON Patch operations.
//!
//! Mirrors `packages/json-joy/src/json-patch/codec/binary/`.
//!
//! The binary format encodes a JSON Patch op list as a MsgPack array of arrays.
//! Each op is encoded as:
//!   `[opcode_u8, path_array, ...op_specific_fields]`
//!
//! The encoded representation is structurally equivalent to the compact JSON
//! format (see `compact.rs`) — the binary codec is essentially the compact
//! representation serialised as MsgPack.
//!
//! Key differences from the JSON compact codec:
//! - Opcodes are always raw u8 bytes (no string form).
//! - Paths are MsgPack arrays (not JSON Pointer strings).
//! - Numeric values are encoded with MsgPack integer types.
//!
//! Implementation strategy: encode/decode directly against the `MsgPackEncoderFast`
//! and `MsgPackDecoderFast` from `json-joy-json-pack`, converting `serde_json::Value`
//! to/from `PackValue` as needed.

use json_joy_json_pack::msgpack::{MsgPackDecoderFast, MsgPackEncoderFast};
use json_joy_json_pack::PackValue;
use serde_json::{Map, Value};

use super::compact::{
    OPCODE_ADD, OPCODE_AND, OPCODE_CONTAINS, OPCODE_COPY, OPCODE_DEFINED, OPCODE_ENDS,
    OPCODE_EXTEND, OPCODE_FLIP, OPCODE_IN, OPCODE_INC, OPCODE_LESS, OPCODE_MATCHES, OPCODE_MERGE,
    OPCODE_MORE, OPCODE_MOVE, OPCODE_NOT, OPCODE_OR, OPCODE_REMOVE, OPCODE_REPLACE, OPCODE_SPLIT,
    OPCODE_STARTS, OPCODE_STR_DEL, OPCODE_STR_INS, OPCODE_TEST, OPCODE_TEST_STRING,
    OPCODE_TEST_STRING_LEN, OPCODE_TEST_TYPE, OPCODE_TYPE, OPCODE_UNDEFINED,
};
use crate::json_patch::types::{JsonPatchType, Op, PatchError, Path};

// ── Encode ─────────────────────────────────────────────────────────────────

/// Encodes a list of JSON Patch ops as MsgPack bytes.
///
/// The outer structure is a MsgPack array of N op-arrays.
pub fn encode(ops: &[Op]) -> Vec<u8> {
    let mut enc = MsgPackEncoderFast::new();
    let pack_val = ops_to_pack_value(ops);
    enc.encode(&pack_val)
}

fn ops_to_pack_value(ops: &[Op]) -> PackValue {
    PackValue::Array(ops.iter().map(|op| op_to_pack_value(op, None)).collect())
}

fn path_to_pack_value(path: &[String]) -> PackValue {
    PackValue::Array(
        path.iter()
            .map(|s| {
                // encode integer-looking segments as integers for compactness
                if let Ok(n) = s.parse::<u64>() {
                    PackValue::UInteger(n)
                } else {
                    PackValue::Str(s.clone())
                }
            })
            .collect(),
    )
}

/// Returns a relative path PackValue (relative to parent_path).
fn relative_path_pack(path: &[String], parent_path: Option<&[String]>) -> PackValue {
    if let Some(pp) = parent_path {
        path_to_pack_value(&path[pp.len()..])
    } else {
        path_to_pack_value(path)
    }
}

fn json_val_to_pack(v: &Value) -> PackValue {
    match v {
        Value::Null => PackValue::Null,
        Value::Bool(b) => PackValue::Bool(*b),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                if i >= 0 {
                    PackValue::UInteger(i as u64)
                } else {
                    PackValue::Integer(i)
                }
            } else if let Some(f) = n.as_f64() {
                PackValue::Float(f)
            } else {
                PackValue::Null
            }
        }
        Value::String(s) => PackValue::Str(s.clone()),
        Value::Array(arr) => PackValue::Array(arr.iter().map(json_val_to_pack).collect()),
        Value::Object(obj) => PackValue::Object(
            obj.iter()
                .map(|(k, v)| (k.clone(), json_val_to_pack(v)))
                .collect(),
        ),
    }
}

fn json_map_to_pack(m: &Map<String, Value>) -> PackValue {
    PackValue::Object(
        m.iter()
            .map(|(k, v)| (k.clone(), json_val_to_pack(v)))
            .collect(),
    )
}

fn op_to_pack_value(op: &Op, parent_path: Option<&[String]>) -> PackValue {
    let rp = |path: &[String]| relative_path_pack(path, parent_path);

    match op {
        Op::Add { path, value } => PackValue::Array(vec![
            PackValue::UInteger(OPCODE_ADD as u64),
            path_to_pack_value(path),
            json_val_to_pack(value),
        ]),
        Op::Remove { path, old_value } => {
            let mut arr = vec![
                PackValue::UInteger(OPCODE_REMOVE as u64),
                path_to_pack_value(path),
            ];
            if let Some(ov) = old_value {
                arr.push(json_val_to_pack(ov));
            }
            PackValue::Array(arr)
        }
        Op::Replace {
            path,
            value,
            old_value,
        } => {
            let mut arr = vec![
                PackValue::UInteger(OPCODE_REPLACE as u64),
                path_to_pack_value(path),
                json_val_to_pack(value),
            ];
            if let Some(ov) = old_value {
                arr.push(json_val_to_pack(ov));
            }
            PackValue::Array(arr)
        }
        Op::Copy { path, from } => PackValue::Array(vec![
            PackValue::UInteger(OPCODE_COPY as u64),
            path_to_pack_value(path),
            path_to_pack_value(from),
        ]),
        Op::Move { path, from } => PackValue::Array(vec![
            PackValue::UInteger(OPCODE_MOVE as u64),
            path_to_pack_value(path),
            path_to_pack_value(from),
        ]),
        Op::Test { path, value, not } => {
            let mut arr = vec![
                PackValue::UInteger(OPCODE_TEST as u64),
                rp(path),
                json_val_to_pack(value),
            ];
            if *not {
                arr.push(PackValue::UInteger(1));
            }
            PackValue::Array(arr)
        }
        Op::StrIns { path, pos, str_val } => PackValue::Array(vec![
            PackValue::UInteger(OPCODE_STR_INS as u64),
            path_to_pack_value(path),
            PackValue::UInteger(*pos as u64),
            PackValue::Str(str_val.clone()),
        ]),
        Op::StrDel {
            path,
            pos,
            str_val,
            len,
        } => {
            if let Some(s) = str_val {
                PackValue::Array(vec![
                    PackValue::UInteger(OPCODE_STR_DEL as u64),
                    path_to_pack_value(path),
                    PackValue::UInteger(*pos as u64),
                    PackValue::Str(s.clone()),
                ])
            } else {
                PackValue::Array(vec![
                    PackValue::UInteger(OPCODE_STR_DEL as u64),
                    path_to_pack_value(path),
                    PackValue::UInteger(*pos as u64),
                    PackValue::UInteger(0),
                    PackValue::UInteger(len.unwrap_or(0) as u64),
                ])
            }
        }
        Op::Flip { path } => PackValue::Array(vec![
            PackValue::UInteger(OPCODE_FLIP as u64),
            path_to_pack_value(path),
        ]),
        Op::Inc { path, inc } => PackValue::Array(vec![
            PackValue::UInteger(OPCODE_INC as u64),
            path_to_pack_value(path),
            PackValue::Float(*inc),
        ]),
        Op::Split { path, pos, props } => {
            let mut arr = vec![
                PackValue::UInteger(OPCODE_SPLIT as u64),
                path_to_pack_value(path),
                PackValue::UInteger(*pos as u64),
            ];
            if let Some(p) = props {
                arr.push(json_val_to_pack(p));
            }
            PackValue::Array(arr)
        }
        Op::Merge { path, pos, props } => {
            let mut arr = vec![
                PackValue::UInteger(OPCODE_MERGE as u64),
                path_to_pack_value(path),
                PackValue::UInteger(*pos as u64),
            ];
            if let Some(p) = props {
                arr.push(json_val_to_pack(p));
            }
            PackValue::Array(arr)
        }
        Op::Extend {
            path,
            props,
            delete_null,
        } => {
            let mut arr = vec![
                PackValue::UInteger(OPCODE_EXTEND as u64),
                path_to_pack_value(path),
                json_map_to_pack(props),
            ];
            if *delete_null {
                arr.push(PackValue::UInteger(1));
            }
            PackValue::Array(arr)
        }
        Op::Defined { path } => {
            PackValue::Array(vec![PackValue::UInteger(OPCODE_DEFINED as u64), rp(path)])
        }
        Op::Undefined { path } => {
            PackValue::Array(vec![PackValue::UInteger(OPCODE_UNDEFINED as u64), rp(path)])
        }
        Op::Contains {
            path,
            value,
            ignore_case,
        } => {
            let mut arr = vec![
                PackValue::UInteger(OPCODE_CONTAINS as u64),
                rp(path),
                PackValue::Str(value.clone()),
            ];
            if *ignore_case {
                arr.push(PackValue::UInteger(1));
            }
            PackValue::Array(arr)
        }
        Op::Ends {
            path,
            value,
            ignore_case,
        } => {
            let mut arr = vec![
                PackValue::UInteger(OPCODE_ENDS as u64),
                rp(path),
                PackValue::Str(value.clone()),
            ];
            if *ignore_case {
                arr.push(PackValue::UInteger(1));
            }
            PackValue::Array(arr)
        }
        Op::Starts {
            path,
            value,
            ignore_case,
        } => {
            let mut arr = vec![
                PackValue::UInteger(OPCODE_STARTS as u64),
                rp(path),
                PackValue::Str(value.clone()),
            ];
            if *ignore_case {
                arr.push(PackValue::UInteger(1));
            }
            PackValue::Array(arr)
        }
        Op::In { path, value } => PackValue::Array(vec![
            PackValue::UInteger(OPCODE_IN as u64),
            rp(path),
            PackValue::Array(value.iter().map(json_val_to_pack).collect()),
        ]),
        Op::Less { path, value } => PackValue::Array(vec![
            PackValue::UInteger(OPCODE_LESS as u64),
            rp(path),
            PackValue::Float(*value),
        ]),
        Op::More { path, value } => PackValue::Array(vec![
            PackValue::UInteger(OPCODE_MORE as u64),
            rp(path),
            PackValue::Float(*value),
        ]),
        Op::Matches {
            path,
            value,
            ignore_case,
        } => {
            let mut arr = vec![
                PackValue::UInteger(OPCODE_MATCHES as u64),
                rp(path),
                PackValue::Str(value.clone()),
            ];
            if *ignore_case {
                arr.push(PackValue::UInteger(1));
            }
            PackValue::Array(arr)
        }
        Op::TestType { path, type_vals } => PackValue::Array(vec![
            PackValue::UInteger(OPCODE_TEST_TYPE as u64),
            rp(path),
            PackValue::Array(
                type_vals
                    .iter()
                    .map(|t| PackValue::Str(t.as_str().to_string()))
                    .collect(),
            ),
        ]),
        Op::TestString {
            path,
            pos,
            str_val,
            not,
        } => {
            let mut arr = vec![
                PackValue::UInteger(OPCODE_TEST_STRING as u64),
                rp(path),
                PackValue::UInteger(*pos as u64),
                PackValue::Str(str_val.clone()),
            ];
            if *not {
                arr.push(PackValue::UInteger(1));
            }
            PackValue::Array(arr)
        }
        Op::TestStringLen { path, len, not } => {
            let mut arr = vec![
                PackValue::UInteger(OPCODE_TEST_STRING_LEN as u64),
                rp(path),
                PackValue::UInteger(*len as u64),
            ];
            if *not {
                arr.push(PackValue::UInteger(1));
            }
            PackValue::Array(arr)
        }
        Op::Type { path, value } => PackValue::Array(vec![
            PackValue::UInteger(OPCODE_TYPE as u64),
            rp(path),
            PackValue::Str(value.as_str().to_string()),
        ]),
        Op::And { path, ops } => PackValue::Array(vec![
            PackValue::UInteger(OPCODE_AND as u64),
            rp(path),
            PackValue::Array(
                ops.iter()
                    .map(|op| op_to_pack_value(op, Some(path)))
                    .collect(),
            ),
        ]),
        Op::Not { path, ops } => PackValue::Array(vec![
            PackValue::UInteger(OPCODE_NOT as u64),
            rp(path),
            PackValue::Array(
                ops.iter()
                    .map(|op| op_to_pack_value(op, Some(path)))
                    .collect(),
            ),
        ]),
        Op::Or { path, ops } => PackValue::Array(vec![
            PackValue::UInteger(OPCODE_OR as u64),
            rp(path),
            PackValue::Array(
                ops.iter()
                    .map(|op| op_to_pack_value(op, Some(path)))
                    .collect(),
            ),
        ]),
    }
}

// ── Decode ─────────────────────────────────────────────────────────────────

/// Decode error for the binary codec.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum BinaryDecodeError {
    #[error("MsgPack decode error")]
    MsgPack,
    #[error("Patch decode error: {0}")]
    Patch(#[from] PatchError),
}

/// Decodes MsgPack bytes into a list of JSON Patch ops.
pub fn decode(data: &[u8]) -> Result<Vec<Op>, BinaryDecodeError> {
    let mut dec = MsgPackDecoderFast::new();
    let pack_val = dec.decode(data).map_err(|_| BinaryDecodeError::MsgPack)?;
    decode_pack_ops(&pack_val).map_err(BinaryDecodeError::Patch)
}

fn decode_pack_ops(v: &PackValue) -> Result<Vec<Op>, PatchError> {
    let arr = match v {
        PackValue::Array(a) => a,
        _ => {
            return Err(PatchError::InvalidOp(
                "binary patch must be a MsgPack array".into(),
            ))
        }
    };
    arr.iter().map(|item| decode_pack_op(item, None)).collect()
}

fn pack_to_path(v: &PackValue) -> Result<Path, PatchError> {
    match v {
        PackValue::Array(arr) => arr
            .iter()
            .map(|item| match item {
                PackValue::Str(s) => Ok(s.clone()),
                PackValue::Integer(n) => Ok(n.to_string()),
                PackValue::UInteger(n) => Ok(n.to_string()),
                _ => Err(PatchError::InvalidOp(
                    "path component must be string or integer".into(),
                )),
            })
            .collect(),
        PackValue::Str(s) => Ok(json_joy_json_pointer::parse_json_pointer(s)),
        PackValue::Null => Ok(vec![]),
        _ => Err(PatchError::InvalidOp("path must be array or string".into())),
    }
}

fn pack_relative_path(v: &PackValue, parent_path: Option<&[String]>) -> Result<Path, PatchError> {
    let child = pack_to_path(v)?;
    if let Some(pp) = parent_path {
        let mut full = pp.to_vec();
        full.extend(child);
        Ok(full)
    } else {
        Ok(child)
    }
}

fn pack_to_json_value(v: &PackValue) -> Value {
    match v {
        PackValue::Null => Value::Null,
        PackValue::Bool(b) => Value::Bool(*b),
        PackValue::Integer(i) => Value::Number((*i).into()),
        PackValue::UInteger(u) => Value::Number((*u).into()),
        PackValue::Float(f) => {
            if let Some(n) = serde_json::Number::from_f64(*f) {
                Value::Number(n)
            } else {
                Value::Null
            }
        }
        PackValue::Str(s) => Value::String(s.clone()),
        PackValue::Array(arr) => Value::Array(arr.iter().map(pack_to_json_value).collect()),
        PackValue::Object(pairs) => {
            let mut m = Map::new();
            for (k, v) in pairs {
                m.insert(k.clone(), pack_to_json_value(v));
            }
            Value::Object(m)
        }
        PackValue::Bytes(_) => Value::Null,
        PackValue::Undefined => Value::Null,
        PackValue::BigInt(i) => Value::Number((*i as i64).into()),
        PackValue::Extension(_) => Value::Null,
        PackValue::Blob(_) => Value::Null,
    }
}

fn pack_arr_get(arr: &[PackValue], idx: usize) -> Result<&PackValue, PatchError> {
    arr.get(idx).ok_or_else(|| {
        PatchError::InvalidOp(format!("binary op array too short, missing index {idx}"))
    })
}

fn pack_as_u64(v: &PackValue) -> Result<u64, PatchError> {
    match v {
        PackValue::UInteger(n) => Ok(*n),
        PackValue::Integer(n) => {
            if *n >= 0 {
                Ok(*n as u64)
            } else {
                Err(PatchError::InvalidOp(
                    "expected non-negative integer".into(),
                ))
            }
        }
        _ => Err(PatchError::InvalidOp("expected integer".into())),
    }
}

fn pack_as_f64(v: &PackValue) -> Result<f64, PatchError> {
    match v {
        PackValue::Float(f) => Ok(*f),
        PackValue::UInteger(n) => Ok(*n as f64),
        PackValue::Integer(n) => Ok(*n as f64),
        _ => Err(PatchError::InvalidOp("expected number".into())),
    }
}

fn pack_as_str(v: &PackValue) -> Result<&str, PatchError> {
    match v {
        PackValue::Str(s) => Ok(s),
        _ => Err(PatchError::InvalidOp("expected string".into())),
    }
}

fn pack_as_bool_flag(v: &PackValue) -> bool {
    match v {
        PackValue::UInteger(n) => *n != 0,
        PackValue::Integer(n) => *n != 0,
        PackValue::Bool(b) => *b,
        _ => false,
    }
}

fn decode_pack_op(v: &PackValue, parent_path: Option<&[String]>) -> Result<Op, PatchError> {
    let arr = match v {
        PackValue::Array(a) => a.as_slice(),
        _ => return Err(PatchError::InvalidOp("binary op must be array".into())),
    };
    if arr.is_empty() {
        return Err(PatchError::InvalidOp("binary op array is empty".into()));
    }
    let opcode = pack_as_u64(&arr[0])? as u8;
    let len = arr.len();

    match opcode {
        OPCODE_ADD => {
            let path = pack_to_path(pack_arr_get(arr, 1)?)?;
            let value = pack_to_json_value(pack_arr_get(arr, 2)?);
            Ok(Op::Add { path, value })
        }
        OPCODE_REMOVE => {
            let path = pack_to_path(pack_arr_get(arr, 1)?)?;
            let old_value = arr.get(2).map(pack_to_json_value);
            Ok(Op::Remove { path, old_value })
        }
        OPCODE_REPLACE => {
            let path = pack_to_path(pack_arr_get(arr, 1)?)?;
            let value = pack_to_json_value(pack_arr_get(arr, 2)?);
            let old_value = arr.get(3).map(pack_to_json_value);
            Ok(Op::Replace {
                path,
                value,
                old_value,
            })
        }
        OPCODE_COPY => {
            let path = pack_to_path(pack_arr_get(arr, 1)?)?;
            let from = pack_to_path(pack_arr_get(arr, 2)?)?;
            Ok(Op::Copy { path, from })
        }
        OPCODE_MOVE => {
            let path = pack_to_path(pack_arr_get(arr, 1)?)?;
            let from = pack_to_path(pack_arr_get(arr, 2)?)?;
            Ok(Op::Move { path, from })
        }
        OPCODE_TEST => {
            let path = pack_relative_path(pack_arr_get(arr, 1)?, parent_path)?;
            let value = pack_to_json_value(pack_arr_get(arr, 2)?);
            let not = arr.get(3).map(pack_as_bool_flag).unwrap_or(false);
            Ok(Op::Test { path, value, not })
        }
        OPCODE_STR_INS => {
            let path = pack_to_path(pack_arr_get(arr, 1)?)?;
            let pos = pack_as_u64(pack_arr_get(arr, 2)?)? as usize;
            let str_val = pack_as_str(pack_arr_get(arr, 3)?)?.to_string();
            Ok(Op::StrIns { path, pos, str_val })
        }
        OPCODE_STR_DEL => {
            let path = pack_to_path(pack_arr_get(arr, 1)?)?;
            let pos = pack_as_u64(pack_arr_get(arr, 2)?)? as usize;
            if len < 5 {
                // str form
                let str_val = pack_as_str(pack_arr_get(arr, 3)?)?.to_string();
                Ok(Op::StrDel {
                    path,
                    pos,
                    str_val: Some(str_val),
                    len: None,
                })
            } else {
                // numeric length form: arr[3] == 0, arr[4] == len
                let del_len = pack_as_u64(pack_arr_get(arr, 4)?)? as usize;
                Ok(Op::StrDel {
                    path,
                    pos,
                    str_val: None,
                    len: Some(del_len),
                })
            }
        }
        OPCODE_FLIP => {
            let path = pack_to_path(pack_arr_get(arr, 1)?)?;
            Ok(Op::Flip { path })
        }
        OPCODE_INC => {
            let path = pack_to_path(pack_arr_get(arr, 1)?)?;
            let inc = pack_as_f64(pack_arr_get(arr, 2)?)?;
            Ok(Op::Inc { path, inc })
        }
        OPCODE_SPLIT => {
            let path = pack_to_path(pack_arr_get(arr, 1)?)?;
            let pos = pack_as_u64(pack_arr_get(arr, 2)?)? as usize;
            let props = arr.get(3).map(pack_to_json_value).and_then(|v| {
                if v.is_null() {
                    None
                } else {
                    Some(v)
                }
            });
            Ok(Op::Split { path, pos, props })
        }
        OPCODE_MERGE => {
            let path = pack_to_path(pack_arr_get(arr, 1)?)?;
            let pos = pack_as_u64(pack_arr_get(arr, 2)?)? as usize;
            let props = arr.get(3).map(pack_to_json_value).and_then(|v| {
                if v.is_null() {
                    None
                } else {
                    Some(v)
                }
            });
            Ok(Op::Merge { path, pos, props })
        }
        OPCODE_EXTEND => {
            let path = pack_to_path(pack_arr_get(arr, 1)?)?;
            let props_pack = pack_arr_get(arr, 2)?;
            let props_val = pack_to_json_value(props_pack);
            let props = props_val
                .as_object()
                .ok_or_else(|| PatchError::InvalidOp("extend: props must be object".into()))?
                .clone();
            let delete_null = arr.get(3).map(pack_as_bool_flag).unwrap_or(false);
            Ok(Op::Extend {
                path,
                props,
                delete_null,
            })
        }
        OPCODE_DEFINED => {
            let path = pack_relative_path(pack_arr_get(arr, 1)?, parent_path)?;
            Ok(Op::Defined { path })
        }
        OPCODE_UNDEFINED => {
            let path = pack_relative_path(pack_arr_get(arr, 1)?, parent_path)?;
            Ok(Op::Undefined { path })
        }
        OPCODE_CONTAINS => {
            let path = pack_relative_path(pack_arr_get(arr, 1)?, parent_path)?;
            let value = pack_as_str(pack_arr_get(arr, 2)?)?.to_string();
            let ignore_case = arr.get(3).map(pack_as_bool_flag).unwrap_or(false);
            Ok(Op::Contains {
                path,
                value,
                ignore_case,
            })
        }
        OPCODE_ENDS => {
            let path = pack_relative_path(pack_arr_get(arr, 1)?, parent_path)?;
            let value = pack_as_str(pack_arr_get(arr, 2)?)?.to_string();
            let ignore_case = arr.get(3).map(pack_as_bool_flag).unwrap_or(false);
            Ok(Op::Ends {
                path,
                value,
                ignore_case,
            })
        }
        OPCODE_STARTS => {
            let path = pack_relative_path(pack_arr_get(arr, 1)?, parent_path)?;
            let value = pack_as_str(pack_arr_get(arr, 2)?)?.to_string();
            let ignore_case = arr.get(3).map(pack_as_bool_flag).unwrap_or(false);
            Ok(Op::Starts {
                path,
                value,
                ignore_case,
            })
        }
        OPCODE_IN => {
            let path = pack_relative_path(pack_arr_get(arr, 1)?, parent_path)?;
            let val_arr = match pack_arr_get(arr, 2)? {
                PackValue::Array(a) => a.iter().map(pack_to_json_value).collect(),
                _ => return Err(PatchError::InvalidOp("in: value must be array".into())),
            };
            Ok(Op::In {
                path,
                value: val_arr,
            })
        }
        OPCODE_LESS => {
            let path = pack_relative_path(pack_arr_get(arr, 1)?, parent_path)?;
            let value = pack_as_f64(pack_arr_get(arr, 2)?)?;
            Ok(Op::Less { path, value })
        }
        OPCODE_MORE => {
            let path = pack_relative_path(pack_arr_get(arr, 1)?, parent_path)?;
            let value = pack_as_f64(pack_arr_get(arr, 2)?)?;
            Ok(Op::More { path, value })
        }
        OPCODE_MATCHES => {
            let path = pack_relative_path(pack_arr_get(arr, 1)?, parent_path)?;
            let value = pack_as_str(pack_arr_get(arr, 2)?)?.to_string();
            let ignore_case = arr.get(3).map(pack_as_bool_flag).unwrap_or(false);
            Ok(Op::Matches {
                path,
                value,
                ignore_case,
            })
        }
        OPCODE_TEST_TYPE => {
            let path = pack_relative_path(pack_arr_get(arr, 1)?, parent_path)?;
            let types_pack = match pack_arr_get(arr, 2)? {
                PackValue::Array(a) => a,
                _ => {
                    return Err(PatchError::InvalidOp(
                        "test_type: type must be array".into(),
                    ))
                }
            };
            let type_vals: Result<Vec<JsonPatchType>, PatchError> = types_pack
                .iter()
                .map(|v| {
                    let s = pack_as_str(v)?;
                    JsonPatchType::from_str(s)
                })
                .collect();
            Ok(Op::TestType {
                path,
                type_vals: type_vals?,
            })
        }
        OPCODE_TEST_STRING => {
            let path = pack_relative_path(pack_arr_get(arr, 1)?, parent_path)?;
            let pos = pack_as_u64(pack_arr_get(arr, 2)?)? as usize;
            let str_val = pack_as_str(pack_arr_get(arr, 3)?)?.to_string();
            let not = arr.get(4).map(pack_as_bool_flag).unwrap_or(false);
            Ok(Op::TestString {
                path,
                pos,
                str_val,
                not,
            })
        }
        OPCODE_TEST_STRING_LEN => {
            let path = pack_relative_path(pack_arr_get(arr, 1)?, parent_path)?;
            let the_len = pack_as_u64(pack_arr_get(arr, 2)?)? as usize;
            let not = arr.get(3).map(pack_as_bool_flag).unwrap_or(false);
            Ok(Op::TestStringLen {
                path,
                len: the_len,
                not,
            })
        }
        OPCODE_TYPE => {
            let path = pack_relative_path(pack_arr_get(arr, 1)?, parent_path)?;
            let type_str = pack_as_str(pack_arr_get(arr, 2)?)?;
            let value = JsonPatchType::from_str(type_str)?;
            Ok(Op::Type { path, value })
        }
        OPCODE_AND => {
            let path = pack_relative_path(pack_arr_get(arr, 1)?, parent_path)?;
            let sub_arr = match pack_arr_get(arr, 2)? {
                PackValue::Array(a) => a,
                _ => return Err(PatchError::InvalidOp("and: ops must be array".into())),
            };
            let ops: Result<Vec<Op>, PatchError> = sub_arr
                .iter()
                .map(|item| decode_pack_op(item, Some(&path)))
                .collect();
            Ok(Op::And { path, ops: ops? })
        }
        OPCODE_NOT => {
            let path = pack_relative_path(pack_arr_get(arr, 1)?, parent_path)?;
            let sub_arr = match pack_arr_get(arr, 2)? {
                PackValue::Array(a) => a,
                _ => return Err(PatchError::InvalidOp("not: ops must be array".into())),
            };
            let ops: Result<Vec<Op>, PatchError> = sub_arr
                .iter()
                .map(|item| decode_pack_op(item, Some(&path)))
                .collect();
            Ok(Op::Not { path, ops: ops? })
        }
        OPCODE_OR => {
            let path = pack_relative_path(pack_arr_get(arr, 1)?, parent_path)?;
            let sub_arr = match pack_arr_get(arr, 2)? {
                PackValue::Array(a) => a,
                _ => return Err(PatchError::InvalidOp("or: ops must be array".into())),
            };
            let ops: Result<Vec<Op>, PatchError> = sub_arr
                .iter()
                .map(|item| decode_pack_op(item, Some(&path)))
                .collect();
            Ok(Op::Or { path, ops: ops? })
        }
        _ => Err(PatchError::InvalidOp(format!(
            "OP_UNKNOWN: opcode {opcode}"
        ))),
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_patch::codec::json::to_json;
    use serde_json::json;

    fn roundtrip(op: Op) {
        let json_before = to_json(&op);
        let ops = vec![op];
        let bytes = encode(&ops);
        let decoded = decode(&bytes).expect("decode failed");
        assert_eq!(decoded.len(), 1);
        let json_after = to_json(&decoded[0]);
        assert_eq!(
            json_after, json_before,
            "JSON representation changed after binary roundtrip"
        );
    }

    #[test]
    fn roundtrip_add() {
        roundtrip(Op::Add {
            path: vec!["a".to_string()],
            value: json!(42),
        });
    }

    #[test]
    fn roundtrip_remove() {
        roundtrip(Op::Remove {
            path: vec!["a".to_string()],
            old_value: None,
        });
    }

    #[test]
    fn roundtrip_remove_with_old_value() {
        roundtrip(Op::Remove {
            path: vec!["x".to_string()],
            old_value: Some(json!("prev")),
        });
    }

    #[test]
    fn roundtrip_replace() {
        roundtrip(Op::Replace {
            path: vec!["x".to_string()],
            value: json!("new"),
            old_value: Some(json!("old")),
        });
    }

    #[test]
    fn roundtrip_copy() {
        roundtrip(Op::Copy {
            path: vec!["b".to_string()],
            from: vec!["a".to_string()],
        });
    }

    #[test]
    fn roundtrip_move() {
        roundtrip(Op::Move {
            path: vec!["b".to_string()],
            from: vec!["a".to_string()],
        });
    }

    #[test]
    fn roundtrip_test() {
        roundtrip(Op::Test {
            path: vec!["a".to_string()],
            value: json!(1),
            not: false,
        });
    }

    #[test]
    fn roundtrip_test_not() {
        roundtrip(Op::Test {
            path: vec!["a".to_string()],
            value: json!(null),
            not: true,
        });
    }

    #[test]
    fn roundtrip_str_ins() {
        roundtrip(Op::StrIns {
            path: vec!["s".to_string()],
            pos: 3,
            str_val: "hello".to_string(),
        });
    }

    #[test]
    fn roundtrip_str_del_str_form() {
        roundtrip(Op::StrDel {
            path: vec!["s".to_string()],
            pos: 1,
            str_val: Some("ell".to_string()),
            len: None,
        });
    }

    #[test]
    fn roundtrip_str_del_len_form() {
        let op = Op::StrDel {
            path: vec!["s".to_string()],
            pos: 1,
            str_val: None,
            len: Some(3),
        };
        let bytes = encode(&[op]);
        let decoded = decode(&bytes).expect("decode failed");
        match &decoded[0] {
            Op::StrDel {
                pos, str_val, len, ..
            } => {
                assert_eq!(*pos, 1);
                assert!(str_val.is_none());
                assert_eq!(*len, Some(3));
            }
            _ => panic!("wrong op type"),
        }
    }

    #[test]
    fn roundtrip_flip() {
        roundtrip(Op::Flip {
            path: vec!["b".to_string()],
        });
    }

    #[test]
    fn roundtrip_inc() {
        roundtrip(Op::Inc {
            path: vec!["n".to_string()],
            inc: 5.0,
        });
    }

    #[test]
    fn roundtrip_and() {
        roundtrip(Op::And {
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
        });
    }

    #[test]
    fn roundtrip_or() {
        roundtrip(Op::Or {
            path: vec!["x".to_string()],
            ops: vec![Op::Defined {
                path: vec!["x".to_string()],
            }],
        });
    }

    #[test]
    fn roundtrip_not() {
        roundtrip(Op::Not {
            path: vec![],
            ops: vec![Op::Undefined {
                path: vec!["x".to_string()],
            }],
        });
    }

    #[test]
    fn roundtrip_extend() {
        let mut props = serde_json::Map::new();
        props.insert("k".to_string(), json!("v"));
        roundtrip(Op::Extend {
            path: vec![],
            props,
            delete_null: true,
        });
    }

    #[test]
    fn decode_invalid_msgpack() {
        let result = decode(&[0xff, 0xfe, 0xfd]);
        assert!(result.is_err());
    }

    #[test]
    fn encode_produces_bytes() {
        let op = Op::Add {
            path: vec!["x".to_string()],
            value: json!(1),
        };
        let bytes = encode(&[op]);
        assert!(!bytes.is_empty());
        // should be a fixarray starting with 0x91 (array of 1)
        assert_eq!(bytes[0], 0x91);
    }

    #[test]
    fn multiple_ops_roundtrip() {
        let ops = vec![
            Op::Add {
                path: vec!["a".to_string()],
                value: json!(1),
            },
            Op::Remove {
                path: vec!["b".to_string()],
                old_value: None,
            },
            Op::Test {
                path: vec!["c".to_string()],
                value: json!(true),
                not: false,
            },
        ];
        let json_before: Vec<Value> = ops.iter().map(|o| to_json(o)).collect();
        let bytes = encode(&ops);
        let decoded = decode(&bytes).expect("decode failed");
        let json_after: Vec<Value> = decoded.iter().map(|o| to_json(o)).collect();
        assert_eq!(json_after, json_before);
    }
}
