use json_joy_core::model_api::NativeModelApi;
use serde_json::json;

#[test]
fn upstream_port_model_api_proxy_matrix_path_builder_semantics() {
    let sid = 99101;
    let compat = json_joy_core::less_db_compat::create_model(
        &json!({"doc":{"title":"ab","list":[1]}}),
        sid,
    )
    .unwrap();
    let binary = json_joy_core::less_db_compat::model_to_binary(&compat);
    let mut api = NativeModelApi::from_model_binary(&binary, Some(sid)).unwrap();

    api.node()
        .at_key("doc")
        .at_key("list")
        .arr_push(json!(2))
        .unwrap();
    api.node()
        .at_key("doc")
        .at_key("title")
        .str_ins(1, "Z")
        .unwrap();
    api.node()
        .at_key("doc")
        .at_key("list")
        .at_index(0)
        .replace(json!(7))
        .unwrap();
    api.node()
        .at_key("doc")
        .obj_put("flag", json!(true))
        .unwrap();

    assert_eq!(api.view(), json!({"doc":{"title":"aZb","list":[7,2],"flag":true}}));
}
