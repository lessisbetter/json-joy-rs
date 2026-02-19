//! Factory function for creating base64 encoders that write to byte slices.

use crate::constants::ALPHABET;
use crate::Base64Error;

/// Creates a base64 encoder function that writes directly to a mutable byte slice.
#[allow(clippy::type_complexity)]
///
/// This is the Rust equivalent of the JavaScript `createToBase64Bin` which writes
/// to a DataView. In Rust, we use a mutable byte slice.
///
/// # Arguments
///
/// * `chars` - A 64-character string representing the base64 alphabet.
/// * `pad` - The padding character. Defaults to '='. Use empty string for no padding.
///
/// # Returns
///
/// A function that encodes bytes from a source slice to a destination slice,
/// returning the number of bytes written.
///
/// # Errors
///
/// Returns an error if `chars` is not exactly 64 characters long.
///
/// # Example
///
/// ```
/// use json_joy_base64::create_to_base64_bin;
///
/// let encode = create_to_base64_bin(None, None).unwrap();
/// let data = b"hello";
/// let mut dest = vec![0u8; 100];
/// let len = encode(data, 0, data.len(), &mut dest, 0);
/// assert_eq!(&dest[..len], b"aGVsbG8=");
/// ```
pub fn create_to_base64_bin(
    chars: Option<&str>,
    pad: Option<&str>,
) -> Result<impl Fn(&[u8], usize, usize, &mut [u8], usize) -> usize, Base64Error> {
    let chars = chars.unwrap_or(ALPHABET);
    let pad = pad.unwrap_or("=");

    if chars.len() != 64 {
        return Err(Base64Error::InvalidCharSetLength);
    }

    // Build single-byte lookup table
    let table: Vec<u8> = chars.bytes().collect();

    // Build two-byte lookup table (4096 entries)
    let mut table2: Vec<u16> = Vec::with_capacity(4096);
    for &c1 in &table {
        for &c2 in &table {
            let two = ((c1 as u16) << 8) | (c2 as u16);
            table2.push(two);
        }
    }

    let do_add_padding = pad.len() == 1;
    let e: u8 = if do_add_padding { pad.as_bytes()[0] } else { 0 };
    let ee: u16 = if do_add_padding {
        ((e as u16) << 8) | (e as u16)
    } else {
        0
    };

    Ok(
        move |uint8: &[u8],
              mut start: usize,
              length: usize,
              dest: &mut [u8],
              mut offset: usize|
              -> usize {
            let extra_length = length % 3;
            let base_length = length - extra_length;

            while start < base_length {
                let o1 = uint8[start];
                let o2 = uint8[start + 1];
                let o3 = uint8[start + 2];
                let v1 = ((o1 as u16) << 4) | ((o2 as u16) >> 4);
                let v2 = (((o2 & 0b1111) as u16) << 8) | (o3 as u16);

                // Write table2[v1] as big-endian u16 (equivalent to setInt32 big-endian)
                let pair1 = table2[v1 as usize];
                let pair2 = table2[v2 as usize];
                // Write all 4 bytes at once (pair1 << 16 | pair2 as big-endian i32)
                dest[offset] = (pair1 >> 8) as u8;
                dest[offset + 1] = pair1 as u8;
                dest[offset + 2] = (pair2 >> 8) as u8;
                dest[offset + 3] = pair2 as u8;
                offset += 4;
                start += 3;
            }

            if extra_length == 1 {
                let o1 = uint8[base_length];
                if do_add_padding {
                    let pair = table2[(o1 as usize) << 4];
                    dest[offset] = (pair >> 8) as u8;
                    dest[offset + 1] = pair as u8;
                    dest[offset + 2] = (ee >> 8) as u8;
                    dest[offset + 3] = ee as u8;
                    offset += 4;
                } else {
                    let pair = table2[(o1 as usize) << 4];
                    dest[offset] = (pair >> 8) as u8;
                    dest[offset + 1] = pair as u8;
                    offset += 2;
                }
            } else if extra_length == 2 {
                let o1 = uint8[base_length];
                let o2 = uint8[base_length + 1];
                let v1 = ((o1 as u16) << 4) | ((o2 as u16) >> 4);
                let v2 = ((o2 & 0b1111) as u16) << 2;

                if do_add_padding {
                    let pair = table2[v1 as usize];
                    dest[offset] = (pair >> 8) as u8;
                    dest[offset + 1] = pair as u8;
                    dest[offset + 2] = table[v2 as usize];
                    dest[offset + 3] = e;
                    offset += 4;
                } else {
                    let pair = table2[v1 as usize];
                    dest[offset] = (pair >> 8) as u8;
                    dest[offset + 1] = pair as u8;
                    dest[offset + 2] = table[v2 as usize];
                    offset += 3;
                }
            }

            offset
        },
    )
}
