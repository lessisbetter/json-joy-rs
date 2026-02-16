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
    let (_, base_time) = match first_model_clock_sid_time(base_model_binary) {
        Some(v) => v,
        None => return Ok(None),
    };
    let mut emitter = NativeEmitter::new(patch_sid, base_time.saturating_add(1));

    for key in changed {
        let dst = match next_obj.get(key) {
            Some(Value::Array(arr)) => arr,
            _ => return Ok(None),
        };
        let vec_node = match runtime
            .root_object_field(key)
            .and_then(|id| runtime.resolve_vec_node(id))
        {
            Some(id) => id,
            _ => return Ok(None),
        };

        emit_vec_delta_ops(&runtime, &mut emitter, vec_node, dst)?;
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
    let (_, base_time) = match first_model_clock_sid_time(base_model_binary) {
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
        let vec_node = match runtime
            .root_object_field(k)
            .and_then(|id| runtime.resolve_vec_node(id))
        {
            Some(id) => id,
            _ => return Ok(None),
        };
        emit_vec_delta_ops(&runtime, &mut emitter, vec_node, dst)?;
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
    node = match runtime.resolve_vec_node(node) {
        Some(id) => id,
        None => return Ok(None),
    };

    let (_, base_time) = match first_model_clock_sid_time(base_model_binary) {
        Some(v) => v,
        None => return Ok(None),
    };
    let mut emitter = NativeEmitter::new(patch_sid, base_time.saturating_add(1));
    emit_vec_delta_ops(&runtime, &mut emitter, node, new)?;
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
        node = match runtime.resolve_vec_node(node) {
            Some(id) => id,
            None => return Ok(None),
        };
        emit_vec_delta_ops(&runtime, &mut emitter, node, new)?;
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
) -> Result<(), DiffError> {
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
            let old_json = runtime.node_json_value(child);
            if let (Some(old_obj), Some(new_obj)) = (
                old_json.as_ref().and_then(|v| v.as_object()),
                value.as_object(),
            ) {
                if let Some(obj_child) = runtime.resolve_object_node(child) {
                    let _ = try_emit_object_recursive_diff(
                        runtime,
                        emitter,
                        obj_child,
                        old_obj,
                        new_obj,
                    )?;
                    continue;
                }
            }
            if old_json
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
    Ok(())
}

