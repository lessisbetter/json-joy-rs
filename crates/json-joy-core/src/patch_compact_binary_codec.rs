//! Native compact-binary patch codec port (`codec/compact-binary/*`).

use crate::patch::Patch;
use crate::patch_compact_codec::{decode_patch_compact, encode_patch_compact, CompactCodecError};
use json_joy_json_pack::{
    decode_json_from_cbor_bytes, encode_json_to_cbor_bytes, CborError as JsonPackCborError,
};

#[derive(Debug, thiserror::Error)]
pub enum CompactBinaryCodecError {
    #[error("compact codec failed: {0}")]
    Compact(#[from] CompactCodecError),
    #[error("invalid compact-binary cbor payload")]
    InvalidCbor,
}

pub fn encode_patch_compact_binary(patch: &Patch) -> Result<Vec<u8>, CompactBinaryCodecError> {
    let compact = encode_patch_compact(patch)?;
    encode_json_to_cbor_bytes(&compact).map_err(map_cbor_error)
}

pub fn decode_patch_compact_binary(data: &[u8]) -> Result<Patch, CompactBinaryCodecError> {
    let compact = decode_json_from_cbor_bytes(data).map_err(map_cbor_error)?;
    Ok(decode_patch_compact(&compact)?)
}

fn map_cbor_error(error: JsonPackCborError) -> CompactBinaryCodecError {
    match error {
        JsonPackCborError::InvalidPayload | JsonPackCborError::Unsupported => {
            CompactBinaryCodecError::InvalidCbor
        }
    }
}
