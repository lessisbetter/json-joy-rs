//! Binary codec for the JSON CRDT Patch protocol.
//!
//! Mirrors `packages/json-joy/src/json-crdt-patch/codec/binary/`.

mod encoder;
mod decoder;

pub use encoder::Encoder;
pub use decoder::{Decoder, DecodeError};

use crate::json_crdt_patch::patch::Patch;

/// Encodes a patch to binary using a shared encoder instance.
pub fn encode(patch: &Patch) -> Vec<u8> {
    let mut enc = Encoder::new();
    enc.encode(patch)
}

/// Decodes a binary blob into a patch.
pub fn decode(data: &[u8]) -> Result<Patch, DecodeError> {
    let dec = Decoder::new();
    dec.decode(data)
}
