use json_joy_json_path::{
    ComparisonOperator, FilterExpression, JsonPathParser, LogicalOperator, Selector,
    ValueExpression,
};
use serde_json::json;

#[test]
fn parser_union_selector_matrix() {
    let path = JsonPathParser::parse("$['a','b','c']").unwrap();
    assert_eq!(path.segments.len(), 1);
    assert_eq!(path.segments[0].selectors.len(), 3);
    assert!(matches!(path.segments[0].selectors[0], Selector::Name(_)));
    assert!(matches!(path.segments[0].selectors[1], Selector::Name(_)));
    assert!(matches!(path.segments[0].selectors[2], Selector::Name(_)));

    let path = JsonPathParser::parse("$[0, 'name', 2]").unwrap();
    assert_eq!(path.segments.len(), 1);
    assert_eq!(path.segments[0].selectors.len(), 3);
    assert!(matches!(path.segments[0].selectors[0], Selector::Index(0)));
    assert!(matches!(path.segments[0].selectors[1], Selector::Name(_)));
    assert!(matches!(path.segments[0].selectors[2], Selector::Index(2)));

    let path = JsonPathParser::parse("$[0:2, 5, 'key']").unwrap();
    assert_eq!(path.segments.len(), 1);
    assert_eq!(path.segments[0].selectors.len(), 3);
    assert!(matches!(
        path.segments[0].selectors[0],
        Selector::Slice { .. }
    ));
    assert!(matches!(path.segments[0].selectors[1], Selector::Index(5)));
    assert!(matches!(path.segments[0].selectors[2], Selector::Name(_)));

    let path = JsonPathParser::parse("$[*, 0, 'key']").unwrap();
    assert_eq!(path.segments.len(), 1);
    assert_eq!(path.segments[0].selectors.len(), 3);
    assert!(matches!(path.segments[0].selectors[0], Selector::Wildcard));
    assert!(matches!(path.segments[0].selectors[1], Selector::Index(0)));
    assert!(matches!(path.segments[0].selectors[2], Selector::Name(_)));

    let path = JsonPathParser::parse("$[ 0 , 'name' , 2 ]").unwrap();
    assert_eq!(path.segments.len(), 1);
    assert_eq!(path.segments[0].selectors.len(), 3);
    assert!(matches!(path.segments[0].selectors[0], Selector::Index(0)));
    assert!(matches!(path.segments[0].selectors[1], Selector::Name(_)));
    assert!(matches!(path.segments[0].selectors[2], Selector::Index(2)));

    let path = JsonPathParser::parse("$[-1, -2, 0]").unwrap();
    assert_eq!(path.segments.len(), 1);
    assert_eq!(path.segments[0].selectors.len(), 3);
    assert!(matches!(path.segments[0].selectors[0], Selector::Index(-1)));
    assert!(matches!(path.segments[0].selectors[1], Selector::Index(-2)));
    assert!(matches!(path.segments[0].selectors[2], Selector::Index(0)));

    let path = JsonPathParser::parse("$.store['book', 'bicycle'][0, -1, 'title']").unwrap();
    assert_eq!(path.segments.len(), 3);
    assert!(matches!(path.segments[0].selectors[0], Selector::Name(_)));
    assert_eq!(path.segments[1].selectors.len(), 2);
    assert!(matches!(path.segments[1].selectors[0], Selector::Name(_)));
    assert!(matches!(path.segments[1].selectors[1], Selector::Name(_)));
    assert_eq!(path.segments[2].selectors.len(), 3);
    assert!(matches!(path.segments[2].selectors[0], Selector::Index(0)));
    assert!(matches!(path.segments[2].selectors[1], Selector::Index(-1)));
    assert!(matches!(path.segments[2].selectors[2], Selector::Name(_)));
}

