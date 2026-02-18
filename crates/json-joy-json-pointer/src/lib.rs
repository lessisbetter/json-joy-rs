//! JSON Pointer (RFC 6901) utilities.
//!
//! This crate implements helper functions for [JSON Pointer (RFC 6901)](https://tools.ietf.org/html/rfc6901).
//!
//! # Example
//!
//! ```
//! use json_joy_json_pointer::{parse_json_pointer, format_json_pointer, find, get};
//!
//! // Parse a JSON pointer string into path components
//! let path = parse_json_pointer("/foo/bar");
//! assert_eq!(path, vec!["foo".to_string(), "bar".to_string()]);
//!
//! // Format path components back to a JSON pointer string
//! let pointer = format_json_pointer(&path);
//! assert_eq!(pointer, "/foo/bar");
//!
//! // Get a value from a JSON document
//! let doc = serde_json::json!({"foo": {"bar": 42}});
//! let val = get(&doc, &path);
//! assert_eq!(val, Some(&serde_json::json!(42)));
//! ```

use serde_json::Value;
use std::borrow::Cow;
use thiserror::Error;

// Re-export types
pub mod types;
pub use types::{Path, PathStep, Reference, ReferenceKey};

// Re-export validation
pub mod validate;
pub use validate::{validate_json_pointer, validate_path, ValidationError};

/// Unescapes a JSON Pointer path component.
///
/// Per RFC 6901, `~1` is replaced with `/` and `~0` is replaced with `~`.
///
/// # Example
///
/// ```
/// use json_joy_json_pointer::unescape_component;
///
/// assert_eq!(unescape_component("a~0b"), "a~b");
/// assert_eq!(unescape_component("c~1d"), "c/d");
/// assert_eq!(unescape_component("no-escapes"), "no-escapes");
/// ```
pub fn unescape_component(component: &str) -> String {
    if !component.contains('~') {
        return component.to_string();
    }
    // Order matters: ~1 must be replaced before ~0
    component.replace("~1", "/").replace("~0", "~")
}

/// Escapes a JSON Pointer path component.
///
/// Per RFC 6901, `/` is replaced with `~1` and `~` is replaced with `~0`.
///
/// # Example
///
/// ```
/// use json_joy_json_pointer::escape_component;
///
/// assert_eq!(escape_component("a~b"), "a~0b");
/// assert_eq!(escape_component("c/d"), "c~1d");
/// assert_eq!(escape_component("no-escapes"), "no-escapes");
/// ```
pub fn escape_component(component: &str) -> String {
    if !component.contains('/') && !component.contains('~') {
        return component.to_string();
    }
    // Order matters: ~ must be escaped before /
    component.replace('~', "~0").replace('/', "~1")
}

/// Parse a JSON Pointer string into path components.
///
/// Follows the upstream `parseJsonPointer` behavior:
/// - Empty string returns empty vec
/// - The leading `/` is stripped
/// - Each component is unescaped
///
/// # Example
///
/// ```
/// use json_joy_json_pointer::parse_json_pointer;
///
/// assert_eq!(parse_json_pointer(""), Vec::<String>::new());
/// assert_eq!(parse_json_pointer("/"), vec![""]);
/// assert_eq!(parse_json_pointer("/foo/bar"), vec!["foo", "bar"]);
/// assert_eq!(parse_json_pointer("/a~0b/c~1d"), vec!["a~b", "c/d"]);
/// ```
pub fn parse_json_pointer(pointer: &str) -> Vec<String> {
    if pointer.is_empty() {
        return Vec::new();
    }
    // Upstream behavior: slice(1) then split
    pointer[1..].split('/').map(unescape_component).collect()
}

/// Parse a JSON Pointer string that may not have a leading `/`.
///
/// This is a convenience function that handles both absolute and relative pointers.
pub fn parse_json_pointer_relaxed(pointer: &str) -> Vec<String> {
    if pointer.starts_with('/') || pointer.is_empty() {
        return parse_json_pointer(pointer);
    }
    let mut absolute = String::with_capacity(pointer.len() + 1);
    absolute.push('/');
    absolute.push_str(pointer);
    parse_json_pointer(&absolute)
}

