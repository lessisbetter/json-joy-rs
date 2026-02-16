use ciborium::value::Value as CborValue;
use serde_json::Value;
use std::convert::TryFrom;

use super::constants::{
    is_f32_roundtrip, MAJOR_ARRAY, MAJOR_BYTES, MAJOR_MAP, MAJOR_NEGATIVE, MAJOR_TAG,
    MAJOR_UNSIGNED,
};
use super::error::CborError;

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
        write_cbor_uint_major(out, MAJOR_UNSIGNED, n as u64);
    } else {
        let encoded = (-1i128 - n as i128) as u64;
        write_cbor_uint_major(out, MAJOR_NEGATIVE, encoded);
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
                write_cbor_uint_major(out, MAJOR_UNSIGNED, u);
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
            write_cbor_uint_major(out, MAJOR_ARRAY, items.len() as u64);
            for item in items {
                write_json_like_json_pack(out, item)?;
            }
        }
        Value::Object(map) => {
            write_cbor_uint_major(out, MAJOR_MAP, map.len() as u64);
            for (k, v) in map {
                write_cbor_text_like_json_pack(out, k);
                write_json_like_json_pack(out, v)?;
            }
        }
    }
    Ok(())
}

pub fn write_cbor_value_like_json_pack(
    out: &mut Vec<u8>,
    value: &CborValue,
) -> Result<(), CborError> {
    match value {
        CborValue::Null => out.push(0xf6),
        CborValue::Bool(false) => out.push(0xf4),
        CborValue::Bool(true) => out.push(0xf5),
        CborValue::Integer(i) => {
            let signed: i128 = (*i).into();
            if signed >= 0 {
                write_cbor_uint_major(
                    out,
                    MAJOR_UNSIGNED,
                    u64::try_from(signed).map_err(|_| CborError::Unsupported)?,
                );
            } else {
                write_cbor_uint_major(out, MAJOR_NEGATIVE, (-1i128 - signed) as u64);
            }
        }
        CborValue::Float(f) => {
            let f32v = *f as f32;
            if (f32v as f64) == *f {
                out.push(0xfa);
                out.extend_from_slice(&f32v.to_be_bytes());
            } else {
                out.push(0xfb);
                out.extend_from_slice(&f.to_be_bytes());
            }
        }
        CborValue::Bytes(bytes) => {
            write_cbor_uint_major(out, MAJOR_BYTES, bytes.len() as u64);
            out.extend_from_slice(bytes);
        }
        CborValue::Text(s) => write_cbor_text_like_json_pack(out, s),
        CborValue::Array(arr) => {
            write_cbor_uint_major(out, MAJOR_ARRAY, arr.len() as u64);
            for item in arr {
                write_cbor_value_like_json_pack(out, item)?;
            }
        }
        CborValue::Map(map) => {
            write_cbor_uint_major(out, MAJOR_MAP, map.len() as u64);
            for (k, v) in map {
                write_cbor_value_like_json_pack(out, k)?;
                write_cbor_value_like_json_pack(out, v)?;
            }
        }
        CborValue::Tag(tag, inner) => {
            write_cbor_uint_major(out, MAJOR_TAG, *tag);
            write_cbor_value_like_json_pack(out, inner)?;
        }
        _ => return Err(CborError::Unsupported),
    }
    Ok(())
}

pub fn encode_json_to_cbor_bytes(value: &Value) -> Result<Vec<u8>, CborError> {
    let mut out = Vec::new();
    write_json_like_json_pack(&mut out, value)?;
    Ok(out)
}
