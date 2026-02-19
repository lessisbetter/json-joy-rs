//! JSONPath (RFC 9535) implementation.
//!
//! This crate provides parsing and evaluation of JSONPath expressions
//! as specified in [RFC 9535](https://www.rfc-editor.org/rfc/rfc9535.html).
//!
//! # Example
//!
//! ```
//! use json_joy_json_path::{JsonPathParser, JsonPathEval};
//! use serde_json::json;
//!
//! // Parse a JSONPath expression
//! let path = JsonPathParser::parse("$.store.books[*].author").unwrap();
//!
//! // Evaluate against a JSON document
//! let doc = json!({
//!     "store": {
//!         "books": [
//!             {"author": "Nigel Rees", "title": "Sayings of the Century"},
//!             {"author": "Evelyn Waugh", "title": "Sword of Honour"}
//!         ]
//!     }
//! });
//!
//! let results = JsonPathEval::eval(&path, &doc);
//! assert_eq!(results.len(), 2);
//! ```

mod types;
pub use types::*;

mod ast;
pub use ast::Ast;

mod parser;
pub use parser::{JsonPathParser, ParseError};

mod eval;
pub use eval::JsonPathEval;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_root() {
        let path = JsonPathParser::parse("$").unwrap();
        assert_eq!(path.segments.len(), 0);
    }

    #[test]
    fn test_parse_dot_notation() {
        let path = JsonPathParser::parse("$.store.books").unwrap();
        assert_eq!(path.segments.len(), 2);
    }

    #[test]
    fn test_parse_bracket_notation() {
        let path = JsonPathParser::parse("$['store']['books']").unwrap();
        assert_eq!(path.segments.len(), 2);
    }

    #[test]
    fn test_parse_wildcard() {
        let path = JsonPathParser::parse("$.store.*").unwrap();
        assert_eq!(path.segments.len(), 2);
    }

    #[test]
    fn test_parse_index() {
        let path = JsonPathParser::parse("$.books[0]").unwrap();
        assert_eq!(path.segments.len(), 2);
    }

    #[test]
    fn test_parse_slice() {
        let path = JsonPathParser::parse("$.books[1:3]").unwrap();
        assert_eq!(path.segments.len(), 2);
    }

    #[test]
    fn test_eval_root() {
        let doc = json!({"a": 1});
        let path = JsonPathParser::parse("$").unwrap();
        let results = JsonPathEval::eval(&path, &doc);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], &doc);
    }

    #[test]
    fn test_eval_dot_notation() {
        let doc = json!({"a": {"b": 42}});
        let path = JsonPathParser::parse("$.a.b").unwrap();
        let results = JsonPathEval::eval(&path, &doc);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], &json!(42));
    }

    #[test]
    fn test_eval_wildcard() {
        let doc = json!({"a": 1, "b": 2});
        let path = JsonPathParser::parse("$.*").unwrap();
        let results = JsonPathEval::eval(&path, &doc);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_eval_array_index() {
        let doc = json!([1, 2, 3, 4, 5]);
        let path = JsonPathParser::parse("$[2]").unwrap();
        let results = JsonPathEval::eval(&path, &doc);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], &json!(3));
    }

    #[test]
    fn test_eval_array_slice() {
        let doc = json!([1, 2, 3, 4, 5]);
        let path = JsonPathParser::parse("$[1:3]").unwrap();
        let results = JsonPathEval::eval(&path, &doc);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0], &json!(2));
        assert_eq!(results[1], &json!(3));
    }

    #[test]
    fn test_eval_negative_index() {
        let doc = json!([1, 2, 3, 4, 5]);
        let path = JsonPathParser::parse("$[-1]").unwrap();
        let results = JsonPathEval::eval(&path, &doc);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], &json!(5));
    }

    #[test]
    fn test_eval_empty_result() {
        let doc = json!({"a": 1});
        let path = JsonPathParser::parse("$.missing").unwrap();
        let results = JsonPathEval::eval(&path, &doc);
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_eval_array_wildcard() {
        let doc = json!([1, 2, 3]);
        let path = JsonPathParser::parse("$[*]").unwrap();
        let results = JsonPathEval::eval(&path, &doc);
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_eval_nested_path() {
        let doc = json!({"store": {"books": [{"title": "Book 1"}, {"title": "Book 2"}]}});
        let path = JsonPathParser::parse("$.store.books").unwrap();
        let results = JsonPathEval::eval(&path, &doc);
        assert_eq!(results.len(), 1);
        assert!(results[0].is_array());
    }

    #[test]
    fn test_parse_quoted_string() {
        let path = JsonPathParser::parse("$['store name']").unwrap();
        assert_eq!(path.segments.len(), 1);
    }

    // ---- Filter expression parser tests ----

    #[test]
    fn test_parse_filter_existence() {
        // ?(@.field) — existence check
        let path = JsonPathParser::parse("$[?(@.field)]").unwrap();
        assert_eq!(path.segments.len(), 1);
        let seg = &path.segments[0];
        assert_eq!(seg.selectors.len(), 1);
        match &seg.selectors[0] {
            Selector::Filter(FilterExpression::Existence { path: inner }) => {
                assert_eq!(inner.segments.len(), 1);
            }
            other => panic!("Expected existence filter, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_filter_eq_string() {
        // ?(@.field == "value")
        let path = JsonPathParser::parse(r#"$[?(@.field == "value")]"#).unwrap();
        let seg = &path.segments[0];
        match &seg.selectors[0] {
            Selector::Filter(FilterExpression::Comparison {
                operator,
                left,
                right,
            }) => {
                assert_eq!(*operator, ComparisonOperator::Equal);
                match left {
                    ValueExpression::Path(p) => assert_eq!(p.segments.len(), 1),
                    other => panic!("Expected path left, got {:?}", other),
                }
                match right {
                    ValueExpression::Literal(serde_json::Value::String(s)) => {
                        assert_eq!(s, "value");
                    }
                    other => panic!("Expected string literal right, got {:?}", other),
                }
            }
            other => panic!("Expected comparison filter, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_filter_gt_number() {
        // ?(@.price > 5)
        let path = JsonPathParser::parse("$[?(@.price > 5)]").unwrap();
        let seg = &path.segments[0];
        match &seg.selectors[0] {
            Selector::Filter(FilterExpression::Comparison { operator, .. }) => {
                assert_eq!(*operator, ComparisonOperator::Greater);
            }
            other => panic!("Expected comparison filter, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_filter_all_comparison_ops() {
        for (op_str, expected_op) in &[
            ("==", ComparisonOperator::Equal),
            ("!=", ComparisonOperator::NotEqual),
            ("<", ComparisonOperator::Less),
            ("<=", ComparisonOperator::LessEqual),
            (">", ComparisonOperator::Greater),
            (">=", ComparisonOperator::GreaterEqual),
        ] {
            let expr = format!("$[?(@.n {} 1)]", op_str);
            let path = JsonPathParser::parse(&expr)
                .unwrap_or_else(|e| panic!("Failed to parse '{}': {:?}", expr, e));
            match &path.segments[0].selectors[0] {
                Selector::Filter(FilterExpression::Comparison { operator, .. }) => {
                    assert_eq!(operator, expected_op, "operator mismatch for '{}'", op_str);
                }
                other => panic!("Expected comparison for '{}', got {:?}", op_str, other),
            }
        }
    }

    #[test]
    fn test_parse_filter_logical_and() {
        // ?(@.field > 5 && @.other == "x")
        let path = JsonPathParser::parse(r#"$[?(@.field > 5 && @.other == "x")]"#).unwrap();
        match &path.segments[0].selectors[0] {
            Selector::Filter(FilterExpression::Logical {
                operator,
                left,
                right,
            }) => {
                assert_eq!(*operator, LogicalOperator::And);
                assert!(matches!(
                    left.as_ref(),
                    FilterExpression::Comparison {
                        operator: ComparisonOperator::Greater,
                        ..
                    }
                ));
                assert!(matches!(
                    right.as_ref(),
                    FilterExpression::Comparison {
                        operator: ComparisonOperator::Equal,
                        ..
                    }
                ));
            }
            other => panic!("Expected logical AND, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_filter_logical_or() {
        // ?(@.field == "a" || @.field == "b")
        let path = JsonPathParser::parse(r#"$[?(@.field == "a" || @.field == "b")]"#).unwrap();
        match &path.segments[0].selectors[0] {
            Selector::Filter(FilterExpression::Logical { operator, .. }) => {
                assert_eq!(*operator, LogicalOperator::Or);
            }
            other => panic!("Expected logical OR, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_filter_negation() {
        // ?(!@.field)
        let path = JsonPathParser::parse("$[?(!@.field)]").unwrap();
        match &path.segments[0].selectors[0] {
            Selector::Filter(FilterExpression::Negation(inner)) => {
                assert!(matches!(inner.as_ref(), FilterExpression::Existence { .. }));
            }
            other => panic!("Expected negation, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_filter_paren() {
        // ?(@.a == 1 && (@.b == 2 || @.c == 3))
        let path = JsonPathParser::parse("$[?(@.a == 1 && (@.b == 2 || @.c == 3))]").unwrap();
        match &path.segments[0].selectors[0] {
            Selector::Filter(FilterExpression::Logical {
                operator,
                left,
                right,
            }) => {
                assert_eq!(*operator, LogicalOperator::And);
                assert!(matches!(left.as_ref(), FilterExpression::Comparison { .. }));
                // The right side should be a paren wrapping an OR
                match right.as_ref() {
                    FilterExpression::Paren(inner) => {
                        assert!(matches!(
                            inner.as_ref(),
                            FilterExpression::Logical {
                                operator: LogicalOperator::Or,
                                ..
                            }
                        ));
                    }
                    other => panic!("Expected paren on right, got {:?}", other),
                }
            }
            other => panic!("Expected logical AND, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_filter_literal_bool_true() {
        let path = JsonPathParser::parse("$[?(@.active == true)]").unwrap();
        match &path.segments[0].selectors[0] {
            Selector::Filter(FilterExpression::Comparison { right, .. }) => {
                assert_eq!(right, &ValueExpression::Literal(json!(true)));
            }
            other => panic!("Expected comparison, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_filter_literal_null() {
        let path = JsonPathParser::parse("$[?(@.val == null)]").unwrap();
        match &path.segments[0].selectors[0] {
            Selector::Filter(FilterExpression::Comparison { right, .. }) => {
                assert_eq!(right, &ValueExpression::Literal(json!(null)));
            }
            other => panic!("Expected comparison, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_filter_float_literal() {
        let path = JsonPathParser::parse("$[?(@.price > 9.99)]").unwrap();
        match &path.segments[0].selectors[0] {
            Selector::Filter(FilterExpression::Comparison { right, .. }) => match right {
                ValueExpression::Literal(serde_json::Value::Number(n)) => {
                    let v = n.as_f64().unwrap();
                    assert!((v - 9.99).abs() < 1e-9, "expected 9.99, got {}", v);
                }
                other => panic!("Expected number literal, got {:?}", other),
            },
            other => panic!("Expected comparison, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_filter_single_quoted_string() {
        let path = JsonPathParser::parse("$[?(@.field == 'hello')]").unwrap();
        match &path.segments[0].selectors[0] {
            Selector::Filter(FilterExpression::Comparison { right, .. }) => {
                assert_eq!(right, &ValueExpression::Literal(json!("hello")));
            }
            other => panic!("Expected comparison, got {:?}", other),
        }
    }

    // ---- Filter expression evaluator tests ----

    #[test]
    fn test_eval_filter_existence() {
        let doc = json!([
            {"name": "Alice", "age": 30},
            {"age": 25},
            {"name": "Bob"}
        ]);
        let path = JsonPathParser::parse("$[?(@.name)]").unwrap();
        let results = JsonPathEval::eval(&path, &doc);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_eval_filter_eq() {
        let doc = json!([
            {"name": "Alice", "age": 30},
            {"name": "Bob", "age": 25},
            {"name": "Alice", "age": 20}
        ]);
        let path = JsonPathParser::parse(r#"$[?(@.name == "Alice")]"#).unwrap();
        let results = JsonPathEval::eval(&path, &doc);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_eval_filter_ne() {
        let doc = json!([
            {"status": "active"},
            {"status": "inactive"},
            {"status": "active"}
        ]);
        let path = JsonPathParser::parse(r#"$[?(@.status != "inactive")]"#).unwrap();
        let results = JsonPathEval::eval(&path, &doc);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_eval_filter_gt() {
        let doc = json!([
            {"price": 5},
            {"price": 10},
            {"price": 3}
        ]);
        let path = JsonPathParser::parse("$[?(@.price > 5)]").unwrap();
        let results = JsonPathEval::eval(&path, &doc);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0]["price"], json!(10));
    }

    #[test]
    fn test_eval_filter_gte() {
        let doc = json!([
            {"price": 5},
            {"price": 10},
            {"price": 3}
        ]);
        let path = JsonPathParser::parse("$[?(@.price >= 5)]").unwrap();
        let results = JsonPathEval::eval(&path, &doc);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_eval_filter_lt() {
        let doc = json!([
            {"price": 5},
            {"price": 10},
            {"price": 3}
        ]);
        let path = JsonPathParser::parse("$[?(@.price < 5)]").unwrap();
        let results = JsonPathEval::eval(&path, &doc);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0]["price"], json!(3));
    }

    #[test]
    fn test_eval_filter_lte() {
        let doc = json!([
            {"price": 5},
            {"price": 10},
            {"price": 3}
        ]);
        let path = JsonPathParser::parse("$[?(@.price <= 5)]").unwrap();
        let results = JsonPathEval::eval(&path, &doc);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_eval_filter_logical_and() {
        let doc = json!([
            {"field": 10, "other": "x"},
            {"field": 10, "other": "y"},
            {"field": 3, "other": "x"}
        ]);
        let path = JsonPathParser::parse(r#"$[?(@.field > 5 && @.other == "x")]"#).unwrap();
        let results = JsonPathEval::eval(&path, &doc);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0]["field"], json!(10));
        assert_eq!(results[0]["other"], json!("x"));
    }

    #[test]
    fn test_eval_filter_logical_or() {
        let doc = json!([
            {"field": "a"},
            {"field": "b"},
            {"field": "c"}
        ]);
        let path = JsonPathParser::parse(r#"$[?(@.field == "a" || @.field == "b")]"#).unwrap();
        let results = JsonPathEval::eval(&path, &doc);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_eval_filter_negation() {
        let doc = json!([
            {"active": true},
            {},
            {"active": false}
        ]);
        let path = JsonPathParser::parse("$[?(!@.active)]").unwrap();
        let results = JsonPathEval::eval(&path, &doc);
        // Only the element without .active passes the negation
        assert_eq!(results.len(), 1);
        assert!(!results[0].as_object().unwrap().contains_key("active"));
    }

    #[test]
    fn test_eval_filter_nested_paren() {
        let doc = json!([
            {"a": 1, "b": 2, "c": 0},
            {"a": 1, "b": 0, "c": 3},
            {"a": 1, "b": 0, "c": 0},
            {"a": 2, "b": 2, "c": 3}
        ]);
        // a == 1 && (b == 2 || c == 3)
        let path = JsonPathParser::parse("$[?(@.a == 1 && (@.b == 2 || @.c == 3))]").unwrap();
        let results = JsonPathEval::eval(&path, &doc);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0]["b"], json!(2));
        assert_eq!(results[1]["c"], json!(3));
    }

    #[test]
    fn test_eval_filter_object_members() {
        // Filter also works on object values
        let doc = json!({
            "alice": {"age": 30},
            "bob": {"age": 25},
            "carol": {"age": 35}
        });
        let path = JsonPathParser::parse("$[?(@.age > 28)]").unwrap();
        let results = JsonPathEval::eval(&path, &doc);
        assert_eq!(results.len(), 2);
    }
}
