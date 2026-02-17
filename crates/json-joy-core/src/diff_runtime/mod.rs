//! M4 internal diff entrypoint.
//!
//! Compatibility note:
//! - Runtime production path is native-only (no oracle subprocess fallback).
//! - Covered parity envelope today is logical-clock object-root diffs used by
//!   fixture corpus and less-db compatibility workflows.

use crate::crdt_binary::first_model_clock_sid_time;
use crate::model::Model;
use crate::model_runtime::RuntimeModel;
use crate::model_runtime::types::{ConCell, Id, RuntimeNode};
use crate::patch::{ConValue, DecodedOp, Timestamp};
use crate::patch_builder::{encode_patch_from_ops, PatchBuildError};
use serde_json::Value;
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DiffError {
    #[error("unsupported model/view shape for native diff")]
    UnsupportedShape,
    #[error("native patch encode failed: {0}")]
    NativeEncode(#[from] PatchBuildError),
}

pub struct RuntimeDiffer;

#[derive(Debug, Clone)]
pub struct RuntimeDiffResult {
    pub ops: Vec<DecodedOp>,
    pub patch_binary: Vec<u8>,
}

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
    if let Ok(runtime) = RuntimeModel::from_model_binary(base_model_binary) {
        return diff_runtime_to_patch_bytes(&runtime, next_view, sid);
    }
    diff_model_to_patch_bytes_legacy(base_model_binary, next_view, sid)
}

