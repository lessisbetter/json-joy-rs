//! Native compact-binary patch codec port (`codec/compact-binary/*`).

use crate::patch::Patch;
use crate::patch_compact_codec::{decode_patch_compact, encode_patch_compact, CompactCodecError};

#[derive(Debug, thiserror::Error)]
pub enum CompactBinaryCodecError {
    #[error("compact codec failed: {0}")]
    Compact(#[from] CompactCodecError),
    #[error("invalid compact-binary cbor payload")]
    InvalidCbor,
}

pub fn encode_patch_compact_binary(patch: &Patch) -> Result<Vec<u8>, CompactBinaryCodecError> {
    let compact = encode_patch_compact(patch)?;
    let mut out = Vec::new();
    ciborium::ser::into_writer(&compact, &mut out).map_err(|_| CompactBinaryCodecError::InvalidCbor)?;
    Ok(out)
}

pub fn decode_patch_compact_binary(data: &[u8]) -> Result<Patch, CompactBinaryCodecError> {
    let compact: serde_json::Value =
        ciborium::de::from_reader(data).map_err(|_| CompactBinaryCodecError::InvalidCbor)?;
    Ok(decode_patch_compact(&compact)?)
}

