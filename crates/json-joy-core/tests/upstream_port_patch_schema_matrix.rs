use json_joy_core::model_runtime::RuntimeModel;
use json_joy_core::schema::{
    arr_node, con_json, json, obj_node, str_node, val_node, vec_node, SchemaNode,
};
use serde_json::json;

fn apply_schema(schema: &SchemaNode, sid: u64) -> RuntimeModel {
    let patch = schema.to_patch(sid, 1).expect("schema->patch");
    let mut runtime = RuntimeModel::new_logical_empty(sid);
    runtime.apply_patch(&patch).expect("apply schema patch");
    runtime
}

#[test]
fn upstream_port_schema_json_flat_object_matches_explicit_schema() {
    let via_json = json(&json!({
      "num": 123,
      "str": "b",
      "bool": true,
      "nil": null
    }));
    let explicit = obj_node(
        vec![
            ("num".to_string(), con_json(json!(123))),
            ("str".to_string(), str_node("b")),
            ("bool".to_string(), con_json(json!(true))),
            ("nil".to_string(), con_json(serde_json::Value::Null)),
        ],
        vec![],
    );
    let model_a = apply_schema(&via_json, 123456789);
    let model_b = apply_schema(&explicit, 123456789);
    assert_eq!(model_a.view_json(), model_b.view_json());
    assert_eq!(
        model_a.to_model_binary_like().unwrap(),
        model_b.to_model_binary_like().unwrap()
    );
}

#[test]
fn upstream_port_schema_json_array_matches_explicit_schema() {
    let via_json = json(&json!([1, 2, 3, "a", true, null]));
    let explicit = arr_node(vec![
        val_node(con_json(json!(1))),
        val_node(con_json(json!(2))),
        val_node(con_json(json!(3))),
        str_node("a"),
        val_node(con_json(json!(true))),
        val_node(con_json(serde_json::Value::Null)),
    ]);
    let model_a = apply_schema(&via_json, 456789123);
    let model_b = apply_schema(&explicit, 456789123);
    assert_eq!(model_a.view_json(), model_b.view_json());
    assert_eq!(
        model_a.to_model_binary_like().unwrap(),
        model_b.to_model_binary_like().unwrap()
    );
}

#[test]
fn upstream_port_schema_vec_node_builds_expected_shape() {
    let schema = obj_node(
        vec![(
            "vec".to_string(),
            vec_node(vec![
                Some(con_json(json!(1))),
                None,
                Some(val_node(con_json(json!(2)))),
            ]),
        )],
        vec![],
    );
    let model = apply_schema(&schema, 903_001);
    assert_eq!(
        model.view_json(),
        json!({
          "vec": [1, null, 2]
        })
    );
}
