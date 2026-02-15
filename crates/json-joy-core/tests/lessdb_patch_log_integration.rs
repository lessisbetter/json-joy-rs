use json_joy_core::less_db_compat::{create_model, diff_model};
use json_joy_core::patch::Patch;
use json_joy_core::patch_log::{append_patch, deserialize_patches, serialize_patches, PATCH_LOG_VERSION};
use serde_json::json;

#[test]
fn lessdb_patch_log_append_and_deserialize_with_diff_output() {
    let model = create_model(&json!({"title":"a"}), 79001).expect("create model");
    let patch_bytes = diff_model(&model, &json!({"title":"A"}))
        .expect("diff model")
        .expect("patch should exist");
    let patch = Patch::from_binary(&patch_bytes).expect("patch decode");

    let log = append_patch(&[], &patch);
    assert_eq!(log[0], PATCH_LOG_VERSION);

    let decoded = deserialize_patches(&log).expect("deserialize patch log");
    assert_eq!(decoded.len(), 1);
    assert_eq!(decoded[0].to_binary(), patch.to_binary());
}

#[test]
fn lessdb_patch_log_serialize_roundtrip_with_generated_patches() {
    let model = create_model(&json!({"n":1}), 79002).expect("create model");
    let p1 = Patch::from_binary(
        &diff_model(&model, &json!({"n":2}))
            .expect("diff 1")
            .expect("patch 1"),
    )
    .expect("patch decode 1");

    let model2 = create_model(&json!({"txt":"a"}), 79003).expect("create model2");
    let p2 = Patch::from_binary(
        &diff_model(&model2, &json!({"txt":"ab"}))
            .expect("diff 2")
            .expect("patch 2"),
    )
    .expect("patch decode 2");

    let log = serialize_patches(&[p1.clone(), p2.clone()]);
    let decoded = deserialize_patches(&log).expect("deserialize");
    assert_eq!(decoded.len(), 2);
    assert_eq!(decoded[0].to_binary(), p1.to_binary());
    assert_eq!(decoded[1].to_binary(), p2.to_binary());
}
