//! M4 internal diff entrypoint.
//!
//! Compatibility note:
//! - This path delegates to pinned upstream json-joy oracle logic for exact
//!   binary parity while Rust-native diff implementation is hardened.

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
