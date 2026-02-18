//! JSON structural hashing.
//!
//! Mirrors `packages/json-joy/src/json-hash/`.
//!
//! Provides:
//! - `hash` — 32-bit numeric hash of any JSON value
//! - `struct_hash` — printable ASCII structural hash string

pub mod hash;
pub mod struct_hash;

pub use hash::{hash, hash_str, update_json, update_num, update_str, update_bin};
pub use struct_hash::struct_hash;
