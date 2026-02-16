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
use crate::crdt_binary::first_logical_clock_sid_time;
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

    // Native logical empty-object root path.
    if let Some(native) = try_native_empty_obj_diff(base_model_binary, next_view, sid)? {
        return Ok(native);
    }
    if let Some(native) = try_native_root_obj_multi_string_delta_diff(base_model_binary, next_view, sid)? {
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
    Err(DiffError::UnsupportedShape)
}

fn try_native_non_object_root_diff(
    base_model_binary: &[u8],
    next_view: &Value,
    patch_sid: u64,
) -> Result<Option<Option<Vec<u8>>>, DiffError> {
    let model = match Model::from_binary(base_model_binary) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    let base_view = model.view();
    if matches!(base_view, Value::Object(_)) && matches!(next_view, Value::Object(_)) {
        return Ok(None);
    }

    let runtime = match RuntimeModel::from_model_binary(base_model_binary) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    let root = match runtime.root_id() {
        Some(v) => v,
        None => return Ok(None),
    };
    let (_, base_time) = match first_logical_clock_sid_time(base_model_binary) {
        Some(v) => v,
        None => return Ok(None),
    };
    let mut emitter = NativeEmitter::new(patch_sid, base_time.saturating_add(1));

    match (base_view, next_view) {
        (Value::String(old), Value::String(new)) if runtime.node_is_string(root) => {
            let slots = match runtime.string_visible_slots(root) {
                Some(v) => v,
                None => return Ok(None),
            };
            let old_chars: Vec<char> = old.chars().collect();
            let new_chars: Vec<char> = new.chars().collect();
            if old_chars.len() != slots.len() {
                return Ok(None);
            }
            let mut lcp = 0usize;
            while lcp < old_chars.len() && lcp < new_chars.len() && old_chars[lcp] == new_chars[lcp] {
                lcp += 1;
            }
            let mut lcs = 0usize;
            while lcs < (old_chars.len() - lcp)
                && lcs < (new_chars.len() - lcp)
                && old_chars[old_chars.len() - 1 - lcs] == new_chars[new_chars.len() - 1 - lcs]
            {
                lcs += 1;
            }
            let del_len = old_chars.len().saturating_sub(lcp + lcs);
            let ins: String = new_chars[lcp..new_chars.len().saturating_sub(lcs)]
                .iter()
                .collect();
            if !ins.is_empty() {
                let reference = if lcp == 0 {
                    slots.first().copied().unwrap_or(root)
                } else {
                    slots[lcp - 1]
                };
                emitter.push(DecodedOp::InsStr {
                    id: emitter.next_id(),
                    obj: root,
                    reference,
                    data: ins,
                });
            }
            if del_len > 0 {
                let del_slots = &slots[lcp..lcp + del_len];
                let mut spans: Vec<crate::patch::Timespan> = Vec::new();
                for slot in del_slots {
                    if let Some(last) = spans.last_mut() {
                        if last.sid == slot.sid && last.time + last.span == slot.time {
                            last.span += 1;
                            continue;
                        }
                    }
                    spans.push(crate::patch::Timespan {
                        sid: slot.sid,
                        time: slot.time,
                        span: 1,
                    });
                }
                emitter.push(DecodedOp::Del {
                    id: emitter.next_id(),
                    obj: root,
                    what: spans,
                });
            }
        }
        (Value::Array(old), Value::Array(new)) if runtime.node_is_array(root) => {
            if old.iter().any(|v| !is_array_native_supported(v))
                || new.iter().any(|v| !is_array_native_supported(v))
            {
                return Ok(None);
            }
            let slots = match runtime.array_visible_slots(root) {
                Some(v) => v,
                None => return Ok(None),
            };
            if slots.len() != old.len() {
                return Ok(None);
            }
            emit_array_delta_ops(&mut emitter, root, &slots, old, new);
        }
        (Value::Array(_), Value::Array(new)) if runtime.node_is_vec(root) => {
            emit_vec_delta_ops(&runtime, &mut emitter, root, new);
        }
        (old, new) if runtime.node_is_bin(root) => {
            let old_bin = match parse_bin_object(old) {
                Some(v) => v,
                None => return Ok(None),
            };
            let new_bin = match parse_bin_object(new) {
                Some(v) => v,
                None => return Ok(None),
            };
            let slots = match runtime.bin_visible_slots(root) {
                Some(v) => v,
                None => return Ok(None),
            };
            if slots.len() != old_bin.len() {
                return Ok(None);
            }
            let mut lcp = 0usize;
            while lcp < old_bin.len() && lcp < new_bin.len() && old_bin[lcp] == new_bin[lcp] {
                lcp += 1;
            }
            let mut lcs = 0usize;
            while lcs < (old_bin.len() - lcp)
                && lcs < (new_bin.len() - lcp)
                && old_bin[old_bin.len() - 1 - lcs] == new_bin[new_bin.len() - 1 - lcs]
            {
                lcs += 1;
            }
            let del_len = old_bin.len().saturating_sub(lcp + lcs);
            let ins_bytes = &new_bin[lcp..new_bin.len().saturating_sub(lcs)];
            if !ins_bytes.is_empty() {
                let reference = if lcp == 0 { root } else { slots[lcp - 1] };
                emitter.push(DecodedOp::InsBin {
                    id: emitter.next_id(),
                    obj: root,
                    reference,
                    data: ins_bytes.to_vec(),
                });
            }
            if del_len > 0 {
                let del_slots = &slots[lcp..lcp + del_len];
                let mut spans: Vec<crate::patch::Timespan> = Vec::new();
                for slot in del_slots {
                    if let Some(last) = spans.last_mut() {
                        if last.sid == slot.sid && last.time + last.span == slot.time {
                            last.span += 1;
                            continue;
                        }
                    }
                    spans.push(crate::patch::Timespan {
                        sid: slot.sid,
                        time: slot.time,
                        span: 1,
                    });
                }
                emitter.push(DecodedOp::Del {
                    id: emitter.next_id(),
                    obj: root,
                    what: spans,
                });
            }
        }
        _ => {
            // Root replace path (type mismatch or unsupported in-place diff):
            // write ORIGIN register using ins_val.
            let value = emitter.emit_value(next_view);
            emitter.push(DecodedOp::InsVal {
                id: emitter.next_id(),
                obj: Timestamp { sid: 0, time: 0 },
                val: value,
            });
        }
    }

    if emitter.ops.is_empty() {
        return Ok(Some(None));
    }
    let encoded = encode_patch_from_ops(patch_sid, base_time.saturating_add(1), &emitter.ops)?;
    Ok(Some(Some(encoded)))
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
    let base_was_null = matches!(model.view(), Value::Null);
    match model.view() {
        Value::Object(map) if map.is_empty() => {}
        // Logical empty model (`undefined` root) behaves as an empty object
        // root in fixture-covered less-db bootstrap flows.
        Value::Null => {}
        _ => return Ok(None),
    }

    let next_obj = match next_view {
        Value::Object(map) => map,
        _ => return Ok(None),
    };
    let (root_sid, base_time) = match first_logical_clock_sid_time(base_model_binary) {
        Some(v) => v,
        None => return Ok(None),
    };

    let mut emitter = NativeEmitter::new(patch_sid, base_time.saturating_add(1));
    let mut root = Timestamp { sid: root_sid, time: 1 };

    if base_was_null {
        // Undefined-root logical model bootstrap:
        // create root object and bind it to ORIGIN via ins_val.
        let root_obj = emitter.next_id();
        emitter.push(DecodedOp::NewObj { id: root_obj });
        emitter.push(DecodedOp::InsVal {
            id: emitter.next_id(),
            obj: Timestamp { sid: 0, time: 0 },
            val: root_obj,
        });
        root = root_obj;
    }

    if next_obj.is_empty() {
        if base_was_null {
            let encoded =
                encode_patch_from_ops(patch_sid, base_time.saturating_add(1), &emitter.ops)?;
            return Ok(Some(Some(encoded)));
        }
        return Ok(Some(None));
    }

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

    let (root_sid, base_time) = match first_logical_clock_sid_time(base_model_binary) {
        Some(v) => v,
        None => return Ok(None),
    };

    // Constrain native path to scalar-only key replacements at root. If any
    // structural/nested mutation is detected, fall back to compatibility
    // layer oracle path for exact upstream operation-shape parity.
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

    // Pass 1: deletions in base key iteration order (`undefined` writes).
    for (k, _) in base_obj {
        if !next_obj.contains_key(k) {
            let id = emitter.next_id();
            emitter.push(DecodedOp::NewCon {
                id,
                value: ConValue::Undef,
            });
            pairs.push((k.clone(), id));
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

fn try_native_root_obj_mixed_recursive_diff(
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
    let changed: Vec<&String> = base_obj
        .iter()
        .filter_map(|(k, v)| (next_obj.get(k) != Some(v)).then_some(k))
        .collect();
    if changed.len() < 2 {
        return Ok(None);
    }
    let runtime = match RuntimeModel::from_model_binary(base_model_binary) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    let root = match runtime.root_id() {
        Some(v) => v,
        None => return Ok(None),
    };
    let (_, base_time) = match first_logical_clock_sid_time(base_model_binary) {
        Some(v) => v,
        None => return Ok(None),
    };
    let mut emitter = NativeEmitter::new(patch_sid, base_time.saturating_add(1));
    let mut root_pairs: Vec<(String, Timestamp)> = Vec::new();

    for (k, _) in base_obj {
        if !next_obj.contains_key(k) {
            let id = emitter.next_id();
            emitter.push(DecodedOp::NewCon {
                id,
                value: ConValue::Undef,
            });
            root_pairs.push((k.clone(), id));
        }
    }

    for (k, next_v) in next_obj {
        if base_obj.get(k) == Some(next_v) {
            continue;
        }
        let Some(child) = runtime.root_object_field(k) else {
            let id = emitter.emit_value(next_v);
            root_pairs.push((k.clone(), id));
            continue;
        };

        if try_emit_child_recursive_diff(&runtime, &mut emitter, child, base_obj.get(k), next_v)? {
            continue;
        }

        let id = emitter.emit_value(next_v);
        root_pairs.push((k.clone(), id));
    }

    if !root_pairs.is_empty() {
        emitter.push(DecodedOp::InsObj {
            id: emitter.next_id(),
            obj: root,
            data: root_pairs,
        });
    }
    if emitter.ops.is_empty() {
        return Ok(Some(None));
    }
    let encoded = encode_patch_from_ops(patch_sid, base_time.saturating_add(1), &emitter.ops)?;
    Ok(Some(Some(encoded)))
}

fn try_native_root_obj_generic_delta_diff(
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

    let (root_sid, base_time) = match first_logical_clock_sid_time(base_model_binary) {
        Some(v) => v,
        None => return Ok(None),
    };

    let mut emitter = NativeEmitter::new(patch_sid, base_time.saturating_add(1));
    let mut pairs: Vec<(String, Timestamp)> = Vec::new();

    // Upstream JsonCrdtDiff.diffObj ordering: first pass through source keys
    // for deletions (undefined), then destination traversal for inserts/updates.
    for (k, _) in base_obj {
        if !next_obj.contains_key(k) {
            let id = emitter.next_id();
            emitter.push(DecodedOp::NewCon {
                id,
                value: ConValue::Undef,
            });
            pairs.push((k.clone(), id));
        }
    }

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

fn try_native_root_obj_string_delta_diff(
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

    // Native string delta path is constrained to exactly one changed key with
    // no key-set mutation to preserve upstream op ordering and IDs.
    if base_obj.len() != next_obj.len() {
        return Ok(None);
    }
    if base_obj.keys().any(|k| !next_obj.contains_key(k)) {
        return Ok(None);
    }
    let changed: Vec<&String> = base_obj
        .iter()
        .filter_map(|(k, v)| (next_obj.get(k) != Some(v)).then_some(k))
        .collect();
    if changed.len() != 1 {
        return Ok(None);
    }
    let key = changed[0];
    let old = match base_obj.get(key) {
        Some(Value::String(s)) => s,
        _ => return Ok(None),
    };
    let new = match next_obj.get(key) {
        Some(Value::String(s)) => s,
        _ => return Ok(None),
    };

    let runtime = match RuntimeModel::from_model_binary(base_model_binary) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    let str_node = match runtime.root_object_field(key) {
        Some(id) if runtime.node_is_string(id) => id,
        _ => return Ok(None),
    };
    let slots = match runtime.string_visible_slots(str_node) {
        Some(v) => v,
        None => return Ok(None),
    };

    let old_chars: Vec<char> = old.chars().collect();
    if old_chars.len() != slots.len() {
        return Ok(None);
    }

    let encoded = match emit_string_delta_patch(base_model_binary, patch_sid, str_node, &slots, old, new)? {
        Some(v) => v,
        None => return Ok(Some(None)),
    };
    Ok(Some(Some(encoded)))
}

fn try_native_root_obj_multi_string_delta_diff(
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
    if base_obj.len() != next_obj.len() {
        return Ok(None);
    }
    if base_obj.keys().any(|k| !next_obj.contains_key(k)) {
        return Ok(None);
    }

    let changed: Vec<&String> = base_obj
        .iter()
        .filter_map(|(k, v)| (next_obj.get(k) != Some(v)).then_some(k))
        .collect();
    if changed.len() < 2 {
        return Ok(None);
    }

    let runtime = match RuntimeModel::from_model_binary(base_model_binary) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    let (_, base_time) = match first_logical_clock_sid_time(base_model_binary) {
        Some(v) => v,
        None => return Ok(None),
    };
    let mut emitter = NativeEmitter::new(patch_sid, base_time.saturating_add(1));

    for (k, next_v) in next_obj {
        if base_obj.get(k) == Some(next_v) {
            continue;
        }
        let old = match base_obj.get(k) {
            Some(Value::String(s)) => s,
            _ => return Ok(None),
        };
        let new = match next_v {
            Value::String(s) => s,
            _ => return Ok(None),
        };
        let str_node = match runtime.root_object_field(k) {
            Some(id) if runtime.node_is_string(id) => id,
            _ => return Ok(None),
        };
        let slots = match runtime.string_visible_slots(str_node) {
            Some(v) => v,
            None => return Ok(None),
        };

        let old_chars: Vec<char> = old.chars().collect();
        let new_chars: Vec<char> = new.chars().collect();
        if old_chars.len() != slots.len() {
            return Ok(None);
        }
        let mut lcp = 0usize;
        while lcp < old_chars.len() && lcp < new_chars.len() && old_chars[lcp] == new_chars[lcp] {
            lcp += 1;
        }
        let mut lcs = 0usize;
        while lcs < (old_chars.len() - lcp)
            && lcs < (new_chars.len() - lcp)
            && old_chars[old_chars.len() - 1 - lcs] == new_chars[new_chars.len() - 1 - lcs]
        {
            lcs += 1;
        }
        let del_len = old_chars.len().saturating_sub(lcp + lcs);
        let ins: String = new_chars[lcp..new_chars.len().saturating_sub(lcs)]
            .iter()
            .collect();
        let ins_len = ins.chars().count();
        if ins_len > 0 {
            let reference = if lcp == 0 {
                slots.first().copied().unwrap_or(str_node)
            } else {
                slots[lcp - 1]
            };
            emitter.push(DecodedOp::InsStr {
                id: emitter.next_id(),
                obj: str_node,
                reference,
                data: ins,
            });
        }
        if del_len > 0 {
            let del_slots = &slots[lcp..lcp + del_len];
            let mut spans: Vec<crate::patch::Timespan> = Vec::new();
            for slot in del_slots {
                if let Some(last) = spans.last_mut() {
                    if last.sid == slot.sid && last.time + last.span == slot.time {
                        last.span += 1;
                        continue;
                    }
                }
                spans.push(crate::patch::Timespan {
                    sid: slot.sid,
                    time: slot.time,
                    span: 1,
                });
            }
            emitter.push(DecodedOp::Del {
                id: emitter.next_id(),
                obj: str_node,
                what: spans,
            });
        }
    }

    if emitter.ops.is_empty() {
        return Ok(Some(None));
    }
    let encoded = encode_patch_from_ops(patch_sid, base_time.saturating_add(1), &emitter.ops)?;
    Ok(Some(Some(encoded)))
}

fn try_native_root_obj_array_delta_diff(
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
    if base_obj.len() != next_obj.len() {
        return Ok(None);
    }
    if base_obj.keys().any(|k| !next_obj.contains_key(k)) {
        return Ok(None);
    }

    let changed: Vec<&String> = base_obj
        .iter()
        .filter_map(|(k, v)| (next_obj.get(k) != Some(v)).then_some(k))
        .collect();
    if changed.len() != 1 {
        return Ok(None);
    }
    let key = changed[0];
    let old = match base_obj.get(key) {
        Some(Value::Array(a)) => a,
        _ => return Ok(None),
    };
    let new = match next_obj.get(key) {
        Some(Value::Array(a)) => a,
        _ => return Ok(None),
    };
    if old.iter().any(|v| !is_array_native_supported(v))
        || new.iter().any(|v| !is_array_native_supported(v))
    {
        return Ok(None);
    }

    let runtime = match RuntimeModel::from_model_binary(base_model_binary) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    let arr_node = match runtime.root_object_field(key) {
        Some(id) if runtime.node_is_array(id) => id,
        _ => return Ok(None),
    };
    let slots = match runtime.array_visible_slots(arr_node) {
        Some(v) => v,
        None => return Ok(None),
    };
    if slots.len() != old.len() {
        return Ok(None);
    }

    let mut lcp = 0usize;
    while lcp < old.len() && lcp < new.len() && old[lcp] == new[lcp] {
        lcp += 1;
    }
    let mut lcs = 0usize;
    while lcs < (old.len() - lcp)
        && lcs < (new.len() - lcp)
        && old[old.len() - 1 - lcs] == new[new.len() - 1 - lcs]
    {
        lcs += 1;
    }

    let del_len = old.len().saturating_sub(lcp + lcs);
    let ins_items = &new[lcp..new.len().saturating_sub(lcs)];

    if del_len == 0 && ins_items.is_empty() {
        return Ok(Some(None));
    }

    let (_, base_time) = match first_logical_clock_sid_time(base_model_binary) {
        Some(v) => v,
        None => return Ok(None),
    };
    let mut emitter = NativeEmitter::new(patch_sid, base_time.saturating_add(1));

    if !ins_items.is_empty() {
        let mut ids = Vec::with_capacity(ins_items.len());
        for item in ins_items {
            if is_con_scalar(item) {
                let val_id = emitter.next_id();
                emitter.push(DecodedOp::NewVal { id: val_id });
                let con_id = emitter.emit_value(item);
                emitter.push(DecodedOp::InsVal {
                    id: emitter.next_id(),
                    obj: val_id,
                    val: con_id,
                });
                ids.push(val_id);
            } else {
                ids.push(emitter.emit_value(item));
            }
        }
        let reference = if lcp == 0 { arr_node } else { slots[lcp - 1] };
        emitter.push(DecodedOp::InsArr {
            id: emitter.next_id(),
            obj: arr_node,
            reference,
            data: ids,
        });
    }

    if del_len > 0 {
        let del_slots = &slots[lcp..lcp + del_len];
        let mut spans: Vec<crate::patch::Timespan> = Vec::new();
        for slot in del_slots {
            if let Some(last) = spans.last_mut() {
                if last.sid == slot.sid && last.time + last.span == slot.time {
                    last.span += 1;
                    continue;
                }
            }
            spans.push(crate::patch::Timespan {
                sid: slot.sid,
                time: slot.time,
                span: 1,
            });
        }
        emitter.push(DecodedOp::Del {
            id: emitter.next_id(),
            obj: arr_node,
            what: spans,
        });
    }

    let encoded = encode_patch_from_ops(patch_sid, base_time.saturating_add(1), &emitter.ops)?;
    Ok(Some(Some(encoded)))
}

fn try_native_root_obj_bin_delta_diff(
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
    if base_obj.len() != next_obj.len() {
        return Ok(None);
    }
    if base_obj.keys().any(|k| !next_obj.contains_key(k)) {
        return Ok(None);
    }

    // Keep deterministic shape: single changed key only.
    let changed: Vec<&String> = base_obj
        .iter()
        .filter_map(|(k, v)| (next_obj.get(k) != Some(v)).then_some(k))
        .collect();
    if changed.len() != 1 {
        return Ok(None);
    }
    let key = changed[0];
    let old = match base_obj.get(key).and_then(parse_bin_object) {
        Some(v) => v,
        None => return Ok(None),
    };
    let new = match next_obj.get(key).and_then(parse_bin_object) {
        Some(v) => v,
        None => return Ok(None),
    };

    let runtime = match RuntimeModel::from_model_binary(base_model_binary) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    let bin_node = match runtime.root_object_field(key) {
        Some(id) if runtime.node_is_bin(id) => id,
        _ => return Ok(None),
    };
    let slots = match runtime.bin_visible_slots(bin_node) {
        Some(v) => v,
        None => return Ok(None),
    };
    if slots.len() != old.len() {
        return Ok(None);
    }

    let encoded = match emit_bin_delta_patch(base_model_binary, patch_sid, bin_node, &slots, &old, &new)? {
        Some(v) => v,
        None => return Ok(Some(None)),
    };
    Ok(Some(Some(encoded)))
}

fn try_native_root_obj_multi_bin_delta_diff(
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
    if base_obj.len() != next_obj.len() {
        return Ok(None);
    }
    if base_obj.keys().any(|k| !next_obj.contains_key(k)) {
        return Ok(None);
    }

    let changed: Vec<&String> = base_obj
        .iter()
        .filter_map(|(k, v)| (next_obj.get(k) != Some(v)).then_some(k))
        .collect();
    if changed.len() < 2 {
        return Ok(None);
    }

    let runtime = match RuntimeModel::from_model_binary(base_model_binary) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    let (_, base_time) = match first_logical_clock_sid_time(base_model_binary) {
        Some(v) => v,
        None => return Ok(None),
    };
    let mut emitter = NativeEmitter::new(patch_sid, base_time.saturating_add(1));

    for (k, next_v) in next_obj {
        if base_obj.get(k) == Some(next_v) {
            continue;
        }
        let old = match base_obj.get(k).and_then(parse_bin_object) {
            Some(v) => v,
            None => return Ok(None),
        };
        let new = match parse_bin_object(next_v) {
            Some(v) => v,
            None => return Ok(None),
        };
        let bin_node = match runtime.root_object_field(k) {
            Some(id) if runtime.node_is_bin(id) => id,
            _ => return Ok(None),
        };
        let slots = match runtime.bin_visible_slots(bin_node) {
            Some(v) => v,
            None => return Ok(None),
        };
        if slots.len() != old.len() {
            return Ok(None);
        }

        let mut lcp = 0usize;
        while lcp < old.len() && lcp < new.len() && old[lcp] == new[lcp] {
            lcp += 1;
        }
        let mut lcs = 0usize;
        while lcs < (old.len() - lcp)
            && lcs < (new.len() - lcp)
            && old[old.len() - 1 - lcs] == new[new.len() - 1 - lcs]
        {
            lcs += 1;
        }

        let del_len = old.len().saturating_sub(lcp + lcs);
        let ins_bytes = &new[lcp..new.len().saturating_sub(lcs)];
        if !ins_bytes.is_empty() {
            let reference = if lcp == 0 { bin_node } else { slots[lcp - 1] };
            emitter.push(DecodedOp::InsBin {
                id: emitter.next_id(),
                obj: bin_node,
                reference,
                data: ins_bytes.to_vec(),
            });
        }
        if del_len > 0 {
            let del_slots = &slots[lcp..lcp + del_len];
            let mut spans: Vec<crate::patch::Timespan> = Vec::new();
            for slot in del_slots {
                if let Some(last) = spans.last_mut() {
                    if last.sid == slot.sid && last.time + last.span == slot.time {
                        last.span += 1;
                        continue;
                    }
                }
                spans.push(crate::patch::Timespan {
                    sid: slot.sid,
                    time: slot.time,
                    span: 1,
                });
            }
            emitter.push(DecodedOp::Del {
                id: emitter.next_id(),
                obj: bin_node,
                what: spans,
            });
        }
    }

    if emitter.ops.is_empty() {
        return Ok(Some(None));
    }
    let encoded = encode_patch_from_ops(patch_sid, base_time.saturating_add(1), &emitter.ops)?;
    Ok(Some(Some(encoded)))
}

fn try_native_nested_obj_bin_delta_diff(
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

    let (path, old, new) = match find_single_bin_delta_path(base_obj, next_obj) {
        Some(v) => v,
        None => return Ok(None),
    };
    if path.is_empty() {
        return Ok(None);
    }

    let runtime = match RuntimeModel::from_model_binary(base_model_binary) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    let mut node = match runtime.root_object_field(&path[0]) {
        Some(id) => id,
        None => return Ok(None),
    };
    for seg in path.iter().skip(1) {
        node = match runtime.object_field(node, seg) {
            Some(id) => id,
            None => return Ok(None),
        };
    }
    if !runtime.node_is_bin(node) {
        return Ok(None);
    }
    let slots = match runtime.bin_visible_slots(node) {
        Some(v) => v,
        None => return Ok(None),
    };
    let encoded = match emit_bin_delta_patch(base_model_binary, patch_sid, node, &slots, &old, &new)? {
        Some(v) => v,
        None => return Ok(Some(None)),
    };
    Ok(Some(Some(encoded)))
}

fn try_native_multi_root_nested_bin_delta_diff(
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
    if base_obj.len() != next_obj.len() {
        return Ok(None);
    }
    if base_obj.keys().any(|k| !next_obj.contains_key(k)) {
        return Ok(None);
    }

    let changed: Vec<&String> = base_obj
        .iter()
        .filter_map(|(k, v)| (next_obj.get(k) != Some(v)).then_some(k))
        .collect();
    if changed.len() < 2 {
        return Ok(None);
    }

    let runtime = match RuntimeModel::from_model_binary(base_model_binary) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    let (_, base_time) = match first_logical_clock_sid_time(base_model_binary) {
        Some(v) => v,
        None => return Ok(None),
    };
    let mut emitter = NativeEmitter::new(patch_sid, base_time.saturating_add(1));

    for (root_key, next_child) in next_obj {
        let base_child = match base_obj.get(root_key) {
            Some(v) => v,
            None => return Ok(None),
        };
        if base_child == next_child {
            continue;
        }
        let (sub_path, old, new) = match (base_child.as_object(), next_child.as_object()) {
            (Some(bm), Some(nm)) => match find_single_bin_delta_path(bm, nm) {
                Some(v) => v,
                None => return Ok(None),
            },
            _ => return Ok(None),
        };
        if sub_path.is_empty() {
            return Ok(None);
        }
        let mut node = match runtime.root_object_field(root_key) {
            Some(id) => id,
            None => return Ok(None),
        };
        for seg in sub_path {
            node = match runtime.object_field(node, &seg) {
                Some(id) => id,
                None => return Ok(None),
            };
        }
        if !runtime.node_is_bin(node) {
            return Ok(None);
        }
        let slots = match runtime.bin_visible_slots(node) {
            Some(v) => v,
            None => return Ok(None),
        };
        if slots.len() != old.len() {
            return Ok(None);
        }
        let mut lcp = 0usize;
        while lcp < old.len() && lcp < new.len() && old[lcp] == new[lcp] {
            lcp += 1;
        }
        let mut lcs = 0usize;
        while lcs < (old.len() - lcp)
            && lcs < (new.len() - lcp)
            && old[old.len() - 1 - lcs] == new[new.len() - 1 - lcs]
        {
            lcs += 1;
        }
        let del_len = old.len().saturating_sub(lcp + lcs);
        let ins_bytes = &new[lcp..new.len().saturating_sub(lcs)];
        if !ins_bytes.is_empty() {
            let reference = if lcp == 0 { node } else { slots[lcp - 1] };
            emitter.push(DecodedOp::InsBin {
                id: emitter.next_id(),
                obj: node,
                reference,
                data: ins_bytes.to_vec(),
            });
        }
        if del_len > 0 {
            let del_slots = &slots[lcp..lcp + del_len];
            let mut spans: Vec<crate::patch::Timespan> = Vec::new();
            for slot in del_slots {
                if let Some(last) = spans.last_mut() {
                    if last.sid == slot.sid && last.time + last.span == slot.time {
                        last.span += 1;
                        continue;
                    }
                }
                spans.push(crate::patch::Timespan {
                    sid: slot.sid,
                    time: slot.time,
                    span: 1,
                });
            }
            emitter.push(DecodedOp::Del {
                id: emitter.next_id(),
                obj: node,
                what: spans,
            });
        }
    }

    if emitter.ops.is_empty() {
        return Ok(Some(None));
    }
    let encoded = encode_patch_from_ops(patch_sid, base_time.saturating_add(1), &emitter.ops)?;
    Ok(Some(Some(encoded)))
}

fn try_native_root_obj_multi_array_delta_diff(
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
    if base_obj.len() != next_obj.len() {
        return Ok(None);
    }
    if base_obj.keys().any(|k| !next_obj.contains_key(k)) {
        return Ok(None);
    }

    let changed: Vec<&String> = base_obj
        .iter()
        .filter_map(|(k, v)| (next_obj.get(k) != Some(v)).then_some(k))
        .collect();
    if changed.is_empty() {
        return Ok(Some(None));
    }

    let runtime = match RuntimeModel::from_model_binary(base_model_binary) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    let (_, base_time) = match first_logical_clock_sid_time(base_model_binary) {
        Some(v) => v,
        None => return Ok(None),
    };
    let mut emitter = NativeEmitter::new(patch_sid, base_time.saturating_add(1));

    for key in changed {
        let old = match base_obj.get(key) {
            Some(Value::Array(a)) => a,
            _ => return Ok(None),
        };
        let new = match next_obj.get(key) {
            Some(Value::Array(a)) => a,
            _ => return Ok(None),
        };
        if old.iter().any(|v| !is_array_native_supported(v))
            || new.iter().any(|v| !is_array_native_supported(v))
        {
            return Ok(None);
        }
        let arr_node = match runtime.root_object_field(key) {
            Some(id) if runtime.node_is_array(id) => id,
            _ => return Ok(None),
        };
        let slots = match runtime.array_visible_slots(arr_node) {
            Some(v) => v,
            None => return Ok(None),
        };
        if slots.len() != old.len() {
            return Ok(None);
        }
        emit_array_delta_ops(&mut emitter, arr_node, &slots, old, new);
    }

    if emitter.ops.is_empty() {
        return Ok(Some(None));
    }
    let encoded = encode_patch_from_ops(patch_sid, base_time.saturating_add(1), &emitter.ops)?;
    Ok(Some(Some(encoded)))
}

fn try_native_nested_obj_array_delta_diff(
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
    let (path, old, new) = match find_single_array_delta_path(base_obj, next_obj) {
        Some(v) => v,
        None => return Ok(None),
    };
    if path.is_empty() {
        return Ok(None);
    }
    if old.iter().any(|v| !is_array_native_supported(v))
        || new.iter().any(|v| !is_array_native_supported(v))
    {
        return Ok(None);
    }

    let runtime = match RuntimeModel::from_model_binary(base_model_binary) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    let mut node = match runtime.root_object_field(&path[0]) {
        Some(id) => id,
        None => return Ok(None),
    };
    for seg in path.iter().skip(1) {
        node = match runtime.object_field(node, seg) {
            Some(id) => id,
            None => return Ok(None),
        };
    }
    if !runtime.node_is_array(node) {
        return Ok(None);
    }
    let slots = match runtime.array_visible_slots(node) {
        Some(v) => v,
        None => return Ok(None),
    };
    if slots.len() != old.len() {
        return Ok(None);
    }

    let (_, base_time) = match first_logical_clock_sid_time(base_model_binary) {
        Some(v) => v,
        None => return Ok(None),
    };
    let mut emitter = NativeEmitter::new(patch_sid, base_time.saturating_add(1));
    emit_array_delta_ops(&mut emitter, node, &slots, old, new);
    if emitter.ops.is_empty() {
        return Ok(Some(None));
    }

    let encoded = encode_patch_from_ops(patch_sid, base_time.saturating_add(1), &emitter.ops)?;
    Ok(Some(Some(encoded)))
}

fn try_native_multi_root_nested_array_delta_diff(
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
    if base_obj.len() != next_obj.len() {
        return Ok(None);
    }
    if base_obj.keys().any(|k| !next_obj.contains_key(k)) {
        return Ok(None);
    }
    let changed: Vec<&String> = base_obj
        .iter()
        .filter_map(|(k, v)| (next_obj.get(k) != Some(v)).then_some(k))
        .collect();
    if changed.len() < 2 {
        return Ok(None);
    }

    let runtime = match RuntimeModel::from_model_binary(base_model_binary) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    let (_, base_time) = match first_logical_clock_sid_time(base_model_binary) {
        Some(v) => v,
        None => return Ok(None),
    };
    let mut emitter = NativeEmitter::new(patch_sid, base_time.saturating_add(1));

    for (root_key, next_child) in next_obj {
        let base_child = match base_obj.get(root_key) {
            Some(v) => v,
            None => return Ok(None),
        };
        if base_child == next_child {
            continue;
        }
        let (sub_path, old, new) = match (base_child.as_object(), next_child.as_object()) {
            (Some(bm), Some(nm)) => match find_single_array_delta_path(bm, nm) {
                Some(v) => v,
                None => return Ok(None),
            },
            _ => return Ok(None),
        };
        if sub_path.is_empty() {
            return Ok(None);
        }
        if old.iter().any(|v| !is_array_native_supported(v))
            || new.iter().any(|v| !is_array_native_supported(v))
        {
            return Ok(None);
        }
        let mut node = match runtime.root_object_field(root_key) {
            Some(id) => id,
            None => return Ok(None),
        };
        for seg in sub_path {
            node = match runtime.object_field(node, &seg) {
                Some(id) => id,
                None => return Ok(None),
            };
        }
        if !runtime.node_is_array(node) {
            return Ok(None);
        }
        let slots = match runtime.array_visible_slots(node) {
            Some(v) => v,
            None => return Ok(None),
        };
        if slots.len() != old.len() {
            return Ok(None);
        }
        emit_array_delta_ops(&mut emitter, node, &slots, old, new);
    }

    if emitter.ops.is_empty() {
        return Ok(Some(None));
    }
    let encoded = encode_patch_from_ops(patch_sid, base_time.saturating_add(1), &emitter.ops)?;
    Ok(Some(Some(encoded)))
}

fn emit_array_delta_ops(
    emitter: &mut NativeEmitter,
    arr_node: Timestamp,
    slots: &[Timestamp],
    old: &[Value],
    new: &[Value],
) {
    let mut lcp = 0usize;
    while lcp < old.len() && lcp < new.len() && old[lcp] == new[lcp] {
        lcp += 1;
    }
    let mut lcs = 0usize;
    while lcs < (old.len() - lcp)
        && lcs < (new.len() - lcp)
        && old[old.len() - 1 - lcs] == new[new.len() - 1 - lcs]
    {
        lcs += 1;
    }

    let del_len = old.len().saturating_sub(lcp + lcs);
    let ins_items = &new[lcp..new.len().saturating_sub(lcs)];

    if !ins_items.is_empty() {
        let mut ids = Vec::with_capacity(ins_items.len());
        for item in ins_items {
            if is_con_scalar(item) {
                let val_id = emitter.next_id();
                emitter.push(DecodedOp::NewVal { id: val_id });
                let con_id = emitter.emit_value(item);
                emitter.push(DecodedOp::InsVal {
                    id: emitter.next_id(),
                    obj: val_id,
                    val: con_id,
                });
                ids.push(val_id);
            } else {
                ids.push(emitter.emit_value(item));
            }
        }
        let reference = if lcp == 0 { arr_node } else { slots[lcp - 1] };
        emitter.push(DecodedOp::InsArr {
            id: emitter.next_id(),
            obj: arr_node,
            reference,
            data: ids,
        });
    }

    if del_len > 0 {
        let del_slots = &slots[lcp..lcp + del_len];
        let mut spans: Vec<crate::patch::Timespan> = Vec::new();
        for slot in del_slots {
            if let Some(last) = spans.last_mut() {
                if last.sid == slot.sid && last.time + last.span == slot.time {
                    last.span += 1;
                    continue;
                }
            }
            spans.push(crate::patch::Timespan {
                sid: slot.sid,
                time: slot.time,
                span: 1,
            });
        }
        emitter.push(DecodedOp::Del {
            id: emitter.next_id(),
            obj: arr_node,
            what: spans,
        });
    }
}

fn try_emit_child_recursive_diff(
    runtime: &RuntimeModel,
    emitter: &mut NativeEmitter,
    child: Timestamp,
    old_opt: Option<&Value>,
    new_v: &Value,
) -> Result<bool, DiffError> {
    match old_opt {
        Some(Value::String(old)) if matches!(new_v, Value::String(_)) && runtime.node_is_string(child) => {
            let new = match new_v {
                Value::String(v) => v,
                _ => unreachable!(),
            };
            let slots = match runtime.string_visible_slots(child) {
                Some(v) => v,
                None => return Ok(false),
            };
            let old_chars: Vec<char> = old.chars().collect();
            if old_chars.len() != slots.len() {
                return Ok(false);
            }
            let new_chars: Vec<char> = new.chars().collect();
            let mut lcp = 0usize;
            while lcp < old_chars.len() && lcp < new_chars.len() && old_chars[lcp] == new_chars[lcp] {
                lcp += 1;
            }
            let mut lcs = 0usize;
            while lcs < (old_chars.len() - lcp)
                && lcs < (new_chars.len() - lcp)
                && old_chars[old_chars.len() - 1 - lcs] == new_chars[new_chars.len() - 1 - lcs]
            {
                lcs += 1;
            }
            let del_len = old_chars.len().saturating_sub(lcp + lcs);
            let ins: String = new_chars[lcp..new_chars.len().saturating_sub(lcs)]
                .iter()
                .collect();
            if !ins.is_empty() {
                let reference = if lcp == 0 {
                    slots.first().copied().unwrap_or(child)
                } else {
                    slots[lcp - 1]
                };
                emitter.push(DecodedOp::InsStr {
                    id: emitter.next_id(),
                    obj: child,
                    reference,
                    data: ins,
                });
            }
            if del_len > 0 {
                let del_slots = &slots[lcp..lcp + del_len];
                let mut spans: Vec<crate::patch::Timespan> = Vec::new();
                for slot in del_slots {
                    if let Some(last) = spans.last_mut() {
                        if last.sid == slot.sid && last.time + last.span == slot.time {
                            last.span += 1;
                            continue;
                        }
                    }
                    spans.push(crate::patch::Timespan {
                        sid: slot.sid,
                        time: slot.time,
                        span: 1,
                    });
                }
                emitter.push(DecodedOp::Del {
                    id: emitter.next_id(),
                    obj: child,
                    what: spans,
                });
            }
            return Ok(true);
        }
        Some(old) if runtime.node_is_bin(child) => {
            let old_bin = match parse_bin_object(old) {
                Some(v) => v,
                None => return Ok(false),
            };
            let new_bin = match parse_bin_object(new_v) {
                Some(v) => v,
                None => return Ok(false),
            };
            let slots = match runtime.bin_visible_slots(child) {
                Some(v) => v,
                None => return Ok(false),
            };
            if slots.len() != old_bin.len() {
                return Ok(false);
            }
            let mut lcp = 0usize;
            while lcp < old_bin.len() && lcp < new_bin.len() && old_bin[lcp] == new_bin[lcp] {
                lcp += 1;
            }
            let mut lcs = 0usize;
            while lcs < (old_bin.len() - lcp)
                && lcs < (new_bin.len() - lcp)
                && old_bin[old_bin.len() - 1 - lcs] == new_bin[new_bin.len() - 1 - lcs]
            {
                lcs += 1;
            }
            let del_len = old_bin.len().saturating_sub(lcp + lcs);
            let ins_bytes = &new_bin[lcp..new_bin.len().saturating_sub(lcs)];
            if !ins_bytes.is_empty() {
                let reference = if lcp == 0 { child } else { slots[lcp - 1] };
                emitter.push(DecodedOp::InsBin {
                    id: emitter.next_id(),
                    obj: child,
                    reference,
                    data: ins_bytes.to_vec(),
                });
            }
            if del_len > 0 {
                let del_slots = &slots[lcp..lcp + del_len];
                let mut spans: Vec<crate::patch::Timespan> = Vec::new();
                for slot in del_slots {
                    if let Some(last) = spans.last_mut() {
                        if last.sid == slot.sid && last.time + last.span == slot.time {
                            last.span += 1;
                            continue;
                        }
                    }
                    spans.push(crate::patch::Timespan {
                        sid: slot.sid,
                        time: slot.time,
                        span: 1,
                    });
                }
                emitter.push(DecodedOp::Del {
                    id: emitter.next_id(),
                    obj: child,
                    what: spans,
                });
            }
            return Ok(true);
        }
        Some(Value::Array(old_arr)) if matches!(new_v, Value::Array(_)) && runtime.node_is_array(child) => {
            let new_arr = match new_v {
                Value::Array(v) => v,
                _ => unreachable!(),
            };
            if old_arr.iter().any(|v| !is_array_native_supported(v))
                || new_arr.iter().any(|v| !is_array_native_supported(v))
            {
                return Ok(false);
            }
            let slots = match runtime.array_visible_slots(child) {
                Some(v) => v,
                None => return Ok(false),
            };
            if slots.len() != old_arr.len() {
                return Ok(false);
            }
            emit_array_delta_ops(emitter, child, &slots, old_arr, new_arr);
            return Ok(true);
        }
        Some(Value::Array(_)) if matches!(new_v, Value::Array(_)) && runtime.node_is_vec(child) => {
            let new_arr = match new_v {
                Value::Array(v) => v,
                _ => unreachable!(),
            };
            emit_vec_delta_ops(runtime, emitter, child, new_arr);
            return Ok(true);
        }
        Some(Value::Object(old_obj)) if matches!(new_v, Value::Object(_)) && runtime.node_is_object(child) => {
            let new_obj = match new_v {
                Value::Object(v) => v,
                _ => unreachable!(),
            };
            return try_emit_object_recursive_diff(runtime, emitter, child, old_obj, new_obj);
        }
        _ => {}
    }
    Ok(false)
}

fn try_emit_object_recursive_diff(
    runtime: &RuntimeModel,
    emitter: &mut NativeEmitter,
    obj_node: Timestamp,
    old_obj: &serde_json::Map<String, Value>,
    new_obj: &serde_json::Map<String, Value>,
) -> Result<bool, DiffError> {
    let mut pairs: Vec<(String, Timestamp)> = Vec::new();

    for (k, _) in old_obj {
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
        if old_obj.get(k) == Some(v) {
            continue;
        }
        if let Some(child_id) = runtime.object_field(obj_node, k) {
            if try_emit_child_recursive_diff(runtime, emitter, child_id, old_obj.get(k), v)? {
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

fn try_native_root_obj_vec_delta_diff(
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
    if base_obj.len() != next_obj.len() {
        return Ok(None);
    }
    if base_obj.keys().any(|k| !next_obj.contains_key(k)) {
        return Ok(None);
    }

    let changed: Vec<&String> = base_obj
        .iter()
        .filter_map(|(k, v)| (next_obj.get(k) != Some(v)).then_some(k))
        .collect();
    if changed.is_empty() {
        return Ok(Some(None));
    }

    let runtime = match RuntimeModel::from_model_binary(base_model_binary) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    let (_, base_time) = match first_logical_clock_sid_time(base_model_binary) {
        Some(v) => v,
        None => return Ok(None),
    };
    let mut emitter = NativeEmitter::new(patch_sid, base_time.saturating_add(1));

    for key in changed {
        let dst = match next_obj.get(key) {
            Some(Value::Array(arr)) => arr,
            _ => return Ok(None),
        };
        let vec_node = match runtime.root_object_field(key) {
            Some(id) if runtime.node_is_vec(id) => id,
            _ => return Ok(None),
        };

        emit_vec_delta_ops(&runtime, &mut emitter, vec_node, dst);
    }

    if emitter.ops.is_empty() {
        return Ok(Some(None));
    }
    let encoded = encode_patch_from_ops(patch_sid, base_time.saturating_add(1), &emitter.ops)?;
    Ok(Some(Some(encoded)))
}

fn try_native_root_obj_multi_vec_delta_diff(
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
    if base_obj.len() != next_obj.len() {
        return Ok(None);
    }
    if base_obj.keys().any(|k| !next_obj.contains_key(k)) {
        return Ok(None);
    }

    let changed: Vec<&String> = base_obj
        .iter()
        .filter_map(|(k, v)| (next_obj.get(k) != Some(v)).then_some(k))
        .collect();
    if changed.len() < 2 {
        return Ok(None);
    }

    let runtime = match RuntimeModel::from_model_binary(base_model_binary) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    let (_, base_time) = match first_logical_clock_sid_time(base_model_binary) {
        Some(v) => v,
        None => return Ok(None),
    };
    let mut emitter = NativeEmitter::new(patch_sid, base_time.saturating_add(1));

    // Upstream diffObj destination traversal order.
    for (k, next_v) in next_obj {
        if base_obj.get(k) == Some(next_v) {
            continue;
        }
        let dst = match next_v {
            Value::Array(arr) => arr,
            _ => return Ok(None),
        };
        let vec_node = match runtime.root_object_field(k) {
            Some(id) if runtime.node_is_vec(id) => id,
            _ => return Ok(None),
        };
        emit_vec_delta_ops(&runtime, &mut emitter, vec_node, dst);
    }

    if emitter.ops.is_empty() {
        return Ok(Some(None));
    }
    let encoded = encode_patch_from_ops(patch_sid, base_time.saturating_add(1), &emitter.ops)?;
    Ok(Some(Some(encoded)))
}

fn try_native_nested_obj_vec_delta_diff(
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
    let (path, _old, new) = match find_single_array_delta_path(base_obj, next_obj) {
        Some(v) => v,
        None => return Ok(None),
    };
    if path.is_empty() {
        return Ok(None);
    }

    let runtime = match RuntimeModel::from_model_binary(base_model_binary) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    let mut node = match runtime.root_object_field(&path[0]) {
        Some(id) => id,
        None => return Ok(None),
    };
    for seg in path.iter().skip(1) {
        node = match runtime.object_field(node, seg) {
            Some(id) => id,
            None => return Ok(None),
        };
    }
    if !runtime.node_is_vec(node) {
        return Ok(None);
    }

    let (_, base_time) = match first_logical_clock_sid_time(base_model_binary) {
        Some(v) => v,
        None => return Ok(None),
    };
    let mut emitter = NativeEmitter::new(patch_sid, base_time.saturating_add(1));
    emit_vec_delta_ops(&runtime, &mut emitter, node, new);
    if emitter.ops.is_empty() {
        return Ok(Some(None));
    }

    let encoded = encode_patch_from_ops(patch_sid, base_time.saturating_add(1), &emitter.ops)?;
    Ok(Some(Some(encoded)))
}

fn try_native_multi_root_nested_vec_delta_diff(
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
    if base_obj.len() != next_obj.len() {
        return Ok(None);
    }
    if base_obj.keys().any(|k| !next_obj.contains_key(k)) {
        return Ok(None);
    }
    let changed: Vec<&String> = base_obj
        .iter()
        .filter_map(|(k, v)| (next_obj.get(k) != Some(v)).then_some(k))
        .collect();
    if changed.len() < 2 {
        return Ok(None);
    }

    let runtime = match RuntimeModel::from_model_binary(base_model_binary) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    let (_, base_time) = match first_logical_clock_sid_time(base_model_binary) {
        Some(v) => v,
        None => return Ok(None),
    };
    let mut emitter = NativeEmitter::new(patch_sid, base_time.saturating_add(1));

    for (root_key, next_child) in next_obj {
        let base_child = match base_obj.get(root_key) {
            Some(v) => v,
            None => return Ok(None),
        };
        if base_child == next_child {
            continue;
        }
        let (sub_path, _old, new) = match (base_child.as_object(), next_child.as_object()) {
            (Some(bm), Some(nm)) => match find_single_array_delta_path(bm, nm) {
                Some(v) => v,
                None => return Ok(None),
            },
            _ => return Ok(None),
        };
        if sub_path.is_empty() {
            return Ok(None);
        }
        let mut node = match runtime.root_object_field(root_key) {
            Some(id) => id,
            None => return Ok(None),
        };
        for seg in sub_path {
            node = match runtime.object_field(node, &seg) {
                Some(id) => id,
                None => return Ok(None),
            };
        }
        if !runtime.node_is_vec(node) {
            return Ok(None);
        }
        emit_vec_delta_ops(&runtime, &mut emitter, node, new);
    }

    if emitter.ops.is_empty() {
        return Ok(Some(None));
    }
    let encoded = encode_patch_from_ops(patch_sid, base_time.saturating_add(1), &emitter.ops)?;
    Ok(Some(Some(encoded)))
}

fn emit_vec_delta_ops(
    runtime: &RuntimeModel,
    emitter: &mut NativeEmitter,
    vec_node: Timestamp,
    dst: &[Value],
) {
    let src_len = runtime
        .vec_max_index(vec_node)
        .map(|m| m.saturating_add(1) as usize)
        .unwrap_or(0);
    let dst_len = dst.len();
    let min = src_len.min(dst_len);
    let mut edits: Vec<(u64, Timestamp)> = Vec::new();

    // Upstream diffVec: trim trailing src indexes by writing `undefined`.
    for i in dst_len..src_len {
        if let Some(child) = runtime.vec_index_value(vec_node, i as u64) {
            if runtime.node_is_deleted_or_missing(child) {
                continue;
            }
            let undef = emitter.next_id();
            emitter.push(DecodedOp::NewCon {
                id: undef,
                value: ConValue::Undef,
            });
            edits.push((i as u64, undef));
        }
    }

    // Upstream diffVec: update common indexes where recursive diff fails.
    for (i, value) in dst.iter().take(min).enumerate() {
        if let Some(child) = runtime.vec_index_value(vec_node, i as u64) {
            if runtime
                .node_json_value(child)
                .as_ref()
                .is_some_and(|v| v == value)
            {
                continue;
            }
        }
        edits.push((i as u64, emitter.emit_value(value)));
    }

    // Upstream diffVec: append new tail indexes.
    for (i, value) in dst.iter().enumerate().skip(src_len) {
        edits.push((i as u64, emitter.emit_value(value)));
    }

    if !edits.is_empty() {
        emitter.push(DecodedOp::InsVec {
            id: emitter.next_id(),
            obj: vec_node,
            data: edits,
        });
    }
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

fn is_array_native_supported(value: &Value) -> bool {
    is_con_scalar(value) || matches!(value, Value::String(_))
}

fn try_native_nested_obj_scalar_key_delta_diff(
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

    if base_obj.len() != next_obj.len() {
        return Ok(None);
    }
    if base_obj.keys().any(|k| !next_obj.contains_key(k)) {
        return Ok(None);
    }

    let changed: Vec<&String> = base_obj
        .iter()
        .filter_map(|(k, v)| (next_obj.get(k) != Some(v)).then_some(k))
        .collect();
    if changed.len() != 1 {
        return Ok(None);
    }
    let root_key = changed[0];
    let old = match base_obj.get(root_key) {
        Some(v) => v,
        None => return Ok(None),
    };
    let new = match next_obj.get(root_key) {
        Some(v) => v,
        None => return Ok(None),
    };

    let runtime = match RuntimeModel::from_model_binary(base_model_binary) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };

    let target_obj = match (old, new) {
        (Value::Object(old_obj), Value::Object(new_obj)) => {
            let obj_id = match runtime.root_object_field(root_key) {
                Some(id) if runtime.node_is_object(id) => id,
                _ => return Ok(None),
            };
            let (_changed_key, _new_con) = match object_single_scalar_key_delta(old_obj, new_obj) {
                Some(v) => v,
                None => return Ok(None),
            };
            obj_id
        }
        (Value::Array(old_arr), Value::Array(new_arr)) => {
            if old_arr.len() != new_arr.len() || old_arr.is_empty() {
                return Ok(None);
            }
            let mut changed_idx: Option<usize> = None;
            for i in 0..old_arr.len() {
                if old_arr[i] != new_arr[i] {
                    if changed_idx.is_some() {
                        return Ok(None);
                    }
                    changed_idx = Some(i);
                }
            }
            let idx = match changed_idx {
                Some(v) => v,
                None => return Ok(None),
            };
            let old_obj = match &old_arr[idx] {
                Value::Object(v) => v,
                _ => return Ok(None),
            };
            let new_obj = match &new_arr[idx] {
                Value::Object(v) => v,
                _ => return Ok(None),
            };
            let (_changed_key, _new_con) = match object_single_scalar_key_delta(old_obj, new_obj) {
                Some(v) => v,
                None => return Ok(None),
            };
            let arr_id = match runtime.root_object_field(root_key) {
                Some(id) if runtime.node_is_array(id) => id,
                _ => return Ok(None),
            };
            let values = match runtime.array_visible_values(arr_id) {
                Some(v) => v,
                None => return Ok(None),
            };
            if idx >= values.len() {
                return Ok(None);
            }
            let obj_id = values[idx];
            if !runtime.node_is_object(obj_id) {
                return Ok(None);
            }
            obj_id
        }
        _ => return Ok(None),
    };

    let (changed_key, new_con) = match (old, new) {
        (Value::Object(old_obj), Value::Object(new_obj)) => {
            object_single_scalar_key_delta(old_obj, new_obj).expect("checked above")
        }
        (Value::Array(old_arr), Value::Array(new_arr)) => {
            let idx = old_arr
                .iter()
                .zip(new_arr.iter())
                .position(|(a, b)| a != b)
                .expect("checked above");
            let old_obj = old_arr[idx].as_object().expect("checked above");
            let new_obj = new_arr[idx].as_object().expect("checked above");
            object_single_scalar_key_delta(old_obj, new_obj).expect("checked above")
        }
        _ => return Ok(None),
    };

    let (_, base_time) = match first_logical_clock_sid_time(base_model_binary) {
        Some(v) => v,
        None => return Ok(None),
    };
    let mut emitter = NativeEmitter::new(patch_sid, base_time.saturating_add(1));
    let con_id = emitter.next_id();
    emitter.push(DecodedOp::NewCon { id: con_id, value: new_con });
    emitter.push(DecodedOp::InsObj {
        id: emitter.next_id(),
        obj: target_obj,
        data: vec![(changed_key, con_id)],
    });
    let encoded = encode_patch_from_ops(patch_sid, base_time.saturating_add(1), &emitter.ops)?;
    Ok(Some(Some(encoded)))
}

fn try_native_nested_obj_generic_delta_diff(
    base_model_binary: &[u8],
    next_view: &Value,
    patch_sid: u64,
) -> Result<Option<Option<Vec<u8>>>, DiffError> {
    let model = match Model::from_binary(base_model_binary) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    let base_root = match model.view() {
        Value::Object(map) if !map.is_empty() => map,
        _ => return Ok(None),
    };
    let next_root = match next_view {
        Value::Object(map) => map,
        _ => return Ok(None),
    };
    if base_root.len() != next_root.len() {
        return Ok(None);
    }
    if base_root.keys().any(|k| !next_root.contains_key(k)) {
        return Ok(None);
    }

    let changed_root_keys: Vec<&String> = base_root
        .iter()
        .filter_map(|(k, v)| (next_root.get(k) != Some(v)).then_some(k))
        .collect();
    if changed_root_keys.len() != 1 {
        return Ok(None);
    }
    let root_key = changed_root_keys[0];
    let old_child = match base_root.get(root_key) {
        Some(v) => v,
        None => return Ok(None),
    };
    let new_child = match next_root.get(root_key) {
        Some(v) => v,
        None => return Ok(None),
    };
    let old_obj = match old_child {
        Value::Object(map) => map,
        _ => return Ok(None),
    };
    let new_obj = match new_child {
        Value::Object(map) => map,
        _ => return Ok(None),
    };

    let runtime = match RuntimeModel::from_model_binary(base_model_binary) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    let target_obj = match runtime.root_object_field(root_key) {
        Some(id) if runtime.node_is_object(id) => id,
        _ => return Ok(None),
    };
    let (_, base_time) = match first_logical_clock_sid_time(base_model_binary) {
        Some(v) => v,
        None => return Ok(None),
    };

    let mut emitter = NativeEmitter::new(patch_sid, base_time.saturating_add(1));
    let mut pairs: Vec<(String, Timestamp)> = Vec::new();

    // Upstream JsonCrdtDiff.diffObj ordering for nested object: source-key
    // deletion writes first, then destination-key inserts/updates.
    for (k, _) in old_obj {
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
        if old_obj.get(k) == Some(v) {
            continue;
        }
        let id = emitter.emit_value(v);
        pairs.push((k.clone(), id));
    }
    if pairs.is_empty() {
        return Ok(Some(None));
    }

    emitter.push(DecodedOp::InsObj {
        id: emitter.next_id(),
        obj: target_obj,
        data: pairs,
    });
    let encoded = encode_patch_from_ops(patch_sid, base_time.saturating_add(1), &emitter.ops)?;
    Ok(Some(Some(encoded)))
}

fn try_native_multi_root_nested_obj_generic_delta_diff(
    base_model_binary: &[u8],
    next_view: &Value,
    patch_sid: u64,
) -> Result<Option<Option<Vec<u8>>>, DiffError> {
    let model = match Model::from_binary(base_model_binary) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    let base_root = match model.view() {
        Value::Object(map) if !map.is_empty() => map,
        _ => return Ok(None),
    };
    let next_root = match next_view {
        Value::Object(map) => map,
        _ => return Ok(None),
    };
    if base_root.len() != next_root.len() {
        return Ok(None);
    }
    if base_root.keys().any(|k| !next_root.contains_key(k)) {
        return Ok(None);
    }

    let changed_root_keys: Vec<&String> = base_root
        .iter()
        .filter_map(|(k, v)| (next_root.get(k) != Some(v)).then_some(k))
        .collect();
    if changed_root_keys.len() < 2 {
        return Ok(None);
    }

    let runtime = match RuntimeModel::from_model_binary(base_model_binary) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    let (_, base_time) = match first_logical_clock_sid_time(base_model_binary) {
        Some(v) => v,
        None => return Ok(None),
    };
    let mut emitter = NativeEmitter::new(patch_sid, base_time.saturating_add(1));

    // Upstream diffObj recursion follows destination key order.
    for (root_key, new_child) in next_root {
        let old_child = match base_root.get(root_key) {
            Some(v) => v,
            None => return Ok(None),
        };
        if old_child == new_child {
            continue;
        }
        let old_obj = match old_child {
            Value::Object(map) => map,
            _ => return Ok(None),
        };
        let new_obj = match new_child {
            Value::Object(map) => map,
            _ => return Ok(None),
        };
        let target_obj = match runtime.root_object_field(root_key) {
            Some(id) if runtime.node_is_object(id) => id,
            _ => return Ok(None),
        };

        let mut pairs: Vec<(String, Timestamp)> = Vec::new();
        for (k, _) in old_obj {
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
            if old_obj.get(k) == Some(v) {
                continue;
            }
            let id = emitter.emit_value(v);
            pairs.push((k.clone(), id));
        }
        if !pairs.is_empty() {
            emitter.push(DecodedOp::InsObj {
                id: emitter.next_id(),
                obj: target_obj,
                data: pairs,
            });
        }
    }

    if emitter.ops.is_empty() {
        return Ok(Some(None));
    }
    let encoded = encode_patch_from_ops(patch_sid, base_time.saturating_add(1), &emitter.ops)?;
    Ok(Some(Some(encoded)))
}

fn try_native_nested_obj_string_delta_diff(
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

    let (path, old, new) = match find_single_string_delta_path(base_obj, next_obj) {
        Some(v) => v,
        None => return Ok(None),
    };
    if path.is_empty() {
        return Ok(None);
    }

    let runtime = match RuntimeModel::from_model_binary(base_model_binary) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };

    let mut node = match runtime.root_object_field(&path[0]) {
        Some(id) => id,
        None => return Ok(None),
    };
    for seg in path.iter().skip(1) {
        node = match runtime.object_field(node, seg) {
            Some(id) => id,
            None => return Ok(None),
        };
    }
    if !runtime.node_is_string(node) {
        return Ok(None);
    }
    let slots = match runtime.string_visible_slots(node) {
        Some(v) => v,
        None => return Ok(None),
    };
    let encoded = match emit_string_delta_patch(base_model_binary, patch_sid, node, &slots, old, new)? {
        Some(v) => v,
        None => return Ok(Some(None)),
    };
    Ok(Some(Some(encoded)))
}

fn try_native_multi_root_nested_string_delta_diff(
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
    if base_obj.len() != next_obj.len() {
        return Ok(None);
    }
    if base_obj.keys().any(|k| !next_obj.contains_key(k)) {
        return Ok(None);
    }

    let changed: Vec<&String> = base_obj
        .iter()
        .filter_map(|(k, v)| (next_obj.get(k) != Some(v)).then_some(k))
        .collect();
    if changed.len() < 2 {
        return Ok(None);
    }

    let runtime = match RuntimeModel::from_model_binary(base_model_binary) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    let (_, base_time) = match first_logical_clock_sid_time(base_model_binary) {
        Some(v) => v,
        None => return Ok(None),
    };
    let mut emitter = NativeEmitter::new(patch_sid, base_time.saturating_add(1));

    for (root_key, next_child) in next_obj {
        let base_child = match base_obj.get(root_key) {
            Some(v) => v,
            None => return Ok(None),
        };
        if base_child == next_child {
            continue;
        }
        let (sub_path, old, new) = match (base_child.as_object(), next_child.as_object()) {
            (Some(bm), Some(nm)) => match find_single_string_delta_path(bm, nm) {
                Some(v) => v,
                None => return Ok(None),
            },
            _ => return Ok(None),
        };
        if sub_path.is_empty() {
            return Ok(None);
        }
        let mut node = match runtime.root_object_field(root_key) {
            Some(id) => id,
            None => return Ok(None),
        };
        for seg in sub_path {
            node = match runtime.object_field(node, &seg) {
                Some(id) => id,
                None => return Ok(None),
            };
        }
        if !runtime.node_is_string(node) {
            return Ok(None);
        }
        let slots = match runtime.string_visible_slots(node) {
            Some(v) => v,
            None => return Ok(None),
        };
        let old_chars: Vec<char> = old.chars().collect();
        if old_chars.len() != slots.len() {
            return Ok(None);
        }
        let encoded = emit_string_delta_patch(base_model_binary, patch_sid, node, &slots, old, new)?;
        if encoded.is_none() {
            continue;
        }
        // Re-emit ops directly into our shared emitter to preserve one patch.
        let mut lcp = 0usize;
        let new_chars: Vec<char> = new.chars().collect();
        while lcp < old_chars.len() && lcp < new_chars.len() && old_chars[lcp] == new_chars[lcp] {
            lcp += 1;
        }
        let mut lcs = 0usize;
        while lcs < (old_chars.len() - lcp)
            && lcs < (new_chars.len() - lcp)
            && old_chars[old_chars.len() - 1 - lcs] == new_chars[new_chars.len() - 1 - lcs]
        {
            lcs += 1;
        }
        let del_len = old_chars.len().saturating_sub(lcp + lcs);
        let ins: String = new_chars[lcp..new_chars.len().saturating_sub(lcs)]
            .iter()
            .collect();
        let ins_len = ins.chars().count();
        if ins_len > 0 {
            let reference = if lcp == 0 {
                slots.first().copied().unwrap_or(node)
            } else {
                slots[lcp - 1]
            };
            emitter.push(DecodedOp::InsStr {
                id: emitter.next_id(),
                obj: node,
                reference,
                data: ins,
            });
        }
        if del_len > 0 {
            let del_slots = &slots[lcp..lcp + del_len];
            let mut spans: Vec<crate::patch::Timespan> = Vec::new();
            for slot in del_slots {
                if let Some(last) = spans.last_mut() {
                    if last.sid == slot.sid && last.time + last.span == slot.time {
                        last.span += 1;
                        continue;
                    }
                }
                spans.push(crate::patch::Timespan {
                    sid: slot.sid,
                    time: slot.time,
                    span: 1,
                });
            }
            emitter.push(DecodedOp::Del {
                id: emitter.next_id(),
                obj: node,
                what: spans,
            });
        }
    }

    if emitter.ops.is_empty() {
        return Ok(Some(None));
    }
    let encoded = encode_patch_from_ops(patch_sid, base_time.saturating_add(1), &emitter.ops)?;
    Ok(Some(Some(encoded)))
}

fn emit_string_delta_patch(
    base_model_binary: &[u8],
    patch_sid: u64,
    str_node: Timestamp,
    slots: &[Timestamp],
    old: &str,
    new: &str,
) -> Result<Option<Vec<u8>>, DiffError> {
    let old_chars: Vec<char> = old.chars().collect();
    let new_chars: Vec<char> = new.chars().collect();
    if old_chars.len() != slots.len() {
        return Ok(None);
    }

    let mut lcp = 0usize;
    while lcp < old_chars.len() && lcp < new_chars.len() && old_chars[lcp] == new_chars[lcp] {
        lcp += 1;
    }
    let mut lcs = 0usize;
    while lcs < (old_chars.len() - lcp)
        && lcs < (new_chars.len() - lcp)
        && old_chars[old_chars.len() - 1 - lcs] == new_chars[new_chars.len() - 1 - lcs]
    {
        lcs += 1;
    }

    let del_len = old_chars.len().saturating_sub(lcp + lcs);
    let ins: String = new_chars[lcp..new_chars.len().saturating_sub(lcs)]
        .iter()
        .collect();
    let ins_len = ins.chars().count();

    if del_len == 0 && ins_len == 0 {
        return Ok(None);
    }

    let (_, base_time) = match first_logical_clock_sid_time(base_model_binary) {
        Some(v) => v,
        None => return Ok(None),
    };
    let mut emitter = NativeEmitter::new(patch_sid, base_time.saturating_add(1));

    if ins_len > 0 {
        let reference = if lcp == 0 {
            slots.first().copied().unwrap_or(str_node)
        } else {
            slots[lcp - 1]
        };
        emitter.push(DecodedOp::InsStr {
            id: emitter.next_id(),
            obj: str_node,
            reference,
            data: ins,
        });
    }
    if del_len > 0 {
        let del_slots = &slots[lcp..lcp + del_len];
        let mut spans: Vec<crate::patch::Timespan> = Vec::new();
        for slot in del_slots {
            if let Some(last) = spans.last_mut() {
                if last.sid == slot.sid && last.time + last.span == slot.time {
                    last.span += 1;
                    continue;
                }
            }
            spans.push(crate::patch::Timespan {
                sid: slot.sid,
                time: slot.time,
                span: 1,
            });
        }
        emitter.push(DecodedOp::Del {
            id: emitter.next_id(),
            obj: str_node,
            what: spans,
        });
    }

    let encoded = encode_patch_from_ops(patch_sid, base_time.saturating_add(1), &emitter.ops)?;
    Ok(Some(encoded))
}

fn emit_bin_delta_patch(
    base_model_binary: &[u8],
    patch_sid: u64,
    bin_node: Timestamp,
    slots: &[Timestamp],
    old: &[u8],
    new: &[u8],
) -> Result<Option<Vec<u8>>, DiffError> {
    if old.len() != slots.len() {
        return Ok(None);
    }

    let mut lcp = 0usize;
    while lcp < old.len() && lcp < new.len() && old[lcp] == new[lcp] {
        lcp += 1;
    }
    let mut lcs = 0usize;
    while lcs < (old.len() - lcp)
        && lcs < (new.len() - lcp)
        && old[old.len() - 1 - lcs] == new[new.len() - 1 - lcs]
    {
        lcs += 1;
    }

    let del_len = old.len().saturating_sub(lcp + lcs);
    let ins_bytes = &new[lcp..new.len().saturating_sub(lcs)];
    if del_len == 0 && ins_bytes.is_empty() {
        return Ok(None);
    }

    let (_, base_time) = match first_logical_clock_sid_time(base_model_binary) {
        Some(v) => v,
        None => return Ok(None),
    };
    let mut emitter = NativeEmitter::new(patch_sid, base_time.saturating_add(1));

    if !ins_bytes.is_empty() {
        let reference = if lcp == 0 { bin_node } else { slots[lcp - 1] };
        emitter.push(DecodedOp::InsBin {
            id: emitter.next_id(),
            obj: bin_node,
            reference,
            data: ins_bytes.to_vec(),
        });
    }

    if del_len > 0 {
        let del_slots = &slots[lcp..lcp + del_len];
        let mut spans: Vec<crate::patch::Timespan> = Vec::new();
        for slot in del_slots {
            if let Some(last) = spans.last_mut() {
                if last.sid == slot.sid && last.time + last.span == slot.time {
                    last.span += 1;
                    continue;
                }
            }
            spans.push(crate::patch::Timespan {
                sid: slot.sid,
                time: slot.time,
                span: 1,
            });
        }
        emitter.push(DecodedOp::Del {
            id: emitter.next_id(),
            obj: bin_node,
            what: spans,
        });
    }

    let encoded = encode_patch_from_ops(patch_sid, base_time.saturating_add(1), &emitter.ops)?;
    Ok(Some(encoded))
}

fn parse_bin_object(value: &Value) -> Option<Vec<u8>> {
    let obj = value.as_object()?;
    if obj.is_empty() {
        return Some(Vec::new());
    }
    let mut out = Vec::with_capacity(obj.len());
    for i in 0..obj.len() {
        let key = i.to_string();
        let n = obj.get(&key)?.as_u64()?;
        if n > 255 {
            return None;
        }
        out.push(n as u8);
    }
    Some(out)
}

fn find_single_string_delta_path<'a>(
    base: &'a serde_json::Map<String, Value>,
    next: &'a serde_json::Map<String, Value>,
) -> Option<(Vec<String>, &'a str, &'a str)> {
    if base.len() != next.len() {
        return None;
    }
    if base.keys().any(|k| !next.contains_key(k)) {
        return None;
    }

    let mut found: Option<(Vec<String>, &str, &str)> = None;
    for (k, base_v) in base {
        let next_v = next.get(k)?;
        if base_v == next_v {
            continue;
        }
        let delta = find_single_string_delta_value(base_v, next_v)?;
        if found.is_some() {
            return None;
        }
        let mut path = vec![k.clone()];
        path.extend(delta.0);
        found = Some((path, delta.1, delta.2));
    }
    found
}

fn find_single_string_delta_value<'a>(
    base: &'a Value,
    next: &'a Value,
) -> Option<(Vec<String>, &'a str, &'a str)> {
    match (base, next) {
        (Value::String(old), Value::String(new)) => Some((Vec::new(), old, new)),
        (Value::Object(bm), Value::Object(nm)) => {
            if bm.len() != nm.len() {
                return None;
            }
            if bm.keys().any(|k| !nm.contains_key(k)) {
                return None;
            }

            let mut found: Option<(Vec<String>, &str, &str)> = None;
            for (k, bv) in bm {
                let nv = nm.get(k)?;
                if bv == nv {
                    continue;
                }
                let delta = find_single_string_delta_value(bv, nv)?;
                if found.is_some() {
                    return None;
                }
                let mut path = vec![k.clone()];
                path.extend(delta.0);
                found = Some((path, delta.1, delta.2));
            }
            found
        }
        _ => None,
    }
}

fn find_single_array_delta_path<'a>(
    base: &'a serde_json::Map<String, Value>,
    next: &'a serde_json::Map<String, Value>,
) -> Option<(Vec<String>, &'a Vec<Value>, &'a Vec<Value>)> {
    if base.len() != next.len() {
        return None;
    }
    if base.keys().any(|k| !next.contains_key(k)) {
        return None;
    }

    let mut found: Option<(Vec<String>, &Vec<Value>, &Vec<Value>)> = None;
    for (k, base_v) in base {
        let next_v = next.get(k)?;
        if base_v == next_v {
            continue;
        }
        let delta = find_single_array_delta_value(base_v, next_v)?;
        if found.is_some() {
            return None;
        }
        let mut path = vec![k.clone()];
        path.extend(delta.0);
        found = Some((path, delta.1, delta.2));
    }
    found
}

fn find_single_array_delta_value<'a>(
    base: &'a Value,
    next: &'a Value,
) -> Option<(Vec<String>, &'a Vec<Value>, &'a Vec<Value>)> {
    match (base, next) {
        (Value::Array(old), Value::Array(new)) => Some((Vec::new(), old, new)),
        (Value::Object(bm), Value::Object(nm)) => {
            if bm.len() != nm.len() {
                return None;
            }
            if bm.keys().any(|k| !nm.contains_key(k)) {
                return None;
            }

            let mut found: Option<(Vec<String>, &Vec<Value>, &Vec<Value>)> = None;
            for (k, bv) in bm {
                let nv = nm.get(k)?;
                if bv == nv {
                    continue;
                }
                let delta = find_single_array_delta_value(bv, nv)?;
                if found.is_some() {
                    return None;
                }
                let mut path = vec![k.clone()];
                path.extend(delta.0);
                found = Some((path, delta.1, delta.2));
            }
            found
        }
        _ => None,
    }
}

fn find_single_bin_delta_path(
    base: &serde_json::Map<String, Value>,
    next: &serde_json::Map<String, Value>,
) -> Option<(Vec<String>, Vec<u8>, Vec<u8>)> {
    if base.len() != next.len() {
        return None;
    }
    if base.keys().any(|k| !next.contains_key(k)) {
        return None;
    }

    let mut found: Option<(Vec<String>, Vec<u8>, Vec<u8>)> = None;
    for (k, base_v) in base {
        let next_v = next.get(k)?;
        if base_v == next_v {
            continue;
        }
        let delta = find_single_bin_delta_value(base_v, next_v)?;
        if found.is_some() {
            return None;
        }
        let mut path = vec![k.clone()];
        path.extend(delta.0);
        found = Some((path, delta.1, delta.2));
    }
    found
}

fn find_single_bin_delta_value(
    base: &Value,
    next: &Value,
) -> Option<(Vec<String>, Vec<u8>, Vec<u8>)> {
    match (base, next) {
        (Value::Object(_), Value::Object(_)) => {
            if let (Some(old), Some(new)) = (parse_bin_object(base), parse_bin_object(next)) {
                return Some((Vec::new(), old, new));
            }

            let bm = base.as_object()?;
            let nm = next.as_object()?;
            if bm.len() != nm.len() {
                return None;
            }
            if bm.keys().any(|k| !nm.contains_key(k)) {
                return None;
            }

            let mut found: Option<(Vec<String>, Vec<u8>, Vec<u8>)> = None;
            for (k, bv) in bm {
                let nv = nm.get(k)?;
                if bv == nv {
                    continue;
                }
                let delta = find_single_bin_delta_value(bv, nv)?;
                if found.is_some() {
                    return None;
                }
                let mut path = vec![k.clone()];
                path.extend(delta.0);
                found = Some((path, delta.1, delta.2));
            }
            found
        }
        _ => None,
    }
}

fn object_single_scalar_key_delta(
    old: &serde_json::Map<String, Value>,
    new: &serde_json::Map<String, Value>,
) -> Option<(String, ConValue)> {
    let mut changed: Option<(String, ConValue)> = None;

    for (k, old_v) in old {
        match new.get(k) {
            Some(new_v) => {
                if old_v == new_v {
                    continue;
                }
                if !is_con_scalar(new_v) {
                    return None;
                }
                if changed.is_some() {
                    return None;
                }
                changed = Some((k.clone(), ConValue::Json(new_v.clone())));
            }
            None => {
                if changed.is_some() {
                    return None;
                }
                changed = Some((k.clone(), ConValue::Undef));
            }
        }
    }

    for (k, new_v) in new {
        if old.contains_key(k) {
            continue;
        }
        if !is_con_scalar(new_v) {
            return None;
        }
        if changed.is_some() {
            return None;
        }
        changed = Some((k.clone(), ConValue::Json(new_v.clone())));
    }

    changed
}
