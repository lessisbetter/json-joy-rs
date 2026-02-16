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
    let str_node = match runtime
        .root_object_field(key)
        .and_then(|id| runtime.resolve_string_node(id))
    {
        Some(id) => id,
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
    let (_, base_time) = match first_model_clock_sid_time(base_model_binary) {
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
        let str_node = match runtime
            .root_object_field(k)
            .and_then(|id| runtime.resolve_string_node(id))
        {
            Some(id) => id,
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
            let reference = choose_sequence_insert_reference(
                &slots,
                str_node,
                lcp,
                ins_len,
                del_len,
                old_chars.len(),
            );
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

fn try_native_root_obj_string_with_keyset_delta_diff(
    base_model_binary: &[u8],
    patch_sid: u64,
    next_view: &Value,
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

    let changed_existing: Vec<&String> = base_obj
        .iter()
        .filter_map(|(k, v)| next_obj.get(k).filter(|nv| *nv != v).map(|_| k))
        .collect();
    if changed_existing.len() != 1 {
        return Ok(None);
    }
    let changed_key = changed_existing[0];
    let old = match base_obj.get(changed_key) {
        Some(Value::String(s)) => s,
        _ => return Ok(None),
    };
    let new = match next_obj.get(changed_key) {
        Some(Value::String(s)) => s,
        _ => return Ok(None),
    };

    let runtime = match RuntimeModel::from_model_binary(base_model_binary) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    let str_node = match runtime
        .root_object_field(changed_key)
        .and_then(|id| runtime.resolve_string_node(id))
    {
        Some(id) => id,
        None => return Ok(None),
    };
    let slots = match runtime.string_visible_slots(str_node) {
        Some(v) => v,
        None => return Ok(None),
    };
    if old.chars().count() != slots.len() {
        return Ok(None);
    }

    let (_, base_time) = match first_model_clock_sid_time(base_model_binary) {
        Some(v) => v,
        None => return Ok(None),
    };
    let mut emitter = NativeEmitter::new(patch_sid, base_time.saturating_add(1));

    // Upstream diffObj ordering:
    // 1) source-key deletions (`undefined` writes),
    // 2) per-destination-key updates (including nested string diff),
    // 3) ins_obj write with collected key updates.
    let mut pairs: Vec<(String, Timestamp)> = Vec::new();
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

    for (k, v) in next_obj {
        if k == changed_key {
            let old_chars: Vec<char> = old.chars().collect();
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
            let ins_len = ins.chars().count();

            if ins_len > 0 {
                let reference = choose_sequence_insert_reference(
                    &slots,
                    str_node,
                    lcp,
                    ins_len,
                    del_len,
                    old_chars.len(),
                );
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
            continue;
        }
        if base_obj.get(k) == Some(v) {
            continue;
        }
        let id = emitter.emit_value(v);
        pairs.push((k.clone(), id));
    }
    if !pairs.is_empty() {
        let root = match runtime.root_id() {
            Some(id) => id,
            None => return Ok(None),
        };
        emitter.push(DecodedOp::InsObj {
            id: emitter.next_id(),
            obj: root,
            data: pairs,
        });
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
    node = match runtime.resolve_string_node(node) {
        Some(id) => id,
        None => return Ok(None),
    };
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
        if !collect_string_delta_paths(base_child, next_child, Vec::new(), &mut deltas) {
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
            node = match runtime.resolve_string_node(node) {
                Some(id) => id,
                None => return Ok(None),
            };
            let slots = match runtime.string_visible_slots(node) {
                Some(v) => v,
                None => return Ok(None),
            };
            let old_chars: Vec<char> = old.chars().collect();
            if old_chars.len() != slots.len() {
                return Ok(None);
            }
            let encoded = emit_string_delta_patch(base_model_binary, patch_sid, node, &slots, &old, &new)?;
            if encoded.is_none() {
                continue;
            }
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
                let reference = choose_sequence_insert_reference(
                    &slots,
                    node,
                    lcp,
                    ins_len,
                    del_len,
                    old_chars.len(),
                );
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

    let (_, base_time) = match first_model_clock_sid_time(base_model_binary) {
        Some(v) => v,
        None => return Ok(None),
    };
    let mut emitter = NativeEmitter::new(patch_sid, base_time.saturating_add(1));

    if ins_len > 0 {
        let reference = choose_sequence_insert_reference(
            slots,
            str_node,
            lcp,
            ins_len,
            del_len,
            old_chars.len(),
        );
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

fn collect_string_delta_paths(
    base: &Value,
    next: &Value,
    prefix: Vec<String>,
    out: &mut Vec<(Vec<String>, String, String)>,
) -> bool {
    match (base, next) {
        (Value::String(old), Value::String(new)) => {
            if old != new {
                out.push((prefix, old.clone(), new.clone()));
            }
            true
        }
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
                if !collect_string_delta_paths(bv, nv, next_prefix, out) {
                    return false;
                }
            }
            true
        }
        _ => false,
    }
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

