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
fn exec_index_and_slice_matrix_matches_upstream_examples() {
    let array = json!(["a", "b", "c", "d", "e", "f", "g"]);

    assert_eq!(eval_values("$[1]", &array), vec![json!("b")]);
    assert_eq!(eval_values("$[-2]", &array), vec![json!("f")]);
    assert!(eval_values("$[10]", &array).is_empty());
    assert!(eval_values("$[-10]", &array).is_empty());

    assert_eq!(eval_values("$[1:3]", &array), vec![json!("b"), json!("c")]);
    assert_eq!(eval_values("$[5:]", &array), vec![json!("f"), json!("g")]);
    assert_eq!(
        eval_values("$[1:5:2]", &array),
        vec![json!("b"), json!("d")]
    );
    assert_eq!(
        eval_values("$[5:1:-2]", &array),
        vec![json!("f"), json!("d")]
    );
    assert_eq!(
        eval_values("$[::-1]", &array),
        vec![
            json!("g"),
            json!("f"),
            json!("e"),
            json!("d"),
            json!("c"),
            json!("b"),
            json!("a")
        ]
    );
    assert!(eval_values("$[1:5:0]", &array).is_empty());
    assert!(eval_values("$[1:3]", &json!({"not": "array"})).is_empty());
}

#[test]
fn exec_recursive_descent_root_wildcard_matrix() {
    let data = json!({
        "type": "Program",
        "body": [],
        "sourceType": "module",
        "range": [0, 1718]
    });
    let result = eval_values("$..*", &data);
    assert_eq!(result.len(), 6);
    assert!(result.contains(&json!("Program")));
    assert!(result.contains(&json!([])));
    assert!(result.contains(&json!("module")));
    assert!(result.contains(&json!([0, 1718])));
    assert!(result.contains(&json!(0)));
    assert!(result.contains(&json!(1718)));
}

#[test]
fn exec_codegen_eval_parity_for_complex_function_filters_matrix() {
    let data = json!({
        "store": {
            "book": [
                {
                    "category": "reference",
                    "author": "Nigel Rees",
                    "title": "Sayings of the Century",
                    "price": 8.95
                },
                {
                    "category": "fiction",
                    "author": "Evelyn Waugh",
                    "title": "Sword of Honour",
                    "price": 12.99
                },
                {
                    "category": "fiction",
                    "author": "Herman Melville",
                    "title": "Moby Dick",
                    "isbn": "0-553-21311-3",
                    "price": 8.99
                },
                {
                    "category": "fiction",
                    "author": "J. R. R. Tolkien",
                    "title": "The Lord of the Rings",
                    "isbn": "0-395-19395-8",
                    "price": 22.99
                }
            ]
        },
        "authors": ["John", "Jane", "Bob"],
        "info": {
            "name": "Test Store",
            "contacts": {
                "email": "test@store.com",
                "phone": "123-456-7890"
            }
        }
    });

    let expressions = [
        "$[?length(@.authors) == count(@.authors[*])]",
        "$[?count(@.store.book[?length(@.title) > 15]) == 2]",
        "$.store.book[?length(value(@.title)) > 10]",
        "$.store.book[?length(@.title) > 10 && search(@.category, \"fiction\")]",
        "$.store.book[?value(@.isbn) != null]",
        "$.store.book[?match(@.title, \".*Lord.*\")]",
        "$.store.book[?search(@.author, \"[JE].*\")]",
        "$.store.book[?match(@.title, \"[\")]",
        "$[?unknown(@.name) == true]",
        "$[?length(@.name, @.other) == 5]",
    ];

    for expr in expressions {
        let eval_out = eval_values(expr, &data);
        let codegen_out = codegen_values(expr, &data);
        assert_eq!(codegen_out, eval_out, "expression: {expr}");
    }
}

#[test]
fn exec_codegen_eval_parity_for_recursive_and_union_matrix() {
    let data = json!({
        "store": {
            "book": [
                {"title": "Book 1", "price": 10},
                {"title": "Book 2", "price": 20}
            ],
            "bicycle": {"price": 100}
        },
        "other": [
            ["x", "y"],
            ["z"]
        ]
    });

    let expressions = [
        "$..*",
        "$..price",
        "$..[0]",
        "$[0, 0]",
        "$[0:2, 5]",
        "$[*, 0, 'key']",
        "$..nonexistent",
    ];

    for expr in expressions {
        let eval_out = eval_values(expr, &data);
        let codegen_out = codegen_values(expr, &data);
        assert_eq!(codegen_out, eval_out, "expression: {expr}");
    }
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
