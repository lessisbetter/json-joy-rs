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
    let (_, base_time) = match first_model_clock_sid_time(base_model_binary) {
        Some(v) => v,
        None => return Ok(None),
    };
    let mut emitter = NativeEmitter::new(patch_sid, base_time.saturating_add(1));

    match (base_view, next_view) {
        (Value::String(old), Value::String(new)) if runtime.resolve_string_node(root).is_some() => {
            let root = runtime.resolve_string_node(root).expect("checked is_some");
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
                let reference = choose_sequence_insert_reference(
                    &slots,
                    root,
                    lcp,
                    ins.chars().count(),
                    del_len,
                    old_chars.len(),
                );
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
        (Value::Array(old), Value::Array(new)) if runtime.resolve_array_node(root).is_some() => {
            let root = runtime.resolve_array_node(root).expect("checked is_some");
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
        (Value::Array(_), Value::Array(new)) if runtime.resolve_vec_node(root).is_some() => {
            let root = runtime.resolve_vec_node(root).expect("checked is_some");
            emit_vec_delta_ops(&runtime, &mut emitter, root, new)?;
        }
        (old, new) if runtime.resolve_bin_node(root).is_some() => {
            let root = runtime.resolve_bin_node(root).expect("checked is_some");
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
                let reference = choose_sequence_insert_reference(
                    &slots,
                    root,
                    lcp,
                    ins_bytes.len(),
                    del_len,
                    old_bin.len(),
                );
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
    let (root_sid, base_time) = match first_model_clock_sid_time(base_model_binary) {
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

    let (root_sid, base_time) = match first_model_clock_sid_time(base_model_binary) {
        Some(v) => v,
        None => return Ok(None),
    };

    // Constrain this fast path to scalar-only key replacements at root.
    // Structural/nested mutations are handled by broader native recursive
    // object diff paths.
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

fn try_native_root_replace_diff(
    base_model_binary: &[u8],
    next_view: &Value,
    patch_sid: u64,
) -> Result<Option<Option<Vec<u8>>>, DiffError> {
    let (_, base_time) = match first_model_clock_sid_time(base_model_binary) {
        Some(v) => v,
        None => return Ok(None),
    };

    let mut emitter = NativeEmitter::new(patch_sid, base_time.saturating_add(1));
    let next_root = emitter.emit_value(next_view);
    emitter.push(DecodedOp::InsVal {
        id: emitter.next_id(),
        obj: Timestamp { sid: 0, time: 0 },
        val: next_root,
    });
    let encoded = encode_patch_from_ops(patch_sid, base_time.saturating_add(1), &emitter.ops)?;
    Ok(Some(Some(encoded)))
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
            let obj_id = match runtime
                .root_object_field(root_key)
                .and_then(|id| runtime.resolve_object_node(id))
            {
                Some(id) => id,
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
            let arr_id = match runtime
                .root_object_field(root_key)
                .and_then(|id| runtime.resolve_array_node(id))
            {
                Some(id) => id,
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
            match runtime.resolve_object_node(obj_id) {
                Some(id) => id,
                None => return Ok(None),
            }
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

    let (_, base_time) = match first_model_clock_sid_time(base_model_binary) {
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

