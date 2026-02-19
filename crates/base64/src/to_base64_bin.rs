//! Binary base64 encoding function.

use crate::create_to_base64_bin;

/// Encodes bytes to a destination byte slice using standard base64.
///
/// # Arguments
///
/// * `uint8` - The source bytes to encode.
/// * `start` - The starting index in the source slice.
/// * `length` - The number of bytes to encode.
/// * `dest` - The destination byte slice.
/// * `offset` - The starting offset in the destination slice.
///
/// # Returns
///
/// The number of bytes written to the destination.
///
/// # Example
///
/// ```
/// use json_joy_base64::to_base64_bin;
///
/// let data = b"hello";
/// let mut dest = vec![0u8; 100];
/// let len = to_base64_bin(data, 0, data.len(), &mut dest, 0);
/// assert_eq!(&dest[..len], b"aGVsbG8=");
/// ```
pub fn to_base64_bin(
    uint8: &[u8],
    start: usize,
    length: usize,
    dest: &mut [u8],
    offset: usize,
) -> usize {
    let encoder = create_to_base64_bin(None, None).unwrap();
    encoder(uint8, start, length, dest, offset)
}
