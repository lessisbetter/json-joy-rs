//! Mirrors upstream `findByPointer/*` family.
//!
//! Rust divergence note:
//! - Upstream ships multiple optimized versions (`v1`..`v6`).
//! - Rust currently routes all variants to one canonical implementation (`v6`).

mod index;
pub mod v1;
pub mod v2;
pub mod v3;
pub mod v4;
pub mod v5;
pub mod v6;

pub use index::*;

use serde_json::Value;

use crate::JsonPointerError;

/// Default find-by-pointer function.
pub fn find_by_pointer(
    pointer: &str,
    val: &Value,
) -> Result<(Option<Value>, String), JsonPointerError> {
    v6::find_by_pointer_v6(pointer, val)
}
