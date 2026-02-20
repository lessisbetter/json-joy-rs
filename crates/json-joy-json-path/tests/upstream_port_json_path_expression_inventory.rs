use json_joy_json_path::JsonPathParser;

#[test]
fn upstream_valid_expression_inventory_matrix() {
    let valid = [
        "$",
        "$.name",
        "$['name']",
        "$[\"name\"]",
        "$[0]",
        "$[-1]",
        "$.*",
        "$[*]",
        "$[1:3]",
        "$[1:10:2]",
        "$[::4]",
        "$[2:]",
        "$[:3]",
        "$..author",
        "$..*",
        "$..[0]",
        "$[0,1]",
        "$[0, 'name', 2]",
        "$[0:2, 5, 'key']",
        "$[?(@.price < 10)]",
        "$[?@.price < 10]",
        "$[?@.isbn && @.price < 20]",
        "$[?(!@.isbn)]",
        "$[?((@.price < 10) && (@.category == \"fiction\"))]",
        "$[?(@.book[0].author == \"Tolkien\")]",
        "$[?length(@.name) > 5]",
        "$[?count(@.items) == 3]",
        "$[?match(@.email, \".*@example\\\\.com\")]",
        "$[?search(@.description, \"test\")]",
        "$[?@['single-quotes']]",
        "$[?@[-1]]",
        "$..book[?@.isbn]",
        "$..book[?@.price<10]",
        "$.store.book[*].author",
        "$.store.book[0,1]",
        "$.store.book[-1]",
        "$.store.book[0:2]",
        "$.store['book', 'bicycle'][0, -1, 'title']",
    ];

    for expr in valid {
        if let Err(e) = JsonPathParser::parse(expr) {
            panic!("expected valid expression '{expr}', got error: {e}");
        }
    }
}

#[test]
fn upstream_invalid_expression_inventory_matrix() {
    let invalid = [
        "",
        ".name",
        "$.",
        "$..",
        "$[]",
        "$..[]",
        "$['unterminated",
        "$[invalid]",
        "$[0",
        "$[?(@.price < 10]",
    ];

    for expr in invalid {
        assert!(
            JsonPathParser::parse(expr).is_err(),
            "expected invalid expression '{expr}' to fail"
        );
    }
}
