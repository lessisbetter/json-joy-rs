//! `CborEncoderStable` — stable CBOR encoder (sorts object keys).
//!
//! Direct port of `cbor/CborEncoderStable.ts` from upstream.
//! Extends `CborEncoder` by sorting object keys before encoding.

use json_joy_buffers::{is_float32, Writer};

use super::constants::*;

/// Stable CBOR encoder.
///
/// Same as [`super::encoder::CborEncoder`] but sorts object keys
/// lexicographically (consistent, deterministic output).
/// Also uses the optimized `write_str` with pre-computed header.
pub struct CborEncoderStable {
    pub writer: Writer,
}

impl Default for CborEncoderStable {
    fn default() -> Self {
        Self::new()
    }
}

impl CborEncoderStable {
    pub fn new() -> Self {
        Self {
            writer: Writer::new(),
        }
    }

    pub fn encode(&mut self, value: &crate::PackValue) -> Vec<u8> {
        self.writer.reset();
        self.write_any(value);
        self.writer.flush()
    }

    pub fn encode_json(&mut self, value: &serde_json::Value) -> Vec<u8> {
        self.writer.reset();
        self.write_any(&crate::PackValue::from(value.clone()));
        self.writer.flush()
    }

    pub fn write_any(&mut self, value: &crate::PackValue) {
        use crate::PackValue::*;
        match value {
            Null | Undefined => self.write_null(), // stable maps undefined → null
            Bool(b) => self.write_boolean(*b),
            Integer(i) => self.write_integer(*i),
            UInteger(u) => self.write_u_integer(*u),
            Float(f) => self.write_float(*f),
            BigInt(i) => self.write_big_int(*i),
            Bytes(b) => self.write_bin(b),
            Str(s) => self.write_str(s),
            Array(arr) => self.write_arr_values(arr),
            Object(obj) => {
                // Sort keys before encoding
                let mut sorted: Vec<&(String, crate::PackValue)> = obj.iter().collect();
                sorted.sort_by(|a, b| cmp_obj_key(&a.0, &b.0));
                self.write_obj_hdr(sorted.len());
                for (key, val) in sorted {
                    self.write_str(key);
                    self.write_any(val);
                }
            }
            Extension(ext) => self.write_tag(ext.tag, &ext.val),
            Blob(blob) => self.writer.buf(&blob.val),
        }
    }

    pub fn write_null(&mut self) {
        self.writer.u8(0xf6);
    }

    pub fn write_boolean(&mut self, b: bool) {
        self.writer.u8(if b { 0xf5 } else { 0xf4 });
    }

    pub fn write_integer(&mut self, int: i64) {
        if int >= 0 {
            self.write_u_integer(int as u64);
        } else {
            self.encode_nint(int);
        }
    }

    pub fn write_u_integer(&mut self, uint: u64) {
        let w = &mut self.writer;
        w.ensure_capacity(9);
        let x = w.x;
        if uint <= 23 {
            w.uint8[x] = OVERLAY_UIN | uint as u8;
            w.x = x + 1;
        } else if uint <= 0xff {
            w.uint8[x] = 0x18;
            w.uint8[x + 1] = uint as u8;
            w.x = x + 2;
        } else if uint <= 0xffff {
            w.uint8[x] = 0x19;
            let b = (uint as u16).to_be_bytes();
            w.uint8[x + 1] = b[0];
            w.uint8[x + 2] = b[1];
            w.x = x + 3;
        } else if uint <= 0xffffffff {
            w.uint8[x] = 0x1a;
            let b = (uint as u32).to_be_bytes();
            w.uint8[x + 1..x + 5].copy_from_slice(&b);
            w.x = x + 5;
        } else {
            w.uint8[x] = 0x1b;
            let b = uint.to_be_bytes();
            w.uint8[x + 1..x + 9].copy_from_slice(&b);
            w.x = x + 9;
        }
    }

    pub fn encode_nint(&mut self, int: i64) {
        let uint = (-1i64).wrapping_sub(int) as u64;
        let w = &mut self.writer;
        w.ensure_capacity(9);
        let x = w.x;
        if uint < 24 {
            w.uint8[x] = OVERLAY_NIN | uint as u8;
            w.x = x + 1;
        } else if uint <= 0xff {
            w.uint8[x] = 0x38;
            w.uint8[x + 1] = uint as u8;
            w.x = x + 2;
        } else if uint <= 0xffff {
            w.uint8[x] = 0x39;
            let b = (uint as u16).to_be_bytes();
            w.uint8[x + 1] = b[0];
            w.uint8[x + 2] = b[1];
            w.x = x + 3;
        } else if uint <= 0xffffffff {
            w.uint8[x] = 0x3a;
            let b = (uint as u32).to_be_bytes();
            w.uint8[x + 1..x + 5].copy_from_slice(&b);
            w.x = x + 5;
        } else {
            w.uint8[x] = 0x3b;
            let b = uint.to_be_bytes();
            w.uint8[x + 1..x + 9].copy_from_slice(&b);
            w.x = x + 9;
        }
    }

    pub fn write_big_int(&mut self, int: i128) {
        if int >= 0 {
            if int as u128 <= u64::MAX as u128 {
                self.write_u_integer(int as u64);
            } else {
                self.writer.u8u64(0x1b, u64::MAX);
            }
        } else {
            if int >= i64::MIN as i128 {
                self.encode_nint(int as i64);
            } else {
                let uint = (-1i128 - int) as u64;
                self.writer.u8u64(0x3b, uint);
            }
        }
    }

