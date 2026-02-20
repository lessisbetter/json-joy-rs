//! SSH 2.0 binary encoder (RFC 4251).
//!
//! Upstream reference: `json-pack/src/ssh/SshEncoder.ts`

use json_joy_buffers::Writer;

use crate::JsonPackMpint;
use crate::PackValue;

/// SSH 2.0 binary encoder.
///
/// Implements RFC 4251 binary encoding. All multi-byte quantities are
/// big-endian. Strings are uint32-length-prefixed with no padding.
pub struct SshEncoder {
    pub writer: Writer,
}

impl Default for SshEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl SshEncoder {
    pub fn new() -> Self {
        Self {
            writer: Writer::new(),
        }
    }

    /// Encodes a [`PackValue`] and returns the encoded bytes.
    ///
    /// Mapping:
    /// - `Bool` → SSH boolean (1 byte)
    /// - `Integer`/`UInteger` → uint32 or uint64 depending on range
    /// - `Str` → SSH string (uint32 length + UTF-8 bytes)
    /// - `Bytes` → SSH string (uint32 length + raw bytes)
    /// - `Array` → name-list (comma-separated ASCII strings; all elements must be `Str`)
    pub fn encode(&mut self, value: &PackValue) -> Vec<u8> {
        self.writer.reset();
        self.write_any(value);
        self.writer.flush()
    }

    pub fn write_any(&mut self, value: &PackValue) {
        match value {
            PackValue::Bool(b) => self.write_boolean(*b),
            PackValue::Integer(i) => self.write_number_i64(*i),
            PackValue::UInteger(u) => self.write_number_u64(*u),
            PackValue::Float(_) => panic!("SSH protocol does not support floating point numbers"),
            PackValue::Str(s) => self.write_str(s),
            PackValue::Bytes(b) => self.write_bin_str(b),
            PackValue::Array(arr) => self.write_name_list(arr),
            PackValue::Null | PackValue::Undefined => {
                panic!("SSH protocol does not have a null type")
            }
            PackValue::Object(_) => panic!("SSH protocol does not have an object type"),
            PackValue::BigInt(_) | PackValue::Extension(_) | PackValue::Blob(_) => {
                panic!("SSH encoder does not support this value type")
            }
        }
    }

    /// Writes an SSH boolean (1 byte: 0=false, 1=true).
    pub fn write_boolean(&mut self, b: bool) {
        self.writer.u8(if b { 1 } else { 0 });
    }

    /// Writes a single byte.
    pub fn write_byte(&mut self, byte: u8) {
        self.writer.u8(byte);
    }

    /// Writes a big-endian uint32.
    pub fn write_uint32(&mut self, val: u32) {
        self.writer.u32(val);
    }

    /// Writes a big-endian uint64.
    pub fn write_uint64(&mut self, val: u64) {
        self.writer.u64(val);
    }

    /// Writes an SSH binary string (uint32 length + raw bytes).
    pub fn write_bin_str(&mut self, data: &[u8]) {
        self.write_uint32(data.len() as u32);
        self.writer.buf(data);
    }

    /// Writes an SSH UTF-8 string (uint32 length + UTF-8 bytes).
    pub fn write_str(&mut self, s: &str) {
        let bytes = s.as_bytes();
        self.write_uint32(bytes.len() as u32);
        self.writer.buf(bytes);
    }

    /// Writes an SSH ASCII string (uint32 length + ASCII bytes).
    pub fn write_ascii_str(&mut self, s: &str) {
        self.write_uint32(s.len() as u32);
        for ch in s.bytes() {
            self.writer.u8(ch & 0x7f);
        }
    }

    /// Writes an SSH mpint (uint32 length + two's-complement MSB-first bytes).
    pub fn write_mpint(&mut self, mpint: &JsonPackMpint) {
        self.write_uint32(mpint.data.len() as u32);
        self.writer.buf(&mpint.data);
    }

    /// Writes an SSH name-list (comma-separated names, length-prefixed).
    ///
    /// All elements of `arr` must be `PackValue::Str`.
    pub fn write_name_list(&mut self, arr: &[PackValue]) {
        let names: Vec<&str> = arr
            .iter()
            .map(|v| match v {
                PackValue::Str(s) => s.as_str(),
                _ => panic!("name-list elements must be strings"),
            })
            .collect();
        let joined = names.join(",");
        self.write_ascii_str(&joined);
    }

    fn write_number_i64(&mut self, n: i64) {
        if (0..=0xffff_ffff).contains(&n) {
            self.write_uint32(n as u32);
        } else {
            self.write_uint64(n as u64);
        }
    }

    fn write_number_u64(&mut self, n: u64) {
        if n <= 0xffff_ffff {
            self.write_uint32(n as u32);
        } else {
            self.write_uint64(n);
        }
    }
}
