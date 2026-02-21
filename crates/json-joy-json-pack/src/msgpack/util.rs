//! Convenience MessagePack helpers.
//!
//! Upstream reference: `json-pack/src/msgpack/util.ts`

use crate::PackValue;

use super::{types::MsgPack, MsgPackDecoderFast, MsgPackEncoder, MsgPackEncoderFast, MsgPackError};

/// Encode using the fast MessagePack encoder.
pub fn encode(value: &PackValue) -> MsgPack {
    let mut encoder = MsgPackEncoderFast::new();
    encoder.encode(value)
}

/// Encode using the full MessagePack encoder.
pub fn encode_full(value: &PackValue) -> MsgPack {
    let mut encoder = MsgPackEncoder::new();
    encoder.encode(value)
}

/// Decode using the fast MessagePack decoder.
pub fn decode(blob: &[u8]) -> Result<PackValue, MsgPackError> {
    let mut decoder = MsgPackDecoderFast::new();
    decoder.decode(blob)
}
