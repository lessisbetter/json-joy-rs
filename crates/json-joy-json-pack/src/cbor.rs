use ciborium::value::Value as CborValue;
use serde_json::{Map, Number, Value};
use std::convert::TryFrom;
use std::io::Cursor;
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CborError {
    #[error("invalid cbor payload")]
    InvalidPayload,
    #[error("unsupported cbor feature for json conversion")]
    Unsupported,
}

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

pub fn encode_cbor_value(value: &CborValue) -> Result<Vec<u8>, CborError> {
    let mut out = Vec::new();
    ciborium::ser::into_writer(value, &mut out).map_err(|_| CborError::InvalidPayload)?;
    Ok(out)
}

pub fn cbor_to_json(v: &CborValue) -> Result<Value, CborError> {
    cbor_to_json_owned(v.clone())
}

pub fn cbor_to_json_owned(v: CborValue) -> Result<Value, CborError> {
    Ok(match v {
        CborValue::Null => Value::Null,
        CborValue::Bool(b) => Value::Bool(b),
        CborValue::Integer(i) => {
            let signed: i128 = i.into();
            if signed >= 0 {
                let u = u64::try_from(signed).map_err(|_| CborError::Unsupported)?;
                Value::Number(Number::from(u))
            } else {
                let s = i64::try_from(signed).map_err(|_| CborError::Unsupported)?;
                Value::Number(Number::from(s))
            }
        }
        CborValue::Float(f) => Number::from_f64(f)
            .map(Value::Number)
            .ok_or(CborError::Unsupported)?,
        CborValue::Text(s) => Value::String(s),
        CborValue::Bytes(bytes) => Value::Array(
            bytes
                .into_iter()
                .map(|b| Value::Number(Number::from(b)))
                .collect(),
        ),
        CborValue::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(cbor_to_json_owned(item)?);
            }
            Value::Array(out)
        }
        CborValue::Map(entries) => {
            let mut out = Map::new();
            for (k, v) in entries {
                let key = match k {
                    CborValue::Text(s) => s,
                    _ => return Err(CborError::Unsupported),
                };
                out.insert(key, cbor_to_json_owned(v)?);
            }
            Value::Object(out)
        }
        CborValue::Tag(_, _) => return Err(CborError::Unsupported),
        _ => return Err(CborError::Unsupported),
    })
}

pub fn json_to_cbor(v: &Value) -> CborValue {
    match v {
        Value::Null => CborValue::Null,
        Value::Bool(b) => CborValue::Bool(*b),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                CborValue::Integer(i.into())
            } else if let Some(u) = n.as_u64() {
                CborValue::Integer(u.into())
            } else {
                CborValue::Float(n.as_f64().unwrap_or(0.0))
            }
        }
        Value::String(s) => CborValue::Text(s.clone()),
        Value::Array(arr) => CborValue::Array(arr.iter().map(json_to_cbor).collect()),
        Value::Object(map) => CborValue::Map(
            map.iter()
                .map(|(k, v)| (CborValue::Text(k.clone()), json_to_cbor(v)))
                .collect(),
        ),
    }
}

pub fn decode_json_from_cbor_bytes(bytes: &[u8]) -> Result<Value, CborError> {
    let value = decode_cbor_value(bytes)?;
    cbor_to_json(&value)
}

pub fn encode_json_to_cbor_bytes(value: &Value) -> Result<Vec<u8>, CborError> {
    let mut out = Vec::new();
    write_json_like_json_pack(&mut out, value)?;
    Ok(out)
}

pub fn write_cbor_uint_major(out: &mut Vec<u8>, major: u8, n: u64) {
    let major_bits = major << 5;
    if n <= 23 {
        out.push(major_bits | (n as u8));
    } else if n <= 0xff {
        out.push(major_bits | 24);
        out.push(n as u8);
    } else if n <= 0xffff {
        out.push(major_bits | 25);
        out.extend_from_slice(&(n as u16).to_be_bytes());
    } else if n <= 0xffff_ffff {
        out.push(major_bits | 26);
        out.extend_from_slice(&(n as u32).to_be_bytes());
    } else {
        out.push(major_bits | 27);
        out.extend_from_slice(&n.to_be_bytes());
    }
}

pub fn write_cbor_signed(out: &mut Vec<u8>, n: i64) {
    if n >= 0 {
        write_cbor_uint_major(out, 0, n as u64);
    } else {
        let encoded = (-1i128 - n as i128) as u64;
        write_cbor_uint_major(out, 1, encoded);
    }
}

pub fn write_cbor_text_like_json_pack(out: &mut Vec<u8>, value: &str) {
    let utf8 = value.as_bytes();
    let bytes_len = utf8.len();
    let max_size = value.chars().count().saturating_mul(4);

    if max_size <= 23 {
        out.push(0x60u8.saturating_add(bytes_len as u8));
    } else if max_size <= 0xff {
        out.push(0x78);
        out.push(bytes_len as u8);
    } else if max_size <= 0xffff {
        out.push(0x79);
        out.extend_from_slice(&(bytes_len as u16).to_be_bytes());
    } else {
        out.push(0x7a);
        out.extend_from_slice(&(bytes_len as u32).to_be_bytes());
    }
    out.extend_from_slice(utf8);
}

pub fn write_json_like_json_pack(out: &mut Vec<u8>, value: &Value) -> Result<(), CborError> {
    match value {
        Value::Null => out.push(0xf6),
        Value::Bool(b) => out.push(if *b { 0xf5 } else { 0xf4 }),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                write_cbor_signed(out, i);
            } else if let Some(u) = n.as_u64() {
                write_cbor_uint_major(out, 0, u);
            } else if let Some(f) = n.as_f64() {
                if !f.is_finite() {
                    return Err(CborError::Unsupported);
                }
                if is_f32_roundtrip(f) {
                    out.push(0xfa);
                    out.extend_from_slice(&(f as f32).to_bits().to_be_bytes());
                } else {
                    out.push(0xfb);
                    out.extend_from_slice(&f.to_bits().to_be_bytes());
                }
            } else {
                return Err(CborError::Unsupported);
            }
        }
        Value::String(s) => write_cbor_text_like_json_pack(out, s),
        Value::Array(items) => {
            write_cbor_uint_major(out, 4, items.len() as u64);
            for item in items {
                write_json_like_json_pack(out, item)?;
            }
        }
        Value::Object(map) => {
            write_cbor_uint_major(out, 5, map.len() as u64);
            for (k, v) in map {
                write_cbor_text_like_json_pack(out, k);
                write_json_like_json_pack(out, v)?;
            }
        }
    }
    Ok(())
}

fn is_f32_roundtrip(value: f64) -> bool {
    (value as f32) as f64 == value
}
