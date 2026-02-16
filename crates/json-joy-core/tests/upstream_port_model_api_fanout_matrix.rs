use std::sync::{Arc, Mutex};

use json_joy_core::model_api::{ChangeEventOrigin, NativeModelApi, PathStep};
use serde_json::json;

#[test]
fn upstream_port_model_api_fanout_matrix_change_and_scoped_counts() {
    let sid = 99102;
    let compat =
        json_joy_core::less_db_compat::create_model(&json!({"doc":{"title":"ab","count":1}}), sid)
            .unwrap();
    let binary = json_joy_core::less_db_compat::model_to_binary(&compat);
    let mut api = NativeModelApi::from_model_binary(&binary, Some(sid)).unwrap();

    let changes = Arc::new(Mutex::new(Vec::new()));
    let changes_clone = Arc::clone(&changes);
    api.on_change(move |ev| {
        changes_clone.lock().unwrap().push(ev.origin);
    });

    let scoped = Arc::new(Mutex::new(0usize));
    let scoped_clone = Arc::clone(&scoped);
    api.on_change_at(
        vec![PathStep::Key("doc".into()), PathStep::Key("title".into())],
        move |_| {
            *scoped_clone.lock().unwrap() += 1;
        },
    );

    api.set(
        &[PathStep::Key("doc".into()), PathStep::Key("count".into())],
        json!(2),
    )
    .unwrap();
    api.set(
        &[PathStep::Key("doc".into()), PathStep::Key("title".into())],
        json!("aZb"),
    )
    .unwrap();

    assert_eq!(changes.lock().unwrap().len(), 2);
    assert_eq!(changes.lock().unwrap()[0], ChangeEventOrigin::Local);
    assert_eq!(*scoped.lock().unwrap(), 1);
}
