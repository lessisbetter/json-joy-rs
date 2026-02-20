use json_joy_json_path::{JsonPathEval, JsonPathParser};
use serde_json::{json, Value};

fn eval_values(path: &str, data: &Value) -> Vec<Value> {
    let parsed =
        JsonPathParser::parse(path).unwrap_or_else(|e| panic!("parse failed for '{path}': {e}"));
    JsonPathEval::eval(&parsed, data)
        .into_iter()
        .cloned()
        .collect()
}

#[test]
fn exec_root_selector_and_malformed_root_matrix() {
    let data = json!({"foo": "bar"});
    let root = eval_values("$", &data);
    assert_eq!(root, vec![data.clone()]);

    assert!(JsonPathParser::parse("$.").is_err());
    assert!(JsonPathParser::parse("").is_err());
}

#[test]
fn exec_combined_selector_matrix() {
    let data = json!(["a", "b", "c", "d", "e", "f", "g"]);

    let multi = eval_values("$[0, 3]", &data);
    assert_eq!(multi, vec![json!("a"), json!("d")]);

    let slice_and_index = eval_values("$[0:2, 5]", &data);
    assert_eq!(slice_and_index, vec![json!("a"), json!("b"), json!("f")]);

    let dup = eval_values("$[0, 0]", &data);
    assert_eq!(dup, vec![json!("a"), json!("a")]);
}

#[test]
fn exec_edge_case_matrix() {
    assert!(eval_values("$[*]", &json!([])).is_empty());
    assert!(eval_values("$[*]", &json!({})).is_empty());
    assert!(eval_values("$[*]", &json!("hello")).is_empty());

    let null_case = eval_values("$.a", &json!({"a": null}));
    assert_eq!(null_case, vec![json!(null)]);

    let deep = json!({"a": {"b": {"c": {"d": {"e": "deep"}}}}});
    let deep_res = eval_values("$.a.b.c.d.e", &deep);
    assert_eq!(deep_res, vec![json!("deep")]);
}

#[test]
fn exec_real_world_examples_matrix() {
    let jsonpath_dot_com_example = json!({
        "firstName": "John",
        "lastName": "doe",
        "age": 26,
        "address": {
            "streetAddress": "naist street",
            "city": "Nara",
            "postalCode": "630-0192"
        },
        "phoneNumbers": [
            {"type": "iPhone", "number": "0123-4567-8888"},
            {"type": "home", "number": "0123-4567-8910"}
        ]
    });
    let phone = eval_values("$.phoneNumbers[:1].type", &jsonpath_dot_com_example);
    assert_eq!(phone, vec![json!("iPhone")]);

    let hevo = json!({
        "event": {
            "agency": "MI6",
            "data": {
                "name": "James Bond",
                "id": "007"
            }
        }
    });
    let name = eval_values("$.event.data.name", &hevo);
    assert_eq!(name, vec![json!("James Bond")]);
}
