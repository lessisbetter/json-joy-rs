//! Shared `EjsonValue` type used by both encoder and decoder.
//!
//! This mirrors the full set of value types handled by the upstream
//! `EjsonEncoder.writeAny` / `EjsonDecoder.readAny` implementations.

use crate::bson::{
    BsonBinary, BsonDbPointer, BsonDecimal128, BsonFloat, BsonInt32, BsonInt64, BsonJavascriptCode,
    BsonJavascriptCodeWithScope, BsonMaxKey, BsonMinKey, BsonObjectId, BsonSymbol, BsonTimestamp,
};

/// A value that the EJSON codec knows how to encode and decode.
///
/// Variants that begin with `Bson` wrap typed BSON values.  The plain
/// `Integer` / `Float` variants come from vanilla JSON numbers.  `Number` is
/// used when the caller provides a raw `f64` for encoding in either canonical
/// or relaxed mode.
#[derive(Debug, Clone, PartialEq)]
pub enum EjsonValue {
    Null,
    Undefined,
    Bool(bool),
    /// Raw JSON integer (from decoded input or a literal `i64`).
    Integer(i64),
    /// Raw JSON float (from decoded input or a literal `f64`).
    Float(f64),
    /// A JavaScript-style number value that should be encoded according to
    /// the canonical/relaxed mode rules.  Prefer using `Integer` or `Float`
    /// when the variant is known; use `Number` when mirroring the upstream
    /// TypeScript `typeof value === 'number'` check.
    Number(f64),
    Str(String),
    Array(Vec<EjsonValue>),
    Object(Vec<(String, EjsonValue)>),
    // ---- Date / RegExp ----
    /// Milliseconds since Unix epoch.  `iso` holds the pre-formatted ISO
    /// string for relaxed-mode encoding if available.
    Date {
        timestamp_ms: i64,
        /// Pre-formatted ISO 8601 string (e.g. `"2023-01-01T00:00:00.000Z"`).
        /// Required for relaxed mode years 1970-9999; if absent the encoder
        /// falls back to `{"$numberLong":"..."}`.
        iso: Option<String>,
    },
    RegExp(String, String),
    // ---- BSON types ----
    ObjectId(BsonObjectId),
    Int32(BsonInt32),
    Int64(BsonInt64),
    BsonFloat(BsonFloat),
    Decimal128(BsonDecimal128),
    Binary(BsonBinary),
    Code(BsonJavascriptCode),
    CodeWithScope(BsonJavascriptCodeWithScope),
    Symbol(BsonSymbol),
    Timestamp(BsonTimestamp),
    DbPointer(BsonDbPointer),
    MinKey(BsonMinKey),
    MaxKey(BsonMaxKey),
}
