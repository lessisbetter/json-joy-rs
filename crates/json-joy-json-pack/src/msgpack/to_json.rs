//! `MsgPackToJsonConverter` — converts MessagePack to a JSON string directly.
//!
//! Direct port of `msgpack/MsgPackToJsonConverter.ts` from upstream.
//!
//! Converts a binary MessagePack blob to a JSON string without allocating
//! intermediate `PackValue` objects. Binary and extension data are encoded
//! as data URI strings.

use crate::json_binary::constants::BIN_URI_START;

pub struct MsgPackToJsonConverter {
    data: Vec<u8>,
    x: usize,
}

impl Default for MsgPackToJsonConverter {
    fn default() -> Self {
        Self::new()
    }
}

impl MsgPackToJsonConverter {
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            x: 0,
        }
    }

    /// Convert a MessagePack blob to a JSON string.
    pub fn convert(&mut self, input: &[u8]) -> String {
        self.data = input.to_vec();
        self.x = 0;
        self.val()
    }

    fn u8(&mut self) -> u8 {
        let v = self.data[self.x];
        self.x += 1;
        v
    }

    fn u16(&mut self) -> u16 {
        let v = u16::from_be_bytes([self.data[self.x], self.data[self.x + 1]]);
        self.x += 2;
        v
    }

    fn u32(&mut self) -> u32 {
        let v = u32::from_be_bytes([
            self.data[self.x],
            self.data[self.x + 1],
            self.data[self.x + 2],
            self.data[self.x + 3],
        ]);
        self.x += 4;
        v
    }

    fn i8(&mut self) -> i8 {
        let v = self.data[self.x] as i8;
        self.x += 1;
        v
    }

    fn i16(&mut self) -> i16 {
        let v = i16::from_be_bytes([self.data[self.x], self.data[self.x + 1]]);
        self.x += 2;
        v
    }

    fn i32(&mut self) -> i32 {
        let v = i32::from_be_bytes([
            self.data[self.x],
            self.data[self.x + 1],
            self.data[self.x + 2],
            self.data[self.x + 3],
        ]);
        self.x += 4;
        v
    }

    fn f32(&mut self) -> f32 {
        let v = f32::from_be_bytes([
            self.data[self.x],
            self.data[self.x + 1],
            self.data[self.x + 2],
            self.data[self.x + 3],
        ]);
        self.x += 4;
        v
    }

    fn f64(&mut self) -> f64 {
        let v = f64::from_be_bytes([
            self.data[self.x],
            self.data[self.x + 1],
            self.data[self.x + 2],
            self.data[self.x + 3],
            self.data[self.x + 4],
            self.data[self.x + 5],
            self.data[self.x + 6],
            self.data[self.x + 7],
        ]);
        self.x += 8;
        v
    }

    fn val(&mut self) -> String {
        if self.x >= self.data.len() {
            return "null".to_string();
        }
        let byte = self.u8();

        if byte >= 0xe0 {
            return (byte as i8 as i32).to_string();
        }
        if byte <= 0x7f {
            return byte.to_string();
        }
        if (0x80..=0x8f).contains(&byte) {
            return self.obj((byte & 0xf) as usize);
        }
        if (0x90..=0x9f).contains(&byte) {
            return self.arr((byte & 0xf) as usize);
        }
        if (0xa0..=0xbf).contains(&byte) {
            let n = (byte & 0x1f) as usize;
            return self.str(n);
        }

        match byte {
            0xc0 => "null".to_string(),
            0xc1 => "null".to_string(), // undefined → null in JSON
            0xc2 => "false".to_string(),
            0xc3 => "true".to_string(),
            0xc4 => {
                let n = self.u8() as usize;
                self.bin(n)
            }
            0xc5 => {
                let n = self.u16() as usize;
                self.bin(n)
            }
            0xc6 => {
                let n = self.u32() as usize;
                self.bin(n)
            }
            0xc7 => {
                let n = self.u8() as usize;
                self.ext_val(n)
            }
            0xc8 => {
                let n = self.u16() as usize;
                self.ext_val(n)
            }
            0xc9 => {
                let n = self.u32() as usize;
                self.ext_val(n)
            }
            0xca => self.f32().to_string(),
            0xcb => self.f64().to_string(),
            0xcc => self.u8().to_string(),
            0xcd => self.u16().to_string(),
            0xce => self.u32().to_string(),
            0xcf => {
                let hi = self.u32() as u64;
                let lo = self.u32() as u64;
                (hi * 4294967296 + lo).to_string()
            }
            0xd0 => self.i8().to_string(),
            0xd1 => self.i16().to_string(),
            0xd2 => self.i32().to_string(),
            0xd3 => {
                let hi = self.i32() as i64;
                let lo = self.u32() as i64;
                (hi * 4294967296 + lo).to_string()
            }
            0xd4 => self.ext_val(1),
            0xd5 => self.ext_val(2),
            0xd6 => self.ext_val(4),
            0xd7 => self.ext_val(8),
            0xd8 => self.ext_val(16),
            0xd9 => {
                let n = self.u8() as usize;
                self.str(n)
            }
            0xda => {
                let n = self.u16() as usize;
                self.str(n)
            }
            0xdb => {
                let n = self.u32() as usize;
                self.str(n)
            }
            0xdc => {
                let n = self.u16() as usize;
                self.arr(n)
            }
            0xdd => {
                let n = self.u32() as usize;
                self.arr(n)
            }
            0xde => {
                let n = self.u16() as usize;
                self.obj(n)
            }
            0xdf => {
                let n = self.u32() as usize;
                self.obj(n)
            }
            _ => "null".to_string(),
        }
    }

    fn str(&mut self, size: usize) -> String {
        let slice = &self.data[self.x..self.x + size];
        let s = std::str::from_utf8(slice).unwrap_or("").to_string();
        self.x += size;
        // JSON-encode the string
        serde_json::to_string(&s).unwrap_or_else(|_| "\"\"".to_string())
    }

    fn obj(&mut self, size: usize) -> String {
        let mut out = "{".to_string();
        for i in 0..size {
            if i > 0 {
                out.push(',');
            }
            out.push_str(&self.val()); // key (as JSON string)
            out.push(':');
            out.push_str(&self.val());
        }
        out.push('}');
        out
    }

    fn arr(&mut self, size: usize) -> String {
        let mut out = "[".to_string();
        for i in 0..size {
            if i > 0 {
                out.push(',');
            }
            out.push_str(&self.val());
        }
        out.push(']');
        out
    }

    fn bin(&mut self, size: usize) -> String {
        let end = self.x + size;
        let buf = &self.data[self.x..end];
        let b64 = json_joy_base64::to_base64(buf);
        self.x = end;
        format!("\"{}{}\"", BIN_URI_START, b64)
    }

    fn ext_val(&mut self, size: usize) -> String {
        let _ext_type = self.u8();
        let end = self.x + size;
        let buf = &self.data[self.x..end];
        let b64 = json_joy_base64::to_base64(buf);
        self.x = end;
        // Extensions are also encoded as data URIs (simplified: use octet-stream)
        format!("\"{}{}\"", BIN_URI_START, b64)
    }
}
