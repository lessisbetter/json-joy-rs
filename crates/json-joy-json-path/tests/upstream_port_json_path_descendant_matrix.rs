use json_joy_json_path::{JsonPathCodegen, JsonPathEval, JsonPathParser};
use serde_json::{json, Value};

fn eval_values(path: &str, data: &Value) -> Vec<Value> {
    let parsed =
        JsonPathParser::parse(path).unwrap_or_else(|e| panic!("parse failed for '{path}': {e}"));
    JsonPathEval::eval(&parsed, data)
        .into_iter()
        .cloned()
        .collect()
}

fn codegen_values(path: &str, data: &Value) -> Vec<Value> {
    JsonPathCodegen::run(path, data)
        .unwrap_or_else(|e| panic!("codegen failed for '{path}': {e}"))
        .into_iter()
        .cloned()
        .collect()
}

#[test]
fn descendant_invalid_pattern_matrix() {
    assert!(JsonPathParser::parse("$..").is_err());
    assert!(JsonPathCodegen::compile("$..").is_err());
}

#[test]
fn descendant_wildcard_matrix() {
    let data = json!({
        "store": {
            "book": [
                {"title": "Book 1", "price": 10},
                {"title": "Book 2", "price": 20}
            ],
            "bicycle": {"color": "red", "price": 100}
        }
    });
    let wildcard = eval_values("$..*", &data);
    let bracket = eval_values("$..[*]", &data);
    assert_eq!(wildcard, bracket);
    assert!(wildcard.contains(&json!(10)));
    assert!(wildcard.contains(&json!(20)));
    assert!(wildcard.contains(&json!("red")));
    assert!(wildcard.contains(&json!(100)));
}

#[test]
fn descendant_name_equivalence_matrix() {
    let data = json!({
        "store": {
            "book": [
                {"title": "Book 1", "price": 10},
                {"title": "Book 2", "price": 20}
            ],
            "bicycle": {"price": 100}
        }
    });
    let prices = eval_values("$..price", &data);
    assert_eq!(prices.len(), 3);
    assert!(prices.contains(&json!(10)));
    assert!(prices.contains(&json!(20)));
    assert!(prices.contains(&json!(100)));

    let dot = eval_values("$..title", &data);
    let bracket = eval_values("$..['title']", &data);
    assert_eq!(dot, bracket);
}

#[test]
fn descendant_codegen_parity_matrix() {
    let data = json!({
        "a": {"b": {"price": 1}},
        "c": [{"price": 2}, {"price": 3}]
    });
    let eval_all = eval_values("$..*", &data);
    let codegen_all = codegen_values("$..*", &data);
    assert_eq!(codegen_all, eval_all);

    let eval_price = eval_values("$..price", &data);
    let codegen_price = codegen_values("$..price", &data);
    assert_eq!(codegen_price, eval_price);
}
