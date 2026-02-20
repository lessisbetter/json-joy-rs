//! Apache Avro encoder (no schema).
//!
//! Upstream reference: `json-pack/src/avro/AvroEncoder.ts`
//!
//! Encoding rules:
//! - null: 0 bytes
//! - boolean: 1 byte (0 or 1)
//! - int/long: zigzag + varint
//! - float: 4 bytes IEEE 754 little-endian
//! - double: 8 bytes IEEE 754 little-endian
//! - bytes/string: varint(length) + raw bytes
//! - array/map: varint(count) + items + varint(0)

use json_joy_buffers::Writer;

use crate::PackValue;

/// Apache Avro encoder (schema-free).
pub struct AvroEncoder {
    pub writer: Writer,
}

impl Default for AvroEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl AvroEncoder {
    pub fn new() -> Self {
        Self {
            writer: Writer::new(),
        }
    }

    // ---------------------------------------------------------------- varint

    /// Writes a zigzag-encoded signed integer as a varint.
    pub fn write_int(&mut self, n: i32) {
        let encoded = ((n << 1) ^ (n >> 31)) as u32;
        self.write_varint_u64(encoded as u64);
    }

    /// Writes a zigzag-encoded signed long as a varint.
    pub fn write_long(&mut self, n: i64) {
        let encoded = ((n << 1) ^ (n >> 63)) as u64;
        self.write_varint_u64(encoded);
    }

    /// Writes a variable-length unsigned integer (no zigzag).
    pub fn write_varint_u64(&mut self, mut n: u64) {
        loop {
            let low7 = (n & 0x7f) as u8;
            n >>= 7;
            if n == 0 {
                self.writer.u8(low7);
                return;
            }
            self.writer.u8(low7 | 0x80);
        }
    }

    /// Writes a variable-length unsigned 32-bit integer.
    pub fn write_varint_u32(&mut self, mut n: u32) {
        loop {
            let low7 = (n & 0x7f) as u8;
            n >>= 7;
            if n == 0 {
                self.writer.u8(low7);
                return;
            }
            self.writer.u8(low7 | 0x80);
        }
    }

    // ---------------------------------------------------------------- primitives

    pub fn write_null(&mut self) {
        // No bytes.
    }

    pub fn write_boolean(&mut self, b: bool) {
        self.writer.u8(if b { 1 } else { 0 });
    }

    pub fn write_float(&mut self, f: f32) {
        let bits = f.to_bits().to_le_bytes();
        self.writer.buf(&bits);
    }

    pub fn write_double(&mut self, f: f64) {
        let bits = f.to_bits().to_le_bytes();
        self.writer.buf(&bits);
    }

    pub fn write_bytes(&mut self, data: &[u8]) {
        self.write_varint_u32(data.len() as u32);
        self.writer.buf(data);
    }

    pub fn write_str(&mut self, s: &str) {
        let bytes = s.as_bytes();
        self.write_varint_u32(bytes.len() as u32);
        self.writer.buf(bytes);
    }

    pub fn write_ascii_str(&mut self, s: &str) {
        self.write_varint_u32(s.len() as u32);
        self.writer.ascii(s);
    }

    /// Writes an array block: varint(count) + items + varint(0).
    pub fn write_array_start(&mut self, count: usize) {
        self.write_varint_u32(count as u32);
    }

    pub fn write_array_end(&mut self) {
        self.write_varint_u32(0);
    }

    pub fn write_map_start(&mut self, count: usize) {
        self.write_varint_u32(count as u32);
    }

    pub fn write_map_end(&mut self) {
        self.write_varint_u32(0);
    }

    /// Writes any [`PackValue`] using type inference.
    pub fn write_any(&mut self, value: &PackValue) {
        match value {
            PackValue::Null | PackValue::Undefined => self.write_null(),
            PackValue::Bool(b) => self.write_boolean(*b),
            PackValue::Integer(n) => {
                if *n >= i32::MIN as i64 && *n <= i32::MAX as i64 {
                    self.write_int(*n as i32);
                } else {
                    self.write_long(*n);
                }
            }
            PackValue::UInteger(n) => {
                if *n <= i32::MAX as u64 {
                    self.write_int(*n as i32);
                } else {
                    self.write_long(*n as i64);
                }
            }
            PackValue::Float(f) => self.write_double(*f),
            PackValue::BigInt(n) => self.write_long(*n as i64),
            PackValue::Str(s) => self.write_str(s),
            PackValue::Bytes(b) => self.write_bytes(b),
            PackValue::Array(arr) => {
                self.write_varint_u32(arr.len() as u32);
                for item in arr {
                    self.write_any(item);
                }
                self.write_varint_u32(0);
            }
            PackValue::Object(obj) => {
                self.write_varint_u32(obj.len() as u32);
                for (key, val) in obj {
                    self.write_str(key);
                    self.write_any(val);
                }
                self.write_varint_u32(0);
            }
            PackValue::Extension(_) | PackValue::Blob(_) => self.write_null(),
        }
    }

    // ---------------------------------------------------------------- encode top-level

    pub fn encode_null(&mut self) -> Vec<u8> {
        self.writer.flush()
    }

    pub fn encode_boolean(&mut self, b: bool) -> Vec<u8> {
        self.write_boolean(b);
        self.writer.flush()
    }

    pub fn encode_int(&mut self, n: i32) -> Vec<u8> {
        self.write_int(n);
        self.writer.flush()
    }

    pub fn encode_long(&mut self, n: i64) -> Vec<u8> {
        self.write_long(n);
        self.writer.flush()
    }

    pub fn encode_float(&mut self, f: f32) -> Vec<u8> {
        self.write_float(f);
        self.writer.flush()
    }

    pub fn encode_double(&mut self, f: f64) -> Vec<u8> {
        self.write_double(f);
        self.writer.flush()
    }

    pub fn encode_bytes(&mut self, data: &[u8]) -> Vec<u8> {
        self.write_bytes(data);
        self.writer.flush()
    }

    pub fn encode_str(&mut self, s: &str) -> Vec<u8> {
        self.write_str(s);
        self.writer.flush()
    }
}
