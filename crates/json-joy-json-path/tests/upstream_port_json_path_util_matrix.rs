use json_joy_json_path::{
    get_accessed_properties, json_path_equals, json_path_to_string, JsonPathParser,
};

fn parse(path: &str) -> json_joy_json_path::JSONPath {
    JsonPathParser::parse(path).unwrap_or_else(|e| panic!("parse failed for '{path}': {e}"))
}

#[test]
fn util_json_path_to_string_matrix() {
    assert_eq!(json_path_to_string(&parse("$.name")), "$.name");
    assert_eq!(
        json_path_to_string(&parse("$.store.book[0].title")),
        "$.store.book[0].title"
    );
    assert_eq!(json_path_to_string(&parse("$.store.*")), "$.store.*");
    assert_eq!(json_path_to_string(&parse("$.items[1:3]")), "$.items[1:3]");
    assert_eq!(
        json_path_to_string(&parse("$.items[1:10:2]")),
        "$.items[1:10:2]"
    );
    assert_eq!(
        json_path_to_string(&parse("$.store['book', 'bicycle'][0, 1]")),
        "$.store[.book,.bicycle][[0],[1]]"
    );
    assert_eq!(
        json_path_to_string(&parse("$[0, 'name', 2]")),
        "$[[0],.name,[2]]"
    );
}

#[test]
fn util_json_path_equals_matrix() {
    let path1 = parse("$.store.book[0].title");
    let path2 = parse("$.store.book[0].title");
    assert!(json_path_equals(&path1, &path2));

    let path1 = parse("$.store.book[0].title");
    let path2 = parse("$.store.book[1].title");
    assert!(!json_path_equals(&path1, &path2));

    let path1 = parse("$.store.book");
    let path2 = parse("$['store']['book']");
    assert!(json_path_equals(&path1, &path2));

    let path1 = parse("$.store.book");
    let path2 = parse("$.store.book[0]");
    assert!(!json_path_equals(&path1, &path2));
}

#[test]
fn util_get_accessed_properties_matrix() {
    assert_eq!(
        get_accessed_properties(&parse("$.store.book")),
        vec!["store".to_string(), "book".to_string()]
    );
    assert_eq!(
        get_accessed_properties(&parse("$.store.book[0].title")),
        vec!["store".to_string(), "book".to_string(), "title".to_string()]
    );
    assert_eq!(
        get_accessed_properties(&parse("$..author")),
        vec!["author".to_string()]
    );
    assert_eq!(
        get_accessed_properties(&parse("$.store.*[1:3]")),
        vec!["store".to_string()]
    );
    assert!(get_accessed_properties(&parse("$")).is_empty());
}
