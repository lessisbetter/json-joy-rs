use json_joy_json_pointer::{find, find_by_pointer, parse_json_pointer};
use serde_json::json;

#[test]
fn test_find_by_pointer_empty_component() {
    let doc = json!({"": "value", "foo": "bar"});

    // RFC 6901: "/" addresses the key "" in the root object.
    let result = find_by_pointer("/", &doc);
    assert!(result.is_ok(), "Should find empty key at root");

    // "/foo/" means key "" inside the value of "foo" ("bar" is a string, not an object).
    // RFC 6901: traversing into a scalar is an error.
    let result = find_by_pointer("/foo/", &doc);
    assert!(
        result.is_err(),
        "Trailing slash into a string should return an error"
    );
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
    // RFC 6901: "/foo//" means key "" inside doc["foo"][""].
    // doc["foo"] = {"": "value"}, doc["foo"][""] = "value" (a string),
    // so doc["foo"][""][""] cannot exist → should be an error.
    let doc = json!({"foo": {"": "value"}});
    let result = find_by_pointer("/foo//", &doc);
    assert!(
        result.is_err(),
        "Double slash traversing into a string should return an error"
    );

    // But "/foo/" → doc["foo"][""] = "value" → should succeed.
    let result = find_by_pointer("/foo/", &doc);
    assert!(
        result.is_ok(),
        "Single trailing slash into nested empty key should succeed"
    );
}
