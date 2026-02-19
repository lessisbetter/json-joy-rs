//! URL-safe base64 encoding function.

use crate::create_to_base64;

/// Encodes a byte slice to a URL-safe base64 string.
///
/// This uses the URL-safe alphabet (`-` and `_` instead of `+` and `/`)
/// and does not add padding.
///
/// # Arguments
///
/// * `uint8` - The bytes to encode.
/// * `length` - The number of bytes to encode from the slice.
///
/// # Returns
///
/// A URL-safe base64-encoded string without padding.
///
/// # Example
///
/// ```
/// use json_joy_base64::to_base64_url;
///
/// let encoded = to_base64_url(b"hello world", 11);
/// assert_eq!(encoded, "aGVsbG8gd29ybGQ");
/// ```
pub fn to_base64_url(uint8: &[u8], length: usize) -> String {
    let encoder = create_to_base64(
        Some("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_"),
        Some(""),
    )
    .unwrap();
    encoder(uint8, length)
}
