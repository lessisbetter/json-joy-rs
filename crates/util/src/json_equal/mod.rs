//! JSON equality utilities.
//!
//! Provides deep equality comparison functions for JSON values.

mod deep_equal;

pub use deep_equal::{deep_equal, deep_equal_binary};

// Re-export JsonBinary for convenience
pub use crate::json_clone::JsonBinary;