/// Format path components into a JSON Pointer string.
///
/// Returns an empty string for the root path (empty components).
///
/// # Example
///
/// ```
/// use json_joy_json_pointer::format_json_pointer;
///
/// assert_eq!(format_json_pointer(&[]), "");
/// assert_eq!(format_json_pointer(&["foo".to_string()]), "/foo");
/// assert_eq!(format_json_pointer(&["foo".to_string(), "bar".to_string()]), "/foo/bar");
/// ```
pub fn format_json_pointer(path: &[String]) -> String {
    if path.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    for component in path {
        out.push('/');
        out.push_str(&escape_component(component));
    }
    out
}

/// Convert a pointer string to a path.
///
/// If already a path (vec), returns it as-is.
pub fn to_path<'a>(pointer: impl Into<Cow<'a, str>>) -> Vec<String> {
    parse_json_pointer(&pointer.into())
}

/// Check if a path points to the root value.
///
/// # Example
///
/// ```
/// use json_joy_json_pointer::is_root;
///
/// assert!(is_root(&[]));
/// assert!(!is_root(&["foo".to_string()]));
/// ```
pub fn is_root(path: &[String]) -> bool {
    path.is_empty()
}

/// Check if `parent` path contains the `child` path.
///
/// # Example
///
/// ```
/// use json_joy_json_pointer::is_child;
///
/// let parent = vec!["foo".to_string()];
/// let child = vec!["foo".to_string(), "bar".to_string()];
/// assert!(is_child(&parent, &child));
/// assert!(!is_child(&child, &parent));
/// ```
pub fn is_child(parent: &[String], child: &[String]) -> bool {
    if parent.len() >= child.len() {
        return false;
    }
    for i in 0..parent.len() {
        if parent[i] != child[i] {
            return false;
        }
    }
    true
}

/// Check if two paths are equal.
pub fn is_path_equal(p1: &[String], p2: &[String]) -> bool {
    if p1.len() != p2.len() {
        return false;
    }
    for i in 0..p1.len() {
        if p1[i] != p2[i] {
            return false;
        }
    }
    true
}

/// Get the parent path of a given path.
///
/// # Errors
///
/// Returns an error if the path has no parent (is empty/root).
///
/// # Example
///
/// ```
/// use json_joy_json_pointer::parent;
///
/// assert_eq!(parent(&["foo".to_string(), "bar".to_string()]).unwrap(), vec!["foo"]);
/// assert!(parent(&[]).is_err());
/// ```
pub fn parent(path: &[String]) -> Result<Vec<String>, JsonPointerError> {
    if path.is_empty() {
        return Err(JsonPointerError::NoParent);
    }
    Ok(path[..path.len() - 1].to_vec())
}

/// Check if a string represents a valid non-negative integer array index.
///
/// # Example
///
/// ```
/// use json_joy_json_pointer::is_valid_index;
///
/// assert!(is_valid_index("0"));
/// assert!(is_valid_index("123"));
/// assert!(!is_valid_index("-1"));
/// assert!(!is_valid_index("1.5"));
/// assert!(!is_valid_index("abc"));
/// ```
pub fn is_valid_index(index: &str) -> bool {
    if index.is_empty() {
        return false;
    }
    let bytes = index.as_bytes();
    // First char can't be leading zero unless it's just "0"
    if bytes.len() > 1 && bytes[0] == b'0' {
        return false;
    }
    bytes.iter().all(|&b| b.is_ascii_digit())
}

/// Check if a string consists only of ASCII digits.
pub fn is_integer(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    s.bytes().all(|b| b.is_ascii_digit())
}

