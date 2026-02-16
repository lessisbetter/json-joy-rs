use serde_json::Value;

use super::decoder::decode_json_from_cbor_bytes;
use super::encoder_fast::encode_json_to_cbor_bytes;
use super::error::CborError;

#[derive(Debug, Default, Clone, Copy)]
pub struct CborJsonValueCodec;

impl CborJsonValueCodec {
    pub fn encode(self, value: &Value) -> Result<Vec<u8>, CborError> {
        encode_json_to_cbor_bytes(value)
    }

    pub fn decode(self, bytes: &[u8]) -> Result<Value, CborError> {
        decode_json_from_cbor_bytes(bytes)
    }
}
