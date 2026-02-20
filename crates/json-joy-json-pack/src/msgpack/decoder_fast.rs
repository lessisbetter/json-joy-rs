//! `MsgPackDecoderFast` — fast MessagePack decoder.
//!
//! Direct port of `msgpack/MsgPackDecoderFast.ts` from upstream.

use super::error::MsgPackError;
use crate::{JsonPackExtension, PackValue};

pub struct MsgPackDecoderFast {
    pub data: Vec<u8>,
    pub x: usize,
}

impl Default for MsgPackDecoderFast {
    fn default() -> Self {
        Self::new()
    }
}

impl MsgPackDecoderFast {
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            x: 0,
        }
    }

    pub fn decode(&mut self, input: &[u8]) -> Result<PackValue, MsgPackError> {
        self.data = input.to_vec();
        self.x = 0;
        self.read_any()
    }

    #[inline]
    fn check(&self, n: usize) -> Result<(), MsgPackError> {
        if self.x + n > self.data.len() {
            Err(MsgPackError::UnexpectedEof)
        } else {
            Ok(())
        }
    }

    #[inline]
    fn u8(&mut self) -> Result<u8, MsgPackError> {
        self.check(1)?;
        let v = self.data[self.x];
        self.x += 1;
        Ok(v)
    }

    #[inline]
    fn u16(&mut self) -> Result<u16, MsgPackError> {
        self.check(2)?;
        let v = u16::from_be_bytes([self.data[self.x], self.data[self.x + 1]]);
        self.x += 2;
        Ok(v)
    }

    #[inline]
    fn u32(&mut self) -> Result<u32, MsgPackError> {
        self.check(4)?;
        let v = u32::from_be_bytes([
            self.data[self.x],
            self.data[self.x + 1],
            self.data[self.x + 2],
            self.data[self.x + 3],
        ]);
        self.x += 4;
        Ok(v)
    }

    #[inline]
    fn i8(&mut self) -> Result<i8, MsgPackError> {
        self.check(1)?;
        let v = self.data[self.x] as i8;
        self.x += 1;
        Ok(v)
    }

    #[inline]
    fn i16(&mut self) -> Result<i16, MsgPackError> {
        self.check(2)?;
        let v = i16::from_be_bytes([self.data[self.x], self.data[self.x + 1]]);
        self.x += 2;
        Ok(v)
    }

    #[inline]
    fn i32(&mut self) -> Result<i32, MsgPackError> {
        self.check(4)?;
        let v = i32::from_be_bytes([
            self.data[self.x],
            self.data[self.x + 1],
            self.data[self.x + 2],
            self.data[self.x + 3],
        ]);
        self.x += 4;
        Ok(v)
    }

    #[inline]
    fn f32(&mut self) -> Result<f32, MsgPackError> {
        self.check(4)?;
        let v = f32::from_be_bytes([
            self.data[self.x],
            self.data[self.x + 1],
            self.data[self.x + 2],
            self.data[self.x + 3],
        ]);
        self.x += 4;
        Ok(v)
    }

    #[inline]
    fn f64(&mut self) -> Result<f64, MsgPackError> {
        self.check(8)?;
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
        Ok(v)
    }

    #[inline]
    fn utf8(&mut self, size: usize) -> Result<String, MsgPackError> {
        if self.x + size > self.data.len() {
            return Err(MsgPackError::UnexpectedEof);
        }
        let slice = &self.data[self.x..self.x + size];
        let s = std::str::from_utf8(slice)
            .map_err(|_| MsgPackError::InvalidUtf8)?
            .to_string();
        self.x += size;
        Ok(s)
    }

    #[inline]
    fn buf(&mut self, size: usize) -> Result<Vec<u8>, MsgPackError> {
        if self.x + size > self.data.len() {
            return Err(MsgPackError::UnexpectedEof);
        }
        let v = self.data[self.x..self.x + size].to_vec();
        self.x += size;
        Ok(v)
    }

    pub fn read_any(&mut self) -> Result<PackValue, MsgPackError> {
        if self.x >= self.data.len() {
            return Err(MsgPackError::UnexpectedEof);
        }
        let byte = self.u8()?;

        // negative fixint: 0xe0–0xff → -32..–1
        if byte >= 0xe0 {
            return Ok(PackValue::Integer(byte as i8 as i64));
        }
        // positive fixint: 0x00–0x7f
        if byte <= 0x7f {
            return Ok(PackValue::Integer(byte as i64));
        }
        // fixmap: 0x80–0x8f
        if (0x80..=0x8f).contains(&byte) {
            return self.read_obj(byte as usize & 0xf);
        }
        // fixarray: 0x90–0x9f
        if (0x90..=0x9f).contains(&byte) {
            return self.read_arr(byte as usize & 0xf);
        }
        // fixstr: 0xa0–0xbf
        if (0xa0..=0xbf).contains(&byte) {
            let len = byte as usize & 0x1f;
            return self.utf8(len).map(PackValue::Str);
        }

        match byte {
            0xc0 => Ok(PackValue::Null),
            0xc1 => Ok(PackValue::Undefined),
            0xc2 => Ok(PackValue::Bool(false)),
            0xc3 => Ok(PackValue::Bool(true)),
            // bin8, bin16, bin32
            0xc4 => {
                let n = self.u8()? as usize;
                Ok(PackValue::Bytes(self.buf(n)?))
            }
            0xc5 => {
                let n = self.u16()? as usize;
                Ok(PackValue::Bytes(self.buf(n)?))
            }
            0xc6 => {
                let n = self.u32()? as usize;
                Ok(PackValue::Bytes(self.buf(n)?))
            }
            // ext8, ext16, ext32
            0xc7 => {
                let n = self.u8()? as usize;
                self.read_ext(n)
            }
            0xc8 => {
                let n = self.u16()? as usize;
                self.read_ext(n)
            }
            0xc9 => {
                let n = self.u32()? as usize;
                self.read_ext(n)
            }
            // float32, float64
            0xca => Ok(PackValue::Float(self.f32()? as f64)),
            0xcb => Ok(PackValue::Float(self.f64()?)),
            // uint8, uint16, uint32, uint64
            0xcc => Ok(PackValue::Integer(self.u8()? as i64)),
            0xcd => Ok(PackValue::Integer(self.u16()? as i64)),
            0xce => Ok(PackValue::Integer(self.u32()? as i64)),
            0xcf => {
                let hi = self.u32()? as u64;
                let lo = self.u32()? as u64;
                Ok(PackValue::UInteger(hi * 4294967296 + lo))
            }
            // int8, int16, int32, int64
            0xd0 => Ok(PackValue::Integer(self.i8()? as i64)),
            0xd1 => Ok(PackValue::Integer(self.i16()? as i64)),
            0xd2 => Ok(PackValue::Integer(self.i32()? as i64)),
            0xd3 => {
                let hi = self.i32()? as i64;
                let lo = self.u32()? as i64;
                Ok(PackValue::Integer(hi * 4294967296 + lo))
            }
            // fixext1, fixext2, fixext4, fixext8, fixext16
            0xd4 => self.read_ext(1),
            0xd5 => self.read_ext(2),
            0xd6 => self.read_ext(4),
            0xd7 => self.read_ext(8),
            0xd8 => self.read_ext(16),
            // str8, str16, str32
            0xd9 => {
                let n = self.u8()? as usize;
                self.utf8(n).map(PackValue::Str)
            }
            0xda => {
                let n = self.u16()? as usize;
                self.utf8(n).map(PackValue::Str)
            }
            0xdb => {
                let n = self.u32()? as usize;
                self.utf8(n).map(PackValue::Str)
            }
            // array16, array32
            0xdc => {
                let n = self.u16()? as usize;
                self.read_arr(n)
            }
            0xdd => {
                let n = self.u32()? as usize;
                self.read_arr(n)
            }
            // map16, map32
            0xde => {
                let n = self.u16()? as usize;
                self.read_obj(n)
            }
            0xdf => {
                let n = self.u32()? as usize;
                self.read_obj(n)
            }
            _ => Err(MsgPackError::InvalidByte(self.x - 1)),
        }
    }

    fn read_obj(&mut self, size: usize) -> Result<PackValue, MsgPackError> {
        let mut obj = Vec::with_capacity(size);
        for _ in 0..size {
            let key = self.read_key()?;
            if key == "__proto__" {
                return Err(MsgPackError::InvalidKey);
            }
            let val = self.read_any()?;
            obj.push((key, val));
        }
        Ok(PackValue::Object(obj))
    }

    fn read_arr(&mut self, size: usize) -> Result<PackValue, MsgPackError> {
        let mut arr = Vec::with_capacity(size);
        for _ in 0..size {
            arr.push(self.read_any()?);
        }
        Ok(PackValue::Array(arr))
    }

    fn read_ext(&mut self, size: usize) -> Result<PackValue, MsgPackError> {
        let tag = self.i8()?;
        let data = self.buf(size)?;
        // Encode MsgPack extension as Extension(tag=ext_type, val=Bytes(data))
        Ok(PackValue::Extension(Box::new(JsonPackExtension::new(
            tag as u8 as u64,
            PackValue::Bytes(data),
        ))))
    }

    /// Read a string key (no __proto__ check — caller must check).
    pub fn read_key(&mut self) -> Result<String, MsgPackError> {
        if self.x >= self.data.len() {
            return Err(MsgPackError::UnexpectedEof);
        }
        let byte = self.data[self.x];
        // fixstr
        if (0xa0..=0xbf).contains(&byte) {
            let size = (byte & 0x1f) as usize;
            self.x += 1;
            return self.utf8(size);
        }
        // str8
        if byte == 0xd9 {
            self.x += 1;
            let size = self.u8()? as usize;
            return self.utf8(size);
        }
        // str16
        if byte == 0xda {
            self.x += 1;
            let size = self.u16()? as usize;
            return self.utf8(size);
        }
        // str32
        if byte == 0xdb {
            self.x += 1;
            let size = self.u32()? as usize;
            return self.utf8(size);
        }
        Err(MsgPackError::NotStr)
    }
}
