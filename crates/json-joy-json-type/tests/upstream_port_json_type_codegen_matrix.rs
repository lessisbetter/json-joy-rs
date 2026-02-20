use json_joy_json_pack::cbor::decode_json_from_cbor_bytes;
use json_joy_json_pack::msgpack::MsgPackDecoderFast;
use json_joy_json_type::codegen::binary::{CborCodegen, JsonCodegen, MsgPackCodegen};
use json_joy_json_type::codegen::capacity::CapacityEstimatorCodegen;
use json_joy_json_type::codegen::discriminator::DiscriminatorCodegen;
use json_joy_json_type::codegen::json::JsonTextCodegen;
use json_joy_json_type::schema::{NumSchema, ObjSchema, Schema, StrSchema};
use json_joy_json_type::type_def::{KeyType, ModuleType, OrType, TypeBuilder, TypeNode};
use json_joy_util::json_size::max_encoding_capacity;
use serde_json::json;
use std::sync::Arc;

fn t() -> TypeBuilder {
    TypeBuilder::new()
}

#[test]
fn capacity_any_matches_max_encoding_capacity() {
    let typ = t().any();
    let estimator = CapacityEstimatorCodegen::get(&typ);

    let cases = [
        json!(null),
        json!(true),
        json!(123),
        json!(""),
        json!([1, 2, 3]),
        json!({"foo": ["bar", null]}),
    ];

    for case in cases {
        assert_eq!(estimator(&case), max_encoding_capacity(&case));
    }
}

#[test]
fn capacity_const_uses_literal_not_input() {
    let typ = t().Const(json!({"foo": [123]}), None);
    let estimator = CapacityEstimatorCodegen::get(&typ);

    assert_eq!(
        estimator(&json!(null)),
        max_encoding_capacity(&json!({"foo": [123]}))
    );
    assert_eq!(
        estimator(&json!({"ignored": true})),
        max_encoding_capacity(&json!({"foo": [123]}))
    );
}

#[test]
fn capacity_array_tuple_and_object_variants() {
    let tuple = t().Tuple(vec![t().str(), t().num()], Some(t().bool()), None);
    let estimate_tuple = CapacityEstimatorCodegen::get(&tuple);
    let tuple_val = json!(["x", 10, true, false]);
    assert_eq!(
        estimate_tuple(&tuple_val),
        max_encoding_capacity(&tuple_val)
    );

    let map = t().Map(t().str(), None, None);
    let estimate_map = CapacityEstimatorCodegen::get(&map);
    let map_val = json!({"a": "x", "b": "yy"});
    assert_eq!(estimate_map(&map_val), max_encoding_capacity(&map_val));
}

#[test]
fn capacity_tuple_head_and_tail_matrix_matches_upstream_expectations() {
    let cases: Vec<(TypeNode, serde_json::Value)> = vec![
        (
            t().Tuple(
                vec![
                    t().Key("first", t().Const(json!("abc"), None)),
                    t().Key("second", t().Const(json!("xxxxxxxxx"), None)),
                ],
                Some(t().num()),
                None,
            ),
            json!(["abc", "xxxxxxxxx", 1]),
        ),
        (
            t().Tuple(
                vec![],
                Some(t().num()),
                Some(vec![t().Key("very_important", t().str()), t().str()]),
            ),
            json!([1, "abc", "xxxxxxxxx"]),
        ),
        (
            t().Tuple(
                vec![t().Const(json!("start"), None)],
                Some(t().str()),
                Some(vec![t().Const(json!("end"), None)]),
            ),
            json!(["start", "middle1", "middle2", "end"]),
        ),
    ];

    for (typ, value) in cases {
        let estimator = CapacityEstimatorCodegen::get(&typ);
        assert_eq!(estimator(&value), max_encoding_capacity(&value));
    }
}

#[test]
fn capacity_object_encode_unknown_keys_matches_max_capacity() {
    let mut obj = json_joy_json_type::type_def::ObjType::new(vec![KeyType::new("foo", t().str())]);
    obj.schema = ObjSchema {
        encode_unknown_keys: Some(true),
        ..ObjSchema::default()
    };
    let typ = TypeNode::Obj(obj);
    let estimator = CapacityEstimatorCodegen::get(&typ);

    let value = json!({"foo": "bar", "zzz": 1, "nested": {"x": true}});
    assert_eq!(estimator(&value), max_encoding_capacity(&value));
}

