//! `UbjsonEncoder` — Universal Binary JSON encoder.
//!
//! Direct port of `ubjson/UbjsonEncoder.ts` from upstream.
//!
//! Wire format markers:
//! - `Z` (0x5a) = null
//! - `N` (0x4e) = undefined/no-op
//! - `T` (0x54) = true
//! - `F` (0x46) = false
//! - `U` (0x55) = uint8 (1 byte unsigned)
//! - `i` (0x69) = int8 (1 byte signed)
//! - `I` (0x49) = int16 (2 bytes big-endian)
//! - `l` (0x6c) = int32 (4 bytes big-endian)
//! - `L` (0x4c) = int64 (8 bytes big-endian)
//! - `d` (0x64) = float32 (4 bytes big-endian)
//! - `D` (0x44) = float64 (8 bytes big-endian)
//! - `S` (0x53) = string: type byte + string-length integer + UTF-8 bytes
//! - `[` (0x5b) = array start, `]` (0x5d) = array end
//! - `{` (0x7b) = object start, `}` (0x7d) = object end
//! - Binary shorthand: `[$U#<count>` then raw bytes

use json_joy_buffers::Writer;

use crate::PackValue;

pub struct UbjsonEncoder {
    pub writer: Writer,
}

impl Default for UbjsonEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl UbjsonEncoder {
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

    pub fn encode_json(&mut self, value: &serde_json::Value) -> Vec<u8> {
        self.writer.reset();
        self.write_json(value);
        self.writer.flush()
    }

