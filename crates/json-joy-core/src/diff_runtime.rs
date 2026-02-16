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

    // Native logical empty-object root path.
    if let Some(native) = try_native_empty_obj_diff(base_model_binary, next_view, sid)? {
        return Ok(native);
    }
    if let Some(native) = try_native_root_obj_string_delta_diff(base_model_binary, next_view, sid)? {
        return Ok(native);
    }
    if let Some(native) = try_native_nested_obj_string_delta_diff(base_model_binary, next_view, sid)? {
        return Ok(native);
    }
    if let Some(native) = try_native_root_obj_array_delta_diff(base_model_binary, next_view, sid)? {
        return Ok(native);
    }
    if let Some(native) = try_native_nested_obj_scalar_key_delta_diff(base_model_binary, next_view, sid)? {
        return Ok(native);
    }
    // Native non-empty root-object scalar delta path (add/update/remove).
    if let Some(native) = try_native_root_obj_scalar_delta_diff(base_model_binary, next_view, sid)? {
        return Ok(native);
    }
    Err(DiffError::UnsupportedShape)
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
