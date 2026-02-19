//! Factory function for creating base64 decoders that read from byte slices.

use crate::constants::ALPHABET;
use crate::Base64Error;

/// Creates a base64 decoder function that reads from a byte slice.
#[allow(clippy::type_complexity)]
///
/// This is the Rust equivalent of the JavaScript `createFromBase64Bin` which reads
/// from a DataView. In Rust, we use a byte slice.
///
/// # Arguments
///
/// * `chars` - A 64-character string representing the base64 alphabet.
/// * `pad` - The padding character. Defaults to '='. Use empty string for no padding expected.
///
/// # Returns
///
/// A function that decodes base64 bytes from a source slice to a `Vec<u8>`.
///
/// # Errors
///
/// Returns an error if `chars` is not exactly 64 characters long.
/// The returned function may return `InvalidBase64Sequence` for invalid input.
///
/// # Example
///
/// ```
/// use json_joy_base64::create_from_base64_bin;
///
/// let decode = create_from_base64_bin(None, None).unwrap();
/// let encoded = b"aGVsbG8=";
/// let result = decode(encoded, 0, encoded.len()).unwrap();
/// assert_eq!(result, b"hello");
/// ```
pub fn create_from_base64_bin(
    chars: Option<&str>,
    pad: Option<&str>,
) -> Result<impl Fn(&[u8], usize, usize) -> Result<Vec<u8>, Base64Error>, Base64Error> {
    let chars = chars.unwrap_or(ALPHABET);
    let pad = pad.unwrap_or("=");

    if chars.len() != 64 {
        return Err(Base64Error::InvalidCharSetLength);
    }

    // Build reverse lookup table
    let chars_bytes: Vec<u8> = chars.bytes().collect();
    let max = *chars_bytes.iter().max().unwrap() as usize;
    let mut table: Vec<i16> = vec![-1; max + 1];
    for (i, &c) in chars_bytes.iter().enumerate() {
        table[c as usize] = i as i16;
    }

    let do_expect_padding = pad.len() == 1;
    let pad_byte = if do_expect_padding {
        pad.as_bytes()[0]
    } else {
        0
    };

    Ok(
        move |view: &[u8], offset: usize, length: usize| -> Result<Vec<u8>, Base64Error> {
            if length == 0 {
                return Ok(Vec::new());
            }

            let mut length = length;
            let mut padding = 0;

            if !length.is_multiple_of(4) {
                padding = 4 - (length % 4);
                length += padding;
            } else if do_expect_padding {
                let end = offset + length;
                let last = end - 1;
                if view[last] == pad_byte {
                    padding = 1;
                    if length > 1 && view[last - 1] == pad_byte {
                        padding = 2;
                    }
                }
            }

            if !length.is_multiple_of(4) {
                return Err(Base64Error::InvalidLength);
            }

            let main_end = offset + length - if padding > 0 { 4 } else { 0 };
            let buffer_length = (length >> 2) * 3 - padding;
            let mut buf = vec![0u8; buffer_length];

            let mut j = 0;
            let mut i = offset;

            while i < main_end {
                // Read 4 bytes (equivalent to getUint32 big-endian)
                let octet0 = view[i];
                let octet1 = view[i + 1];
                let octet2 = view[i + 2];
                let octet3 = view[i + 3];

                let sextet0 = if (octet0 as usize) < table.len() {
                    table[octet0 as usize]
                } else {
                    -1
                };
                let sextet1 = if (octet1 as usize) < table.len() {
                    table[octet1 as usize]
                } else {
                    -1
                };
                let sextet2 = if (octet2 as usize) < table.len() {
                    table[octet2 as usize]
                } else {
                    -1
                };
                let sextet3 = if (octet3 as usize) < table.len() {
                    table[octet3 as usize]
                } else {
                    -1
                };

                if sextet0 < 0 || sextet1 < 0 || sextet2 < 0 || sextet3 < 0 {
                    return Err(Base64Error::InvalidBase64Sequence);
                }

                let sextet0 = sextet0 as u8;
                let sextet1 = sextet1 as u8;
                let sextet2 = sextet2 as u8;
                let sextet3 = sextet3 as u8;

                buf[j] = (sextet0 << 2) | (sextet1 >> 4);
                buf[j + 1] = (sextet1 << 4) | (sextet2 >> 2);
                buf[j + 2] = (sextet2 << 6) | sextet3;
                j += 3;
                i += 4;
            }

            if padding == 0 {
                return Ok(buf);
            }

            if padding == 1 {
                // Read 2 bytes (equivalent to getUint16 big-endian)
                let octet0 = view[main_end];
                let octet1 = view[main_end + 1];
                let octet2 = view[main_end + 2];

                let sextet0 = if (octet0 as usize) < table.len() {
                    table[octet0 as usize]
                } else {
                    -1
                };
                let sextet1 = if (octet1 as usize) < table.len() {
                    table[octet1 as usize]
                } else {
                    -1
                };
                let sextet2 = if (octet2 as usize) < table.len() {
                    table[octet2 as usize]
                } else {
                    -1
                };

                if sextet0 < 0 || sextet1 < 0 || sextet2 < 0 {
                    return Err(Base64Error::InvalidBase64Sequence);
                }

                let sextet0 = sextet0 as u8;
                let sextet1 = sextet1 as u8;
                let sextet2 = sextet2 as u8;

                buf[j] = (sextet0 << 2) | (sextet1 >> 4);
                buf[j + 1] = (sextet1 << 4) | (sextet2 >> 2);
                return Ok(buf);
            }

            // padding == 2
            let octet0 = view[main_end];
            let octet1 = view[main_end + 1];

            let sextet0 = if (octet0 as usize) < table.len() {
                table[octet0 as usize]
            } else {
                -1
            };
            let sextet1 = if (octet1 as usize) < table.len() {
                table[octet1 as usize]
            } else {
                -1
            };

            if sextet0 < 0 || sextet1 < 0 {
                return Err(Base64Error::InvalidBase64Sequence);
            }

            let sextet0 = sextet0 as u8;
            let sextet1 = sextet1 as u8;

            buf[j] = (sextet0 << 2) | (sextet1 >> 4);
            Ok(buf)
        },
    )
}