#[test]
fn json_text_encodes_primitives_and_constants_like_upstream() {
    let encode_bool = JsonTextCodegen::get(&t().bool());
    assert_eq!(encode_bool(&json!(true)).unwrap(), "true");
    assert_eq!(encode_bool(&json!(false)).unwrap(), "false");
    assert_eq!(encode_bool(&json!(1)).unwrap(), "true");
    assert_eq!(encode_bool(&json!(0)).unwrap(), "false");

    let encode_num = JsonTextCodegen::get(&t().num());
    assert_eq!(encode_num(&json!(1)).unwrap(), "1");
    assert_eq!(encode_num(&json!(-1)).unwrap(), "-1");

    let encode_con = JsonTextCodegen::get(&t().Const(json!("xyz"), None));
    assert_eq!(encode_con(&json!("ignored")).unwrap(), "\"xyz\"");
}

#[test]
fn json_text_encodes_binary_optional_fields_and_unknown_keys() {
    let encode_bin = JsonTextCodegen::get(&t().bin());
    assert_eq!(
        encode_bin(&json!([97, 115, 100, 102])).unwrap(),
        "\"data:application/octet-stream;base64,YXNkZg==\""
    );

    let mut obj = json_joy_json_type::type_def::ObjType::new(vec![
        KeyType::new("foo", t().str()),
        KeyType::new_opt("bar", t().num()),
    ]);
    obj.schema.encode_unknown_keys = Some(true);
    let encode_obj = JsonTextCodegen::get(&TypeNode::Obj(obj));

    assert_eq!(encode_obj(&json!({"foo": "x"})).unwrap(), "{\"foo\":\"x\"}");
    assert_eq!(
        encode_obj(&json!({"foo": "x", "bar": 1, "extra": true})).unwrap(),
        "{\"foo\":\"x\",\"bar\":1,\"extra\":true}"
    );
}

#[test]
fn json_text_encodes_map_and_any_like_upstream_examples() {
    let map_encoder = JsonTextCodegen::get(&t().Map(t().num(), None, None));
    assert_eq!(
        map_encoder(&json!({"a": 1, "b": 2, "c": 3})).unwrap(),
        "{\"a\":1,\"b\":2,\"c\":3}"
    );
    assert_eq!(map_encoder(&json!({})).unwrap(), "{}");

    let any_encoder = JsonTextCodegen::get(&t().any());
    assert_eq!(
        any_encoder(&json!({"foo": "bar"})).unwrap(),
        "{\"foo\":\"bar\"}"
    );
    assert_eq!(any_encoder(&json!(-1)).unwrap(), "-1");
}

#[test]
fn json_text_ref_and_or_behaviour_matches_expected_output() {
    let module = Arc::new(ModuleType::new());
    module.alias("ID", Schema::Str(StrSchema::default()));
    module.alias("Amount", Schema::Num(NumSchema::default()));

    let tb = TypeBuilder::with_system(Arc::clone(&module));
    let ref_encoder = JsonTextCodegen::get(&tb.Ref("ID"));
    assert_eq!(ref_encoder(&json!("abc")).unwrap(), "\"abc\"");

    let or_type = TypeNode::Or(OrType::new(vec![tb.Ref("ID"), tb.Ref("Amount")]));
    let or_encoder = JsonTextCodegen::get(&or_type);
    assert_eq!(or_encoder(&json!("xyz")).unwrap(), "\"xyz\"");
    assert_eq!(or_encoder(&json!(123)).unwrap(), "123");
}

#[test]
fn discriminator_codegen_selects_union_branch_index() {
    let typ = TypeNode::Or(OrType::new(vec![t().str(), t().num()]));
    let TypeNode::Or(or_typ) = typ else {
        panic!("expected Or type");
    };

    let discriminator = DiscriminatorCodegen::get(&or_typ).expect("build discriminator");
    assert_eq!(discriminator(&json!("abc")), 0);
    assert_eq!(discriminator(&json!(123)), 1);
}

#[test]
fn discriminator_codegen_handles_nested_constant_discriminators() {
    let kind_a = t().Object(vec![
        KeyType::new("kind", t().Const(json!("a"), None)),
        KeyType::new("v", t().num()),
    ]);
    let kind_b = t().Object(vec![
        KeyType::new("kind", t().Const(json!("b"), None)),
        KeyType::new("v", t().str()),
    ]);
    let typ = TypeNode::Or(OrType::new(vec![kind_a, kind_b]));
    let TypeNode::Or(or_typ) = typ else {
        panic!("expected Or type");
    };

    let discriminator = DiscriminatorCodegen::get(&or_typ).expect("build discriminator");
    assert_eq!(discriminator(&json!({"kind": "a", "v": 1})), 0);
    assert_eq!(discriminator(&json!({"kind": "b", "v": "x"})), 1);
}