/// Find a value in a JSON document by path.
///
/// Returns a [`Reference`] containing the value, its container object, and key.
///
/// Key semantics (mirrors upstream TypeScript):
/// - `ref.key` is `ReferenceKey::Index(n)` when the container is an array.
/// - `ref.key` is `ReferenceKey::String(s)` when the container is an object.
/// - `ref.val` is `None` when the location does not exist (missing key or
///   out-of-bounds index).  An explicit JSON `null` is returned as
///   `Some(Value::Null)`, so callers can distinguish null from missing.
///
/// # Errors
///
/// - `JsonPointerError::NotFound` - if a *parent* path step doesn't exist
/// - `JsonPointerError::InvalidIndex` - if an invalid array index is used
///
/// # Example
///
/// ```
/// use json_joy_json_pointer::{find, ReferenceKey};
/// use serde_json::json;
///
/// let doc = json!({"foo": {"bar": 42}});
/// let ref_val = find(&doc, &["foo".to_string(), "bar".to_string()]).unwrap();
/// assert_eq!(ref_val.val, Some(json!(42)));
/// assert_eq!(ref_val.key, Some(ReferenceKey::String("bar".to_string())));
/// ```
pub fn find(val: &Value, path: &[String]) -> Result<Reference, JsonPointerError> {
    if path.is_empty() {
        return Ok(Reference {
            val: Some(val.clone()),
            obj: None,
            key: None,
        });
    }

    let path_len = path.len();
    let mut current: &Value = val;
    let mut obj: Option<Value> = None;
    let mut key: Option<ReferenceKey> = None;

    for (step_idx, path_step) in path.iter().enumerate() {
        let is_last = step_idx == path_len - 1;
        obj = Some(current.clone());

        match current {
            Value::Array(arr) => {
                // Handle "-" as end-of-array sentinel (one past last).
                let idx: usize = if path_step == "-" {
                    arr.len()
                } else {
                    if !is_valid_index(path_step) {
                        return Err(JsonPointerError::InvalidIndex);
                    }
                    path_step
                        .parse()
                        .map_err(|_| JsonPointerError::InvalidIndex)?
                };
                key = Some(ReferenceKey::Index(idx));
                match arr.get(idx) {
                    Some(v) => current = v,
                    None => {
                        // Out-of-bounds. Only valid as the last step; a
                        // mid-path out-of-bounds means we cannot continue
                        // traversal → NotFound.
                        if !is_last {
                            return Err(JsonPointerError::NotFound);
                        }
                        return Ok(Reference {
                            val: None,
                            obj,
                            key,
                        });
                    }
                }
            }
            Value::Object(map) => {
                let step_key = path_step.clone();
                key = Some(ReferenceKey::String(step_key.clone()));
                match map.get(&step_key) {
                    Some(v) => current = v,
                    None => {
                        // Missing key. Only valid as the last step; a
                        // mid-path missing key means we cannot continue
                        // traversal → NotFound.
                        if !is_last {
                            return Err(JsonPointerError::NotFound);
                        }
                        return Ok(Reference {
                            val: None,
                            obj,
                            key,
                        });
                    }
                }
            }
            _ => return Err(JsonPointerError::NotFound),
        }
    }

    Ok(Reference {
        val: Some(current.clone()),
        obj,
        key,
    })
}

/// Get a value from a JSON document by path.
///
/// Returns `None` if the path doesn't exist or is invalid.
///
/// # Example
///
/// ```
/// use json_joy_json_pointer::get;
/// use serde_json::json;
///
/// let doc = json!({"foo": {"bar": 42}});
/// let val = get(&doc, &["foo".to_string(), "bar".to_string()]);
/// assert_eq!(val, Some(&json!(42)));
///
/// let missing = get(&doc, &["missing".to_string()]);
/// assert_eq!(missing, None);
/// ```
pub fn get<'a>(val: &'a Value, path: &[String]) -> Option<&'a Value> {
    let path_length = path.len();
    if path_length == 0 {
        return Some(val);
    }

    let mut current = val;
    for path_step in path {
        match current {
            Value::Array(arr) => {
                // Handle "-" as end of array (returns None)
                if path_step == "-" {
                    return None;
                }
                // Parse index
                let idx: usize = match path_step.parse() {
                    Ok(i) => i,
                    Err(_) => return None,
                };
                current = arr.get(idx)?;
            }
            Value::Object(map) => {
                current = map.get(path_step)?;
            }
            _ => return None,
        }
    }
    Some(current)
}

/// Get a mutable reference to a value in a JSON document by path.
///
/// Returns `None` if the path doesn't exist or is invalid.
pub fn get_mut<'a>(val: &'a mut Value, path: &[String]) -> Option<&'a mut Value> {
    let path_length = path.len();
    if path_length == 0 {
        return Some(val);
    }

    let mut current = val;
    for path_step in path {
        match current {
            Value::Array(arr) => {
                if path_step == "-" {
                    return None;
                }
                let idx: usize = path_step.parse().ok()?;
                current = arr.get_mut(idx)?;
            }
            Value::Object(map) => {
                current = map.get_mut(path_step)?;
            }
            _ => return None,
        }
    }
    Some(current)
}

