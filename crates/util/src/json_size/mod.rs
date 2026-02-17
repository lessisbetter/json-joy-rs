//! JSON size calculation utilities.
//!
//! Provides functions for calculating and approximating the size of JSON values
//! when serialized.

mod json;
mod json_size_fast;
mod max_encoding_capacity;

pub use json::{json_size, json_size_approx, utf8_size};
pub use json_size_fast::{json_size_fast, MaxEncodingOverhead};
pub use max_encoding_capacity::{max_encoding_capacity, max_encoding_capacity_binary};
