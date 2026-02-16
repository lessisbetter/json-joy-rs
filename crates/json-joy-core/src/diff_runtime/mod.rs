//! M4 internal diff entrypoint.
//!
//! Compatibility note:
//! - Runtime production path is native-only (no oracle subprocess fallback).
//! - Covered parity envelope today is logical-clock object-root diffs used by
//!   fixture corpus and less-db compatibility workflows.

use crate::model::Model;
use crate::model_runtime::RuntimeModel;
use crate::patch::{ConValue, DecodedOp, Timestamp};
use crate::patch_builder::{encode_patch_from_ops, PatchBuildError};
use crate::crdt_binary::first_model_clock_sid_time;
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DiffError {
    #[error("unsupported model/view shape for native diff")]
    UnsupportedShape,
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

    pub fn diff_model_dst_keys_to_patch_bytes(
        base_model_binary: &[u8],
        dst_keys_view: &Value,
        sid: u64,
    ) -> Result<Option<Vec<u8>>, DiffError> {
        diff_model_dst_keys_to_patch_bytes(base_model_binary, dst_keys_view, sid)
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

    if let Some(native) = try_native_non_object_root_diff(base_model_binary, next_view, sid)? {
        return Ok(native);
    }

    // Broad upstream-style object recursion path.
    //
    // Mirrors JsonCrdtDiff.diffObj two-pass ordering (deletes first, then
    // destination traversal with recursive child diff attempts) and covers
    // mixed nested families in one native dispatcher.
    if let Some(native) = try_native_root_obj_recursive_diff(base_model_binary, next_view, sid)? {
        return Ok(native);
    }

    // Native logical empty-object root path.
    if let Some(native) = try_native_empty_obj_diff(base_model_binary, next_view, sid)? {
        return Ok(native);
    }
    if let Some(native) = try_native_root_obj_multi_string_delta_diff(base_model_binary, next_view, sid)? {
        return Ok(native);
    }
    if let Some(native) =
        try_native_root_obj_string_with_keyset_delta_diff(base_model_binary, sid, next_view)?
    {
        return Ok(native);
    }
    if let Some(native) = try_native_multi_root_nested_string_delta_diff(base_model_binary, next_view, sid)? {
        return Ok(native);
    }
    if let Some(native) = try_native_root_obj_string_delta_diff(base_model_binary, next_view, sid)? {
        return Ok(native);
    }
    if let Some(native) = try_native_nested_obj_string_delta_diff(base_model_binary, next_view, sid)? {
        return Ok(native);
    }
    if let Some(native) = try_native_root_obj_multi_bin_delta_diff(base_model_binary, next_view, sid)? {
        return Ok(native);
    }
    if let Some(native) = try_native_multi_root_nested_bin_delta_diff(base_model_binary, next_view, sid)? {
        return Ok(native);
    }
    if let Some(native) = try_native_root_obj_bin_delta_diff(base_model_binary, next_view, sid)? {
        return Ok(native);
    }
    if let Some(native) = try_native_nested_obj_bin_delta_diff(base_model_binary, next_view, sid)? {
        return Ok(native);
    }
    if let Some(native) = try_native_root_obj_array_delta_diff(base_model_binary, next_view, sid)? {
        return Ok(native);
    }
    if let Some(native) = try_native_root_obj_multi_array_delta_diff(base_model_binary, next_view, sid)? {
        return Ok(native);
    }
    if let Some(native) = try_native_multi_root_nested_array_delta_diff(base_model_binary, next_view, sid)? {
        return Ok(native);
    }
    if let Some(native) = try_native_nested_obj_array_delta_diff(base_model_binary, next_view, sid)? {
        return Ok(native);
    }
    if let Some(native) = try_native_root_obj_multi_vec_delta_diff(base_model_binary, next_view, sid)? {
        return Ok(native);
    }
    if let Some(native) = try_native_multi_root_nested_vec_delta_diff(base_model_binary, next_view, sid)? {
        return Ok(native);
    }
    if let Some(native) = try_native_root_obj_vec_delta_diff(base_model_binary, next_view, sid)? {
        return Ok(native);
    }
    if let Some(native) = try_native_nested_obj_vec_delta_diff(base_model_binary, next_view, sid)? {
        return Ok(native);
    }
    if let Some(native) = try_native_root_obj_mixed_recursive_diff(base_model_binary, next_view, sid)? {
        return Ok(native);
    }
    if let Some(native) = try_native_nested_obj_scalar_key_delta_diff(base_model_binary, next_view, sid)? {
        return Ok(native);
    }
    if let Some(native) = try_native_multi_root_nested_obj_generic_delta_diff(base_model_binary, next_view, sid)? {
        return Ok(native);
    }
    if let Some(native) = try_native_nested_obj_generic_delta_diff(base_model_binary, next_view, sid)? {
        return Ok(native);
    }
    // Native non-empty root-object scalar delta path (add/update/remove).
    if let Some(native) = try_native_root_obj_scalar_delta_diff(base_model_binary, next_view, sid)? {
        return Ok(native);
    }
    // Native generic root-object delta path.
    if let Some(native) = try_native_root_obj_generic_delta_diff(base_model_binary, next_view, sid)? {
        return Ok(native);
    }

    // Native catch-all: replace root value when no specialized shape matched.
    //
    // Compatibility policy:
    // runtime-core JSON shape paths should never surface UnsupportedShape.
    // This mirrors upstream `diffAny` fallback semantics where type mismatch
    // yields replacement behavior rather than terminating the diff pipeline.
    if let Some(native) = try_native_root_replace_diff(base_model_binary, next_view, sid)? {
        return Ok(native);
    }
    let base_time = first_model_clock_sid_time(base_model_binary)
        .map(|(_, t)| t)
        .unwrap_or(0);
    let mut emitter = NativeEmitter::new(sid, base_time.saturating_add(1));
    let next_root = emitter.emit_value(next_view);
    emitter.push(DecodedOp::InsVal {
        id: emitter.next_id(),
        obj: Timestamp { sid: 0, time: 0 },
        val: next_root,
    });
    let encoded = encode_patch_from_ops(sid, base_time.saturating_add(1), &emitter.ops)?;
    Ok(Some(encoded))
}

include!("dst_keys.rs");
include!("common.rs");
include!("scalar.rs");
include!("object.rs");
include!("string.rs");
include!("array.rs");
include!("bin.rs");
include!("vec.rs");
