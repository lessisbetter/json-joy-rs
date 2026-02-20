//! `MsgPackEncoderFast` — fast MessagePack encoder.
//!
//! Direct port of `msgpack/MsgPackEncoderFast.ts` from upstream.

use json_joy_buffers::Writer;

use crate::{JsonPackExtension, JsonPackValue, PackValue};

pub struct MsgPackEncoderFast {
    pub writer: Writer,
}

impl Default for MsgPackEncoderFast {
    fn default() -> Self {
        Self::new()
    }
}

impl MsgPackEncoderFast {
    pub fn new() -> Self {
        Self {
            writer: Writer::new(),
        }
    }

    pub fn encode(&mut self, value: &PackValue) -> Vec<u8> {
        self.writer.reset();
        self.write_any(value);
        self.writer.flush()
    }

    pub fn write_any(&mut self, value: &PackValue) {
        match value {
            PackValue::Null => self.write_null(),
            PackValue::Bool(b) => self.write_boolean(*b),
            PackValue::Integer(i) => self.write_integer(*i),
            PackValue::UInteger(u) => self.write_u_integer(*u),
            PackValue::Float(f) => self.write_float(*f),
            PackValue::BigInt(i) => self.write_float(*i as f64),
            PackValue::Bytes(b) => self.write_bin(b),
            PackValue::Str(s) => self.write_str(s),
            PackValue::Array(arr) => self.write_arr(arr),
            PackValue::Object(obj) => self.write_obj_pairs(obj),
            PackValue::Undefined => self.writer.u8(0xc1),
            PackValue::Extension(ext) => self.encode_ext(ext),
            PackValue::Blob(blob) => self.write_blob(blob),
        }
    }

    /// Write pre-encoded MessagePack value bytes as-is.
    pub fn write_blob(&mut self, blob: &JsonPackValue) {
        self.writer.buf(&blob.val);
    }

    pub fn write_null(&mut self) {
        self.writer.u8(0xc0);
    }

    pub fn write_boolean(&mut self, b: bool) {
        self.writer.u8(if b { 0xc3 } else { 0xc2 });
    }

    pub fn write_float(&mut self, float: f64) {
        self.writer.u8f64(0xcb, float);
    }

    /// Encode a non-negative integer (u32 range) efficiently.
    pub fn u32_int(&mut self, num: u32) {
        let writer = &mut self.writer;
        writer.ensure_capacity(5);
        if num <= 0x7f {
            writer.uint8[writer.x] = num as u8;
            writer.x += 1;
        } else if num <= 0xffff {
            writer.uint8[writer.x] = 0xcd;
            writer.x += 1;
            writer.u16(num as u16);
        } else {
            writer.uint8[writer.x] = 0xce;
            writer.x += 1;
            writer.u32(num);
        }
    }

    /// Encode a negative integer (i32 range) efficiently.
    pub fn n32_int(&mut self, num: i32) {
        let writer = &mut self.writer;
        writer.ensure_capacity(5);
        if num >= -0x20 {
            // negative fixint: 0xe0..0xff
            writer.uint8[writer.x] = (0x100i32 + num) as u8;
            writer.x += 1;
        } else if num >= -0x8000 {
            writer.uint8[writer.x] = 0xd1;
            writer.x += 1;
            writer.u16(num as u16);
        } else {
            writer.uint8[writer.x] = 0xd2;
            writer.x += 1;
            writer.i32(num);
        }
    }

    pub fn write_integer(&mut self, int: i64) {
        if int >= 0 {
            if int <= 0xffff_ffff {
                self.u32_int(int as u32);
            } else {
                self.write_float(int as f64);
            }
        } else if int >= -0x8000_0000 {
            self.n32_int(int as i32);
        } else {
            self.write_float(int as f64);
        }
    }

    pub fn write_u_integer(&mut self, uint: u64) {
        if uint <= 0xffff_ffff {
            self.u32_int(uint as u32);
        } else {
            self.write_float(uint as f64);
        }
    }

    pub fn write_str_hdr(&mut self, length: usize) {
        if length <= 0x1f {
            self.writer.u8(0xa0 | length as u8);
        } else if length <= 0xff {
            self.writer.u16(0xd900 | length as u16);
        } else if length <= 0xffff {
            self.writer.u8u16(0xda, length as u16);
        } else {
            self.writer.u8u32(0xdb, length as u32);
        }
    }

