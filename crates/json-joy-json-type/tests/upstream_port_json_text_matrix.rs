use std::sync::Arc;

use json_joy_json_pack::json_binary;
use json_joy_json_pack::PackValue;
use json_joy_json_type::codegen::json::JsonTextCodegen;
use json_joy_json_type::schema::Schema;
use json_joy_json_type::type_def::{KeyType, ModuleType, ObjType, TypeBuilder, TypeNode};
use serde_json::{json, Value};

fn t() -> TypeBuilder {
    TypeBuilder::new()
}

fn encode(type_: TypeNode, value: &Value) -> String {
    let encoder = JsonTextCodegen::get(type_);
    encoder(value).expect("json text encoding")
}

fn encode_and_decode(type_: TypeNode, value: Value) -> Value {
    let encoded = encode(type_, &value);
    serde_json::from_str(&encoded).expect("valid json")
}

#[test]
fn json_text_any_bool_num_str_matrix() {
    assert_eq!(
        encode(t().any(), &json!({"foo": "bar"})),
        r#"{"foo":"bar"}"#
    );

    assert_eq!(encode(t().bool(), &json!(true)), "true");
    assert_eq!(encode(t().bool(), &json!(false)), "false");
    assert_eq!(encode(t().bool(), &json!(1)), "true");
    assert_eq!(encode(t().bool(), &json!(0)), "false");

    assert_eq!(encode(t().num(), &json!(1)), "1");
    assert_eq!(encode(t().num(), &json!(0)), "0");
    assert_eq!(encode(t().num(), &json!(-1)), "-1");

    assert_eq!(encode(t().str(), &json!("")), r#""""#);
    assert_eq!(encode(t().str(), &json!("asdf")), r#""asdf""#);
}

#[test]
fn json_text_binary_and_const_matrix() {
    let bin_encoder = JsonTextCodegen::get(t().bin());
    assert_eq!(
        bin_encoder(&json!([])).expect("encode"),
        r#""data:application/octet-stream;base64,""#
    );
    assert_eq!(
        bin_encoder(&json!([97, 115, 100, 102])).expect("encode"),
        r#""data:application/octet-stream;base64,YXNkZg==""#
    );

    let parsed =
        json_binary::parse(r#""data:application/octet-stream;base64,YXNkZg==""#).expect("parse");
    assert_eq!(parsed, PackValue::Bytes(vec![97, 115, 100, 102]));

    assert_eq!(
        encode(t().Const(json!("xyz"), None), &json!("ignored")),
        r#""xyz""#
    );
    assert_eq!(
        encode(t().Const(json!({"foo": "bar"}), None), &json!({})),
        r#"{"foo":"bar"}"#
    );
}

#[test]
fn json_text_arr_obj_map_nil_matrix() {
    let arr = t().Array(t().num(), None);
    assert_eq!(encode(arr, &json!([1, 2, 3])), "[1,2,3]");

    let obj = TypeNode::Obj(ObjType::new(vec![
        KeyType::new("a", t().num()),
        KeyType::new("b", t().str()),
    ]));
    assert_eq!(
        encode(obj, &json!({"a": 123, "b": "asdf"})),
        r#"{"a":123,"b":"asdf"}"#
    );

    let obj_opt = TypeNode::Obj(ObjType::new(vec![
        KeyType::new("a", t().num()),
        KeyType::new("b", t().Const(json!("asdf"), None)),
        KeyType::new_opt("c", t().str()),
        KeyType::new_opt("d", t().num()),
    ]));
    assert_eq!(
        encode_and_decode(obj_opt.clone(), json!({"a": 123, "b": "asdf"})),
        json!({"a": 123, "b": "asdf"})
    );
    assert_eq!(
        encode_and_decode(
            obj_opt.clone(),
            json!({"a": 123, "b": "asdf", "c": "qwerty"})
        ),
        json!({"a": 123, "b": "asdf", "c": "qwerty"})
    );
    assert_eq!(
        encode_and_decode(
            obj_opt,
            json!({"a": 123, "d": 4343.3, "b": "asdf", "c": "qwerty"})
        ),
        json!({"a": 123, "d": 4343.3, "b": "asdf", "c": "qwerty"})
    );

    let map = t().Map(t().num(), None, None);
    assert_eq!(encode_and_decode(map.clone(), json!({})), json!({}));
    assert_eq!(
        encode_and_decode(map, json!({"a": 1, "b": 2, "c": 3})),
        json!({"a": 1, "b": 2, "c": 3})
    );

    assert_eq!(encode(t().nil(), &json!(123)), "null");
}

#[test]
fn json_text_or_and_ref_matrix() {
    let or_type = t().Or(vec![t().str(), t().num()]);
    assert_eq!(encode(or_type.clone(), &json!("xyz")), r#""xyz""#);
    assert_eq!(encode(or_type, &json!(123)), "123");

    let system = Arc::new(ModuleType::new());
    system.alias("ID", Schema::Str(Default::default()));

    let tb = TypeBuilder::with_system(system.clone());
    let ref_type = TypeNode::Obj(ObjType::new(vec![
        KeyType::new("name", tb.str()),
        KeyType::new("id", tb.Ref("ID")),
        KeyType::new("createdAt", tb.num()),
    ]));

    let value = json!({"name": "John", "id": "123", "createdAt": 123});
    assert_eq!(encode_and_decode(ref_type, value.clone()), value);
}

#[test]
fn json_text_encode_unknown_keys_in_ref_matrix() {
    let system = Arc::new(ModuleType::new());
    let tb = TypeBuilder::with_system(system.clone());

    let mut obj = ObjType::new(vec![
        KeyType::new("foo", tb.str()),
        KeyType::new_opt("zzz", tb.num()),
    ]);
    obj.schema.encode_unknown_keys = Some(true);

    system.alias("foo", TypeNode::Obj(obj).get_schema());
    let ref_type = tb.Ref("foo");

    let encoded = encode(ref_type, &json!({"foo": "bar", "zzz": 1, "baz": 123}));
    assert_eq!(encoded, r#"{"foo":"bar","zzz":1,"baz":123}"#);
}

#[test]
fn json_text_circular_ref_shape_matrix() {
    let system = Arc::new(ModuleType::new());
    let tb = TypeBuilder::with_system(system.clone());

    system.alias(
        "User",
        TypeNode::Obj(ObjType::new(vec![
            KeyType::new("id", tb.str()),
            KeyType::new_opt("address", tb.Ref("Address")),
        ]))
        .get_schema(),
    );
    system.alias(
        "Address",
        TypeNode::Obj(ObjType::new(vec![
            KeyType::new("id", tb.str()),
            KeyType::new_opt("user", tb.Ref("User")),
        ]))
        .get_schema(),
    );

    let user_type = tb.Ref("User");
    let address_type = tb.Ref("Address");

    let value1 = json!({
        "id": "user-1",
        "address": {
            "id": "address-1",
            "user": {
                "id": "user-2",
                "address": {
                    "id": "address-2",
                    "user": {
                        "id": "user-3"
                    }
                }
            }
        }
    });
    assert_eq!(encode_and_decode(user_type, value1.clone()), value1);

    let value2 = json!({
        "id": "address-1",
        "user": {
            "id": "user-1",
            "address": {
                "id": "address-2",
                "user": {
                    "id": "user-2",
                    "address": {
                        "id": "address-3"
                    }
                }
            }
        }
    });
    assert_eq!(encode_and_decode(address_type, value2.clone()), value2);
}

#[test]
fn json_text_chain_of_refs_matrix() {
    let system = Arc::new(ModuleType::new());
    let tb = TypeBuilder::with_system(system.clone());

    system.alias(
        "User0",
        TypeNode::Obj(ObjType::new(vec![
            KeyType::new("id", tb.str()),
            KeyType::new_opt("address", tb.Ref("Address")),
        ]))
        .get_schema(),
    );
    system.alias("User1", tb.Ref("User0").get_schema());
    system.alias("User", tb.Ref("User1").get_schema());

    system.alias(
        "Address0",
        TypeNode::Obj(ObjType::new(vec![
            KeyType::new("id", tb.str()),
            KeyType::new_opt("user", tb.Ref("User")),
        ]))
        .get_schema(),
    );
    system.alias("Address1", tb.Ref("Address0").get_schema());
    system.alias("Address", tb.Ref("Address1").get_schema());

    let user_type = tb.Ref("User");
    let address_type = tb.Ref("Address");

    let value1 = json!({
        "id": "user-1",
        "address": {
            "id": "address-1",
            "user": {
                "id": "user-2",
                "address": {
                    "id": "address-2",
                    "user": {
                        "id": "user-3"
                    }
                }
            }
        }
    });
    assert_eq!(encode_and_decode(user_type, value1.clone()), value1);

    let value2 = json!({
        "id": "address-1",
        "user": {
            "id": "user-1",
            "address": {
                "id": "address-2",
                "user": {
                    "id": "user-2",
                    "address": {
                        "id": "address-3"
                    }
                }
            }
        }
    });
    assert_eq!(encode_and_decode(address_type, value2.clone()), value2);
}
