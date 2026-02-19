//! RESP3 encoder.
//!
//! Upstream reference: `json-pack/src/resp/RespEncoder.ts`

use json_joy_buffers::Writer;

use super::constants::{
    Resp, RESP_EXTENSION_ATTRIBUTES, RESP_EXTENSION_PUSH, RESP_EXTENSION_VERBATIM_STRING,
};
use crate::PackValue;

/// RESP3 protocol encoder.
///
/// Encodes [`PackValue`] values to RESP3 wire format. All methods accumulate
/// output in the internal [`Writer`]; call [`Writer::flush`] to obtain the bytes.
///
/// Extension mappings:
/// - `Extension(tag=1)` → Push frame (`>`)
/// - `Extension(tag=2)` → Attributes frame (`|`)
/// - `Extension(tag=3)` → Verbatim string (`=`)
pub struct RespEncoder {
    pub writer: Writer,
}

impl Default for RespEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl RespEncoder {
    pub fn new() -> Self {
        Self {
            writer: Writer::new(),
        }
    }

    /// Encodes a value and returns the RESP bytes.
    pub fn encode(&mut self, value: &PackValue) -> Vec<u8> {
        self.write_any(value);
        self.writer.flush()
    }

    /// Writes a value into the internal writer without flushing.
    pub fn write_any(&mut self, value: &PackValue) {
        match value {
            PackValue::Null | PackValue::Undefined => self.write_null(),
            PackValue::Bool(b) => self.write_boolean(*b),
            PackValue::Integer(i) => self.write_integer(*i),
            PackValue::UInteger(u) => self.write_integer(*u as i64),
            PackValue::Float(f) => self.write_float(*f),
            PackValue::BigInt(i) => self.write_big_int(*i),
            PackValue::Str(s) => self.write_str(s),
            PackValue::Bytes(b) => self.write_bin(b),
            PackValue::Array(arr) => self.write_arr(arr),
            PackValue::Object(obj) => self.write_obj(obj),
            PackValue::Extension(ext) => {
                let tag = ext.tag;
                match tag {
                    RESP_EXTENSION_PUSH => {
                        if let PackValue::Array(arr) = ext.val.as_ref() {
                            self.write_push(arr);
                        } else {
                            self.write_null();
                        }
                    }
                    RESP_EXTENSION_ATTRIBUTES => {
                        if let PackValue::Object(obj) = ext.val.as_ref() {
                            self.write_attr(obj);
                        } else {
                            self.write_null();
                        }
                    }
                    RESP_EXTENSION_VERBATIM_STRING => {
                        if let PackValue::Str(s) = ext.val.as_ref() {
                            self.write_verbatim_str("txt", s);
                        } else {
                            self.write_null();
                        }
                    }
                    _ => self.write_null(),
                }
            }
            PackValue::Blob(_) => self.write_null(),
        }
    }

    /// Writes `\r\n` (0x0d 0x0a).
    #[inline]
    fn write_rn(&mut self) {
        self.writer.u16(Resp::RN);
    }

    /// Writes the ASCII decimal representation of `n`.
    fn write_length(&mut self, n: usize) {
        if n < 10 {
            self.writer.u8(n as u8 + 48);
            return;
        }
        if n < 100 {
            let d1 = (n / 10) as u8 + 48;
            let d0 = (n % 10) as u8 + 48;
            self.writer.u16(((d1 as u16) << 8) | d0 as u16);
            return;
        }
        let s = n.to_string();
        self.writer.ascii(&s);
    }

    pub fn write_null(&mut self) {
        self.writer.u8(Resp::NULL);
        self.write_rn();
    }

    pub fn write_boolean(&mut self, b: bool) {
        // #t\r\n or #f\r\n — 4 bytes packed as a u32 big-endian
        let val: u32 = if b {
            (Resp::BOOL as u32) << 24 | (b't' as u32) << 16 | Resp::RN as u32
        } else {
            (Resp::BOOL as u32) << 24 | (b'f' as u32) << 16 | Resp::RN as u32
        };
        self.writer.u32(val);
    }

    pub fn write_integer(&mut self, n: i64) {
        self.writer.u8(Resp::INT); // :
        let s = n.to_string();
        self.writer.ascii(&s);
        self.write_rn();
    }

    pub fn write_big_int(&mut self, n: i128) {
        self.writer.u8(Resp::BIG); // (
        let s = n.to_string();
        self.writer.ascii(&s);
        self.write_rn();
    }

    pub fn write_float(&mut self, f: f64) {
        self.writer.u8(Resp::FLOAT); // ,
        if f == f64::INFINITY {
            self.writer.u8(b'i');
            self.writer.u16(u16::from_be_bytes([b'n', b'f']));
        } else if f == f64::NEG_INFINITY {
            self.writer
                .u32(u32::from_be_bytes([b'-', b'i', b'n', b'f']));
        } else if f.is_nan() {
            self.writer.u8(b'n');
            self.writer.u16(u16::from_be_bytes([b'a', b'n']));
        } else {
            let s = format!("{}", f);
            self.writer.ascii(&s);
        }
        self.write_rn();
    }

    pub fn write_bin(&mut self, buf: &[u8]) {
        let length = buf.len();
        self.writer.u8(Resp::STR_BULK); // $
        self.write_length(length);
        self.write_rn();
        self.writer.buf(buf);
        self.write_rn();
    }

