use ciborium::value::Value as CborValue;

use super::error::CborError;

pub fn encode_cbor_value(value: &CborValue) -> Result<Vec<u8>, CborError> {
    let mut out = Vec::new();
    ciborium::ser::into_writer(value, &mut out).map_err(|_| CborError::InvalidPayload)?;
    Ok(out)
}
