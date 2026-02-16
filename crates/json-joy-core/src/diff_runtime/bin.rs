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
    let bin_node = match runtime
        .root_object_field(key)
        .and_then(|id| runtime.resolve_bin_node(id))
    {
        Some(id) => id,
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
    let (_, base_time) = match first_model_clock_sid_time(base_model_binary) {
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
        let bin_node = match runtime
            .root_object_field(k)
            .and_then(|id| runtime.resolve_bin_node(id))
        {
            Some(id) => id,
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
            let reference = choose_sequence_insert_reference(
                &slots,
                bin_node,
                lcp,
                ins_bytes.len(),
                del_len,
                old.len(),
            );
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
    node = match runtime.resolve_bin_node(node) {
        Some(id) => id,
        None => return Ok(None),
    };
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
        let mut deltas = Vec::new();
        if !collect_bin_delta_paths(base_child, next_child, Vec::new(), &mut deltas) {
            return Ok(None);
        }
        if deltas.is_empty() {
            continue;
        }
        for (sub_path, old, new) in deltas {
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
            node = match runtime.resolve_bin_node(node) {
                Some(id) => id,
                None => return Ok(None),
            };
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
                let reference = choose_sequence_insert_reference(
                    &slots,
                    node,
                    lcp,
                    ins_bytes.len(),
                    del_len,
                    old.len(),
                );
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
    }

    if emitter.ops.is_empty() {
        return Ok(Some(None));
    }
    let encoded = encode_patch_from_ops(patch_sid, base_time.saturating_add(1), &emitter.ops)?;
    Ok(Some(Some(encoded)))
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

    let (_, base_time) = match first_model_clock_sid_time(base_model_binary) {
        Some(v) => v,
        None => return Ok(None),
    };
    let mut emitter = NativeEmitter::new(patch_sid, base_time.saturating_add(1));

    if !ins_bytes.is_empty() {
        let reference = choose_sequence_insert_reference(
            slots,
            bin_node,
            lcp,
            ins_bytes.len(),
            del_len,
            old.len(),
        );
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

fn collect_bin_delta_paths(
    base: &Value,
    next: &Value,
    prefix: Vec<String>,
    out: &mut Vec<(Vec<String>, Vec<u8>, Vec<u8>)>,
) -> bool {
    if let (Some(old), Some(new)) = (parse_bin_object(base), parse_bin_object(next)) {
        if old != new {
            out.push((prefix, old, new));
        }
        return true;
    }
    match (base, next) {
        (Value::Object(bm), Value::Object(nm)) => {
            if bm.len() != nm.len() {
                return false;
            }
            if bm.keys().any(|k| !nm.contains_key(k)) {
                return false;
            }
            for (k, bv) in bm {
                let nv = match nm.get(k) {
                    Some(v) => v,
                    None => return false,
                };
                if bv == nv {
                    continue;
                }
                let mut next_prefix = prefix.clone();
                next_prefix.push(k.clone());
                if !collect_bin_delta_paths(bv, nv, next_prefix, out) {
                    return false;
                }
            }
            true
        }
        _ => false,
    }
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