/// Find by pointer string directly.
///
/// Returns `(container_object, key_string)` where `key_string` is the last
/// path component (for callers that only need a string key).  For full
/// parity — including a numeric key for array targets — use [`find`] instead.
///
/// # Example
///
/// ```
/// use json_joy_json_pointer::find_by_pointer;
/// use serde_json::json;
///
/// let doc = json!({"foo": {"bar": 42}});
/// let (obj, key) = find_by_pointer("/foo/bar", &doc).unwrap();
/// assert_eq!(key, "bar");
/// ```
#[allow(unused_assignments)]
pub fn find_by_pointer(
    pointer: &str,
    val: &Value,
) -> Result<(Option<Value>, String), JsonPointerError> {
    if pointer.is_empty() {
        return Ok((Some(val.clone()), String::new()));
    }

    let mut current: &Value = val;
    let mut obj: Option<Value> = None;
    let mut key = String::new();

    // Parse and traverse in one pass
    let mut start = 1; // Skip leading /
    for (i, c) in pointer.char_indices() {
        if c == '/' && i > 0 {
            let component = &pointer[start..i];
            key = unescape_component(component);
            obj = Some(current.clone());

            match current {
                Value::Array(arr) => {
                    let idx: usize = if key == "-" {
                        arr.len()
                    } else {
                        if !is_valid_index(&key) {
                            return Err(JsonPointerError::InvalidIndex);
                        }
                        key.parse().map_err(|_| JsonPointerError::InvalidIndex)?
                    };
                    current = arr.get(idx).unwrap_or(&Value::Null);
                }
                Value::Object(map) => {
                    current = map.get(&key).unwrap_or(&Value::Null);
                }
                _ => return Err(JsonPointerError::NotFound),
            }
            start = i + 1;
        }
    }

    // Handle last component (including empty-string key when pointer ends with '/').
    // RFC 6901: "/" (single slash) addresses the key "" in the root object.
    if start <= pointer.len() {
        let component = &pointer[start..];
        key = unescape_component(component);
        obj = Some(current.clone());

        match current {
            Value::Array(arr) => {
                let idx: usize = if key == "-" {
                    arr.len()
                } else {
                    if !is_valid_index(&key) {
                        return Err(JsonPointerError::InvalidIndex);
                    }
                    key.parse().map_err(|_| JsonPointerError::InvalidIndex)?
                };
                current = arr.get(idx).unwrap_or(&Value::Null);
            }
            Value::Object(map) => {
                current = map.get(&key).unwrap_or(&Value::Null);
            }
            _ => return Err(JsonPointerError::NotFound),
        }
    }

    Ok((obj, key))
}

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
    use serde_json::json;

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
        let ref_val = find(
            &doc,
            &["a".to_string(), "b".to_string(), "1".to_string()],
        )
        .unwrap();
        assert_eq!(ref_val.val, Some(json!(2)));
        assert_eq!(ref_val.key, Some(ReferenceKey::Index(1)));
        assert_eq!(ref_val.index(), Some(1));
    }

    #[test]
    fn test_find_array_dash_key_is_length() {
        // "-" resolves to arr.len() as a numeric index.
        let doc = json!({"a": {"b": [1, 2, 3]}});
        let ref_val = find(
            &doc,
            &["a".to_string(), "b".to_string(), "-".to_string()],
        )
        .unwrap();
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
        let ref_val = find(
            &doc,
            &["a".to_string(), "b".to_string(), "3".to_string()],
        )
        .unwrap();
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
        let r = find(
            &doc,
            &["a".to_string(), "b".to_string(), "1".to_string()],
        )
        .unwrap();
        assert_eq!(r.val, Some(json!(2)));
        assert_eq!(r.obj, Some(json!([1, 2, 3])));
        assert_eq!(r.key, Some(ReferenceKey::Index(1)));
    }

    #[test]
    fn test_upstream_find_end_of_array() {
        // Upstream: { val: undefined, obj: [1,2,3], key: 3 }
        let doc = json!({"a": {"b": [1, 2, 3]}});
        let r = find(
            &doc,
            &["a".to_string(), "b".to_string(), "-".to_string()],
        )
        .unwrap();
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
        let r = find(
            &doc,
            &["a".to_string(), "b".to_string(), "3".to_string()],
        )
        .unwrap();
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
        let result = find(
            &doc,
            &["a".to_string(), "b".to_string(), "-1".to_string()],
        );
        assert!(matches!(result, Err(JsonPointerError::InvalidIndex)));
    }
}
