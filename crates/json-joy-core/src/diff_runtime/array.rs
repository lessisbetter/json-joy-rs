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
    let arr_node = match runtime
        .root_object_field(key)
        .and_then(|id| runtime.resolve_array_node(id))
    {
        Some(id) => id,
        _ => return Ok(None),
    };
    let slots = match runtime.array_visible_slots(arr_node) {
        Some(v) => v,
        None => return Ok(None),
    };
    let values = match runtime.array_visible_values(arr_node) {
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

    let (_, base_time) = match first_model_clock_sid_time(base_model_binary) {
        Some(v) => v,
        None => return Ok(None),
    };
    let mut emitter = NativeEmitter::new(patch_sid, base_time.saturating_add(1));

    if try_emit_array_indexwise_diff(&runtime, &mut emitter, arr_node, &slots, &values, old, new)? {
        if emitter.ops.is_empty() {
            return Ok(Some(None));
        }
        let encoded = encode_patch_from_ops(patch_sid, base_time.saturating_add(1), &emitter.ops)?;
        return Ok(Some(Some(encoded)));
    }

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
    let (_, base_time) = match first_model_clock_sid_time(base_model_binary) {
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
        let arr_node = match runtime
            .root_object_field(key)
            .and_then(|id| runtime.resolve_array_node(id))
        {
            Some(id) => id,
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
    node = match runtime.resolve_array_node(node) {
        Some(id) => id,
        None => return Ok(None),
    };
    let slots = match runtime.array_visible_slots(node) {
        Some(v) => v,
        None => return Ok(None),
    };
    if slots.len() != old.len() {
        return Ok(None);
    }

    let (_, base_time) = match first_model_clock_sid_time(base_model_binary) {
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
    let (_, base_time) = match first_model_clock_sid_time(base_model_binary) {
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
        node = match runtime.resolve_array_node(node) {
            Some(id) => id,
            None => return Ok(None),
        };
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

fn try_emit_array_indexwise_diff(
    runtime: &RuntimeModel,
    emitter: &mut NativeEmitter,
    arr_node: Timestamp,
    slots: &[Timestamp],
    values: &[Timestamp],
    old: &[Value],
    new: &[Value],
) -> Result<bool, DiffError> {
    if old.len() != values.len() || old.len() != slots.len() {
        return Ok(false);
    }
    // Keep this path aligned to append-or-update semantics only. For shrinking
    // arrays upstream diff tends to emit direct deletions rather than index
    // rewrites plus tail delete.
    if new.len() < old.len() {
        return Ok(false);
    }
    let mut changed = false;
    let overlap = old.len().min(new.len());

    for i in 0..overlap {
        if old[i] == new[i] {
            continue;
        }
        let child = values[i];
        if runtime.node_is_val(child) {
            // Upstream array diffs recurse for object children but otherwise
            // prefer array-level edits (`ins_arr`/`del`) over `ins_val`
            // rewrites for element replacement.
            if let Some(inner) = runtime.val_child(child) {
                if try_emit_child_recursive_diff(runtime, emitter, inner, Some(&old[i]), &new[i])? {
                    changed = true;
                    continue;
                }
            }
            return Ok(false);
        }
        if try_emit_child_recursive_diff(runtime, emitter, child, Some(&old[i]), &new[i])? {
            changed = true;
            continue;
        }
        return Ok(false);
    }

    if new.len() > old.len() {
        let extras = &new[old.len()..];
        if !extras.is_empty() {
            let mut ids = Vec::with_capacity(extras.len());
            for item in extras {
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
            let reference = slots.last().copied().unwrap_or(arr_node);
            emitter.push(DecodedOp::InsArr {
                id: emitter.next_id(),
                obj: arr_node,
                reference,
                data: ids,
            });
            changed = true;
        }
    } else if old.len() > new.len() {
        let del_slots = &slots[new.len()..old.len()];
        if !del_slots.is_empty() {
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
            changed = true;
        }
    }

    Ok(changed)
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

