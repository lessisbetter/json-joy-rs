use serde_json::Value;

use crate::JsonPointerError;

use super::v6::find_by_pointer_v6;

/// Upstream `findByPointer/v4` compatibility entrypoint.
pub fn find_by_pointer_v4(
    pointer: &str,
    val: &Value,
) -> Result<(Option<Value>, String), JsonPointerError> {
    find_by_pointer_v6(pointer, val)
}