    pub fn write_json(&mut self, value: &serde_json::Value) {
        match value {
            serde_json::Value::Null => self.write_null(),
            serde_json::Value::Bool(b) => self.write_boolean(*b),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    self.write_integer(i);
                } else if let Some(f) = n.as_f64() {
                    self.write_float(f);
                }
            }
            serde_json::Value::String(s) => self.write_str(s),
            serde_json::Value::Array(arr) => {
                self.writer.u8(0x5b); // '['
                for item in arr {
                    self.write_json(item);
                }
                self.writer.u8(0x5d); // ']'
            }
            serde_json::Value::Object(obj) => {
                self.writer.u8(0x7b); // '{'
                for (key, val) in obj {
                    self.write_key(key);
                    self.write_json(val);
                }
                self.writer.u8(0x7d); // '}'
            }
        }
    }

    pub fn write_any(&mut self, value: &PackValue) {
        match value {
            PackValue::Null => self.write_null(),
            PackValue::Undefined => self.write_undef(),
            PackValue::Bool(b) => self.write_boolean(*b),
            PackValue::Integer(i) => self.write_integer(*i),
            PackValue::UInteger(u) => self.write_u_integer(*u),
            PackValue::Float(f) => self.write_float(*f),
            PackValue::BigInt(i) => self.write_big_int(*i),
            PackValue::Bytes(b) => self.write_bin(b),
            PackValue::Str(s) => self.write_str(s),
            PackValue::Array(arr) => self.write_arr(arr),
            PackValue::Object(obj) => self.write_obj(obj),
            PackValue::Extension(_) | PackValue::Blob(_) => self.write_null(),
        }
    }

    pub fn write_null(&mut self) {
        self.writer.u8(0x5a); // 'Z'
    }

    pub fn write_undef(&mut self) {
        self.writer.u8(0x4e); // 'N'
    }

    pub fn write_boolean(&mut self, b: bool) {
        self.writer.u8(if b { 0x54 } else { 0x46 }); // 'T' or 'F'
    }

    /// Write an integer using the smallest UBJSON integer type that fits.
    pub fn write_integer(&mut self, int: i64) {
        if (0..=0xff).contains(&int) {
            // uint8
            self.writer.u8(0x55); // 'U'
            self.writer.u8(int as u8);
        } else if (-128..=127).contains(&int) {
            // int8
            self.writer.u8(0x69); // 'i'
            self.writer.u8(int as i8 as u8);
        } else if (-32768..=32767).contains(&int) {
            // int16
            self.writer.ensure_capacity(3);
            let x = self.writer.x;
            self.writer.uint8[x] = 0x49; // 'I'
            let b = (int as i16).to_be_bytes();
            self.writer.uint8[x + 1] = b[0];
            self.writer.uint8[x + 2] = b[1];
            self.writer.x = x + 3;
        } else if (-2147483648..=2147483647).contains(&int) {
            // int32
            self.writer.u8(0x6c); // 'l'
            self.writer.i32(int as i32);
        } else {
            // int64
            self.writer.u8(0x4c); // 'L'
            self.writer.ensure_capacity(8);
            let x = self.writer.x;
            let b = int.to_be_bytes();
            self.writer.uint8[x..x + 8].copy_from_slice(&b);
            self.writer.x = x + 8;
        }
    }

    pub fn write_u_integer(&mut self, uint: u64) {
        if uint <= 0xff {
            self.writer.u8(0x55); // 'U'
            self.writer.u8(uint as u8);
        } else {
            self.write_integer(uint as i64);
        }
    }

    pub fn write_float(&mut self, float: f64) {
        self.writer.u8(0x44); // 'D'
        self.writer.f64(float);
    }

    pub fn write_big_int(&mut self, int: i128) {
        if int >= i64::MIN as i128 && int <= i64::MAX as i128 {
            self.write_integer(int as i64);
        } else {
            // Clamp to i64 range for UBJSON (no native i128 support)
            self.write_integer(if int > 0 { i64::MAX } else { i64::MIN });
        }
    }

    /// Write binary data using the typed array shorthand `[$U#<count>`.
    pub fn write_bin(&mut self, buf: &[u8]) {
        let length = buf.len();
        self.writer.u32(0x5b_24_55_23); // "[$U#"
        self.write_integer(length as i64);
        self.writer.buf(buf);
    }

    /// Write a UBJSON string: `S` + UBJSON-encoded length + UTF-8 bytes.
    /// Uses max-size-guess strategy to reserve length slot.
    pub fn write_str(&mut self, s: &str) {
        let char_count = s.chars().count();
        let max_len = char_count * 4;
        self.writer.ensure_capacity(max_len + 1 + 5);

        // Write 'S' type byte
        self.writer.uint8[self.writer.x] = 0x53;
        self.writer.x += 1;

        self.write_str_length_and_bytes(s, max_len);
    }

    /// Write a UBJSON object key: UBJSON-encoded length + UTF-8 bytes (no 'S' type byte).
    pub fn write_key(&mut self, s: &str) {
        let char_count = s.chars().count();
        let max_len = char_count * 4;
        self.writer.ensure_capacity(max_len + 5);
        self.write_str_length_and_bytes(s, max_len);
    }

    /// Internal: write the length-prefixed UTF-8 bytes using max-size-guess.
    fn write_str_length_and_bytes(&mut self, s: &str, max_len: usize) {
        let x = self.writer.x;
        let one_byte = max_len < 0xff;
        if one_byte {
            self.writer.uint8[x] = 0x55; // 'U'
            self.writer.x = x + 2; // reserve 1 byte for length
        } else {
            self.writer.uint8[x] = 0x6c; // 'l'
            self.writer.x = x + 5; // reserve 4 bytes for length
        }
        let actual_size = self.writer.utf8(s);
        if one_byte {
            self.writer.uint8[x + 1] = actual_size as u8;
        } else {
            let b = (actual_size as u32).to_be_bytes();
            self.writer.uint8[x + 1..x + 5].copy_from_slice(&b);
        }
    }

    pub fn write_arr(&mut self, arr: &[PackValue]) {
        self.writer.u8(0x5b); // '['
        for item in arr {
            self.write_any(item);
        }
        self.writer.u8(0x5d); // ']'
    }

    pub fn write_obj(&mut self, obj: &[(String, PackValue)]) {
        self.writer.u8(0x7b); // '{'
        for (key, val) in obj {
            self.write_key(key);
            self.write_any(val);
        }
        self.writer.u8(0x7d); // '}'
    }

    // ---- Streaming ----

    pub fn write_start_arr(&mut self) {
        self.writer.u8(0x5b);
    }

    pub fn write_end_arr(&mut self) {
        self.writer.u8(0x5d);
    }

    pub fn write_start_obj(&mut self) {
        self.writer.u8(0x7b);
    }

    pub fn write_end_obj(&mut self) {
        self.writer.u8(0x7d);
    }
}
