//! Compact JSON codec for JSON CRDT Patches.
//!
//! The compact format represents each operation as a JSON array with a numeric
//! opcode as the first element. Timestamps are relative to the patch ID where
//! possible.
//!
//! Mirrors `packages/json-joy/src/json-crdt-patch/codec/compact/`.

mod decode;
mod encode;
pub mod types;

pub use decode::decode;
pub use encode::encode;
