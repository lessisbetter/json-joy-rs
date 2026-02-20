//! json-binary codec: wrap/unwrap binary data in JSON via data URIs.
//!
//! Direct port of `json-binary/codec.ts` from upstream.
//!
//! # Overview
//! JSON cannot natively encode binary data. This module converts:
//! - `PackValue::Bytes` ↔ `"data:application/octet-stream;base64,<base64>"`
//! - `PackValue::Blob` ↔ `"data:application/msgpack;base64,<base64>"`
//! - `PackValue::Extension(tag, data)` ↔ `"data:application/msgpack;base64;ext=<tag>,<base64>"`

use serde_json::Value as JsonValue;

use super::constants::{BIN_URI_START, MSGPACK_EXT_START, MSGPACK_URI_START};
use crate::{JsonPackExtension, JsonPackValue, PackValue};

/// Convert a `PackValue` tree into `serde_json::Value`, encoding binary blobs
/// as data URI strings (wrap step before JSON serialization).
pub fn wrap_binary(value: PackValue) -> JsonValue {
    match value {
        PackValue::Null | PackValue::Undefined => JsonValue::Null,
        PackValue::Bool(b) => JsonValue::Bool(b),
        PackValue::Integer(i) => serde_json::json!(i),
        PackValue::UInteger(u) => serde_json::json!(u),
        PackValue::Float(f) => serde_json::json!(f),
        PackValue::BigInt(i) => serde_json::json!(i),
        PackValue::Str(s) => JsonValue::String(s),
        PackValue::Bytes(b) => {
            let uri = format!("{}{}", BIN_URI_START, json_joy_base64::to_base64(&b));
            JsonValue::String(uri)
        }
        PackValue::Blob(blob) => {
            let uri = format!(
                "{}{}",
                MSGPACK_URI_START,
                json_joy_base64::to_base64(&blob.val)
            );
            JsonValue::String(uri)
        }
        PackValue::Extension(ext) => {
            let uri = format!(
                "{}{},{}",
                MSGPACK_EXT_START,
                ext.tag,
                match *ext.val {
                    PackValue::Bytes(ref b) => json_joy_base64::to_base64(b),
                    _ => String::new(),
                }
            );
            JsonValue::String(uri)
        }
        PackValue::Array(arr) => JsonValue::Array(arr.into_iter().map(wrap_binary).collect()),
        PackValue::Object(obj) => {
            JsonValue::Object(obj.into_iter().map(|(k, v)| (k, wrap_binary(v))).collect())
        }
    }
}

/// Convert a `serde_json::Value` tree into `PackValue`, decoding data URI strings
/// back to binary blobs (unwrap step after JSON parsing).
pub fn unwrap_binary(value: JsonValue) -> PackValue {
    match value {
        JsonValue::Null => PackValue::Null,
        JsonValue::Bool(b) => PackValue::Bool(b),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                PackValue::Integer(i)
            } else if let Some(u) = n.as_u64() {
                PackValue::UInteger(u)
            } else {
                PackValue::Float(n.as_f64().unwrap_or(0.0))
            }
        }
        JsonValue::String(s) => {
            if let Some(b64) = s.strip_prefix(BIN_URI_START) {
                // Binary data URI
                match json_joy_base64::from_base64(b64) {
                    Ok(bytes) => PackValue::Bytes(bytes),
                    Err(_) => PackValue::Str(s),
                }
            } else if let Some(b64) = s.strip_prefix(MSGPACK_URI_START) {
                // MsgPack value URI
                match json_joy_base64::from_base64(b64) {
                    Ok(bytes) => PackValue::Blob(JsonPackValue::new(bytes)),
                    Err(_) => PackValue::Str(s),
                }
            } else if let Some(rest) = s.strip_prefix(MSGPACK_EXT_START) {
                // MsgPack extension URI: `<tag>,<base64>`
                if let Some(comma) = rest.find(',') {
                    let tag_str = &rest[..comma];
                    let b64 = &rest[comma + 1..];
                    if let Ok(tag) = tag_str.parse::<u64>() {
                        if let Ok(bytes) = json_joy_base64::from_base64(b64) {
                            return PackValue::Extension(Box::new(JsonPackExtension::new(
                                tag,
                                PackValue::Bytes(bytes),
                            )));
                        }
                    }
                }
                PackValue::Str(s)
            } else {
                PackValue::Str(s)
            }
        }
        JsonValue::Array(arr) => PackValue::Array(arr.into_iter().map(unwrap_binary).collect()),
        JsonValue::Object(obj) => PackValue::Object(
            obj.into_iter()
                .map(|(k, v)| (k, unwrap_binary(v)))
                .collect(),
        ),
    }
}

/// Serialize a `PackValue` to a JSON string, encoding binary blobs as data URIs.
pub fn stringify(value: PackValue) -> Result<String, serde_json::Error> {
    let wrapped = wrap_binary(value);
    serde_json::to_string(&wrapped)
}

/// Parse a JSON string and decode any binary data URI strings.
pub fn parse(json: &str) -> Result<PackValue, serde_json::Error> {
    let parsed: JsonValue = serde_json::from_str(json)?;
    Ok(unwrap_binary(parsed))
}

/// Encode bytes as a binary data URI string.
pub fn stringify_binary(buf: &[u8]) -> String {
    format!("{}{}", BIN_URI_START, json_joy_base64::to_base64(buf))
}
