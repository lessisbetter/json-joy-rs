use json_joy_json_type::codegen::capacity::CapacityEstimatorCodegen;
use json_joy_json_type::type_def::{KeyType, ModuleType, ObjType, TypeBuilder, TypeNode};
use json_joy_util::max_encoding_capacity;
use serde_json::json;

fn t() -> TypeBuilder {
    TypeBuilder::new()
}

#[test]
fn capacity_any_matches_max_encoding_capacity_matrix() {
    let estimator = CapacityEstimatorCodegen::get(t().any());
    let values = vec![
        json!(null),
        json!(true),
        json!(false),
        json!(1),
        json!(123.123),
        json!(""),
        json!("asdf"),
        json!([]),
        json!({}),
        json!({"foo": "bar"}),
        json!([{"a": [{"b": null}]}]),
    ];
    for value in values {
        assert_eq!(estimator(&value), max_encoding_capacity(&value));
    }
}

#[test]
fn capacity_const_bool_num_matrix() {
    let const_type = t().Const(json!({"foo": [123]}), None);
    let const_estimator = CapacityEstimatorCodegen::get(const_type);
    assert_eq!(
        const_estimator(&json!(null)),
        max_encoding_capacity(&json!({"foo": [123]}))
    );

    let bool_estimator = CapacityEstimatorCodegen::get(t().bool());
    assert_eq!(bool_estimator(&json!(false)), 5);

    let num_estimator = CapacityEstimatorCodegen::get(t().num());
    assert_eq!(num_estimator(&json!(123)), 22);
}

#[test]
fn capacity_str_and_bin_matrix() {
    let str_estimator = CapacityEstimatorCodegen::get(t().str());
    assert_eq!(str_estimator(&json!("")), max_encoding_capacity(&json!("")));
    assert_eq!(str_estimator(&json!("asdf")), max_encoding_capacity(&json!("asdf")));

    let bin_estimator = CapacityEstimatorCodegen::get(t().bin());
    let empty = json!([]);
    let small = json!([1, 2, 3]);
    assert_eq!(bin_estimator(&empty), 41);
    assert_eq!(bin_estimator(&small), 47);
}

#[test]
fn capacity_arrays_matrix() {
    let arr_any = CapacityEstimatorCodegen::get(t().arr());
    assert_eq!(arr_any(&json!([])), max_encoding_capacity(&json!([])));
    assert_eq!(arr_any(&json!([1, true, "asdf"])), max_encoding_capacity(&json!([1, true, "asdf"])));

    let arr_num = CapacityEstimatorCodegen::get(t().Array(t().num(), None));
    assert_eq!(arr_num(&json!([1, 2, 3])), max_encoding_capacity(&json!([1, 2, 3])));

    let tuple = CapacityEstimatorCodegen::get(t().Tuple(vec![t().num(), t().str()], None, None));
    assert_eq!(tuple(&json!([1, "asdf"])), max_encoding_capacity(&json!([1, "asdf"])));

    let head_tail = CapacityEstimatorCodegen::get(t().Tuple(
        vec![t().Const(json!("start"), None)],
        Some(t().str()),
        Some(vec![t().Const(json!("end"), None)]),
    ));
    let data = json!(["start", "middle1", "middle2", "end"]);
    assert_eq!(head_tail(&data), max_encoding_capacity(&data));
}

#[test]
fn capacity_objects_and_maps_matrix() {
    let obj = TypeNode::Obj(ObjType::new(vec![
        KeyType::new("a", t().num()),
        KeyType::new_opt("b", t().str()),
    ]));
    let obj_estimator = CapacityEstimatorCodegen::get(obj);
    assert_eq!(obj_estimator(&json!({"a": 1})), max_encoding_capacity(&json!({"a": 1})));
    assert_eq!(
        obj_estimator(&json!({"a": 1, "b": "x"})),
        max_encoding_capacity(&json!({"a": 1, "b": "x"}))
    );

    let mut encode_unknown = ObjType::new(vec![KeyType::new("a", t().num())]);
    encode_unknown.schema.encode_unknown_keys = Some(true);
    let encode_unknown_estimator = CapacityEstimatorCodegen::get(TypeNode::Obj(encode_unknown));
    let value = json!({"a": 1, "extra": [1, 2, 3]});
    assert_eq!(encode_unknown_estimator(&value), max_encoding_capacity(&value));

    let map_estimator = CapacityEstimatorCodegen::get(t().Map(t().num(), None, None));
    let map_value = json!({"x": 1, "y": 2});
    assert_eq!(map_estimator(&map_value), max_encoding_capacity(&map_value));
}

#[test]
fn capacity_ref_and_or_matrix() {
    let system = ModuleType::new();
    system.alias("NumAlias", t().num().get_schema());

    let tb = TypeBuilder::with_system(std::sync::Arc::new(system));
    let ref_estimator = CapacityEstimatorCodegen::get(tb.Ref("NumAlias"));
    assert_eq!(ref_estimator(&json!(123)), 22);

    let or_estimator = CapacityEstimatorCodegen::get(t().Or(vec![t().num(), t().str()]));
    assert_eq!(or_estimator(&json!(123)), 22);
    assert_eq!(or_estimator(&json!("abc")), max_encoding_capacity(&json!("abc")));
}