#[test]
fn parser_filter_existence_path_matrix() {
    let path = JsonPathParser::parse("$[?@.nested.property]").unwrap();
    let selector = &path.segments[0].selectors[0];
    match selector {
        Selector::Filter(FilterExpression::Existence { path }) => {
            assert_eq!(path.segments.len(), 2);
            assert!(matches!(path.segments[0].selectors[0], Selector::Name(_)));
            assert!(matches!(path.segments[1].selectors[0], Selector::Name(_)));
        }
        other => panic!("expected existence filter, got {other:?}"),
    }

    let path = JsonPathParser::parse("$[?@['key with spaces']]").unwrap();
    let selector = &path.segments[0].selectors[0];
    match selector {
        Selector::Filter(FilterExpression::Existence { path }) => {
            assert_eq!(path.segments.len(), 1);
            assert!(matches!(path.segments[0].selectors[0], Selector::Name(_)));
        }
        other => panic!("expected existence filter, got {other:?}"),
    }
}

#[test]
fn parser_recursive_with_filter_matrix() {
    let path = JsonPathParser::parse("$..book[?@.isbn]").unwrap();
    assert_eq!(path.segments.len(), 2);
    assert!(path.segments[0].recursive);
    assert!(matches!(path.segments[0].selectors[0], Selector::Name(_)));
    assert!(!path.segments[1].recursive);
    assert!(matches!(
        path.segments[1].selectors[0],
        Selector::Filter(FilterExpression::Existence { .. })
    ));

    let path = JsonPathParser::parse("$..book[?@.price<10]").unwrap();
    assert_eq!(path.segments.len(), 2);
    let filter = &path.segments[1].selectors[0];
    match filter {
        Selector::Filter(FilterExpression::Comparison {
            operator,
            left,
            right,
        }) => {
            assert_eq!(*operator, ComparisonOperator::Less);
            assert!(matches!(left, ValueExpression::Path(_)));
            assert!(matches!(right, ValueExpression::Literal(_)));
        }
        other => panic!("expected comparison filter, got {other:?}"),
    }
}

#[test]
fn parser_logical_filter_matrix() {
    let path = JsonPathParser::parse("$[?@.isbn && @.price < 20]").unwrap();
    let selector = &path.segments[0].selectors[0];
    match selector {
        Selector::Filter(FilterExpression::Logical {
            operator,
            left,
            right,
        }) => {
            assert_eq!(*operator, LogicalOperator::And);
            assert!(matches!(left.as_ref(), FilterExpression::Existence { .. }));
            assert!(matches!(
                right.as_ref(),
                FilterExpression::Comparison { .. }
            ));
        }
        other => panic!("expected logical filter, got {other:?}"),
    }
}

#[test]
fn parser_function_and_nested_filter_matrix() {
    let path = JsonPathParser::parse("$[?length(@.name)]").unwrap();
    let selector = &path.segments[0].selectors[0];
    match selector {
        Selector::Filter(FilterExpression::Function { name, args }) => {
            assert_eq!(name, "length");
            assert_eq!(args.len(), 1);
        }
        other => panic!("expected function filter, got {other:?}"),
    }

    let path =
        JsonPathParser::parse("$[?((@.price < 10 || @.price > 100) && @.category == \"book\")]")
            .unwrap();
    let selector = &path.segments[0].selectors[0];
    match selector {
        Selector::Filter(FilterExpression::Logical {
            operator,
            left,
            right,
        }) => {
            assert_eq!(*operator, LogicalOperator::And);
            assert!(matches!(left.as_ref(), FilterExpression::Paren(_)));
            assert!(matches!(
                right.as_ref(),
                FilterExpression::Comparison {
                    operator: ComparisonOperator::Equal,
                    ..
                }
            ));
        }
        other => panic!("expected nested logical filter, got {other:?}"),
    }

    let function_cases = [
        ("$[?length(@.name)]", "length"),
        ("$[?count(@.items)]", "count"),
        ("$[?match(@.email, \".*@example\\\\.com\")]", "match"),
        ("$[?search(@.description, \"test\")]", "search"),
    ];
    for (expr, expected_name) in function_cases {
        let path = JsonPathParser::parse(expr).unwrap();
        let selector = &path.segments[0].selectors[0];
        match selector {
            Selector::Filter(FilterExpression::Function { name, .. }) => {
                assert_eq!(name, expected_name, "expression: {expr}");
            }
            other => panic!("expected function filter for {expr}, got {other:?}"),
        }
    }
}

