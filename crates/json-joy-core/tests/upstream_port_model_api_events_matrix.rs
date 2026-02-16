use std::sync::{Arc, Mutex};

use json_joy_core::model_api::{ChangeEventOrigin, NativeModelApi, PathStep};
use serde_json::json;

#[test]
fn upstream_port_model_api_events_local_change_and_unsubscribe_matrix() {
    let mut api = NativeModelApi::from_model_binary(
        &json_joy_core::less_db_compat::model_to_binary(
            &json_joy_core::less_db_compat::create_model(&json!({"k":1}), 700_001).unwrap(),
        ),
        Some(700_001),
    )
    .unwrap();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let seen2 = Arc::clone(&seen);
    let id = api.on_change(move |ev| {
        seen2.lock().unwrap().push((ev.origin, ev.before, ev.after));
    });

    api.set(&[PathStep::Key("k".to_string())], json!(2))
        .unwrap();
    {
        let v = seen.lock().unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].0, ChangeEventOrigin::Local);
        assert_eq!(v[0].1, json!({"k":1}));
        assert_eq!(v[0].2, json!({"k":2}));
    }

    assert!(api.off_change(id));
    api.set(&[PathStep::Key("k".to_string())], json!(3))
        .unwrap();
    assert_eq!(seen.lock().unwrap().len(), 1);
}

#[test]
fn upstream_port_model_api_events_remote_origin_matrix() {
    let base = json_joy_core::less_db_compat::model_to_binary(
        &json_joy_core::less_db_compat::create_model(&json!({"k":1}), 701_001).unwrap(),
    );
    let mut api = NativeModelApi::from_model_binary(&base, Some(701_001)).unwrap();
    let next = json!({"k": 9});
    let patch = json_joy_core::diff_runtime::diff_model_to_patch_bytes(&base, &next, 900_123)
        .unwrap()
        .unwrap();
    let patch = json_joy_core::patch::Patch::from_binary(&patch).unwrap();

    let seen = Arc::new(Mutex::new(Vec::new()));
    let seen2 = Arc::clone(&seen);
    api.on_change(move |ev| {
        seen2.lock().unwrap().push(ev.origin);
    });

    api.apply_patch(&patch).unwrap();
    assert_eq!(
        seen.lock().unwrap().as_slice(),
        &[ChangeEventOrigin::Remote]
    );
}

#[test]
fn upstream_port_model_api_events_batch_fanout_matrix() {
    // Upstream mapping:
    // - json-crdt/model/api/fanout.ts merged/batched change propagation.
    let sid = 99003;
    let model = json_joy_core::less_db_compat::create_model(&json!({"n":1}), sid).unwrap();
    let binary = json_joy_core::less_db_compat::model_to_binary(&model);
    let mut api = NativeModelApi::from_model_binary(&binary, Some(sid)).unwrap();

    let seen = Arc::new(Mutex::new(Vec::new()));
    let seen_clone = Arc::clone(&seen);
    let sub = api.on_changes(move |ev| {
        seen_clone.lock().unwrap().push(ev);
    });

    let p1 = api.diff(&json!({"n":2})).unwrap().unwrap();
    let mut tmp = NativeModelApi::from_model_binary(&binary, Some(sid)).unwrap();
    tmp.apply_patch(&p1).unwrap();
    let p2 = tmp.diff(&json!({"n":3})).unwrap().unwrap();
    api.apply_batch(&[p1, p2]).unwrap();

    let events = seen.lock().unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].before, json!({"n":1}));
    assert_eq!(events[0].after, json!({"n":3}));
    assert_eq!(events[0].patch_ids.len(), 2);
    drop(events);

    assert!(api.off_changes(sub));
    api.apply_batch(&[]).unwrap();
    assert_eq!(seen.lock().unwrap().len(), 1);
}

#[test]
fn upstream_port_model_api_events_scoped_path_matrix() {
    // Upstream mapping:
    // - json-crdt/model/api/NodeEvents.ts path-scoped view change subscriptions.
    let sid = 99004;
    let model =
        json_joy_core::less_db_compat::create_model(&json!({"doc":{"a":1,"b":1}}), sid).unwrap();
    let binary = json_joy_core::less_db_compat::model_to_binary(&model);
    let mut api = NativeModelApi::from_model_binary(&binary, Some(sid)).unwrap();

    let seen = Arc::new(Mutex::new(Vec::new()));
    let seen_clone = Arc::clone(&seen);
    let sub = api.on_change_at(
        vec![PathStep::Key("doc".into()), PathStep::Key("a".into())],
        move |ev| {
            seen_clone.lock().unwrap().push((ev.before, ev.after));
        },
    );

    api.set(
        &[PathStep::Key("doc".into()), PathStep::Key("b".into())],
        json!(2),
    )
    .unwrap();
    api.set(
        &[PathStep::Key("doc".into()), PathStep::Key("a".into())],
        json!(3),
    )
    .unwrap();

    let events = seen.lock().unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].0, Some(json!(1)));
    assert_eq!(events[0].1, Some(json!(3)));
    drop(events);

    assert!(api.off_change(sub));
}
