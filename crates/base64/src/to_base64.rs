//! Standard base64 encoding function.

use crate::constants::ALPHABET_BYTES;

/// Pre-computed two-character lookup table for base64 encoding.
/// Each entry is two bytes (big-endian) representing two base64 characters.
static TABLE2: [[u8; 2]; 4096] = {
    let mut table = [[0u8; 2]; 4096];
    let mut i = 0;
    while i < 64 {
        let mut j = 0;
        while j < 64 {
            let idx = i * 64 + j;
            table[idx][0] = ALPHABET_BYTES[i];
            table[idx][1] = ALPHABET_BYTES[j];
            j += 1;
        }
        i += 1;
    }
    table
};

/// Encodes a byte slice to a standard base64 string.
///
/// # Arguments
///
/// * `uint8` - The bytes to encode.
///
/// # Returns
///
/// A base64-encoded string with standard padding.
///
/// # Example
///
/// ```
/// use json_joy_base64::to_base64;
///
/// let encoded = to_base64(b"hello world");
/// assert_eq!(encoded, "aGVsbG8gd29ybGQ=");
/// ```
pub fn to_base64(uint8: &[u8]) -> String {
    let length = uint8.len();
    let mut out = String::with_capacity((length * 4 / 3) + 4);

    let extra_length = length % 3;
    let base_length = length - extra_length;

    let mut i = 0;
    while i < base_length {
        let o1 = uint8[i];
        let o2 = uint8[i + 1];
        let o3 = uint8[i + 2];
        let v1 = ((o1 as usize) << 4) | ((o2 as usize) >> 4);
        let v2 = (((o2 & 0b1111) as usize) << 8) | (o3 as usize);

        out.push(TABLE2[v1][0] as char);
        out.push(TABLE2[v1][1] as char);
        out.push(TABLE2[v2][0] as char);
        out.push(TABLE2[v2][1] as char);
        i += 3;
    }

    if extra_length == 0 {
        return out;
    }

    if extra_length == 1 {
        let o1 = uint8[base_length];
        let v1 = (o1 as usize) << 4;
        out.push(TABLE2[v1][0] as char);
        out.push(TABLE2[v1][1] as char);
        out.push('=');
        out.push('=');
    } else {
        // extra_length == 2
        let o1 = uint8[base_length];
        let o2 = uint8[base_length + 1];
        let v1 = ((o1 as usize) << 4) | ((o2 as usize) >> 4);
        let v2 = ((o2 & 0b1111) as usize) << 2;

        out.push(TABLE2[v1][0] as char);
        out.push(TABLE2[v1][1] as char);
        out.push(ALPHABET_BYTES[v2] as char);
        out.push('=');
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty() {
        assert_eq!(to_base64(b""), "");
    }

    #[test]
    fn test_single_byte() {
        assert_eq!(to_base64(b"f"), "Zg==");
    }

    #[test]
    fn test_two_bytes() {
        assert_eq!(to_base64(b"fo"), "Zm8=");
    }

    #[test]
    fn test_three_bytes() {
        assert_eq!(to_base64(b"foo"), "Zm9v");
    }

    #[test]
    fn test_hello_world() {
        assert_eq!(to_base64(b"hello world"), "aGVsbG8gd29ybGQ=");
    }

    #[test]
    fn test_various_lengths() {
        // Known test vectors
        assert_eq!(to_base64(b""), "");
        assert_eq!(to_base64(b"f"), "Zg==");
        assert_eq!(to_base64(b"fo"), "Zm8=");
        assert_eq!(to_base64(b"foo"), "Zm9v");
        assert_eq!(to_base64(b"foob"), "Zm9vYg==");
        assert_eq!(to_base64(b"fooba"), "Zm9vYmE=");
        assert_eq!(to_base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn test_binary_data() {
        // Test all byte values
        let data: Vec<u8> = (0..=255).collect();
        let encoded = to_base64(&data);
        assert!(!encoded.is_empty());
        // Verify it's valid base64 (only contains expected characters)
        for c in encoded.chars() {
            assert!(
                c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=',
                "Invalid base64 character: {}",
                c
            );
        }
    }
}
