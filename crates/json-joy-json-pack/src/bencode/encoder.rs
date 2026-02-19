//! `BencodeEncoder` — BitTorrent Bencode encoder.
//!
//! Direct port of `bencode/BencodeEncoder.ts` from upstream.
//!
//! Wire format:
//! - Integer: `i<decimal>e`    e.g. `i42e`, `i-7e`
//! - String:  `<byte_len>:<bytes>` e.g. `5:hello`
//! - List:    `l<items>e`
//! - Dict:    `d<sorted key-value pairs>e`
//! - Boolean: `t` (true) or `f` (false)   [extension]
//! - Null:    `n`                           [extension]
//! - Undef:   `u`                           [extension]

use json_joy_buffers::Writer;

use crate::PackValue;

pub struct BencodeEncoder {
    pub writer: Writer,
}

impl Default for BencodeEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl BencodeEncoder {
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
                self.writer.u8(b'l');
                for item in arr {
                    self.write_json(item);
                }
                self.writer.u8(b'e');
            }
            serde_json::Value::Object(obj) => {
                self.writer.u8(b'd');
                let mut keys: Vec<&String> = obj.keys().collect();
                keys.sort();
                for key in keys {
                    self.write_str(key);
                    self.write_json(&obj[key]);
                }
                self.writer.u8(b'e');
            }
        }
    }

    pub fn write_null(&mut self) {
        self.writer.u8(b'n');
    }

    pub fn write_undef(&mut self) {
        self.writer.u8(b'u');
    }

    pub fn write_boolean(&mut self, b: bool) {
        self.writer.u8(if b { b't' } else { b'f' });
    }

    pub fn write_integer(&mut self, int: i64) {
        self.writer.u8(b'i');
        self.writer.ascii(&int.to_string());
        self.writer.u8(b'e');
    }

    pub fn write_u_integer(&mut self, uint: u64) {
        self.writer.u8(b'i');
        self.writer.ascii(&uint.to_string());
        self.writer.u8(b'e');
    }

    pub fn write_float(&mut self, float: f64) {
        self.writer.u8(b'i');
        self.writer.ascii(&(float.round() as i64).to_string());
        self.writer.u8(b'e');
    }

    pub fn write_big_int(&mut self, int: i128) {
        self.writer.u8(b'i');
        self.writer.ascii(&int.to_string());
        self.writer.u8(b'e');
    }

    pub fn write_bin(&mut self, buf: &[u8]) {
        self.writer.ascii(&buf.len().to_string());
        self.writer.u8(b':');
        self.writer.buf(buf);
    }

    pub fn write_str(&mut self, s: &str) {
        let byte_len = s.len();
        self.writer.ascii(&byte_len.to_string());
        self.writer.u8(b':');
        self.writer.buf(s.as_bytes());
    }

    pub fn write_ascii_str(&mut self, s: &str) {
        self.write_str(s);
    }

    pub fn write_arr(&mut self, arr: &[PackValue]) {
        self.writer.u8(b'l');
        for item in arr {
            self.write_any(item);
        }
        self.writer.u8(b'e');
    }

    pub fn write_obj(&mut self, obj: &[(String, PackValue)]) {
        self.writer.u8(b'd');
        // Bencode requires dict keys to be sorted lexicographically
        let mut sorted: Vec<&(String, PackValue)> = obj.iter().collect();
        sorted.sort_by(|a, b| a.0.cmp(&b.0));
        for (key, val) in sorted {
            self.write_str(key);
            self.write_any(val);
        }
        self.writer.u8(b'e');
    }
}
