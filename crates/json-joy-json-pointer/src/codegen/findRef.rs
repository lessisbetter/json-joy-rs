use serde_json::Value;

use crate::{find_by_pointer, JsonPointerError};

/// Mirrors upstream `codegen/findRef.ts` API surface.
///
/// Rust divergence: upstream emits specialized JavaScript functions;
/// Rust returns a closure that forwards to the runtime implementation.
pub fn codegen_find_ref(
    pointer: String,
) -> impl Fn(&Value) -> Result<(Option<Value>, String), JsonPointerError> {
    move |value| find_by_pointer(&pointer, value)
}