    pub fn write_str(&mut self, s: &str) {
        let char_count = s.chars().count();
        let max_size = char_count * 4;
        self.writer.ensure_capacity(5 + max_size);

        // Reserve space for the header, then write UTF-8, then patch header.
        let length_offset;
        if max_size <= 0x1f {
            length_offset = self.writer.x;
            self.writer.x += 1; // 1-byte header
        } else if max_size <= 0xff {
            self.writer.uint8[self.writer.x] = 0xd9;
            self.writer.x += 1;
            length_offset = self.writer.x;
            self.writer.x += 1; // 1-byte length
        } else if max_size <= 0xffff {
            self.writer.uint8[self.writer.x] = 0xda;
            self.writer.x += 1;
            length_offset = self.writer.x;
            self.writer.x += 2; // 2-byte length
        } else {
            self.writer.uint8[self.writer.x] = 0xdb;
            self.writer.x += 1;
            length_offset = self.writer.x;
            self.writer.x += 4; // 4-byte length
        }

        let bytes_written = self.writer.utf8(s);

        // Patch the header with the actual byte count
        if max_size <= 0x1f {
            self.writer.uint8[length_offset] = 0xa0 | bytes_written as u8;
        } else if max_size <= 0xff {
            self.writer.uint8[length_offset] = bytes_written as u8;
        } else if max_size <= 0xffff {
            let b = (bytes_written as u16).to_be_bytes();
            self.writer.uint8[length_offset] = b[0];
            self.writer.uint8[length_offset + 1] = b[1];
        } else {
            let b = (bytes_written as u32).to_be_bytes();
            self.writer.uint8[length_offset..length_offset + 4].copy_from_slice(&b);
        }
    }

    pub fn write_ascii_str(&mut self, s: &str) {
        self.write_str_hdr(s.len());
        self.writer.ascii(s);
    }

    pub fn write_arr_hdr(&mut self, length: usize) {
        if length <= 0xf {
            self.writer.u8(0x90 | length as u8);
        } else if length <= 0xffff {
            self.writer.u8u16(0xdc, length as u16);
        } else {
            self.writer.u8u32(0xdd, length as u32);
        }
    }

    pub fn write_arr(&mut self, arr: &[PackValue]) {
        let length = arr.len();
        self.write_arr_hdr(length);
        for item in arr {
            self.write_any(item);
        }
    }

    pub fn write_obj_hdr(&mut self, length: usize) {
        if length <= 0xf {
            self.writer.u8(0x80 | length as u8);
        } else if length <= 0xffff {
            self.writer.u8u16(0xde, length as u16);
        } else {
            self.writer.u8u32(0xdf, length as u32);
        }
    }

    pub fn write_obj_pairs(&mut self, pairs: &[(String, PackValue)]) {
        let length = pairs.len();
        self.write_obj_hdr(length);
        for (key, val) in pairs {
            self.write_str(key);
            self.write_any(val);
        }
    }

    pub fn write_bin_hdr(&mut self, length: usize) {
        if length <= 0xff {
            self.writer.u16(0xc400 | length as u16);
        } else if length <= 0xffff {
            self.writer.u8u16(0xc5, length as u16);
        } else {
            self.writer.u8u32(0xc6, length as u32);
        }
    }

    pub fn write_bin(&mut self, buf: &[u8]) {
        self.write_bin_hdr(buf.len());
        self.writer.buf(buf);
    }

    pub fn encode_ext_header(&mut self, tag: i8, length: usize) {
        match length {
            1 => self.writer.u16((0xd4u16 << 8) | (tag as u8 as u16)),
            2 => self.writer.u16((0xd5u16 << 8) | (tag as u8 as u16)),
            4 => self.writer.u16((0xd6u16 << 8) | (tag as u8 as u16)),
            8 => self.writer.u16((0xd7u16 << 8) | (tag as u8 as u16)),
            16 => self.writer.u16((0xd8u16 << 8) | (tag as u8 as u16)),
            _ => {
                if length <= 0xff {
                    self.writer.u16((0xc7u16 << 8) | length as u16);
                    self.writer.u8(tag as u8);
                } else if length <= 0xffff {
                    self.writer.u8u16(0xc8, length as u16);
                    self.writer.u8(tag as u8);
                } else {
                    self.writer.u8u32(0xc9, length as u32);
                    self.writer.u8(tag as u8);
                }
            }
        }
    }

    pub fn encode_ext(&mut self, ext: &JsonPackExtension) {
        // MsgPack extension: tag is the ext type byte, val is Bytes payload
        let tag = ext.tag as i8;
        if let PackValue::Bytes(data) = ext.val.as_ref() {
            self.encode_ext_header(tag, data.len());
            self.writer.buf(data);
        } else {
            // Fallback: encode the value and treat as bin
            self.write_any(ext.val.as_ref());
        }
    }
}