#[test]
fn binary_codegen_roundtrips_for_json_msgpack_and_cbor() {
    let typ = t().Object(vec![
        KeyType::new("foo", t().str()),
        KeyType::new("count", t().num()),
    ]);
    let value = json!({"foo": "bar", "count": 3});

    let encode_json = JsonCodegen::get(&typ);
    let json_bytes = encode_json(&value).expect("json encode");
    let decoded_json: serde_json::Value =
        serde_json::from_slice(&json_bytes).expect("decode json bytes");
    assert_eq!(decoded_json, value);

    let encode_cbor = CborCodegen::get(&typ);
    let cbor_bytes = encode_cbor(&value).expect("cbor encode");
    let decoded_cbor = decode_json_from_cbor_bytes(&cbor_bytes).expect("decode cbor bytes");
    assert_eq!(decoded_cbor, value);

    let encode_msgpack = MsgPackCodegen::get(&typ);
    let msgpack_bytes = encode_msgpack(&value).expect("msgpack encode");
    let mut decoder = MsgPackDecoderFast::new();
    let decoded_msgpack: serde_json::Value = serde_json::Value::from(
        decoder
            .decode(&msgpack_bytes)
            .expect("decode msgpack bytes"),
    );
    assert_eq!(decoded_msgpack, value);
}

#[test]
fn json_text_ref_preserves_unknown_keys_when_schema_option_enabled() {
    let module = Arc::new(ModuleType::new());
    let tb = TypeBuilder::with_system(Arc::clone(&module));

    let mut obj = json_joy_json_type::type_def::ObjType::new(vec![
        KeyType::new("foo", tb.str()),
        KeyType::new_opt("zzz", tb.num()),
    ]);
    obj.schema.encode_unknown_keys = Some(true);

    module.alias("foo", TypeNode::Obj(obj).get_schema());
    let encoded =
        JsonTextCodegen::get(&tb.Ref("foo"))(&json!({"foo": "bar", "zzz": 1, "baz": 123})).unwrap();
    assert_eq!(encoded, "{\"foo\":\"bar\",\"zzz\":1,\"baz\":123}");
}

#[test]
fn json_text_handles_recursive_ref_shapes() {
    let module = Arc::new(ModuleType::new());
    let tb = TypeBuilder::with_system(Arc::clone(&module));

    let user_schema = tb
        .Object(vec![
            KeyType::new("id", tb.str()),
            KeyType::new_opt("address", tb.Ref("Address")),
        ])
        .get_schema();
    module.alias("User", user_schema);
    let address_schema = tb
        .Object(vec![
            KeyType::new("id", tb.str()),
            KeyType::new_opt("user", tb.Ref("User")),
        ])
        .get_schema();
    module.alias("Address", address_schema);

    let value = json!({
        "id": "user-1",
        "address": {
            "id": "address-1",
            "user": {
                "id": "user-2",
                "address": {
                    "id": "address-2",
                    "user": {"id": "user-3"}
                }
            }
        }
    });

    let encoded = JsonTextCodegen::get(&tb.Ref("User"))(&value).unwrap();
    let decoded: serde_json::Value = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, value);
}

#[test]
fn json_text_handles_recursive_chain_of_ref_aliases() {
    let module = Arc::new(ModuleType::new());
    let tb = TypeBuilder::with_system(Arc::clone(&module));

    module.alias(
        "User0",
        tb.Object(vec![
            KeyType::new("id", tb.str()),
            KeyType::new_opt("address", tb.Ref("Address")),
        ])
        .get_schema(),
    );
    module.alias("User1", tb.Ref("User0").get_schema());
    module.alias("User", tb.Ref("User1").get_schema());

    module.alias(
        "Address0",
        tb.Object(vec![
            KeyType::new("id", tb.str()),
            KeyType::new_opt("user", tb.Ref("User")),
        ])
        .get_schema(),
    );
    module.alias("Address1", tb.Ref("Address0").get_schema());
    module.alias("Address", tb.Ref("Address1").get_schema());

    let value = json!({
        "id": "address-1",
        "user": {
            "id": "user-1",
            "address": {
                "id": "address-2",
                "user": {
                    "id": "user-2",
                    "address": {"id": "address-3"}
                }
            }
        }
    });

    let encoded = JsonTextCodegen::get(&tb.Ref("Address"))(&value).unwrap();
    let decoded: serde_json::Value = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, value);
}