fn diff_model_to_patch_bytes_legacy(
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

    if let Some(native) = try_native_root_obj_recursive_diff(base_model_binary, next_view, sid)? {
        return Ok(native);
    }
    if let Some(native) = try_native_empty_obj_diff(base_model_binary, next_view, sid)? {
        return Ok(native);
    }
    if let Some(native) =
        try_native_root_obj_multi_string_delta_diff(base_model_binary, next_view, sid)?
    {
        return Ok(native);
    }
    if let Some(native) =
        try_native_root_obj_string_with_keyset_delta_diff(base_model_binary, sid, next_view)?
    {
        return Ok(native);
    }
    if let Some(native) =
        try_native_multi_root_nested_string_delta_diff(base_model_binary, next_view, sid)?
    {
        return Ok(native);
    }
    if let Some(native) = try_native_root_obj_string_delta_diff(base_model_binary, next_view, sid)?
    {
        return Ok(native);
    }
    if let Some(native) =
        try_native_nested_obj_string_delta_diff(base_model_binary, next_view, sid)?
    {
        return Ok(native);
    }
    if let Some(native) =
        try_native_root_obj_multi_bin_delta_diff(base_model_binary, next_view, sid)?
    {
        return Ok(native);
    }
    if let Some(native) =
        try_native_multi_root_nested_bin_delta_diff(base_model_binary, next_view, sid)?
    {
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
    if let Some(native) =
        try_native_root_obj_multi_array_delta_diff(base_model_binary, next_view, sid)?
    {
        return Ok(native);
    }
    if let Some(native) =
        try_native_multi_root_nested_array_delta_diff(base_model_binary, next_view, sid)?
    {
        return Ok(native);
    }
    if let Some(native) = try_native_nested_obj_array_delta_diff(base_model_binary, next_view, sid)?
    {
        return Ok(native);
    }
    if let Some(native) =
        try_native_root_obj_multi_vec_delta_diff(base_model_binary, next_view, sid)?
    {
        return Ok(native);
    }
    if let Some(native) =
        try_native_multi_root_nested_vec_delta_diff(base_model_binary, next_view, sid)?
    {
        return Ok(native);
    }
    if let Some(native) = try_native_root_obj_vec_delta_diff(base_model_binary, next_view, sid)? {
        return Ok(native);
    }
    if let Some(native) = try_native_nested_obj_vec_delta_diff(base_model_binary, next_view, sid)? {
        return Ok(native);
    }
    if let Some(native) =
        try_native_root_obj_mixed_recursive_diff(base_model_binary, next_view, sid)?
    {
        return Ok(native);
    }
    if let Some(native) =
        try_native_nested_obj_scalar_key_delta_diff(base_model_binary, next_view, sid)?
    {
        return Ok(native);
    }
    if let Some(native) =
        try_native_multi_root_nested_obj_generic_delta_diff(base_model_binary, next_view, sid)?
    {
        return Ok(native);
    }
    if let Some(native) =
        try_native_nested_obj_generic_delta_diff(base_model_binary, next_view, sid)?
    {
        return Ok(native);
    }
    if let Some(native) = try_native_root_obj_scalar_delta_diff(base_model_binary, next_view, sid)?
    {
        return Ok(native);
    }
    if let Some(native) = try_native_root_obj_generic_delta_diff(base_model_binary, next_view, sid)?
    {
        return Ok(native);
    }
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

fn is_visible_child(runtime: &RuntimeModel, id: Timestamp) -> bool {
    !matches!(
        runtime.nodes.get(&Id::from(id)),
        None | Some(RuntimeNode::Con(ConCell::Undef))
    )
}

fn runtime_base_time(runtime: &RuntimeModel) -> u64 {
    if let Some(server_time) = runtime.server_clock_time {
        return server_time;
    }

    let mut base = runtime.clock_table.first().map(|c| c.time).unwrap_or(0);
    if let Some(local_sid) = runtime.clock_table.first().map(|c| c.sid) {
        if let Some(ranges) = runtime.clock.observed.get(&local_sid) {
            if let Some((_, end)) = ranges.last() {
                base = base.max(*end);
            }
        }
    }
    base
}

fn object_visible_fields(runtime: &RuntimeModel, obj: Timestamp) -> Option<BTreeMap<String, Timestamp>> {
    let obj = runtime.resolve_object_node(obj)?;
    let entries = match runtime.nodes.get(&Id::from(obj))? {
        RuntimeNode::Obj(entries) => entries,
        _ => return None,
    };
    let mut out = BTreeMap::new();
    for (k, v) in entries {
        let ts: Timestamp = (*v).into();
        if is_visible_child(runtime, ts) {
            out.insert(k.clone(), ts);
        } else {
            out.remove(k);
        }
    }
    Some(out)
}

fn try_emit_object_recursive_diff_runtime(
    runtime: &RuntimeModel,
    emitter: &mut NativeEmitter,
    obj_node: Timestamp,
    new_obj: &serde_json::Map<String, Value>,
) -> Result<bool, DiffError> {
    let old_fields = match object_visible_fields(runtime, obj_node) {
        Some(v) => v,
        None => return Ok(false),
    };
    let mut pairs: Vec<(String, Timestamp)> = Vec::new();

    for k in old_fields.keys() {
        if !new_obj.contains_key(k) {
            let id = emitter.next_id();
            emitter.push(DecodedOp::NewCon {
                id,
                value: ConValue::Undef,
            });
            pairs.push((k.clone(), id));
        }
    }

    for (k, v) in new_obj {
        if let Some(child_id) = old_fields.get(k) {
            if runtime.node_json_value(*child_id).as_ref() == Some(v) {
                continue;
            }
            let old_v = runtime.node_json_value(*child_id);
            if try_emit_child_recursive_diff(runtime, emitter, *child_id, old_v.as_ref(), v)? {
                continue;
            }
        }
        let id = emitter.emit_value(v);
        pairs.push((k.clone(), id));
    }

    if !pairs.is_empty() {
        emitter.push(DecodedOp::InsObj {
            id: emitter.next_id(),
            obj: obj_node,
            data: pairs,
        });
    }
    Ok(true)
}

pub fn diff_runtime_to_ops(
    runtime: &RuntimeModel,
    next_view: &Value,
    sid: u64,
) -> Result<Option<Vec<DecodedOp>>, DiffError> {
    let base_time = runtime_base_time(runtime);
    let mut emitter = NativeEmitter::new(sid, base_time.saturating_add(1));

    match (runtime.root_id(), next_view) {
        (Some(root_id), Value::Object(next_obj))
            if runtime.resolve_object_node(root_id).is_some() =>
        {
            let root_obj = runtime
                .resolve_object_node(root_id)
                .expect("checked is_some");
            let _ = try_emit_object_recursive_diff_runtime(runtime, &mut emitter, root_obj, next_obj)?;
        }
        (Some(root_id), _) => {
            if runtime.node_json_value(root_id).as_ref() == Some(next_view) {
                return Ok(None);
            }
            let old = runtime.node_json_value(root_id);
            if !try_emit_child_recursive_diff(runtime, &mut emitter, root_id, old.as_ref(), next_view)?
            {
                let next_root = emitter.emit_value(next_view);
                emitter.push(DecodedOp::InsVal {
                    id: emitter.next_id(),
                    obj: Timestamp { sid: 0, time: 0 },
                    val: next_root,
                });
            }
        }
        (None, _) => {
            let next_root = emitter.emit_value(next_view);
            emitter.push(DecodedOp::InsVal {
                id: emitter.next_id(),
                obj: Timestamp { sid: 0, time: 0 },
                val: next_root,
            });
        }
    }

    if emitter.ops.is_empty() {
        Ok(None)
    } else {
        Ok(Some(emitter.ops))
    }
}

pub fn diff_runtime(
    runtime: &RuntimeModel,
    next_view: &Value,
    sid: u64,
) -> Result<Option<RuntimeDiffResult>, DiffError> {
    let Some(ops) = diff_runtime_to_ops(runtime, next_view, sid)? else {
        return Ok(None);
    };
    let base_time = runtime_base_time(runtime);
    let patch_binary = encode_patch_from_ops(sid, base_time.saturating_add(1), &ops)?;
    Ok(Some(RuntimeDiffResult { ops, patch_binary }))
}

pub fn diff_runtime_to_patch_bytes(
    runtime: &RuntimeModel,
    next_view: &Value,
    sid: u64,
) -> Result<Option<Vec<u8>>, DiffError> {
    Ok(diff_runtime(runtime, next_view, sid)?.map(|r| r.patch_binary))
}

include!("dst_keys.rs");
include!("common.rs");
include!("scalar.rs");
include!("object.rs");
include!("string.rs");
include!("array.rs");
include!("bin.rs");
include!("vec.rs");
