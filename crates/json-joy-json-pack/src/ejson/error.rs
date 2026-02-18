//! Error types for EJSON encoding and decoding.

use std::fmt;

/// Errors that can occur during EJSON encoding.
#[derive(Debug, Clone, PartialEq)]
pub enum EjsonEncodeError {
    /// Attempted to encode an invalid Date (NaN timestamp).
    InvalidDate,
}

impl fmt::Display for EjsonEncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EjsonEncodeError::InvalidDate => write!(f, "Invalid Date"),
        }
    }
}

impl std::error::Error for EjsonEncodeError {}

/// Errors that can occur during EJSON decoding.
#[derive(Debug, Clone, PartialEq)]
pub enum EjsonDecodeError {
    /// Generic JSON parse error at the given byte offset.
    InvalidJson(usize),
    /// Invalid UTF-8 in input.
    InvalidUtf8,
    /// Invalid `{"$oid": "..."}` ObjectId format.
    InvalidObjectId,
    /// Invalid `{"$numberInt": "..."}` format.
    InvalidInt32,
    /// Invalid `{"$numberLong": "..."}` format.
    InvalidInt64,
    /// Invalid `{"$numberDouble": "..."}` format.
    InvalidDouble,
    /// Invalid `{"$numberDecimal": "..."}` format.
    InvalidDecimal128,
    /// Invalid `{"$binary": {...}}` format.
    InvalidBinary,
    /// Invalid `{"$uuid": "..."}` format.
    InvalidUuid,
    /// Invalid `{"$code": "..."}` format.
    InvalidCode,
    /// Invalid `{"$code": "...", "$scope": {...}}` format.
    InvalidCodeWithScope,
    /// Invalid `{"$symbol": "..."}` format.
    InvalidSymbol,
    /// Invalid `{"$timestamp": {"t": ..., "i": ...}}` format.
    InvalidTimestamp,
    /// Invalid `{"$regularExpression": {"pattern": ..., "options": ...}}` format.
    InvalidRegularExpression,
    /// Invalid `{"$dbPointer": {"$ref": ..., "$id": {...}}}` format.
    InvalidDbPointer,
    /// Invalid `{"$date": ...}` format.
    InvalidDate,
    /// Invalid `{"$minKey": 1}` format.
    InvalidMinKey,
    /// Invalid `{"$maxKey": 1}` format.
    InvalidMaxKey,
    /// Invalid `{"$undefined": true}` format.
    InvalidUndefined,
    /// Extra keys found where not allowed (strict single-key wrapper).
    ExtraKeys(&'static str),
}

impl fmt::Display for EjsonDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EjsonDecodeError::InvalidJson(pos) => write!(f, "Invalid JSON at position {pos}"),
            EjsonDecodeError::InvalidUtf8 => write!(f, "Invalid UTF-8"),
            EjsonDecodeError::InvalidObjectId => write!(f, "Invalid ObjectId format"),
            EjsonDecodeError::InvalidInt32 => write!(f, "Invalid Int32 format"),
            EjsonDecodeError::InvalidInt64 => write!(f, "Invalid Int64 format"),
            EjsonDecodeError::InvalidDouble => write!(f, "Invalid Double format"),
            EjsonDecodeError::InvalidDecimal128 => write!(f, "Invalid Decimal128 format"),
            EjsonDecodeError::InvalidBinary => write!(f, "Invalid Binary format"),
            EjsonDecodeError::InvalidUuid => write!(f, "Invalid UUID format"),
            EjsonDecodeError::InvalidCode => write!(f, "Invalid Code format"),
            EjsonDecodeError::InvalidCodeWithScope => write!(f, "Invalid CodeWScope format"),
            EjsonDecodeError::InvalidSymbol => write!(f, "Invalid Symbol format"),
            EjsonDecodeError::InvalidTimestamp => write!(f, "Invalid Timestamp format"),
            EjsonDecodeError::InvalidRegularExpression => write!(f, "Invalid RegularExpression format"),
            EjsonDecodeError::InvalidDbPointer => write!(f, "Invalid DBPointer format"),
            EjsonDecodeError::InvalidDate => write!(f, "Invalid Date format"),
            EjsonDecodeError::InvalidMinKey => write!(f, "Invalid MinKey format"),
            EjsonDecodeError::InvalidMaxKey => write!(f, "Invalid MaxKey format"),
            EjsonDecodeError::InvalidUndefined => write!(f, "Invalid Undefined format"),
            EjsonDecodeError::ExtraKeys(kind) => write!(f, "Invalid {kind} format: extra keys not allowed"),
        }
    }
}

impl std::error::Error for EjsonDecodeError {}
