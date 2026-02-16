//! JSON Pointer utilities aligned with `packages/json-pointer` upstream shape.
//!
//! Upstream references:
//! - `/Users/nchapman/Code/json-joy/packages/json-pointer/src/util.ts`

use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum JsonPointerError {
    #[error("json pointer must be absolute or empty")]
    NotAbsolute,
}

/// Unescapes one JSON Pointer token component.
pub fn unescape_component(component: &str) -> String {
    if !component.contains('~') {
        return component.to_string();
    }
    component.replace("~1", "/").replace("~0", "~")
}

/// Escapes one JSON Pointer token component.
pub fn escape_component(component: &str) -> String {
    if !component.contains('/') && !component.contains('~') {
        return component.to_string();
    }
    component.replace('~', "~0").replace('/', "~1")
}

/// Parse RFC6901 absolute pointer into unescaped path components.
///
/// Examples:
/// - `"" -> []`
/// - `"/" -> [""]`
/// - `"/a~1b/~0k/0" -> ["a/b", "~k", "0"]`
pub fn parse_json_pointer(pointer: &str) -> Result<Vec<String>, JsonPointerError> {
    if pointer.is_empty() {
        return Ok(Vec::new());
    }
    if !pointer.starts_with('/') {
        return Err(JsonPointerError::NotAbsolute);
    }
    Ok(pointer.split('/').skip(1).map(unescape_component).collect())
}

/// Parse pointer with upstream convenience behavior:
/// - relative strings are accepted by prefixing `/`.
pub fn parse_json_pointer_relaxed(pointer: &str) -> Result<Vec<String>, JsonPointerError> {
    if pointer.starts_with('/') || pointer.is_empty() {
        return parse_json_pointer(pointer);
    }
    let mut absolute = String::with_capacity(pointer.len() + 1);
    absolute.push('/');
    absolute.push_str(pointer);
    parse_json_pointer(&absolute)
}

/// Format unescaped path components into RFC6901 pointer.
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
