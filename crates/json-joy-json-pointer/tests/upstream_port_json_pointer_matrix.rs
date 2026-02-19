use json_joy_json_pointer::{
    find, find_by_pointer, format_json_pointer, get, is_child, parent, parse_json_pointer,
    validate_json_pointer, JsonPointerError, ReferenceKey,
};
use serde_json::json;

#[test]
fn pointer_parse_format_roundtrip_matrix() {
    let cases = [
        "",
        "/",
        "/foo",
        "/foo/bar",
        "/a~0b/c~1d",
        "/arr/0",
        "/~0/~1",
    ];

    for pointer in cases {
        let path = parse_json_pointer(pointer);
        let out = format_json_pointer(&path);
        assert_eq!(out, pointer);
    }
}

#[test]
fn pointer_find_and_get_matrix() {
    let doc = json!({"foo": {"bar": [10, 20, null]}});

    assert_eq!(
        get(&doc, &parse_json_pointer("/foo/bar/0")),
        Some(&json!(10))
    );
    assert_eq!(get(&doc, &parse_json_pointer("/foo/bar/3")), None);

    let r = find(&doc, &parse_json_pointer("/foo/bar/1")).expect("find ok");
    assert_eq!(r.val, Some(json!(20)));
    assert_eq!(r.key, Some(ReferenceKey::Index(1)));

    let r = find(&doc, &parse_json_pointer("/foo/bar/2")).expect("find null ok");
    assert_eq!(r.val, Some(json!(null)));
}

#[test]
fn pointer_find_by_pointer_matrix() {
    let doc = json!({"foo": {"": 1, "bar": [10, 20, 30]}});

    let (obj, key) = find_by_pointer("/foo/", &doc).expect("empty-key path");
    assert_eq!(key, "");
    assert_eq!(obj, Some(json!({"": 1, "bar": [10, 20, 30]})));

    let (obj, key) = find_by_pointer("/foo/bar/1", &doc).expect("array path");
    assert_eq!(key, "1");
    assert_eq!(obj, Some(json!([10, 20, 30])));
}

#[test]
fn pointer_validation_and_relationships() {
    assert!(validate_json_pointer("/foo/bar").is_ok());
    assert!(validate_json_pointer("foo/bar").is_err());

    let p = parse_json_pointer("/foo/bar");
    let q = parse_json_pointer("/foo/bar/baz");
    assert!(is_child(&p, &q));

    let parent_path = parent(&p).expect("has parent");
    assert_eq!(parent_path, vec!["foo".to_string()]);
}

#[test]
fn pointer_error_on_invalid_array_index() {
    let doc = json!({"arr": [1, 2, 3]});
    let result = find(&doc, &parse_json_pointer("/arr/-1"));
    assert!(matches!(result, Err(JsonPointerError::InvalidIndex)));
}
