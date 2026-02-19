//! Factory function for creating base64 encoders that write to byte slices (Uint8Array variant).

use crate::constants::ALPHABET;
use crate::Base64Error;

/// Creates a base64 encoder function that writes directly to a mutable byte slice.
#[allow(clippy::type_complexity)]
///
/// This is similar to `create_to_base64_bin` but uses a simpler byte-by-byte write
/// pattern that matches the JavaScript `createToBase64BinUint8` implementation.
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
pub fn create_to_base64_bin_uint8(
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

    let pad_char: u8 = if pad.len() == 1 { pad.as_bytes()[0] } else { 0 };

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

                let u16 = table2[v1 as usize];
                dest[offset] = (u16 >> 8) as u8;
                dest[offset + 1] = u16 as u8;
                offset += 2;

                let u16 = table2[v2 as usize];
                dest[offset] = (u16 >> 8) as u8;
                dest[offset + 1] = u16 as u8;
                offset += 2;

                start += 3;
            }

            if extra_length == 1 {
                let o1 = uint8[base_length];
                let u16 = table2[(o1 as usize) << 4];
                dest[offset] = (u16 >> 8) as u8;
                dest[offset + 1] = u16 as u8;
                offset += 2;

                if pad_char != 0 {
                    dest[offset] = pad_char;
                    dest[offset + 1] = pad_char;
                    offset += 2;
                }
            } else if extra_length == 2 {
                let o1 = uint8[base_length];
                let o2 = uint8[base_length + 1];
                let v1 = ((o1 as u16) << 4) | ((o2 as u16) >> 4);
                let v2 = ((o2 & 0b1111) as u16) << 2;

                let u16 = table2[v1 as usize];
                dest[offset] = (u16 >> 8) as u8;
                dest[offset + 1] = u16 as u8;
                offset += 2;

                dest[offset] = table[v2 as usize];
                offset += 1;

                if pad_char != 0 {
                    dest[offset] = pad_char;
                    offset += 1;
                }
            }

            offset
        },
    )
}
