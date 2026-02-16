use ciborium::value::Value as CborValue;
use serde_json::{Map, Number, Value};
use std::convert::TryFrom;

use super::error::CborError;

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
