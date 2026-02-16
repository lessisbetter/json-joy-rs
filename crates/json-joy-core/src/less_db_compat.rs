//! less-db-js compatibility layer.

use crate::diff_runtime;
use crate::crdt_binary::first_logical_clock_sid_time;
use crate::{generate_session_id, is_valid_session_id};
use crate::model::Model;
use crate::model_runtime::RuntimeModel;
use crate::patch::{ConValue, DecodedOp, Patch, Timestamp};
use crate::patch_builder::encode_patch_from_ops;
use serde_json::Value;
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
    // Reserved for compatibility accounting. Runtime-core JSON flows are
    // expected to avoid this via native diff/apply fallback behavior.
    #[error("unsupported shape for native compatibility path")]
    UnsupportedShape,
    #[error("compat runtime failure: {0}")]
    ProcessFailure(String),
}

pub fn create_model(data: &Value, sid: u64) -> Result<CompatModel, CompatError> {
    if !is_valid_session_id(sid) {
        return Err(CompatError::InvalidSessionId(sid));
    }
    let mut runtime = RuntimeModel::new_logical_empty(sid);
    let ops = build_create_ops(data, sid, 1);
    let patch_bytes = encode_patch_from_ops(sid, 1, &ops)
        .map_err(|e| CompatError::ProcessFailure(format!("create patch encode failed: {e}")))?;
    let patch = Patch::from_binary(&patch_bytes)
        .map_err(|e| CompatError::ProcessFailure(format!("create patch decode failed: {e}")))?;
    runtime
        .apply_patch(&patch)
        .map_err(|e| CompatError::ProcessFailure(format!("create apply failed: {e}")))?;
    let model_binary = runtime
        .to_model_binary_like()
        .map_err(|e| CompatError::ProcessFailure(format!("create model encode failed: {e}")))?;

    Ok(CompatModel {
        model_binary,
        view: runtime.view_json(),
        sid,
    })
}

pub fn diff_model(model: &CompatModel, next: &Value) -> Result<Option<PatchBytes>, CompatError> {
    diff_runtime::diff_model_to_patch_bytes(&model.model_binary, next, model.sid)
        .map_err(|e| CompatError::ProcessFailure(e.to_string()))
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
        // patch contains no `nop`, keep binary/view unchanged. `nop` is
        // excluded because upstream may advance clock state even when view
        // stays the same.
        if !decoded
            .decoded_ops()
            .iter()
            .any(|op| matches!(op, crate::patch::DecodedOp::Nop { .. }))
        {
            // Runtime apply coverage is currently strongest for empty-object
            // logical base models. Restrict no-op fast path to that envelope
            // to avoid false no-op decisions for richer model states.
            // This is a deliberate safety bound, not a semantic requirement of
            // upstream `Model.applyPatch`.
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
    let fork_sid = match sid {
        Some(s) => {
            if !is_valid_session_id(s) {
                return Err(CompatError::InvalidSessionId(s));
            }
            s
        }
        None => {
            let mut sid = generate_session_id();
            while sid == model.sid {
                sid = generate_session_id();
            }
            sid
        }
    };
    let runtime = RuntimeModel::from_model_binary(&model.model_binary)
        .map_err(|e| CompatError::ProcessFailure(format!("model decode failed: {e}")))?;
    let forked = runtime.fork_with_sid(fork_sid);
    let model_binary = forked
        .to_model_binary_like()
        .map_err(|e| CompatError::ProcessFailure(format!("model encode failed: {e}")))?;
    Ok(CompatModel {
        model_binary,
        view: forked.view_json(),
        sid: fork_sid,
    })
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
    // Mirror upstream Model.load(..., sid) clock/session semantics by forking
    // runtime-local session id for logical-clock models.
    if data.first().is_some_and(|b| (b & 0x80) == 0) {
        let runtime = RuntimeModel::from_model_binary(data)
            .map_err(|e| CompatError::ProcessFailure(format!("model decode failed: {e}")))?;
        let loaded = runtime.fork_with_sid(sid);
        let model_binary = loaded
            .to_model_binary_like()
            .map_err(|e| CompatError::ProcessFailure(format!("model encode failed: {e}")))?;
        return Ok(CompatModel {
            model_binary,
            view: loaded.view_json(),
            sid,
        });
    }
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

fn primary_sid_from_model_binary(data: &[u8]) -> Option<u64> {
    if data.is_empty() {
        return None;
    }
    if (data[0] & 0x80) != 0 {
        return Some(1);
    }
    first_logical_clock_sid_time(data).map(|(sid, _)| sid)
}

fn build_create_ops(data: &Value, sid: u64, start_time: u64) -> Vec<DecodedOp> {
    let mut emitter = CreateEmitter {
        sid,
        cursor: start_time,
        ops: Vec::new(),
    };
    let root_val = emitter.const_or_json(data);
    emitter.push(DecodedOp::InsVal {
        id: emitter.next_id(),
        obj: Timestamp { sid: 0, time: 0 },
        val: root_val,
    });
    emitter.ops
}

struct CreateEmitter {
    sid: u64,
    cursor: u64,
    ops: Vec<DecodedOp>,
}

impl CreateEmitter {
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

    fn con(&mut self, value: ConValue) -> Timestamp {
        let id = self.next_id();
        self.push(DecodedOp::NewCon { id, value });
        id
    }

    fn val(&mut self, json: &Value) -> Timestamp {
        let val_id = self.next_id();
        self.push(DecodedOp::NewVal { id: val_id });
        let con_id = self.con(ConValue::Json(json.clone()));
        self.push(DecodedOp::InsVal {
            id: self.next_id(),
            obj: val_id,
            val: con_id,
        });
        val_id
    }

    fn json(&mut self, json: &Value) -> Timestamp {
        match json {
            Value::Null | Value::Bool(_) | Value::Number(_) => self.val(json),
            Value::String(s) => {
                let id = self.next_id();
                self.push(DecodedOp::NewStr { id });
                if !s.is_empty() {
                    self.push(DecodedOp::InsStr {
                        id: self.next_id(),
                        obj: id,
                        reference: id,
                        data: s.clone(),
                    });
                }
                id
            }
            Value::Array(items) => {
                let id = self.next_id();
                self.push(DecodedOp::NewArr { id });
                if !items.is_empty() {
                    let mut values = Vec::with_capacity(items.len());
                    for item in items {
                        values.push(self.json(item));
                    }
                    self.push(DecodedOp::InsArr {
                        id: self.next_id(),
                        obj: id,
                        reference: id,
                        data: values,
                    });
                }
                id
            }
            Value::Object(map) => {
                let id = self.next_id();
                self.push(DecodedOp::NewObj { id });
                if !map.is_empty() {
                    let mut data = Vec::with_capacity(map.len());
                    for (k, v) in map {
                        let child = if is_const(v) {
                            self.con(ConValue::Json(v.clone()))
                        } else {
                            self.json(v)
                        };
                        data.push((k.clone(), child));
                    }
                    self.push(DecodedOp::InsObj {
                        id: self.next_id(),
                        obj: id,
                        data,
                    });
                }
                id
            }
        }
    }

    fn const_or_json(&mut self, value: &Value) -> Timestamp {
        if is_const(value) {
            self.con(ConValue::Json(value.clone()))
        } else {
            self.json(value)
        }
    }
}

fn is_const(value: &Value) -> bool {
    matches!(value, Value::Null | Value::Bool(_) | Value::Number(_))
}
