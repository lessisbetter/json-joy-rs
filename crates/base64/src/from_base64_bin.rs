//! Binary base64 decoding function.

use crate::create_from_base64_bin;

/// Decodes base64 bytes from a source slice.
///
/// # Arguments
///
/// * `view` - The source byte slice containing base64-encoded data.
/// * `offset` - The starting offset in the source slice.
/// * `length` - The number of bytes to decode.
///
/// # Returns
///
/// The decoded bytes, or an error if the input is invalid.
///
/// # Example
///
/// ```
/// use json_joy_base64::from_base64_bin;
///
/// let encoded = b"aGVsbG8=";
/// let decoded = from_base64_bin(encoded, 0, encoded.len()).unwrap();
/// assert_eq!(decoded, b"hello");
/// ```
pub fn from_base64_bin(
    view: &[u8],
    offset: usize,
    length: usize,
) -> Result<Vec<u8>, crate::Base64Error> {
    let decoder = create_from_base64_bin(None, None)?;
    decoder(view, offset, length)
}
