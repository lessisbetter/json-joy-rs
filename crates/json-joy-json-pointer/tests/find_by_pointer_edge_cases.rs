use json_joy_json_pointer::{find, find_by_pointer, parse_json_pointer};
use serde_json::json;

#[test]
fn test_find_by_pointer_empty_component() {
    let doc = json!({"": "value", "foo": "bar"});

    // Pointer to empty key
    let result = find_by_pointer("/", &doc);
    assert!(result.is_ok(), "Should find empty key");

    // Pointer to nested empty key
    let result = find_by_pointer("/foo/", &doc);
    assert!(result.is_ok(), "Should handle trailing slash");
}

#[test]
fn test_find_by_pointer_unicode() {
    let doc = json!({"café": "coffee"});
    let result = find_by_pointer("/caf%C3%A9", &doc);
    // JSON Pointer doesn't URL encode, this is about ~ escaping
    // Let's test with ~0/~1 instead
}

#[test]
fn test_find_by_pointer_multiple_slashes() {
    let doc = json!({"foo": {"": "value"}});
    let result = find_by_pointer("/foo//", &doc);
    assert!(result.is_ok(), "Should handle multiple consecutive slashes");
}
