//! JSON Pointer (RFC 6901) utilities.
//!
//! Upstream source family mapping (`json-pointer/src`):
//! - `find.ts` -> `find.rs`
//! - `get.ts` -> `get.rs`
//! - `util.ts` -> `util.rs`
//! - `findByPointer/*` -> `findByPointer/*`
//! - `codegen/*` -> `codegen/*`
//! - `index.ts` -> `index.rs`
//!
//! Rust divergence note:
//! - Upstream path casing is preserved for easier file-by-file sync.
//! - Rust module identifiers stay snake_case via `#[path = "..."]`.
//! - The `findByPointer` `v1..v6` and `codegen/*` families are mirrored for
//!   layout parity, but currently route to shared runtime implementations.

use thiserror::Error;

pub mod codegen;
mod find;
#[path = "findByPointer/mod.rs"]
pub mod find_by_pointer;
mod get;
mod index;
pub mod types;
mod util;
pub mod validate;

pub use index::*;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum JsonPointerError {
    #[error("NOT_FOUND")]
    NotFound,
    #[error("INVALID_INDEX")]
    InvalidIndex,
    #[error("NO_PARENT")]
    NoParent,
    #[error("POINTER_INVALID")]
    PointerInvalid,
    #[error("POINTER_TOO_LONG")]
    PointerTooLong,
    #[error("Invalid path")]
    InvalidPath,
    #[error("Path too long")]
    PathTooLong,
    #[error("Invalid path step")]
    InvalidPathStep,
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    #[test]
    fn test_unescape_component() {
        // No escapes needed
        assert_eq!(unescape_component("foo"), "foo");

        // Escape sequences
        assert_eq!(unescape_component("a~0b"), "a~b");
        assert_eq!(unescape_component("c~1d"), "c/d");
        assert_eq!(unescape_component("a~0b~1c"), "a~b/c");

        // Multiple of same
        assert_eq!(unescape_component("~0~0"), "~~");
        assert_eq!(unescape_component("~1~1"), "//");
    }

    #[test]
    fn test_escape_component() {
        // No escapes needed
        assert_eq!(escape_component("foo"), "foo");

        // Escape sequences
        assert_eq!(escape_component("a~b"), "a~0b");
        assert_eq!(escape_component("c/d"), "c~1d");
        assert_eq!(escape_component("a~b/c"), "a~0b~1c");

        // Multiple of same
        assert_eq!(escape_component("~~"), "~0~0");
        assert_eq!(escape_component("//"), "~1~1");
    }

    #[test]
    fn test_parse_json_pointer() {
        // Root
        assert_eq!(parse_json_pointer(""), Vec::<String>::new());

        // Single empty component
        assert_eq!(parse_json_pointer("/"), vec![""]);

        // Normal path
        assert_eq!(parse_json_pointer("/foo/bar"), vec!["foo", "bar"]);

        // With escapes
        assert_eq!(parse_json_pointer("/a~0b/c~1d"), vec!["a~b", "c/d"]);

        // Trailing slashes
        assert_eq!(parse_json_pointer("/foo///"), vec!["foo", "", "", ""]);

        // Numeric step
        assert_eq!(parse_json_pointer("/a~0b/c~1d/1"), vec!["a~b", "c/d", "1"]);
    }

    #[test]
    fn test_format_json_pointer() {
        // Root
        assert_eq!(format_json_pointer(&[]), "");

        // Single component
        assert_eq!(format_json_pointer(&["foo".to_string()]), "/foo");

        // Multiple components
        assert_eq!(
            format_json_pointer(&["foo".to_string(), "bar".to_string()]),
            "/foo/bar"
        );

        // With escapes
        assert_eq!(
            format_json_pointer(&["a~b".to_string(), "c/d".to_string()]),
            "/a~0b/c~1d"
        );

        // Empty string component
        assert_eq!(format_json_pointer(&["".to_string()]), "/");
    }

    #[test]
    fn test_is_root() {
        assert!(is_root(&[]));
        assert!(!is_root(&["foo".to_string()]));
    }

    #[test]
    fn test_is_child() {
        let parent = vec!["foo".to_string()];
        let child = vec!["foo".to_string(), "bar".to_string()];
        let sibling = vec!["baz".to_string()];

        assert!(is_child(&parent, &child));
        assert!(!is_child(&child, &parent));
        assert!(!is_child(&parent, &sibling));
        assert!(!is_child(&parent, &parent));
    }

    #[test]
    fn test_is_path_equal() {
        let p1 = vec!["foo".to_string(), "bar".to_string()];
        let p2 = vec!["foo".to_string(), "bar".to_string()];
        let p3 = vec!["foo".to_string(), "baz".to_string()];

        assert!(is_path_equal(&p1, &p2));
        assert!(!is_path_equal(&p1, &p3));
    }

    #[test]
    fn test_parent() {
        let path = vec!["foo".to_string(), "bar".to_string()];
        assert_eq!(parent(&path).unwrap(), vec!["foo"]);

        let single = vec!["foo".to_string()];
        assert_eq!(parent(&single).unwrap(), Vec::<String>::new());

        let root: Vec<String> = vec![];
        assert!(parent(&root).is_err());
    }

    #[test]
    fn test_is_valid_index() {
        assert!(is_valid_index("0"));
        assert!(is_valid_index("123"));
        assert!(!is_valid_index("-1"));
        assert!(!is_valid_index("1.5"));
        assert!(!is_valid_index("abc"));
        assert!(!is_valid_index(""));
        assert!(!is_valid_index("01")); // Leading zero not allowed
    }

    #[test]
    fn test_is_integer() {
        assert!(is_integer("0"));
        assert!(is_integer("123"));
        assert!(!is_integer("-1"));
        assert!(!is_integer("1.5"));
        assert!(!is_integer(""));
        assert!(!is_integer("abc"));
    }

    #[test]
    fn test_get_scalar_root() {
        assert_eq!(get(&json!(123), &[]), Some(&json!(123)));
        assert_eq!(get(&json!("foo"), &[]), Some(&json!("foo")));
    }

    #[test]
    fn test_get_object_key() {
        let doc = json!({"foo": "bar"});
        assert_eq!(get(&doc, &["foo".to_string()]), Some(&json!("bar")));
        assert_eq!(get(&doc, &["missing".to_string()]), None);
    }

    #[test]
    fn test_get_nested() {
        let doc = json!({"foo": {"bar": {"baz": "qux"}}});
        assert_eq!(
            get(
                &doc,
                &["foo".to_string(), "bar".to_string(), "baz".to_string()]
            ),
            Some(&json!("qux"))
        );
    }

    #[test]
    fn test_get_array_element() {
        let doc = json!([1, 2, 3]);
        assert_eq!(get(&doc, &["0".to_string()]), Some(&json!(1)));
        assert_eq!(get(&doc, &["1".to_string()]), Some(&json!(2)));
        assert_eq!(get(&doc, &["3".to_string()]), None);
    }

    #[test]
    fn test_get_array_dash() {
        let doc = json!([1, 2, 3]);
        assert_eq!(get(&doc, &["-".to_string()]), None);
    }

    #[test]
    fn test_get_mixed() {
        let doc = json!({"a": {"b": [1, 2, 3]}});
        assert_eq!(
            get(&doc, &["a".to_string(), "b".to_string(), "1".to_string()]),
            Some(&json!(2))
        );
    }

    #[test]
    fn test_find_scalar_root() {
        let ref_val = find(&json!(123), &[]).unwrap();
        assert_eq!(ref_val.val, Some(json!(123)));
        assert!(ref_val.obj.is_none());
        assert!(ref_val.key.is_none());
    }

    #[test]
    fn test_find_object_key() {
        let doc = json!({"foo": "bar"});
        let ref_val = find(&doc, &["foo".to_string()]).unwrap();
        assert_eq!(ref_val.val, Some(json!("bar")));
        assert_eq!(ref_val.obj, Some(doc.clone()));
        assert_eq!(ref_val.key, Some(ReferenceKey::String("foo".to_string())));
    }

    // Bug 1: missing key must be None; explicit null must be Some(Null).

    #[test]
    fn test_find_missing_key_returns_none() {
        let doc = json!({"foo": 123});
        let ref_val = find(&doc, &["bar".to_string()]).unwrap();
        // Missing key → val is None
        assert_eq!(ref_val.val, None);
        assert_eq!(ref_val.obj, Some(doc.clone()));
        assert_eq!(ref_val.key, Some(ReferenceKey::String("bar".to_string())));
    }

    #[test]
    fn test_find_explicit_null_returns_some_null() {
        // Bug 1 fix: a key that exists with a null value returns Some(Null),
        // not None. This is distinct from a missing key.
        let doc = json!({"foo": null});
        let ref_val = find(&doc, &["foo".to_string()]).unwrap();
        assert_eq!(ref_val.val, Some(Value::Null));
        assert_eq!(ref_val.obj, Some(doc.clone()));
        assert_eq!(ref_val.key, Some(ReferenceKey::String("foo".to_string())));
    }

    // Bug 2: array access yields ReferenceKey::Index, not ReferenceKey::String.

    #[test]
    fn test_find_array_element_key_is_index() {
        // Bug 2 fix: key for array element is ReferenceKey::Index(n).
        let doc = json!({"a": {"b": [1, 2, 3]}});
        let ref_val = find(&doc, &["a".to_string(), "b".to_string(), "1".to_string()]).unwrap();
        assert_eq!(ref_val.val, Some(json!(2)));
        assert_eq!(ref_val.key, Some(ReferenceKey::Index(1)));
        assert_eq!(ref_val.index(), Some(1));
    }

    #[test]
    fn test_find_array_dash_key_is_length() {
        // "-" resolves to arr.len() as a numeric index.
        let doc = json!({"a": {"b": [1, 2, 3]}});
        let ref_val = find(&doc, &["a".to_string(), "b".to_string(), "-".to_string()]).unwrap();
        // val is None (one past the end)
        assert_eq!(ref_val.val, None);
        // key is the array length (3) as a numeric index
        assert_eq!(ref_val.key, Some(ReferenceKey::Index(3)));
        assert!(ref_val.is_array_end());
    }

    #[test]
    fn test_find_invalid_index() {
        let doc = json!({"a": [1, 2, 3]});
        let result = find(&doc, &["a".to_string(), "-1".to_string()]);
        assert!(matches!(result, Err(JsonPointerError::InvalidIndex)));
    }

    #[test]
    fn test_find_not_found() {
        let doc = json!({"a": 123});
        let result = find(&doc, &["b".to_string(), "c".to_string()]);
        // "b" is missing → returns Reference with val=None, not an error,
        // because it is the *last* step.  But "c" needs to traverse through
        // the result of "b", which is missing — that is a mid-path miss →
        // NotFound.
        assert!(matches!(result, Err(JsonPointerError::NotFound)));
    }

    #[test]
    fn test_find_array_past_end_key_is_index() {
        let doc = json!({"a": {"b": [1, 2, 3]}});
        let ref_val = find(&doc, &["a".to_string(), "b".to_string(), "3".to_string()]).unwrap();
        assert_eq!(ref_val.val, None);
        assert_eq!(ref_val.key, Some(ReferenceKey::Index(3)));
        assert!(ref_val.is_array_end());
    }

    #[test]
    fn test_find_by_pointer() {
        let doc = json!({"foo": {"bar": 42}});

        let (obj, key) = find_by_pointer("/foo/bar", &doc).unwrap();
        assert_eq!(key, "bar");
        assert_eq!(obj, Some(json!({"bar": 42})));

        // Root
        let (obj, key) = find_by_pointer("", &doc).unwrap();
        assert_eq!(key, "");
        assert_eq!(obj, Some(doc.clone()));
    }

    #[test]
    fn test_get_explicit_null() {
        // get() borrows and returns the null reference directly.
        let doc = json!({"foo": null});
        let val = get(&doc, &["foo".to_string()]);
        assert_eq!(val, Some(&Value::Null));
    }

    #[test]
    fn test_roundtrip() {
        let pointers = vec![
            "",
            "/",
            "/foo",
            "/foo/bar",
            "/a~0b",
            "/c~1d",
            "/a~0b/c~1d/1",
            "/foo///",
        ];

        for pointer in pointers {
            let path = parse_json_pointer(pointer);
            let formatted = format_json_pointer(&path);
            assert_eq!(formatted, pointer, "Failed roundtrip for: {:?}", pointer);
        }
    }

    // --- Upstream parity: findByPointer spec scenarios ---

    #[test]
    fn test_upstream_find_key_in_object() {
        let doc = json!({"foo": "bar"});
        let r = find(&doc, &["foo".to_string()]).unwrap();
        assert_eq!(r.val, Some(json!("bar")));
        assert_eq!(r.key, Some(ReferenceKey::String("foo".to_string())));
    }

    #[test]
    fn test_upstream_find_returns_container_and_key() {
        let doc = json!({"foo": {"bar": {"baz": "qux", "a": 1}}});
        let r = find(
            &doc,
            &["foo".to_string(), "bar".to_string(), "baz".to_string()],
        )
        .unwrap();
        assert_eq!(r.val, Some(json!("qux")));
        assert_eq!(r.obj, Some(json!({"baz": "qux", "a": 1})));
        assert_eq!(r.key, Some(ReferenceKey::String("baz".to_string())));
    }

    #[test]
    fn test_upstream_find_array_element_numeric_key() {
        // Upstream: { val: 2, obj: [1,2,3], key: 1 }
        let doc = json!({"a": {"b": [1, 2, 3]}});
        let r = find(&doc, &["a".to_string(), "b".to_string(), "1".to_string()]).unwrap();
        assert_eq!(r.val, Some(json!(2)));
        assert_eq!(r.obj, Some(json!([1, 2, 3])));
        assert_eq!(r.key, Some(ReferenceKey::Index(1)));
    }

    #[test]
    fn test_upstream_find_end_of_array() {
        // Upstream: { val: undefined, obj: [1,2,3], key: 3 }
        let doc = json!({"a": {"b": [1, 2, 3]}});
        let r = find(&doc, &["a".to_string(), "b".to_string(), "-".to_string()]).unwrap();
        assert_eq!(r.val, None);
        assert_eq!(r.obj, Some(json!([1, 2, 3])));
        assert_eq!(r.key, Some(ReferenceKey::Index(3)));
        assert!(r.is_array_reference());
        assert!(r.is_array_end());
    }

    #[test]
    fn test_upstream_find_one_past_array_boundary() {
        // Upstream: { val: undefined, obj: [1,2,3], key: 3 }
        let doc = json!({"a": {"b": [1, 2, 3]}});
        let r = find(&doc, &["a".to_string(), "b".to_string(), "3".to_string()]).unwrap();
        assert_eq!(r.val, None);
        assert_eq!(r.obj, Some(json!([1, 2, 3])));
        assert_eq!(r.key, Some(ReferenceKey::Index(3)));
        assert!(r.is_array_reference());
        assert!(r.is_array_end());
    }

    #[test]
    fn test_upstream_find_missing_object_key() {
        // Upstream: { val: undefined, obj: {foo:123}, key: 'bar' }
        let doc = json!({"foo": 123});
        let r = find(&doc, &["bar".to_string()]).unwrap();
        assert_eq!(r.val, None);
        assert_eq!(r.obj, Some(json!({"foo": 123})));
        assert_eq!(r.key, Some(ReferenceKey::String("bar".to_string())));
    }

    #[test]
    fn test_upstream_find_missing_array_key_within_bounds_numeric() {
        // Upstream: { val: undefined, obj: [1,2,3], key: 3 }
        let doc = json!({"foo": 123, "bar": [1, 2, 3]});
        let r = find(&doc, &["bar".to_string(), "3".to_string()]).unwrap();
        assert_eq!(r.val, None);
        assert_eq!(r.obj, Some(json!([1, 2, 3])));
        assert_eq!(r.key, Some(ReferenceKey::Index(3)));
    }

    #[test]
    fn test_upstream_throws_missing_key_mid_path() {
        // Upstream: findByPointer('/b/c', {a:123}) throws
        let doc = json!({"a": 123});
        let result = find(&doc, &["b".to_string(), "c".to_string()]);
        assert!(result.is_err());
    }

    #[test]
    fn test_upstream_throws_invalid_index() {
        // Upstream: findByPointer('/a/b/-1', doc) throws
        let doc = json!({"a": {"b": [1, 2, 3]}});
        let result = find(&doc, &["a".to_string(), "b".to_string(), "-1".to_string()]);
        assert!(matches!(result, Err(JsonPointerError::InvalidIndex)));
    }
}
