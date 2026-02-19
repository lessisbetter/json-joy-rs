//! `JsonEncoder` — binary JSON encoder (writes UTF-8 JSON to a Writer buffer).
//!
//! Direct port of `json/JsonEncoder.ts` from upstream.
//!
//! Unlike standard JSON serializers, this encoder:
//! - Writes binary data (`PackValue::Bytes`) as data URI strings
//! - Writes `undefined` as the CBOR-undefined data URI
//! - Outputs directly to a [`json_joy_buffers::Writer`] for performance

use json_joy_buffers::Writer;

use crate::PackValue;

/// CBOR undefined encoded as `"data:application/cbor,base64;9w=="`
/// (37 bytes total including surrounding quotes).
const UNDEF_STR: &[u8] = b"\"data:application/cbor,base64;9w==\"";

/// `data:application/octet-stream;base64,` prefix (38 bytes).
const BIN_URI_PREFIX: &[u8] = b"\"data:application/octet-stream;base64,";

pub struct JsonEncoder {
    pub writer: Writer,
}

impl Default for JsonEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl JsonEncoder {
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

    pub fn write_json(&mut self, value: &serde_json::Value) {
        match value {
            serde_json::Value::Null => self.write_null(),
            serde_json::Value::Bool(b) => self.write_boolean(*b),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    self.write_integer(i);
                } else if let Some(u) = n.as_u64() {
                    self.write_u_integer(u);
                } else if let Some(f) = n.as_f64() {
                    self.write_float(f);
                }
            }
            serde_json::Value::String(s) => self.write_str(s),
            serde_json::Value::Array(arr) => {
                self.writer.u8(b'[');
                let last = arr.len().saturating_sub(1);
                for (i, item) in arr.iter().enumerate() {
                    self.write_json(item);
                    if i < last {
                        self.writer.u8(b',');
                    }
                }
                self.writer.u8(b']');
            }
            serde_json::Value::Object(obj) => {
                if obj.is_empty() {
                    self.writer.u8(b'{');
                    self.writer.u8(b'}');
                    return;
                }
                self.writer.u8(b'{');
                let keys: Vec<&String> = obj.keys().collect();
                let last = keys.len() - 1;
                for (i, key) in keys.iter().enumerate() {
                    self.write_str(key);
                    self.writer.u8(b':');
                    self.write_json(&obj[*key]);
                    if i < last {
                        self.writer.u8(b',');
                    }
                }
                self.writer.u8(b'}');
            }
        }
    }

    pub fn write_null(&mut self) {
        self.writer.u32(0x6e756c6c); // "null"
    }

    /// Write the CBOR-undefined sentinel string.
    pub fn write_undef(&mut self) {
        self.writer.buf(UNDEF_STR);
    }

    pub fn write_boolean(&mut self, b: bool) {
        if b {
            self.writer.u32(0x74727565); // "true"
        } else {
            // "false" = 0x66 0x61 0x6c 0x73 0x65
            self.writer.u8(0x66);
            self.writer.u32(0x616c7365);
        }
    }

    pub fn write_number(&mut self, num: f64) {
        // Use Rust's default float-to-string which produces minimal representation
        let s = format_float(num);
        self.writer.ascii(&s);
    }

    pub fn write_integer(&mut self, int: i64) {
        self.writer.ascii(&int.to_string());
    }

    pub fn write_u_integer(&mut self, uint: u64) {
        self.writer.ascii(&uint.to_string());
    }

    pub fn write_float(&mut self, float: f64) {
        self.writer.ascii(&format_float(float));
    }

    pub fn write_big_int(&mut self, int: i128) {
        self.writer.ascii(&int.to_string());
    }

    /// Write binary data as a data URI JSON string:
    /// `"data:application/octet-stream;base64,<base64>"`
    pub fn write_bin(&mut self, buf: &[u8]) {
        let b64 = json_joy_base64::to_base64(buf);
        self.writer.buf(BIN_URI_PREFIX);
        self.writer.buf(b64.as_bytes());
        self.writer.u8(b'"');
    }

    /// Write a JSON-encoded string (with escaping).
    pub fn write_str(&mut self, s: &str) {
        let bytes = s.as_bytes();
        let len = bytes.len();

        // Fast path: pure ASCII printable, no quotes or backslash
        if len < 256 {
            let mut has_special = false;
            for &b in bytes {
                if b < 32 || b > 126 || b == b'"' || b == b'\\' {
                    has_special = true;
                    break;
                }
            }
            if !has_special {
                self.writer.ensure_capacity(len + 2);
                let x = self.writer.x;
                self.writer.uint8[x] = b'"';
                self.writer.uint8[x + 1..x + 1 + len].copy_from_slice(bytes);
                self.writer.uint8[x + 1 + len] = b'"';
                self.writer.x = x + 2 + len;
                return;
            }
        }

        // Fall back to serde_json for proper escaping
        let json_str = serde_json::to_string(s).unwrap_or_else(|_| "\"\"".to_string());
        self.writer.buf(json_str.as_bytes());
    }

    pub fn write_ascii_str(&mut self, s: &str) {
        let len = s.len();
        self.writer.ensure_capacity(len * 2 + 2);
        self.writer.u8(b'"');
        for &b in s.as_bytes() {
            if b == b'"' || b == b'\\' {
                self.writer.u8(b'\\');
            }
            self.writer.u8(b);
        }
        self.writer.u8(b'"');
    }

    pub fn write_arr(&mut self, arr: &[PackValue]) {
        self.writer.u8(b'[');
        let last = arr.len().saturating_sub(1);
        for (i, item) in arr.iter().enumerate() {
            self.write_any(item);
            if i < last {
                self.writer.u8(b',');
            }
        }
        self.writer.u8(b']');
    }

    pub fn write_obj(&mut self, obj: &[(String, PackValue)]) {
        if obj.is_empty() {
            self.writer.u8(b'{');
            self.writer.u8(b'}');
            return;
        }
        self.writer.u8(b'{');
        let last = obj.len() - 1;
        for (i, (key, val)) in obj.iter().enumerate() {
            self.write_str(key);
            self.writer.u8(b':');
            self.write_any(val);
            if i < last {
                self.writer.u8(b',');
            }
        }
        self.writer.u8(b'}');
    }

    // ---- Streaming ----

    pub fn write_start_arr(&mut self) {
        self.writer.u8(b'[');
    }
    pub fn write_end_arr(&mut self) {
        self.writer.u8(b']');
    }
    pub fn write_start_obj(&mut self) {
        self.writer.u8(b'{');
    }
    pub fn write_end_obj(&mut self) {
        self.writer.u8(b'}');
    }
    pub fn write_arr_separator(&mut self) {
        self.writer.u8(b',');
    }
    pub fn write_obj_separator(&mut self) {
        self.writer.u8(b',');
    }
    pub fn write_obj_key_separator(&mut self) {
        self.writer.u8(b':');
    }
}

fn format_float(f: f64) -> String {
    if f.is_nan() {
        "null".to_string()
    } else if f.is_infinite() {
        if f > 0.0 {
            "1e308".to_string()
        } else {
            "-1e308".to_string()
        }
    } else if f.fract() == 0.0 && f.abs() < 1e15 {
        format!("{}", f as i64)
    } else {
        // Use Rust's default float repr (shortest round-trip representation)
        format!("{}", f)
    }
}
