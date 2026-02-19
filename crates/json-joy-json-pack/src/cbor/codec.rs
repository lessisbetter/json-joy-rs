//! `CborJsonValueCodec` — combined encoder/decoder pair.
//!
//! Mirrors `codecs/cbor.ts` from upstream.

use serde_json::Value;

use super::decoder::decode_json_from_cbor_bytes;
use super::encoder::CborEncoder;
use super::error::CborError;

#[derive(Default)]
pub struct CborJsonValueCodec {
    encoder: CborEncoder,
}

impl CborJsonValueCodec {
    pub fn new() -> Self {
        Self {
            encoder: CborEncoder::new(),
        }
    }

    pub fn encode(&mut self, value: &Value) -> Result<Vec<u8>, CborError> {
        Ok(self.encoder.encode_json(value))
    }

    pub fn decode(&self, bytes: &[u8]) -> Result<Value, CborError> {
        decode_json_from_cbor_bytes(bytes)
    }
}
