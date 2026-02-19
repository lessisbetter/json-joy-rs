use serde_json::Value;

use crate::{find, JsonPointerError, Reference};

/// Mirrors upstream `codegen/find.ts` API surface.
///
/// Rust divergence: upstream emits specialized JavaScript functions;
/// Rust returns a closure that forwards to the runtime implementation.
pub fn codegen_find(path: Vec<String>) -> impl Fn(&Value) -> Result<Reference, JsonPointerError> {
    move |value| find(value, &path)
}
