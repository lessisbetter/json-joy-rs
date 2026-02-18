//! BSON decoder error type.

use thiserror::Error;

/// Error type for BSON decoding operations.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum BsonError {
    #[error("unexpected end of input")]
    UnexpectedEof,
    #[error("unsupported BSON element type: 0x{0:02x}")]
    UnsupportedType(u8),
    #[error("invalid UTF-8")]
    InvalidUtf8,
}
