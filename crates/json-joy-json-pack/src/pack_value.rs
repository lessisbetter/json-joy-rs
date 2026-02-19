//! [`PackValue`] — the universal value type for all json-pack format encoders/decoders.
//!
//! Mirrors the TypeScript `PackValue` union from `types.ts`.

use crate::{JsonPackExtension, JsonPackValue};

/// Universal value type that spans all JSON-pack binary formats.
///
/// Covers everything TypeScript `PackValue` represents:
/// - JSON primitives (null, bool, numbers, strings, arrays, objects)
/// - Binary data (`Uint8Array`)
/// - Pre-encoded blobs (`JsonPackValue`)
/// - Extension/tag values (`JsonPackExtension`)
/// - Undefined
/// - Big integers
#[derive(Debug, Clone, PartialEq)]
pub enum PackValue {
    /// JSON null / CBOR null / MsgPack nil
    Null,
    /// undefined (supported by some formats)
    Undefined,
    /// Boolean value
    Bool(bool),
    /// Safe integer (fits in i64, negative or positive)
    Integer(i64),
    /// Unsigned integer > i64::MAX
    UInteger(u64),
    /// Floating-point number
    Float(f64),
    /// Big integer (two's complement)
    BigInt(i128),
    /// Binary data
    Bytes(Vec<u8>),
    /// String
    Str(String),
    /// Array of pack values
    Array(Vec<PackValue>),
    /// Object (ordered key-value pairs)
    Object(Vec<(String, PackValue)>),
    /// Extension / CBOR tag
    Extension(Box<JsonPackExtension>),
    /// Pre-encoded blob (written as-is to the output)
    Blob(JsonPackValue),
}

impl From<serde_json::Value> for PackValue {
    fn from(v: serde_json::Value) -> Self {
        match v {
            serde_json::Value::Null => PackValue::Null,
            serde_json::Value::Bool(b) => PackValue::Bool(b),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    PackValue::Integer(i)
                } else if let Some(u) = n.as_u64() {
                    PackValue::UInteger(u)
                } else {
                    PackValue::Float(n.as_f64().unwrap_or(0.0))
                }
            }
            serde_json::Value::String(s) => PackValue::Str(s),
            serde_json::Value::Array(arr) => {
                PackValue::Array(arr.into_iter().map(PackValue::from).collect())
            }
            serde_json::Value::Object(obj) => PackValue::Object(
                obj.into_iter()
                    .map(|(k, v)| (k, PackValue::from(v)))
                    .collect(),
            ),
        }
    }
}

impl From<PackValue> for serde_json::Value {
    fn from(v: PackValue) -> Self {
        match v {
            PackValue::Null => serde_json::Value::Null,
            PackValue::Undefined => serde_json::Value::Null,
            PackValue::Bool(b) => serde_json::Value::Bool(b),
            PackValue::Integer(i) => serde_json::json!(i),
            PackValue::UInteger(u) => serde_json::json!(u),
            PackValue::Float(f) => serde_json::json!(f),
            PackValue::BigInt(i) => serde_json::json!(i),
            PackValue::Bytes(b) => {
                use json_joy_base64::to_base64;
                let b64 = to_base64(&b);
                serde_json::Value::String(format!("data:application/octet-stream;base64,{}", b64))
            }
            PackValue::Str(s) => serde_json::Value::String(s),
            PackValue::Array(arr) => {
                serde_json::Value::Array(arr.into_iter().map(serde_json::Value::from).collect())
            }
            PackValue::Object(obj) => serde_json::Value::Object(
                obj.into_iter()
                    .map(|(k, v)| (k, serde_json::Value::from(v)))
                    .collect(),
            ),
            PackValue::Extension(ext) => serde_json::Value::from(*ext.val),
            PackValue::Blob(_) => serde_json::Value::Null,
        }
    }
}
