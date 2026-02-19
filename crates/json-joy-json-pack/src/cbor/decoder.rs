//! `CborDecoder` — full CBOR decoder.
//!
//! Direct port of `cbor/CborDecoder.ts` from upstream.

use super::decoder_base::CborDecoderBase;
use super::error::CborError;
use crate::PackValue;
use serde_json::Value as JsonValue;

/// Full CBOR decoder.
///
/// Extends [`CborDecoderBase`] with value-skipping, validation,
/// and shallow-read capabilities.
#[derive(Default)]
pub struct CborDecoder {
    base: CborDecoderBase,
}

impl CborDecoder {
    pub fn new() -> Self {
        Self {
            base: CborDecoderBase::new(),
        }
    }

    /// Decode CBOR bytes into a [`PackValue`].
    pub fn decode(&self, input: &[u8]) -> Result<PackValue, CborError> {
        self.base.decode(input)
    }

    /// Decode CBOR bytes, returning value and consumed byte count.
    pub fn decode_with_consumed(&self, input: &[u8]) -> Result<(PackValue, usize), CborError> {
        self.base.decode_with_consumed(input)
    }

    /// Decode CBOR bytes and convert to `serde_json::Value`.
    pub fn decode_json(&self, input: &[u8]) -> Result<JsonValue, CborError> {
        let pv = self.decode(input)?;
        Ok(pack_to_json(pv))
    }

    /// Validate CBOR: check that `data[offset..offset+size]` is a valid single CBOR value.
    pub fn validate(&self, data: &[u8], offset: usize, size: usize) -> Result<(), CborError> {
        self.base.validate(data, offset, size)
    }
}

/// Convert [`PackValue`] to `serde_json::Value`, losing CBOR-specific types.
pub fn pack_to_json(v: PackValue) -> JsonValue {
    match v {
        PackValue::Null | PackValue::Undefined | PackValue::Blob(_) => JsonValue::Null,
        PackValue::Bool(b) => JsonValue::Bool(b),
        PackValue::Integer(i) => JsonValue::Number(i.into()),
        PackValue::UInteger(u) => JsonValue::Number(u.into()),
        PackValue::Float(f) => serde_json::Number::from_f64(f)
            .map(JsonValue::Number)
            .unwrap_or(JsonValue::Null),
        PackValue::BigInt(i) => {
            // Attempt to fit in i64
            if i >= i64::MIN as i128 && i <= i64::MAX as i128 {
                JsonValue::Number((i as i64).into())
            } else {
                JsonValue::Null // out of range for JSON numbers
            }
        }
        PackValue::Bytes(b) => {
            use json_joy_base64::to_base64;
            let b64 = to_base64(&b);
            JsonValue::String(format!("data:application/octet-stream;base64,{}", b64))
        }
        PackValue::Str(s) => JsonValue::String(s),
        PackValue::Array(arr) => JsonValue::Array(arr.into_iter().map(pack_to_json).collect()),
        PackValue::Object(obj) => {
            let map: serde_json::Map<String, JsonValue> =
                obj.into_iter().map(|(k, v)| (k, pack_to_json(v))).collect();
            JsonValue::Object(map)
        }
        PackValue::Extension(ext) => pack_to_json(*ext.val),
    }
}

// ---- Backward-compat functions (replacing the old ciborium-based API) ----

/// Decode CBOR bytes into a [`PackValue`].
pub fn decode_cbor_value(bytes: &[u8]) -> Result<PackValue, CborError> {
    CborDecoder::new().decode(bytes)
}

/// Decode CBOR bytes, returning value and consumed byte count.
pub fn decode_cbor_value_with_consumed(bytes: &[u8]) -> Result<(PackValue, usize), CborError> {
    CborDecoder::new().decode_with_consumed(bytes)
}

/// Decode CBOR bytes into a `serde_json::Value`.
pub fn decode_json_from_cbor_bytes(bytes: &[u8]) -> Result<JsonValue, CborError> {
    CborDecoder::new().decode_json(bytes)
}

/// Validate CBOR: check that the value at offset spans exactly `expected_size` bytes.
pub fn validate_cbor_exact_size(bytes: &[u8], expected_size: usize) -> Result<(), CborError> {
    let (_, consumed) = decode_cbor_value_with_consumed(bytes)?;
    if consumed == expected_size {
        Ok(())
    } else {
        Err(CborError::InvalidSize)
    }
}