    pub fn write_str(&mut self, s: &str) {
        // Short strings without \r or \n use simple string format
        if s.len() < 64 && !s.contains('\r') && !s.contains('\n') {
            self.write_simple_str(s);
        } else {
            self.write_verbatim_str("txt", s);
        }
    }

    pub fn write_simple_str(&mut self, s: &str) {
        self.writer.u8(Resp::STR_SIMPLE); // +
        self.writer.utf8(s);
        self.write_rn();
    }

    pub fn write_bulk_str(&mut self, s: &str) {
        let bytes = s.as_bytes();
        let size = bytes.len();
        self.writer.u8(Resp::STR_BULK); // $
        self.write_length(size);
        self.write_rn();
        self.writer.buf(bytes);
        self.write_rn();
    }

    pub fn write_verbatim_str(&mut self, encoding: &str, s: &str) {
        let bytes = s.as_bytes();
        let size = bytes.len();
        self.writer.u8(Resp::STR_VERBATIM); // =
        self.write_length(size + 4);
        self.write_rn();
        // encoding prefix: "txt:" or "bin:" (4 bytes)
        let enc_bytes = encoding.as_bytes();
        assert_eq!(enc_bytes.len(), 3, "encoding must be 3 chars");
        let prefix = u32::from_be_bytes([enc_bytes[0], enc_bytes[1], enc_bytes[2], b':']);
        self.writer.u32(prefix);
        self.writer.buf(bytes);
        self.write_rn();
    }

    pub fn write_simple_err(&mut self, s: &str) {
        self.writer.u8(Resp::ERR_SIMPLE); // -
        self.writer.utf8(s);
        self.write_rn();
    }

    pub fn write_bulk_err(&mut self, s: &str) {
        let bytes = s.as_bytes();
        let size = bytes.len();
        self.writer.u8(Resp::ERR_BULK); // !
        self.write_length(size);
        self.write_rn();
        self.writer.buf(bytes);
        self.write_rn();
    }

    pub fn write_arr(&mut self, arr: &[PackValue]) {
        let length = arr.len();
        self.writer.u8(Resp::ARR); // *
        self.write_length(length);
        self.write_rn();
        for item in arr {
            self.write_any(item);
        }
    }

    pub fn write_arr_hdr(&mut self, length: usize) {
        self.writer.u8(Resp::ARR);
        self.write_length(length);
        self.write_rn();
    }

    pub fn write_obj(&mut self, obj: &[(String, PackValue)]) {
        let length = obj.len();
        self.writer.u8(Resp::OBJ); // %
        self.write_length(length);
        self.write_rn();
        for (key, value) in obj {
            self.write_str(key);
            self.write_any(value);
        }
    }

    pub fn write_obj_hdr(&mut self, length: usize) {
        self.writer.u8(Resp::OBJ);
        self.write_length(length);
        self.write_rn();
    }

    pub fn write_attr(&mut self, obj: &[(String, PackValue)]) {
        let length = obj.len();
        self.writer.u8(Resp::ATTR); // |
        self.write_length(length);
        self.write_rn();
        for (key, value) in obj {
            self.write_str(key);
            self.write_any(value);
        }
    }

    pub fn write_push(&mut self, arr: &[PackValue]) {
        let length = arr.len();
        self.writer.u8(Resp::PUSH); // >
        self.write_length(length);
        self.write_rn();
        for item in arr {
            self.write_any(item);
        }
    }

    // -------------------------------------------------------- Command encoding

    /// Encodes a Redis command (RESP2-style inline array of bulk strings).
    pub fn encode_cmd(&mut self, args: &[&str]) -> Vec<u8> {
        self.write_cmd(args);
        self.writer.flush()
    }

    pub fn write_cmd(&mut self, args: &[&str]) {
        self.write_arr_hdr(args.len());
        for arg in args {
            self.write_bulk_str(arg);
        }
    }

    // -------------------------------------------------------- Streaming

    pub fn write_start_str(&mut self) {
        // $?\r\n
        self.writer
            .u32((Resp::STR_BULK as u32) << 24 | (b'?' as u32) << 16 | Resp::RN as u32);
    }

    pub fn write_str_chunk(&mut self, s: &str) {
        let bytes = s.as_bytes();
        let size = bytes.len();
        self.writer.u8(b';');
        self.write_length(size);
        self.write_rn();
        self.writer.buf(bytes);
        self.write_rn();
    }

    pub fn write_end_str(&mut self) {
        // ;0\r\n
        self.writer
            .u32((b';' as u32) << 24 | (b'0' as u32) << 16 | Resp::RN as u32);
    }

    pub fn write_start_arr(&mut self) {
        // *?\r\n
        self.writer
            .u32((Resp::ARR as u32) << 24 | (b'?' as u32) << 16 | Resp::RN as u32);
    }

    pub fn write_end_arr(&mut self) {
        // .\r\n
        self.writer.u8(b'.');
        self.write_rn();
    }

    pub fn write_start_obj(&mut self) {
        // %?\r\n
        self.writer
            .u32((Resp::OBJ as u32) << 24 | (b'?' as u32) << 16 | Resp::RN as u32);
    }

    pub fn write_end_obj(&mut self) {
        // .\r\n
        self.writer.u8(b'.');
        self.write_rn();
    }
}
