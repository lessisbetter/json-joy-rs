//! `json-cli` — command-line tools for JSON encoding/decoding and patching.
//!
//! Mirrors `packages/json-joy/src/json-cli/`.
//!
//! Provides the core logic used by the binary entry points:
//! - `json-pack`   — encode JSON → MessagePack or CBOR
//! - `json-unpack` — decode MessagePack or CBOR → JSON
//! - `json-patch`  — apply a JSON Patch to a document
//! - `json-pointer`— look up a JSON Pointer in a document

use json_joy_json_pack::cbor::{decode_cbor_value, CborEncoder};
use json_joy_json_pack::msgpack::{MsgPackDecoder, MsgPackEncoder};
use json_joy_json_pack::PackValue;
use json_joy_json_pointer::find_by_pointer;
use serde_json::Value;

use crate::json_patch::{apply_patch, ApplyPatchOptions};
use crate::json_patch::codec::json::from_json_patch;

// ── Errors ────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum CliError {
    Json(serde_json::Error),
    MsgPack(String),
    Cbor(String),
    Patch(String),
    Pointer(String),
    UnknownFormat(String),
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliError::Json(e)           => write!(f, "{e}"),
            CliError::MsgPack(e)        => write!(f, "{e}"),
            CliError::Cbor(e)           => write!(f, "{e}"),
            CliError::Patch(e)          => write!(f, "{e}"),
            CliError::Pointer(e)        => write!(f, "{e}"),
            CliError::UnknownFormat(e)  => write!(f, "Unknown format: {e}"),
        }
    }
}

impl From<serde_json::Error> for CliError {
    fn from(e: serde_json::Error) -> Self { CliError::Json(e) }
}

// ── json-pack ─────────────────────────────────────────────────────────────

/// Encode a JSON string to MessagePack bytes.
pub fn pack_msgpack(json: &str) -> Result<Vec<u8>, CliError> {
    let value: Value = serde_json::from_str(json)?;
    let pack = PackValue::from(value);
    let mut encoder = MsgPackEncoder::new();
    Ok(encoder.encode(&pack))
}

/// Encode a JSON string to CBOR bytes.
pub fn pack_cbor(json: &str) -> Result<Vec<u8>, CliError> {
    let value: Value = serde_json::from_str(json)?;
    let pack = PackValue::from(value);
    let mut encoder = CborEncoder::new();
    Ok(encoder.encode(&pack))
}

/// Encode a JSON string to the requested format (`"msgpack"` or `"cbor"`).
pub fn pack(json: &str, format: &str) -> Result<Vec<u8>, CliError> {
    match format.to_lowercase().as_str() {
        "msgpack" | "messagepack" => pack_msgpack(json),
        "cbor" => pack_cbor(json),
        other => Err(CliError::UnknownFormat(other.to_string())),
    }
}

// ── json-unpack ───────────────────────────────────────────────────────────

/// Decode MessagePack bytes to a JSON string.
pub fn unpack_msgpack(bytes: &[u8]) -> Result<String, CliError> {
    let mut decoder = MsgPackDecoder::new();
    let pack = decoder
        .decode(bytes)
        .map_err(|e| CliError::MsgPack(format!("{e:?}")))?;
    let value = Value::from(pack);
    Ok(serde_json::to_string_pretty(&value)?)
}

/// Decode CBOR bytes to a JSON string.
pub fn unpack_cbor(bytes: &[u8]) -> Result<String, CliError> {
    let pack = decode_cbor_value(bytes)
        .map_err(|e| CliError::Cbor(format!("{e:?}")))?;
    let value = Value::from(pack);
    Ok(serde_json::to_string_pretty(&value)?)
}

/// Decode bytes in the requested format to a JSON string.
pub fn unpack(bytes: &[u8], format: &str) -> Result<String, CliError> {
    match format.to_lowercase().as_str() {
        "msgpack" | "messagepack" => unpack_msgpack(bytes),
        "cbor" => unpack_cbor(bytes),
        other => Err(CliError::UnknownFormat(other.to_string())),
    }
}

// ── json-patch ────────────────────────────────────────────────────────────

/// Apply a JSON Patch (RFC 6902) to a document.
///
/// `doc_json`: the document as a JSON string.
/// `patch_json`: the patch operations as a JSON array string.
///
/// Returns the patched document as a pretty-printed JSON string.
pub fn apply_json_patch(doc_json: &str, patch_json: &str) -> Result<String, CliError> {
    let doc: Value = serde_json::from_str(doc_json)?;
    let ops_raw: Value = serde_json::from_str(patch_json)?;
    let ops = from_json_patch(&ops_raw)
        .map_err(|e| CliError::Patch(format!("{e:?}")))?;
    let options = ApplyPatchOptions { mutate: true };
    let result = apply_patch(doc, &ops, &options)
        .map_err(|e| CliError::Patch(format!("{e:?}")))?;
    Ok(serde_json::to_string_pretty(&result.doc)?)
}

// ── json-pointer ──────────────────────────────────────────────────────────