#[test]
fn parser_edge_case_syntax_matrix() {
    let path = JsonPathParser::parse("$['']").unwrap();
    assert_eq!(path.segments.len(), 1);
    assert!(matches!(
        &path.segments[0].selectors[0],
        Selector::Name(name) if name.is_empty()
    ));

    let path = JsonPathParser::parse("$['key with spaces']").unwrap();
    assert_eq!(path.segments.len(), 1);
    assert!(matches!(
        &path.segments[0].selectors[0],
        Selector::Name(name) if name == "key with spaces"
    ));

    let path = JsonPathParser::parse("$['key\\'with\\'quotes']").unwrap();
    assert_eq!(path.segments.len(), 1);
    assert!(matches!(
        &path.segments[0].selectors[0],
        Selector::Name(name) if name == "key'with'quotes"
    ));

    let path = JsonPathParser::parse("$ . store [ 'book' ] [ 0 ] . title ").unwrap();
    assert_eq!(path.segments.len(), 4);
    assert!(matches!(path.segments[0].selectors[0], Selector::Name(_)));
    assert!(matches!(path.segments[1].selectors[0], Selector::Name(_)));
    assert!(matches!(path.segments[2].selectors[0], Selector::Index(0)));
    assert!(matches!(path.segments[3].selectors[0], Selector::Name(_)));

    let path = JsonPathParser::parse("$[\"first\", \"second\", 0]").unwrap();
    assert_eq!(path.segments.len(), 1);
    assert_eq!(path.segments[0].selectors.len(), 3);
    assert!(matches!(path.segments[0].selectors[0], Selector::Name(_)));
    assert!(matches!(path.segments[0].selectors[1], Selector::Name(_)));
    assert!(matches!(path.segments[0].selectors[2], Selector::Index(0)));
}

#[test]
fn parser_existence_and_literal_filter_shapes_matrix() {
    let existence_cases = [
        "$[?@.items[0]]",
        "$[?@.data.values[*].name]",
        "$[?@['single-quotes']]",
        "$[?@[0].name]",
        "$[?@[-1]]",
    ];
    for expr in existence_cases {
        let path = JsonPathParser::parse(expr).unwrap();
        let selector = &path.segments[0].selectors[0];
        match selector {
            Selector::Filter(FilterExpression::Existence { path }) => {
                assert!(
                    !path.segments.is_empty(),
                    "existence path should not be empty for {expr}"
                );
            }
            other => panic!("expected existence filter for {expr}, got {other:?}"),
        }
    }

    let three_point_fourteen = "3.14".parse::<f64>().unwrap();
    let literal_cases = [
        ("$[?(@.active == true)]", json!(true)),
        ("$[?(@.active == false)]", json!(false)),
        ("$[?(@.value == null)]", json!(null)),
        ("$[?(@.price == 3.14)]", json!(three_point_fourteen)),
        ("$[?(@.name == 'test')]", json!("test")),
    ];
    for (expr, expected_right) in literal_cases {
        let path = JsonPathParser::parse(expr).unwrap();
        let selector = &path.segments[0].selectors[0];
        match selector {
            Selector::Filter(FilterExpression::Comparison { right, .. }) => {
                assert_eq!(right, &ValueExpression::Literal(expected_right), "{expr}");
            }
            other => panic!("expected comparison filter for {expr}, got {other:?}"),
        }
    }
}

#[test]
fn parser_error_matrix() {
    assert!(JsonPathParser::parse(".name").is_err());
    assert!(JsonPathParser::parse("$['unterminated").is_err());
    assert!(JsonPathParser::parse("$[invalid]").is_err());
    assert!(JsonPathParser::parse("$[0").is_err());
}
