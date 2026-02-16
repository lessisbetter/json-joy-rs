//! M4 internal diff entrypoint.
//!
//! Compatibility note:
//! - This path now includes a native fast path for logical empty-root object
//!   models. For all other cases it delegates to pinned upstream json-joy
//!   oracle logic for exact binary parity while Rust-native diff implementation
//!   is hardened.

use crate::model::Model;
use crate::patch::{ConValue, DecodedOp, Timestamp};
use crate::patch_builder::{encode_patch_from_ops, PatchBuildError};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::Command;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DiffError {
    #[error("failed to run oracle diff process: {0}")]
    ProcessIo(String),
    #[error("oracle diff process failed: {0}")]
    ProcessFailure(String),
    #[error("invalid oracle diff output")]
    InvalidOutput,
    #[error("invalid patch hex")]
    InvalidPatchHex,
    #[error("native patch encode failed: {0}")]
    NativeEncode(#[from] PatchBuildError),
}

pub struct RuntimeDiffer;

impl RuntimeDiffer {
    pub fn diff_model_to_patch_bytes(
        base_model_binary: &[u8],
        next_view: &Value,
        sid: u64,
    ) -> Result<Option<Vec<u8>>, DiffError> {
        diff_model_to_patch_bytes(base_model_binary, next_view, sid)
    }
}

pub fn diff_model_to_patch_bytes(
    base_model_binary: &[u8],
    next_view: &Value,
    sid: u64,
) -> Result<Option<Vec<u8>>, DiffError> {
    // Native no-op fast path. Upstream diff returns no patch for exact-equal
    // views, so this is parity-safe and avoids subprocess overhead.
    if let Ok(model) = Model::from_binary(base_model_binary) {
        if model.view() == next_view {
            return Ok(None);
        }
    }

    // Native logical empty-object root path. This covers the current
    // model_diff_parity fixture corpus and many less-db create/diff/apply
    // scenarios while preserving oracle fallback for broader semantics.
    if let Some(native) = try_native_empty_obj_diff(base_model_binary, next_view, sid)? {
        return Ok(native);
    }
    // Native non-empty root-object scalar delta path (add/update/remove).
    if let Some(native) = try_native_root_obj_scalar_delta_diff(base_model_binary, next_view, sid)? {
        return Ok(native);
    }

    let script = oracle_diff_script_path();
    let payload = json!({
        "base_model_binary_hex": hex(base_model_binary),
        "next_view_json": next_view,
        "sid": sid,
    });

    let output = Command::new("node")
        .arg(script)
        .arg(payload.to_string())
        .output()
        .map_err(|e| DiffError::ProcessIo(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(DiffError::ProcessFailure(stderr.into_owned()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: Value = serde_json::from_str(&stdout).map_err(|_| DiffError::InvalidOutput)?;
    let patch_present = parsed
        .get("patch_present")
        .and_then(Value::as_bool)
        .ok_or(DiffError::InvalidOutput)?;

    if !patch_present {
        return Ok(None);
    }

    let hex_str = parsed
        .get("patch_binary_hex")
        .and_then(Value::as_str)
        .ok_or(DiffError::InvalidOutput)?;
    Ok(Some(decode_hex(hex_str)?))
}

fn try_native_empty_obj_diff(
    base_model_binary: &[u8],
    next_view: &Value,
    patch_sid: u64,
) -> Result<Option<Option<Vec<u8>>>, DiffError> {
    let model = match Model::from_binary(base_model_binary) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    let base_obj = match model.view() {
        Value::Object(map) if map.is_empty() => map,
        _ => return Ok(None),
    };
    let _ = base_obj;

    let next_obj = match next_view {
        Value::Object(map) => map,
        _ => return Ok(None),
    };
    if next_obj.is_empty() {
        return Ok(Some(None));
    }

    let (root_sid, base_time) = match logical_clock_sid_time(base_model_binary) {
        Some(v) => v,
        None => return Ok(None),
    };

    let mut emitter = NativeEmitter::new(patch_sid, base_time.saturating_add(1));
    let root = Timestamp {
        sid: root_sid,
        time: 1,
    };
    let mut pairs = Vec::with_capacity(next_obj.len());
    for (k, v) in next_obj {
        let id = emitter.emit_value(v);
        pairs.push((k.clone(), id));
    }
    emitter.push(DecodedOp::InsObj {
        id: emitter.next_id(),
        obj: root,
        data: pairs,
    });

    let encoded = encode_patch_from_ops(patch_sid, base_time.saturating_add(1), &emitter.ops)?;
    Ok(Some(Some(encoded)))
}

fn try_native_root_obj_scalar_delta_diff(
    base_model_binary: &[u8],
    next_view: &Value,
    patch_sid: u64,
) -> Result<Option<Option<Vec<u8>>>, DiffError> {
    let model = match Model::from_binary(base_model_binary) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    let base_obj = match model.view() {
        Value::Object(map) if !map.is_empty() => map,
        _ => return Ok(None),
    };
    let next_obj = match next_view {
        Value::Object(map) => map,
        _ => return Ok(None),
    };

    let (root_sid, base_time) = match logical_clock_sid_time(base_model_binary) {
        Some(v) => v,
        None => return Ok(None),
    };

    // Constrain native path to scalar-only key replacements at root. If any
    // structural/nested mutation is detected, fall back to oracle for exact
    // upstream operation-shape parity.
    for (k, next_v) in next_obj {
        let changed = base_obj.get(k) != Some(next_v);
        if !changed {
            continue;
        }
        if !is_con_scalar(next_v) {
            return Ok(None);
        }
    }

    let mut emitter = NativeEmitter::new(patch_sid, base_time.saturating_add(1));
    let mut pairs: Vec<(String, Timestamp)> = Vec::new();

    // Key deletion requires `undefined` constant emission, which is not yet
    // represented in this native path. Fall back for delete cases for now.
    for (k, _) in base_obj {
        if !next_obj.contains_key(k) {
            return Ok(None);
        }
    }

    // Pass 2: additions/updates in next key iteration order.
    for (k, next_v) in next_obj {
        if base_obj.get(k) == Some(next_v) {
            continue;
        }
        let id = emitter.emit_value(next_v);
        pairs.push((k.clone(), id));
    }

    if pairs.is_empty() {
        return Ok(Some(None));
    }

    emitter.push(DecodedOp::InsObj {
        id: emitter.next_id(),
        obj: Timestamp {
            sid: root_sid,
            time: 1,
        },
        data: pairs,
    });

    let encoded = encode_patch_from_ops(patch_sid, base_time.saturating_add(1), &emitter.ops)?;
    Ok(Some(Some(encoded)))
}

struct NativeEmitter {
    sid: u64,
    cursor: u64,
    ops: Vec<DecodedOp>,
}

impl NativeEmitter {
    fn new(sid: u64, start_time: u64) -> Self {
        Self {
            sid,
            cursor: start_time,
            ops: Vec::new(),
        }
    }

    fn next_id(&self) -> Timestamp {
        Timestamp {
            sid: self.sid,
            time: self.cursor,
        }
    }

    fn push(&mut self, op: DecodedOp) {
        self.cursor = self.cursor.saturating_add(op.span());
        self.ops.push(op);
    }

    fn emit_value(&mut self, value: &Value) -> Timestamp {
        match value {
            Value::Null | Value::Bool(_) | Value::Number(_) => {
                let id = self.next_id();
                self.push(DecodedOp::NewCon {
                    id,
                    value: ConValue::Json(value.clone()),
                });
                id
            }
            Value::String(s) => {
                let str_id = self.next_id();
                self.push(DecodedOp::NewStr { id: str_id });
                if !s.is_empty() {
                    let ins_id = self.next_id();
                    self.push(DecodedOp::InsStr {
                        id: ins_id,
                        obj: str_id,
                        reference: str_id,
                        data: s.clone(),
                    });
                }
                str_id
            }
            Value::Array(items) => {
                let arr_id = self.next_id();
                self.push(DecodedOp::NewArr { id: arr_id });
                if !items.is_empty() {
                    let mut children = Vec::with_capacity(items.len());
                    for item in items {
                        if is_con_scalar(item) {
                            // Array scalar elements are emitted as VAL wrappers
                            // around CON nodes to mirror upstream diff op shape.
                            let val_id = self.next_id();
                            self.push(DecodedOp::NewVal { id: val_id });
                            let con_id = self.emit_value(item);
                            let ins_id = self.next_id();
                            self.push(DecodedOp::InsVal {
                                id: ins_id,
                                obj: val_id,
                                val: con_id,
                            });
                            children.push(val_id);
                        } else {
                            children.push(self.emit_value(item));
                        }
                    }
                    let ins_id = self.next_id();
                    self.push(DecodedOp::InsArr {
                        id: ins_id,
                        obj: arr_id,
                        reference: arr_id,
                        data: children,
                    });
                }
                arr_id
            }
            Value::Object(map) => {
                let obj_id = self.next_id();
                self.push(DecodedOp::NewObj { id: obj_id });
                if !map.is_empty() {
                    let mut pairs = Vec::with_capacity(map.len());
                    for (k, v) in map {
                        let id = self.emit_value(v);
                        pairs.push((k.clone(), id));
                    }
                    let ins_id = self.next_id();
                    self.push(DecodedOp::InsObj {
                        id: ins_id,
                        obj: obj_id,
                        data: pairs,
                    });
                }
                obj_id
            }
        }
    }
}

fn is_con_scalar(value: &Value) -> bool {
    matches!(value, Value::Null | Value::Bool(_) | Value::Number(_))
}

fn logical_clock_sid_time(data: &[u8]) -> Option<(u64, u64)> {
    if data.is_empty() {
        return None;
    }
    // server-clock preamble is not handled by native path yet.
    if (data[0] & 0x80) != 0 {
        return None;
    }
    if data.len() < 4 {
        return None;
    }
    let offset = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
    let mut pos = 4usize.checked_add(offset)?;
    let _table_len = read_vu57(data, &mut pos)?;
    let sid = read_vu57(data, &mut pos)?;
    let time = read_vu57(data, &mut pos)?;
    Some((sid, time))
}

fn read_vu57(data: &[u8], pos: &mut usize) -> Option<u64> {
    let mut result: u64 = 0;
    let mut shift: u32 = 0;
    for i in 0..8 {
        let b = *data.get(*pos)?;
        *pos += 1;
        if i < 7 {
            let part = (b & 0x7f) as u64;
            result |= part.checked_shl(shift)?;
            if (b & 0x80) == 0 {
                return Some(result);
            }
            shift += 7;
        } else {
            result |= (b as u64).checked_shl(49)?;
            return Some(result);
        }
    }
    None
}

fn oracle_diff_script_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tools")
        .join("oracle-node")
        .join("diff-runtime.cjs")
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

fn decode_hex(s: &str) -> Result<Vec<u8>, DiffError> {
    if s.len() % 2 != 0 {
        return Err(DiffError::InvalidPatchHex);
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    let bytes = s.as_bytes();
    for i in (0..bytes.len()).step_by(2) {
        let hi = (bytes[i] as char)
            .to_digit(16)
            .ok_or(DiffError::InvalidPatchHex)? as u8;
        let lo = (bytes[i + 1] as char)
            .to_digit(16)
            .ok_or(DiffError::InvalidPatchHex)? as u8;
        out.push((hi << 4) | lo);
    }
    Ok(out)
}
