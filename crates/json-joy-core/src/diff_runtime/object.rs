fn try_native_root_obj_recursive_diff(
    base_model_binary: &[u8],
    next_view: &Value,
    patch_sid: u64,
) -> Result<Option<Option<Vec<u8>>>, DiffError> {
    let model = match Model::from_binary(base_model_binary) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    let base_obj = match model.view() {
        Value::Object(map) => map,
        _ => return Ok(None),
    };
    let next_obj = match next_view {
        Value::Object(map) => map,
        _ => return Ok(None),
    };

    let runtime = match RuntimeModel::from_model_binary(base_model_binary) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    let root = match runtime.root_id().and_then(|id| runtime.resolve_object_node(id)) {
        Some(id) => id,
        None => return Ok(None),
    };
    let (_, base_time) = match first_model_clock_sid_time(base_model_binary) {
        Some(v) => v,
        None => return Ok(None),
    };
    let mut emitter = NativeEmitter::new(patch_sid, base_time.saturating_add(1));
    let _ = try_emit_object_recursive_diff(&runtime, &mut emitter, root, base_obj, next_obj)?;
    if emitter.ops.is_empty() {
        return Ok(Some(None));
    }
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
    let (_, base_time) = match first_model_clock_sid_time(base_model_binary) {
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

    let (root_sid, base_time) = match first_model_clock_sid_time(base_model_binary) {
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
    let target_obj = match runtime
        .root_object_field(root_key)
        .and_then(|id| runtime.resolve_object_node(id))
    {
        Some(id) => id,
        _ => return Ok(None),
    };
    let (_, base_time) = match first_model_clock_sid_time(base_model_binary) {
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
    let (_, base_time) = match first_model_clock_sid_time(base_model_binary) {
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
        let target_obj = match runtime
            .root_object_field(root_key)
            .and_then(|id| runtime.resolve_object_node(id))
        {
            Some(id) => id,
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

