use json_joy_core::diff_runtime::diff_runtime_to_patch_bytes;
use json_joy_core::less_db_compat::{create_model, model_to_binary};
use json_joy_core::model_runtime::RuntimeModel;
use json_joy_core::patch::Patch;

fn make_initial_doc() -> serde_json::Value {
    serde_json::json!({
        "id": "rec-1",
        "title": "Draft",
        "body": "hello",
        "tags": ["a", "b"],
        "counters": {"views": 0, "edits": 0},
        "flags": {"archived": false, "starred": false},
        "items": [{"id": 1, "done": false}, {"id": 2, "done": true}],
        "nested": {"s": "ab", "v": [1, 2, 3]},
    })
}

fn mutate_doc(prev: &serde_json::Value, step: u32) -> serde_json::Value {
    let mut next = prev.clone();
    let obj = next.as_object_mut().expect("doc must be object");
    obj.insert("title".into(), serde_json::json!(format!("Draft-{}", step % 17)));

    let body = obj["body"].as_str().expect("body must be string");
    let mut next_body: String = body.chars().take(20).collect();
    next_body.push((b'a' + (step % 26) as u8) as char);
    obj.insert("body".into(), serde_json::json!(next_body));

    obj["counters"]["views"] = serde_json::json!(obj["counters"]["views"].as_u64().unwrap_or(0) + 1);
    obj["counters"]["edits"] = serde_json::json!(obj["counters"]["edits"].as_u64().unwrap_or(0) + (step % 3) as u64);
    obj["flags"]["starred"] = serde_json::json!(step.is_multiple_of(2));
    if step.is_multiple_of(5) {
        let archived = obj["flags"]["archived"].as_bool().unwrap_or(false);
        obj["flags"]["archived"] = serde_json::json!(!archived);
    }

    let tags = obj["tags"].as_array_mut().expect("tags must be array");
    tags.push(serde_json::json!(format!("t{}", step % 9)));
    if tags.len() > 8 {
        tags.remove(0);
    }

    let items = obj["items"].as_array_mut().expect("items must be array");
    items.push(serde_json::json!({"id": 1000 + step, "done": !step.is_multiple_of(2)}));
    if items.len() > 10 {
        items.remove(0);
    }

    let nested_s = obj["nested"]["s"].as_str().expect("nested.s must be string");
    let mut s = nested_s.to_string();
    s.push((b'0' + (step % 10) as u8) as char);
    if s.chars().count() > 20 {
        let keep: String = s.chars().rev().take(20).collect();
        s = keep.chars().rev().collect();
    }
    obj["nested"]["s"] = serde_json::json!(s);

    let v = obj["nested"]["v"].as_array_mut().expect("nested.v must be array");
    let idx = ((step + 1) as usize) % v.len();
    v[idx] = serde_json::json!((step * 7) % 101);

    next
}

#[test]
fn upstream_port_diff_runtime_stateful_matrix_stays_valid_and_reaches_target_view() {
    let sid = 65_536u64;
    let initial = make_initial_doc();
    let model = create_model(&initial, sid).expect("create_model must succeed");
    let mut runtime = RuntimeModel::from_model_binary(&model_to_binary(&model))
        .expect("runtime decode must succeed");

    let mut current = initial;
    for step in 1..=120u32 {
        let next = mutate_doc(&current, step);
        let patch = diff_runtime_to_patch_bytes(&runtime, &next, sid)
            .expect("runtime diff should succeed");

        if let Some(bytes) = patch {
            let decoded = Patch::from_binary(&bytes).expect("generated patch must decode");
            runtime
                .apply_patch(&decoded)
                .expect("runtime apply must succeed");
            let model = runtime
                .to_model_binary_like()
                .expect("runtime must remain model-encodable");
            let redecoded = RuntimeModel::from_model_binary(&model)
                .expect("model produced by runtime must decode");
            assert_eq!(redecoded.view_json(), next);
        } else {
            assert_eq!(runtime.view_json(), next);
        }
        current = next;
    }
}
