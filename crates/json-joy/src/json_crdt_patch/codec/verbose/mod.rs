//! Verbose JSON codec for JSON CRDT Patches.
//!
//! The verbose format represents each operation as a JSON object with named
//! fields and a string `op` discriminator. Human-readable but larger than
//! the binary or compact formats.
//!
//! Mirrors `packages/json-joy/src/json-crdt-patch/codec/verbose/`.

mod decode;
mod encode;
pub mod types;

pub use decode::decode;
pub use encode::encode;
