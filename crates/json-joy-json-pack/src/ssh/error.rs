//! SSH decoder error type.

use thiserror::Error;

/// Error type for SSH 2.0 binary protocol decoding operations.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SshError {
    #[error("unexpected end of input")]
    UnexpectedEof,
    #[error("invalid UTF-8")]
    InvalidUtf8,
}
