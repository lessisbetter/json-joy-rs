use std::borrow::Cow;

use crate::JsonPointerError;

/// Unescapes a JSON Pointer path component.
pub fn unescape_component(component: &str) -> String {
    if !component.contains('~') {
        return component.to_string();
    }
    component.replace("~1", "/").replace("~0", "~")
}

/// Escapes a JSON Pointer path component.
pub fn escape_component(component: &str) -> String {
    if !component.contains('/') && !component.contains('~') {
        return component.to_string();
    }
    component.replace('~', "~0").replace('/', "~1")
}

/// Parse a JSON Pointer string into path components.
pub fn parse_json_pointer(pointer: &str) -> Vec<String> {
    if pointer.is_empty() {
        return Vec::new();
    }
    pointer[1..].split('/').map(unescape_component).collect()
}

/// Parse a JSON Pointer string that may not have a leading `/`.
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
pub fn to_path<'a>(pointer: impl Into<Cow<'a, str>>) -> Vec<String> {
    parse_json_pointer(&pointer.into())
}

/// Check if a path points to the root value.
pub fn is_root(path: &[String]) -> bool {
    path.is_empty()
}

/// Check if `parent` path contains the `child` path.
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
pub fn parent(path: &[String]) -> Result<Vec<String>, JsonPointerError> {
    if path.is_empty() {
        return Err(JsonPointerError::NoParent);
    }
    Ok(path[..path.len() - 1].to_vec())
}

/// Check if a string represents a valid non-negative integer array index.
pub fn is_valid_index(index: &str) -> bool {
    if index.is_empty() {
        return false;
    }
    let bytes = index.as_bytes();
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
