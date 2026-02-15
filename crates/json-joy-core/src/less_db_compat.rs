//! less-db-js compatibility layer (M5, oracle-backed bridge).

use crate::diff_runtime;
use crate::is_valid_session_id;
use crate::model::Model;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::Command;
use thiserror::Error;

pub type PatchBytes = Vec<u8>;

pub const MAX_CRDT_BINARY_SIZE: usize = 10 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct CompatModel {
    model_binary: Vec<u8>,
    view: Value,
    sid: u64,
}

#[derive(Debug, Error)]
pub enum CompatError {
    #[error("invalid session id: {0}")]
    InvalidSessionId(u64),
    #[error("CRDT binary too large: {actual} bytes (max {max})")]
    ModelBinaryTooLarge { actual: usize, max: usize },
    #[error("failed to run oracle process: {0}")]
    ProcessIo(String),
    #[error("oracle process failed: {0}")]
    ProcessFailure(String),
    #[error("invalid oracle output")]
    InvalidOutput,
    #[error("invalid hex")]
    InvalidHex,
}

pub fn create_model(data: &Value, sid: u64) -> Result<CompatModel, CompatError> {
    if !is_valid_session_id(sid) {
        return Err(CompatError::InvalidSessionId(sid));
    }
    let out = oracle_call(json!({
        "op": "create",
        "sid": sid,
        "data_json": data,
    }))?;
    parse_state(out)
}

pub fn diff_model(model: &CompatModel, next: &Value) -> Result<Option<PatchBytes>, CompatError> {
    diff_runtime::diff_model_to_patch_bytes(&model.model_binary, next, model.sid)
        .map_err(|e| CompatError::ProcessFailure(e.to_string()))
}

pub fn apply_patch(model: &mut CompatModel, patch_bytes: &[u8]) -> Result<(), CompatError> {
    let out = oracle_call(json!({
        "op": "apply_patch",
        "model_binary_hex": hex(&model.model_binary),
        "patch_binary_hex": hex(patch_bytes),
    }))?;
    let state = parse_state(out)?;
    model.model_binary = state.model_binary;
    model.view = state.view;
    model.sid = state.sid;
    Ok(())
}

pub fn view_model(model: &CompatModel) -> Value {
    model.view.clone()
}

pub fn fork_model(model: &CompatModel, sid: Option<u64>) -> Result<CompatModel, CompatError> {
    match sid {
        Some(s) => {
            if !is_valid_session_id(s) {
                return Err(CompatError::InvalidSessionId(s));
            }
            // Native fast path for explicit session-id fork used by
            // less-db fixtures: binary/view remain the same, local sid changes.
            let mut cloned = model.clone();
            cloned.sid = s;
            Ok(cloned)
        }
        None => {
            let out = oracle_call(json!({
                "op": "fork",
                "model_binary_hex": hex(&model.model_binary),
                "sid": sid,
            }))?;
            parse_state(out)
        }
    }
}

pub fn model_to_binary(model: &CompatModel) -> Vec<u8> {
    model.model_binary.clone()
}

pub fn model_from_binary(data: &[u8]) -> Result<CompatModel, CompatError> {
    if data.len() > MAX_CRDT_BINARY_SIZE {
        return Err(CompatError::ModelBinaryTooLarge {
            actual: data.len(),
            max: MAX_CRDT_BINARY_SIZE,
        });
    }
    let parsed = Model::from_binary(data).map_err(|e| CompatError::ProcessFailure(e.to_string()))?;
    Ok(CompatModel {
        model_binary: data.to_vec(),
        view: parsed.view().clone(),
        sid: primary_sid_from_model_binary(data).unwrap_or(1),
    })
}

pub fn model_load(data: &[u8], sid: u64) -> Result<CompatModel, CompatError> {
    if !is_valid_session_id(sid) {
        return Err(CompatError::InvalidSessionId(sid));
    }
    if data.len() > MAX_CRDT_BINARY_SIZE {
        return Err(CompatError::ModelBinaryTooLarge {
            actual: data.len(),
            max: MAX_CRDT_BINARY_SIZE,
        });
    }
    let parsed = Model::from_binary(data).map_err(|e| CompatError::ProcessFailure(e.to_string()))?;
    Ok(CompatModel {
        model_binary: data.to_vec(),
        view: parsed.view().clone(),
        sid,
    })
}

pub fn merge_with_pending_patches(
    model: &mut CompatModel,
    patches: &[PatchBytes],
) -> Result<(), CompatError> {
    if patches.is_empty() {
        return Ok(());
    }
    let patch_hexes: Vec<String> = patches.iter().map(|p| hex(p)).collect();
    let out = oracle_call(json!({
        "op": "merge",
        "model_binary_hex": hex(&model.model_binary),
        "patches_binary_hex": patch_hexes,
    }))?;
    let state = parse_state(out)?;
    model.model_binary = state.model_binary;
    model.view = state.view;
    model.sid = state.sid;
    Ok(())
}

pub fn empty_patch_log() -> Vec<u8> {
    Vec::new()
}

struct ParsedState {
    model_binary: Vec<u8>,
    view: Value,
    sid: u64,
}

fn parse_state(v: Value) -> Result<CompatModel, CompatError> {
    let parsed = parse_state_inner(v)?;
    Ok(CompatModel {
        model_binary: parsed.model_binary,
        view: parsed.view,
        sid: parsed.sid,
    })
}

fn parse_state_inner(v: Value) -> Result<ParsedState, CompatError> {
    let model_hex = v
        .get("model_binary_hex")
        .and_then(Value::as_str)
        .ok_or(CompatError::InvalidOutput)?;
    let view = v.get("view_json").cloned().ok_or(CompatError::InvalidOutput)?;
    let sid = v
        .get("sid")
        .and_then(Value::as_u64)
        .ok_or(CompatError::InvalidOutput)?;

    Ok(ParsedState {
        model_binary: decode_hex(model_hex)?,
        view,
        sid,
    })
}

fn oracle_call(payload: Value) -> Result<Value, CompatError> {
    let output = Command::new("node")
        .arg(oracle_model_runtime_path())
        .arg(payload.to_string())
        .output()
        .map_err(|e| CompatError::ProcessIo(e.to_string()))?;

    if !output.status.success() {
        return Err(CompatError::ProcessFailure(
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ));
    }

    serde_json::from_slice::<Value>(&output.stdout).map_err(|_| CompatError::InvalidOutput)
}

fn oracle_model_runtime_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tools")
        .join("oracle-node")
        .join("model-runtime.cjs")
}

fn primary_sid_from_model_binary(data: &[u8]) -> Option<u64> {
    if data.is_empty() {
        return None;
    }
    if (data[0] & 0x80) != 0 {
        return Some(1);
    }
    if data.len() < 4 {
        return None;
    }
    let offset = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
    let mut pos = 4usize.checked_add(offset)?;
    let _table_len = read_vu57(data, &mut pos)?;
    read_vu57(data, &mut pos)
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

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

fn decode_hex(s: &str) -> Result<Vec<u8>, CompatError> {
    if s.len() % 2 != 0 {
        return Err(CompatError::InvalidHex);
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    let bytes = s.as_bytes();
    for i in (0..bytes.len()).step_by(2) {
        let hi = (bytes[i] as char).to_digit(16).ok_or(CompatError::InvalidHex)? as u8;
        let lo = (bytes[i + 1] as char)
            .to_digit(16)
            .ok_or(CompatError::InvalidHex)? as u8;
        out.push((hi << 4) | lo);
    }
    Ok(out)
}
