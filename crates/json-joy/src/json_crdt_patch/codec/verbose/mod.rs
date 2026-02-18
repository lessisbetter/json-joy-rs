//! Verbose JSON codec for JSON CRDT Patches.
//!
//! The verbose format represents each operation as a JSON object with named
//! fields and a string `op` discriminator. Human-readable but larger than
//! the binary or compact formats.
//!
//! Mirrors `packages/json-joy/src/json-crdt-patch/codec/verbose/`.

mod encode;
mod decode;
pub mod types;

pub use encode::encode;
pub use decode::decode;
