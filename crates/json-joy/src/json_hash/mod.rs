//! JSON structural hashing.
//!
//! Mirrors `packages/json-joy/src/json-hash/`.
//!
//! Provides:
//! - `hash` — 32-bit numeric hash of any JSON value
//! - `struct_hash` — printable ASCII structural hash string

pub mod hash;
pub mod struct_hash;
pub mod struct_hash_crdt;
pub mod struct_hash_schema;

pub use hash::{hash, hash_str, update_bin, update_json, update_num, update_str};
pub use struct_hash::struct_hash;
pub use struct_hash_crdt::struct_hash_crdt;
pub use struct_hash_schema::struct_hash_schema;
