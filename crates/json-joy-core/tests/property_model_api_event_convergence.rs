use std::sync::{Arc, Mutex};

use json_joy_core::less_db_compat::{create_model, model_to_binary};
use json_joy_core::model_api::{NativeModelApi, PathStep};
use serde_json::json;

#[test]
fn property_model_api_event_convergence_for_batched_vs_incremental_apply() {
    let sid = 93001;
    let base = json!({"doc":{"title":"a","count":1},"items":[1]});
    let compat = create_model(&base, sid).expect("create_model must succeed");
    let base_binary = model_to_binary(&compat);

    let targets = vec![
        json!({"doc":{"title":"A","count":1},"items":[1]}),
        json!({"doc":{"title":"A","count":2},"items":[1,2]}),
        json!({"doc":{"title":"AZ","count":2},"items":[2]}),
    ];

    let mut builder = NativeModelApi::from_model_binary(&base_binary, Some(sid)).unwrap();
    let mut patches = Vec::new();
    for next in &targets {
        if let Some(patch) = builder.diff(next).unwrap() {
            builder.apply_patch(&patch).unwrap();
            patches.push(patch);
        }
    }

    let mut incremental = NativeModelApi::from_model_binary(&base_binary, Some(sid)).unwrap();
    let mut batched = NativeModelApi::from_model_binary(&base_binary, Some(sid)).unwrap();

    let inc_change = Arc::new(Mutex::new(0usize));
    let inc_change_clone = Arc::clone(&inc_change);
    incremental.on_change(move |_| {
        *inc_change_clone.lock().unwrap() += 1;
    });

    let inc_scoped = Arc::new(Mutex::new(0usize));
    let inc_scoped_clone = Arc::clone(&inc_scoped);
    incremental.on_change_at(
        vec![PathStep::Key("doc".into()), PathStep::Key("title".into())],
        move |_| {
            *inc_scoped_clone.lock().unwrap() += 1;
        },
    );

    let bat_change = Arc::new(Mutex::new(0usize));
    let bat_change_clone = Arc::clone(&bat_change);
    batched.on_change(move |_| {
        *bat_change_clone.lock().unwrap() += 1;
    });

    let bat_scoped = Arc::new(Mutex::new(0usize));
    let bat_scoped_clone = Arc::clone(&bat_scoped);
    batched.on_change_at(
        vec![PathStep::Key("doc".into()), PathStep::Key("title".into())],
        move |_| {
            *bat_scoped_clone.lock().unwrap() += 1;
        },
    );

    for p in &patches {
        incremental.apply_patch(p).unwrap();
    }
    batched.apply_batch(&patches).unwrap();

    assert_eq!(
        incremental.view(),
        batched.view(),
        "views diverged for event convergence flow"
    );
    assert_eq!(
        incremental.view(),
        builder.view(),
        "builder final view mismatch"
    );

    let inc_changes = *inc_change.lock().unwrap();
    let bat_changes = *bat_change.lock().unwrap();
    let inc_scoped_changes = *inc_scoped.lock().unwrap();
    let bat_scoped_changes = *bat_scoped.lock().unwrap();

    assert!(
        inc_changes >= patches.len(),
        "incremental should see at least one change per patch"
    );
    assert!(bat_changes >= 1, "batched apply should emit change events");
    assert_eq!(
        inc_scoped_changes, bat_scoped_changes,
        "scoped fanout count should converge"
    );
}
