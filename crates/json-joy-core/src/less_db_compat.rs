//! less-db-js compatibility layer.

use crate::diff_runtime;
use crate::crdt_binary::first_logical_clock_sid_time;
use crate::{generate_session_id, is_valid_session_id};
use crate::model::Model;
use crate::model_runtime::RuntimeModel;
use crate::patch::Patch;
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
    // Bootstrap parity for initial model creation is still delegated to
    // upstream oracle runtime to match exact binary initialization behavior.
    let output = Command::new("node")
        .arg(oracle_model_runtime_path())
        .arg(
            json!({
                "op": "create",
                "sid": sid,
                "data_json": data,
            })
            .to_string(),
        )
        .output()
        .map_err(|e| CompatError::ProcessIo(e.to_string()))?;
    if !output.status.success() {
        return Err(CompatError::ProcessFailure(
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ));
    }
    let parsed: Value = serde_json::from_slice(&output.stdout).map_err(|_| CompatError::InvalidOutput)?;
    let model_hex = parsed
        .get("model_binary_hex")
        .and_then(Value::as_str)
        .ok_or(CompatError::InvalidOutput)?;
    let view = parsed
        .get("view_json")
        .cloned()
        .ok_or(CompatError::InvalidOutput)?;
    let sid = parsed
        .get("sid")
        .and_then(Value::as_u64)
        .ok_or(CompatError::InvalidOutput)?;
    Ok(CompatModel {
        model_binary: decode_hex(model_hex)?,
        view,
        sid,
    })
}

pub fn diff_model(model: &CompatModel, next: &Value) -> Result<Option<PatchBytes>, CompatError> {
    match diff_runtime::diff_model_to_patch_bytes(&model.model_binary, next, model.sid) {
        Ok(v) => Ok(v),
        Err(diff_runtime::DiffError::UnsupportedShape) => {
            let output = Command::new("node")
                .arg(oracle_diff_runtime_path())
                .arg(
                    json!({
                        "base_model_binary_hex": hex(&model.model_binary),
                        "next_view_json": next,
                        "sid": model.sid
                    })
                    .to_string(),
                )
                .output()
                .map_err(|e| CompatError::ProcessIo(e.to_string()))?;
            if !output.status.success() {
                return Err(CompatError::ProcessFailure(
                    String::from_utf8_lossy(&output.stderr).into_owned(),
                ));
            }
            let parsed: Value =
                serde_json::from_slice(&output.stdout).map_err(|_| CompatError::InvalidOutput)?;
            let present = parsed
                .get("patch_present")
                .and_then(Value::as_bool)
                .ok_or(CompatError::InvalidOutput)?;
            if !present {
                return Ok(None);
            }
            let patch_hex = parsed
                .get("patch_binary_hex")
                .and_then(Value::as_str)
                .ok_or(CompatError::InvalidOutput)?;
            Ok(Some(decode_hex(patch_hex)?))
        }
        Err(e) => Err(CompatError::ProcessFailure(e.to_string())),
    }
}

pub fn apply_patch(model: &mut CompatModel, patch_bytes: &[u8]) -> Result<(), CompatError> {
    if patch_bytes.is_empty() {
        return Ok(());
    }
    if let Ok(decoded) = Patch::from_binary(patch_bytes) {
        if decoded.op_count() == 0 {
            return Ok(());
        }
        // Native no-op/stale replay fast path.
        //
        // If runtime application does not change materialized view and the
        // patch contains no `nop`, keep binary/view unchanged and avoid oracle
        // subprocess overhead. `nop` is excluded because upstream may advance
        // clock state even when view stays the same.
        if !decoded
            .decoded_ops()
            .iter()
            .any(|op| matches!(op, crate::patch::DecodedOp::Nop { .. }))
        {
            // Runtime apply coverage is currently strongest for empty-object
            // logical base models. Restrict no-op fast path to that envelope
            // to avoid false no-op decisions for richer model states.
            let base_empty_object = Model::from_binary(&model.model_binary)
                .ok()
                .is_some_and(|m| matches!(m.view(), Value::Object(map) if map.is_empty()));

            if base_empty_object {
                if let Ok(mut runtime) = RuntimeModel::from_model_binary(&model.model_binary) {
                    let before = runtime.view_json();
                    if runtime.apply_patch(&decoded).is_ok() && runtime.view_json() == before {
                        return Ok(());
                    }
                }
            }
        }
    }
    let decoded = Patch::from_binary(patch_bytes)
        .map_err(|e| CompatError::ProcessFailure(format!("patch decode failed: {e}")))?;
    let mut runtime = RuntimeModel::from_model_binary(&model.model_binary)
        .map_err(|e| CompatError::ProcessFailure(format!("model decode failed: {e}")))?;
    runtime
        .apply_patch(&decoded)
        .map_err(|e| CompatError::ProcessFailure(format!("runtime apply failed: {e}")))?;
    let next_binary = runtime
        .to_model_binary_like()
        .map_err(|e| CompatError::ProcessFailure(format!("model encode failed: {e}")))?;
    model.view = runtime.view_json();
    model.model_binary = next_binary;
    if let Some((sid, _)) = decoded.id() {
        model.sid = sid.max(model.sid);
    }
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
            let mut cloned = model.clone();
            let mut sid = generate_session_id();
            while sid == cloned.sid {
                sid = generate_session_id();
            }
            cloned.sid = sid;
            Ok(cloned)
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
    for patch in patches {
        apply_patch(model, patch)?;
    }
    Ok(())
}

pub fn empty_patch_log() -> Vec<u8> {
    Vec::new()
}

fn oracle_model_runtime_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tools")
        .join("oracle-node")
        .join("model-runtime.cjs")
}

fn oracle_diff_runtime_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tools")
        .join("oracle-node")
        .join("diff-runtime.cjs")
}

fn primary_sid_from_model_binary(data: &[u8]) -> Option<u64> {
    if data.is_empty() {
        return None;
    }
    if (data[0] & 0x80) != 0 {
        return Some(1);
    }
    first_logical_clock_sid_time(data).map(|(sid, _)| sid)
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
