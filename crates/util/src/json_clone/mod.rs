//! JSON cloning utilities.
//!
//! Provides deep cloning functions for JSON values, including support
//! for binary data (Uint8Array).

mod clone;
mod clone_binary;

pub use clone::clone;
pub use clone_binary::{clone_binary, clone_value_with_binary, JsonBinary};
