use ciborium::value::Value as CborValue;
use serde_json::Value;
use std::io::Cursor;

use super::convert::cbor_to_json;
use super::error::CborError;

pub fn decode_cbor_value(bytes: &[u8]) -> Result<CborValue, CborError> {
    let mut cursor = Cursor::new(bytes);
    ciborium::de::from_reader::<CborValue, _>(&mut cursor).map_err(|_| CborError::InvalidPayload)
}

pub fn decode_cbor_value_with_consumed(bytes: &[u8]) -> Result<(CborValue, usize), CborError> {
    let mut cursor = Cursor::new(bytes);
    let value = ciborium::de::from_reader::<CborValue, _>(&mut cursor)
        .map_err(|_| CborError::InvalidPayload)?;
    Ok((value, cursor.position() as usize))
}

pub fn validate_cbor_exact_size(bytes: &[u8], expected_size: usize) -> Result<(), CborError> {
    let (_, consumed) = decode_cbor_value_with_consumed(bytes)?;
    if consumed == expected_size {
        Ok(())
    } else {
        Err(CborError::InvalidPayload)
    }
}

pub fn decode_json_from_cbor_bytes(bytes: &[u8]) -> Result<Value, CborError> {
    let value = decode_cbor_value(bytes)?;
    cbor_to_json(&value)
}