    pub fn write_float(&mut self, float: f64) {
        if is_float32(float) {
            self.writer.u8f32(0xfa, float as f32);
        } else {
            self.writer.u8f64(0xfb, float);
        }
    }

    pub fn write_bin(&mut self, buf: &[u8]) {
        let length = buf.len();
        self.write_bin_hdr(length);
        self.writer.buf(buf);
    }

    pub fn write_bin_hdr(&mut self, length: usize) {
        let w = &mut self.writer;
        if length <= 23 {
            w.u8(OVERLAY_BIN | length as u8);
        } else if length <= 0xff {
            w.u8(0x58);
            w.u8(length as u8);
        } else if length <= 0xffff {
            w.u8(0x59);
            w.u16(length as u16);
        } else {
            w.u8(0x5a);
            w.u32(length as u32);
        }
    }

    /// Optimized `write_str` — pre-computes exact header based on byte length.
    /// Mirrors `CborEncoderStable.writeStr` from upstream.
    pub fn write_str(&mut self, s: &str) {
        let byte_len = s.len();

        // Header length: bytes needed for the CBOR text header
        let header_len = str_header_length(byte_len);

        self.writer.ensure_capacity(header_len + byte_len);
        let x0 = self.writer.x;
        let x1 = x0 + header_len;
        self.writer.x = x1;

        // Write the string bytes
        let x = self.writer.x;
        self.writer.uint8[x..x + byte_len].copy_from_slice(s.as_bytes());
        self.writer.x = x + byte_len;

        // Write the header at x0
        match header_len {
            1 => self.writer.uint8[x0] = OVERLAY_STR | byte_len as u8,
            2 => {
                self.writer.uint8[x0] = 0x78;
                self.writer.uint8[x0 + 1] = byte_len as u8;
            }
            3 => {
                self.writer.uint8[x0] = 0x79;
                let b = (byte_len as u16).to_be_bytes();
                self.writer.uint8[x0 + 1] = b[0];
                self.writer.uint8[x0 + 2] = b[1];
            }
            5 => {
                self.writer.uint8[x0] = 0x7a;
                let b = (byte_len as u32).to_be_bytes();
                self.writer.uint8[x0 + 1..x0 + 5].copy_from_slice(&b);
            }
            _ => unreachable!(),
        }
    }

    pub fn write_str_hdr(&mut self, length: usize) {
        let w = &mut self.writer;
        if length <= 23 {
            w.u8(OVERLAY_STR | length as u8);
        } else if length <= 0xff {
            w.u8(0x78);
            w.u8(length as u8);
        } else if length <= 0xffff {
            w.u8(0x79);
            w.u16(length as u16);
        } else {
            w.u8(0x7a);
            w.u32(length as u32);
        }
    }

    pub fn write_ascii_str(&mut self, s: &str) {
        self.write_str_hdr(s.len());
        self.writer.ascii(s);
    }

    pub fn write_arr_values(&mut self, arr: &[crate::PackValue]) {
        self.write_arr_hdr(arr.len());
        for item in arr {
            self.write_any(item);
        }
    }

    pub fn write_arr_hdr(&mut self, length: usize) {
        let w = &mut self.writer;
        if length <= 23 {
            w.u8(OVERLAY_ARR | length as u8);
        } else if length <= 0xff {
            w.u8(0x98);
            w.u8(length as u8);
        } else if length <= 0xffff {
            w.u8(0x99);
            w.u16(length as u16);
        } else {
            w.u8(0x9a);
            w.u32(length as u32);
        }
    }

    pub fn write_obj_hdr(&mut self, length: usize) {
        let w = &mut self.writer;
        if length <= 23 {
            w.u8(OVERLAY_MAP | length as u8);
        } else if length <= 0xff {
            w.u8(0xb8);
            w.u8(length as u8);
        } else if length <= 0xffff {
            w.u8(0xb9);
            w.u16(length as u16);
        } else {
            w.u8(0xba);
            w.u32(length as u32);
        }
    }

    pub fn write_tag(&mut self, tag: u64, value: &crate::PackValue) {
        self.write_tag_hdr(tag);
        self.write_any(value);
    }

    pub fn write_tag_hdr(&mut self, tag: u64) {
        let w = &mut self.writer;
        if tag <= 23 {
            w.u8(OVERLAY_TAG | tag as u8);
        } else if tag <= 0xff {
            w.u8(0xd8);
            w.u8(tag as u8);
        } else if tag <= 0xffff {
            w.u8(0xd9);
            w.u16(tag as u16);
        } else if tag <= 0xffffffff {
            w.u8(0xda);
            w.u32(tag as u32);
        } else {
            w.u8(0xdb);
            w.u64(tag);
        }
    }
}

/// Number of bytes needed for the CBOR text string header given the byte length.
fn str_header_length(size: usize) -> usize {
    if size <= 23 {
        1
    } else if size <= 0xff {
        2
    } else if size <= 0xffff {
        3
    } else {
        5
    }
}

/// Compare object keys for stable sort (mirrors `objKeyCmp` from upstream).
/// Keys are compared by byte length first, then lexicographically.
fn cmp_obj_key(a: &str, b: &str) -> std::cmp::Ordering {
    a.len().cmp(&b.len()).then_with(|| a.cmp(b))
}
