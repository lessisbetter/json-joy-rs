use json_joy_core::model_api::NativeModelApi;
use serde_json::json;

#[test]
fn upstream_port_nodes_family_matrix_obj_arr_str_bin_vec_behaviors() {
    let sid = 99103;
    let compat = json_joy_core::less_db_compat::create_model(
        &json!({"obj":{"a":1},"arr":[1],"str":"ab","bin":[1,2],"vec":[null,3]}),
        sid,
    )
    .unwrap();
    let binary = json_joy_core::less_db_compat::model_to_binary(&compat);
    let mut api = NativeModelApi::from_model_binary(&binary, Some(sid)).unwrap();

    api.node().at_key("obj").obj_put("b", json!(2)).unwrap();
    api.node().at_key("arr").arr_push(json!(2)).unwrap();
    api.node().at_key("str").str_ins(1, "Z").unwrap();
    api.node()
        .at_key("bin")
        .as_bin()
        .unwrap()
        .ins(2, &[3, 4])
        .unwrap();
    api.node()
        .at_key("vec")
        .as_vec()
        .unwrap()
        .set(0, Some(json!(9)))
        .unwrap();

    assert_eq!(
        api.view(),
        json!({"obj":{"a":1,"b":2},"arr":[1,2],"str":"aZb","bin":[1,2,3,4],"vec":[9,3]})
    );
}
