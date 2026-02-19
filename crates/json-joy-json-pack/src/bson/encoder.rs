//! BSON document encoder.
//!
//! Upstream reference: `json-pack/src/bson/BsonEncoder.ts`
//!
//! BSON is a little-endian binary format. All multi-byte integers are
//! written in little-endian byte order.

use super::values::{BsonObjectId, BsonValue};

/// Encodes a BSON document (a slice of key-value pairs) to bytes.
///
/// The top-level must always be a document (list of key-value pairs). BSON
/// does not have a scalar top-level encoding.
pub struct BsonEncoder;

impl Default for BsonEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl BsonEncoder {
    pub fn new() -> Self {
        Self
    }

    /// Encodes a BSON document to bytes.
    pub fn encode(&self, fields: &[(String, BsonValue)]) -> Vec<u8> {
        self.write_document(fields)
    }

    fn write_document(&self, fields: &[(String, BsonValue)]) -> Vec<u8> {
        let mut body: Vec<u8> = Vec::new();
        for (key, value) in fields {
            self.write_key_value(&mut body, key, value);
        }
        body.push(0); // terminating null byte
        let size = (body.len() as i32) + 4; // +4 for the 4-byte size field
        let mut result = Vec::with_capacity(4 + body.len());
        result.extend_from_slice(&size.to_le_bytes());
        result.extend_from_slice(&body);
        result
    }

    fn write_key_value(&self, buf: &mut Vec<u8>, key: &str, value: &BsonValue) {
        match value {
            BsonValue::Float(f) => {
                buf.push(0x01);
                self.write_cstring(buf, key);
                buf.extend_from_slice(&f.to_le_bytes());
            }
            BsonValue::Str(s) => {
                buf.push(0x02);
                self.write_cstring(buf, key);
                self.write_string(buf, s);
            }
            BsonValue::Document(fields) => {
                buf.push(0x03);
                self.write_cstring(buf, key);
                let doc = self.write_document(fields);
                buf.extend_from_slice(&doc);
            }
            BsonValue::Array(arr) => {
                buf.push(0x04);
                self.write_cstring(buf, key);
                // Encode array as a document with numeric string keys
                let fields: Vec<(String, BsonValue)> = arr
                    .iter()
                    .enumerate()
                    .map(|(i, v)| (i.to_string(), v.clone()))
                    .collect();
                let doc = self.write_document(&fields);
                buf.extend_from_slice(&doc);
            }
            BsonValue::Binary(bin) => {
                buf.push(0x05);
                self.write_cstring(buf, key);
                buf.extend_from_slice(&(bin.data.len() as i32).to_le_bytes());
                buf.push(bin.subtype);
                buf.extend_from_slice(&bin.data);
            }
            BsonValue::Undefined => {
                buf.push(0x06);
                self.write_cstring(buf, key);
            }
            BsonValue::ObjectId(id) => {
                buf.push(0x07);
                self.write_cstring(buf, key);
                self.write_object_id(buf, id);
            }
            BsonValue::Boolean(b) => {
                buf.push(0x08);
                self.write_cstring(buf, key);
                buf.push(if *b { 1 } else { 0 });
            }
            BsonValue::DateTime(ms) => {
                buf.push(0x09);
                self.write_cstring(buf, key);
                buf.extend_from_slice(&ms.to_le_bytes());
            }
            BsonValue::Null => {
                buf.push(0x0a);
                self.write_cstring(buf, key);
            }
            BsonValue::Regex(pattern, flags) => {
                buf.push(0x0b);
                self.write_cstring(buf, key);
                self.write_cstring(buf, pattern);
                self.write_cstring(buf, flags);
            }
            BsonValue::DbPointer(ptr) => {
                buf.push(0x0c);
                self.write_cstring(buf, key);
                self.write_string(buf, &ptr.name);
                self.write_object_id(buf, &ptr.id);
            }
            BsonValue::JavaScriptCode(jsc) => {
                buf.push(0x0d);
                self.write_cstring(buf, key);
                self.write_string(buf, &jsc.code);
            }
            BsonValue::Symbol(sym) => {
                buf.push(0x0e);
                self.write_cstring(buf, key);
                self.write_string(buf, &sym.symbol);
            }
            BsonValue::JavaScriptCodeWithScope(jscws) => {
                buf.push(0x0f);
                self.write_cstring(buf, key);
                // Reserve space for total length
                let len_start = buf.len();
                buf.extend_from_slice(&[0u8; 4]); // placeholder
                self.write_string(buf, &jscws.code);
                let scope_doc = self.write_document(&jscws.scope);
                buf.extend_from_slice(&scope_doc);
                let total_len = (buf.len() - len_start) as i32;
                buf[len_start..len_start + 4].copy_from_slice(&total_len.to_le_bytes());
            }
            BsonValue::Int32(i) => {
                buf.push(0x10);
                self.write_cstring(buf, key);
                buf.extend_from_slice(&i.to_le_bytes());
            }
            BsonValue::Timestamp(ts) => {
                buf.push(0x11);
                self.write_cstring(buf, key);
                buf.extend_from_slice(&ts.increment.to_le_bytes());
                buf.extend_from_slice(&ts.timestamp.to_le_bytes());
            }
            BsonValue::Int64(i) => {
                buf.push(0x12);
                self.write_cstring(buf, key);
                buf.extend_from_slice(&i.to_le_bytes());
            }
            BsonValue::Decimal128(dec) => {
                buf.push(0x13);
                self.write_cstring(buf, key);
                assert_eq!(dec.data.len(), 16, "Decimal128 data must be 16 bytes");
                buf.extend_from_slice(&dec.data);
            }
            BsonValue::MinKey => {
                buf.push(0xff);
                self.write_cstring(buf, key);
            }
            BsonValue::MaxKey => {
                buf.push(0x7f);
                self.write_cstring(buf, key);
            }
        }
    }

    /// Writes a null-terminated C-string. Stops at any null byte in the input.
    fn write_cstring(&self, buf: &mut Vec<u8>, s: &str) {
        for byte in s.bytes() {
            if byte == 0 {
                break;
            }
            buf.push(byte);
        }
        buf.push(0); // null terminator
    }

    /// Writes a BSON string: little-endian i32 (byte_count+1) + UTF-8 bytes + null byte.
    fn write_string(&self, buf: &mut Vec<u8>, s: &str) {
        let bytes = s.as_bytes();
        let len = (bytes.len() as i32) + 1; // +1 for null terminator
        buf.extend_from_slice(&len.to_le_bytes());
        buf.extend_from_slice(bytes);
        buf.push(0); // null terminator
    }

    /// Writes a 12-byte BSON ObjectId.
    fn write_object_id(&self, buf: &mut Vec<u8>, id: &BsonObjectId) {
        // Timestamp: 4 bytes big-endian
        buf.push((id.timestamp >> 24) as u8);
        buf.push(((id.timestamp >> 16) & 0xff) as u8);
        buf.push(((id.timestamp >> 8) & 0xff) as u8);
        buf.push((id.timestamp & 0xff) as u8);
        // Process: 5 bytes little-endian (low 4 bytes LE + 1 high byte)
        let lo32 = id.process as u32;
        let hi8 = (id.process >> 32) as u8;
        buf.extend_from_slice(&lo32.to_le_bytes()); // 4 bytes LE
        buf.push(hi8);
        // Counter: 3 bytes big-endian
        buf.push(((id.counter >> 16) & 0xff) as u8);
        buf.push(((id.counter >> 8) & 0xff) as u8);
        buf.push((id.counter & 0xff) as u8);
    }
}
