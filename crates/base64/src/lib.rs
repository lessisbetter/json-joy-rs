//! Base64 encoding and decoding utilities.
//!
//! This crate provides base64 encoding/decoding with support for:
//! - Standard base64 with padding
//! - URL-safe base64 without padding
//! - Binary output to DataView/Uint8Array equivalents
//!
//! # Example
//!
//! ```
//! use json_joy_base64::{to_base64, from_base64};
//!
//! let data = b"hello world";
//! let encoded = to_base64(data);
//! let decoded = from_base64(&encoded).unwrap();
//! assert_eq!(decoded.as_slice(), data);
//! ```

mod constants;
mod create_from_base64;
mod create_from_base64_bin;
mod create_to_base64;
mod create_to_base64_bin;
mod create_to_base64_bin_uint8;
mod from_base64;
mod from_base64_bin;
mod from_base64_url;
mod to_base64;
mod to_base64_bin;
mod to_base64_url;

pub use constants::{ALPHABET, ALPHABET_BYTES, ALPHABET_URL, PAD};
pub use create_from_base64::create_from_base64;
pub use create_from_base64_bin::create_from_base64_bin;
pub use create_to_base64::create_to_base64;
pub use create_to_base64_bin::create_to_base64_bin;
pub use create_to_base64_bin_uint8::create_to_base64_bin_uint8;
pub use from_base64::from_base64;
pub use from_base64_bin::from_base64_bin;
pub use from_base64_url::from_base64_url;
pub use to_base64::to_base64;
pub use to_base64_bin::to_base64_bin;
pub use to_base64_url::to_base64_url;

/// Error type for base64 operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Base64Error {
    /// The input string contains invalid base64 characters.
    InvalidBase64String,
    /// The input binary sequence contains invalid base64 bytes.
    InvalidBase64Sequence,
    /// The character set must be exactly 64 characters.
    InvalidCharSetLength,
    /// The base64 string length must be a multiple of 4.
    InvalidLength,
}

impl std::fmt::Display for Base64Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Base64Error::InvalidBase64String => write!(f, "INVALID_BASE64_STRING"),
            Base64Error::InvalidBase64Sequence => write!(f, "INVALID_BASE64_SEQ"),
            Base64Error::InvalidCharSetLength => write!(f, "chars must be 64 characters long"),
            Base64Error::InvalidLength => write!(f, "Base64 string length must be a multiple of 4"),
        }
    }
}

impl std::error::Error for Base64Error {}
