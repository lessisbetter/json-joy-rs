//! Factory function for creating base64 decoders with custom alphabets.

use crate::constants::ALPHABET;
use crate::Base64Error;

const PADDING_CHAR: u8 = b'=';

/// Creates a base64 decoder function with a custom alphabet.
///
/// # Arguments
///
/// * `chars` - A 64-character string representing the base64 alphabet. Defaults to standard base64.
/// * `no_padding` - If true, the decoder will add padding to inputs that are missing it.
///
/// # Returns
///
/// A function that decodes a base64 `&str` to a `Vec<u8>`.
///
/// # Errors
///
/// Returns an error if `chars` is not exactly 64 characters long.
///
/// # Example
///
/// ```
/// use json_joy_base64::create_from_base64;
///
/// let decode = create_from_base64(None, false).unwrap();
/// let result = decode("aGVsbG8=").unwrap();
/// assert_eq!(result, b"hello");
/// ```
pub fn create_from_base64(
    chars: Option<&str>,
    no_padding: bool,
) -> Result<impl Fn(&str) -> Result<Vec<u8>, Base64Error>, Base64Error> {
    let chars = chars.unwrap_or(ALPHABET);

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

    Ok(move |encoded: &str| -> Result<Vec<u8>, Base64Error> {
        if encoded.is_empty() {
            return Ok(Vec::new());
        }

        let length = encoded.len();
        let encoded_bytes = encoded.as_bytes();

        // Handle no_padding mode - calculate effective length with virtual padding
        let (effective_length, padding_suffix) = if no_padding {
            let mod_len = length % 4;
            if mod_len == 2 {
                (length + 2, 2) // Virtual "==" padding
            } else if mod_len == 3 {
                (length + 1, 1) // Virtual "=" padding
            } else {
                (length, 0)
            }
        } else {
            (length, 0)
        };

        if effective_length % 4 != 0 {
            return Err(Base64Error::InvalidLength);
        }

        // Determine main_length - the start of the last quartet (which may have padding)
        let main_length = if padding_suffix > 0 {
            // Virtual padding: last quartet starts at length - (4 - padding_suffix)
            length - (4 - padding_suffix)
        } else if encoded_bytes.get(length - 1) == Some(&PADDING_CHAR) {
            length - 4
        } else {
            length
        };

        let mut buffer_length = (effective_length >> 2) * 3;
        let mut padding = 0;

        if padding_suffix > 0 {
            padding = padding_suffix;
            buffer_length -= padding;
        } else if encoded_bytes.get(length - 2) == Some(&PADDING_CHAR) {
            padding = 2;
            buffer_length -= 2;
        } else if encoded_bytes.get(length - 1) == Some(&PADDING_CHAR) {
            padding = 1;
            buffer_length -= 1;
        }

        let mut buf = vec![0u8; buffer_length];
        let mut j = 0;
        let mut i = 0;

        while i < main_length {
            let sextet0 = if i < length {
                table[encoded_bytes[i] as usize]
            } else {
                -1
            };
            let sextet1 = if i + 1 < length {
                table[encoded_bytes[i + 1] as usize]
            } else {
                -1
            };
            let sextet2 = if i + 2 < length {
                table[encoded_bytes[i + 2] as usize]
            } else {
                -1
            };
            let sextet3 = if i + 3 < length {
                table[encoded_bytes[i + 3] as usize]
            } else {
                -1
            };

            if sextet0 < 0 || sextet1 < 0 || sextet2 < 0 || sextet3 < 0 {
                return Err(Base64Error::InvalidBase64String);
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

        if padding == 2 {
            let sextet0 = table[encoded_bytes[main_length] as usize];
            let sextet1 = if main_length + 1 < length {
                table[encoded_bytes[main_length + 1] as usize]
            } else {
                -1
            };

            if sextet0 < 0 || sextet1 < 0 {
                return Err(Base64Error::InvalidBase64String);
            }

            let sextet0 = sextet0 as u8;
            let sextet1 = sextet1 as u8;

            buf[j] = (sextet0 << 2) | (sextet1 >> 4);
        } else if padding == 1 {
            let sextet0 = table[encoded_bytes[main_length] as usize];
            let sextet1 = table[encoded_bytes[main_length + 1] as usize];
            let sextet2 = if main_length + 2 < length {
                table[encoded_bytes[main_length + 2] as usize]
            } else {
                -1
            };

            if sextet0 < 0 || sextet1 < 0 || sextet2 < 0 {
                return Err(Base64Error::InvalidBase64String);
            }

            let sextet0 = sextet0 as u8;
            let sextet1 = sextet1 as u8;
            let sextet2 = sextet2 as u8;

            buf[j] = (sextet0 << 2) | (sextet1 >> 4);
            buf[j + 1] = (sextet1 << 4) | (sextet2 >> 2);
        }

        Ok(buf)
    })
}