/// Look up a JSON Pointer (RFC 6901) in a document.
///
/// `doc_json`: the document as a JSON string.
/// `pointer`: the JSON Pointer string (e.g., `/foo/bar`).
///
/// Returns the found value as a pretty-printed JSON string, or an error
/// string matching upstream behaviour (`"NOT_FOUND"`, `"INVALID_INDEX"`).
pub fn lookup_pointer(doc_json: &str, pointer: &str) -> Result<String, CliError> {
    let doc: Value = serde_json::from_str(doc_json)?;

    // Empty pointer means root.
    if pointer.is_empty() {
        return Ok(serde_json::to_string_pretty(&doc)?);
    }

    // find_by_pointer returns (parent_container, last_key).
    // We extract the value by indexing into the parent.
    match find_by_pointer(pointer, &doc) {
        Ok((Some(parent), key)) => {
            let val = match &parent {
                Value::Object(m) => m.get(&key).cloned(),
                Value::Array(a) => key.parse::<usize>().ok().and_then(|i| a.get(i)).cloned(),
                _ => None,
            };
            match val {
                Some(v) => Ok(serde_json::to_string_pretty(&v)?),
                None if matches!(parent, Value::Array(_)) => {
                    Err(CliError::Pointer("INVALID_INDEX".to_string()))
                }
                None => Err(CliError::Pointer("NOT_FOUND".to_string())),
            }
        }
        Ok((None, _)) => Err(CliError::Pointer("NOT_FOUND".to_string())),
        Err(e) => Err(CliError::Pointer(e.to_string())),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── pack / unpack roundtrips ───────────────────────────────────────────

    #[test]
    fn pack_unpack_msgpack_null() {
        let bytes = pack_msgpack("null").unwrap();
        let json  = unpack_msgpack(&bytes).unwrap();
        assert_eq!(json.trim(), "null");
    }

    #[test]
    fn pack_unpack_msgpack_number() {
        let bytes = pack_msgpack("42").unwrap();
        let json  = unpack_msgpack(&bytes).unwrap();
        assert_eq!(json.trim(), "42");
    }

    #[test]
    fn pack_unpack_msgpack_string() {
        let bytes = pack_msgpack("\"hello\"").unwrap();
        let json  = unpack_msgpack(&bytes).unwrap();
        assert_eq!(json.trim(), "\"hello\"");
    }

    #[test]
    fn pack_unpack_msgpack_object() {
        let orig = r#"{"a":1,"b":true}"#;
        let bytes = pack_msgpack(orig).unwrap();
        let json  = unpack_msgpack(&bytes).unwrap();
        let v: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["a"], 1);
        assert_eq!(v["b"], true);
    }

    #[test]
    fn pack_unpack_cbor_array() {
        let orig  = "[1,2,3]";
        let bytes = pack_cbor(orig).unwrap();
        let json  = unpack_cbor(&bytes).unwrap();
        let v: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v, serde_json::json!([1, 2, 3]));
    }

    #[test]
    fn pack_dispatch_unknown_format() {
        let r = pack("null", "bson");
        assert!(matches!(r, Err(CliError::UnknownFormat(_))));
    }

    // ── json-patch ─────────────────────────────────────────────────────────

    #[test]
    fn patch_add_key() {
        let doc   = r#"{"a":1}"#;
        let patch = r#"[{"op":"add","path":"/b","value":2}]"#;
        let out   = apply_json_patch(doc, patch).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["b"], 2);
    }

    #[test]
    fn patch_remove_key() {
        let doc   = r#"{"a":1,"b":2}"#;
        let patch = r#"[{"op":"remove","path":"/a"}]"#;
        let out   = apply_json_patch(doc, patch).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert!(v.get("a").is_none());
        assert_eq!(v["b"], 2);
    }

    #[test]
    fn patch_replace_value() {
        let doc   = r#"{"x":1}"#;
        let patch = r#"[{"op":"replace","path":"/x","value":99}]"#;
        let out   = apply_json_patch(doc, patch).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["x"], 99);
    }

    // ── json-pointer ───────────────────────────────────────────────────────

    #[test]
    fn pointer_root() {
        let doc = r#"{"a":1}"#;
        let out = lookup_pointer(doc, "").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["a"], 1);
    }

    #[test]
    fn pointer_nested() {
        let doc = r#"{"foo":{"bar":42}}"#;
        let out = lookup_pointer(doc, "/foo/bar").unwrap();
        assert_eq!(out.trim(), "42");
    }

    #[test]
    fn pointer_not_found() {
        let doc = r#"{"a":1}"#;
        let err = lookup_pointer(doc, "/z").unwrap_err();
        assert!(err.to_string().contains("NOT_FOUND") || matches!(err, CliError::Pointer(_)));
    }

    #[test]
    fn pointer_array_element() {
        let doc = r#"{"arr":[10,20,30]}"#;
        let out = lookup_pointer(doc, "/arr/1").unwrap();
        assert_eq!(out.trim(), "20");
    }
}
