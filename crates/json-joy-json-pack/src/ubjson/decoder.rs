//! `UbjsonDecoder` — Universal Binary JSON decoder.
//!
//! Direct port of `ubjson/UbjsonDecoder.ts` from upstream.

use super::error::UbjsonError;
use crate::{JsonPackExtension, PackValue};

/// Internal cursor used during decoding.
struct Cur<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cur<'a> {
    #[inline]
    fn check(&self, n: usize) -> Result<(), UbjsonError> {
        if self.pos + n > self.data.len() {
            Err(UbjsonError::UnexpectedEof)
        } else {
            Ok(())
        }
    }

    #[inline]
    fn u8(&mut self) -> Result<u8, UbjsonError> {
        self.check(1)?;
        let v = self.data[self.pos];
        self.pos += 1;
        Ok(v)
    }

    #[inline]
    fn peek(&self) -> Result<u8, UbjsonError> {
        self.check(1)?;
        Ok(self.data[self.pos])
    }

    #[inline]
    fn i8(&mut self) -> Result<i8, UbjsonError> {
        self.check(1)?;
        let v = self.data[self.pos] as i8;
        self.pos += 1;
        Ok(v)
    }

    #[inline]
    fn i16_be(&mut self) -> Result<i16, UbjsonError> {
        self.check(2)?;
        let v = i16::from_be_bytes([self.data[self.pos], self.data[self.pos + 1]]);
        self.pos += 2;
        Ok(v)
    }

    #[inline]
    fn i32_be(&mut self) -> Result<i32, UbjsonError> {
        self.check(4)?;
        let v = i32::from_be_bytes([
            self.data[self.pos],
            self.data[self.pos + 1],
            self.data[self.pos + 2],
            self.data[self.pos + 3],
        ]);
        self.pos += 4;
        Ok(v)
    }

    #[inline]
    fn i64_be(&mut self) -> Result<i64, UbjsonError> {
        self.check(8)?;
        let v = i64::from_be_bytes([
            self.data[self.pos],
            self.data[self.pos + 1],
            self.data[self.pos + 2],
            self.data[self.pos + 3],
            self.data[self.pos + 4],
            self.data[self.pos + 5],
            self.data[self.pos + 6],
            self.data[self.pos + 7],
        ]);
        self.pos += 8;
        Ok(v)
    }

    #[inline]
    fn f32_be(&mut self) -> Result<f32, UbjsonError> {
        self.check(4)?;
        let v = f32::from_be_bytes([
            self.data[self.pos],
            self.data[self.pos + 1],
            self.data[self.pos + 2],
            self.data[self.pos + 3],
        ]);
        self.pos += 4;
        Ok(v)
    }

    #[inline]
    fn f64_be(&mut self) -> Result<f64, UbjsonError> {
        self.check(8)?;
        let v = f64::from_be_bytes([
            self.data[self.pos],
            self.data[self.pos + 1],
            self.data[self.pos + 2],
            self.data[self.pos + 3],
            self.data[self.pos + 4],
            self.data[self.pos + 5],
            self.data[self.pos + 6],
            self.data[self.pos + 7],
        ]);
        self.pos += 8;
        Ok(v)
    }

    #[inline]
    fn utf8(&mut self, len: usize) -> Result<&'a str, UbjsonError> {
        self.check(len)?;
        let s = std::str::from_utf8(&self.data[self.pos..self.pos + len])
            .map_err(|_| UbjsonError::InvalidUtf8)?;
        self.pos += len;
        Ok(s)
    }

    #[inline]
    fn buf(&mut self, len: usize) -> Result<&'a [u8], UbjsonError> {
        self.check(len)?;
        let s = &self.data[self.pos..self.pos + len];
        self.pos += len;
        Ok(s)
    }
}

/// Stateless UBJSON decoder.
#[derive(Default)]
pub struct UbjsonDecoder;

impl UbjsonDecoder {
    pub fn new() -> Self {
        Self
    }

    pub fn decode(&self, input: &[u8]) -> Result<PackValue, UbjsonError> {
        let mut c = Cur { data: input, pos: 0 };
        self.read_any(&mut c)
    }

    pub fn read_any(&self, c: &mut Cur) -> Result<PackValue, UbjsonError> {
        let octet = c.u8()?;
        match octet {
            0x5a => Ok(PackValue::Null),           // 'Z'
            0x54 => Ok(PackValue::Bool(true)),     // 'T'
            0x46 => Ok(PackValue::Bool(false)),    // 'F'
            0x4e => Ok(PackValue::Undefined),      // 'N'
            0x55 => Ok(PackValue::Integer(c.u8()? as i64)), // 'U' uint8
            0x69 => Ok(PackValue::Integer(c.i8()? as i64)), // 'i' int8
            0x49 => Ok(PackValue::Integer(c.i16_be()? as i64)), // 'I' int16
            0x6c => Ok(PackValue::Integer(c.i32_be()? as i64)), // 'l' int32
            0x4c => Ok(PackValue::Integer(c.i64_be()?)), // 'L' int64
            0x64 => Ok(PackValue::Float(c.f32_be()? as f64)), // 'd' float32
            0x44 => Ok(PackValue::Float(c.f64_be()?)), // 'D' float64
            0x53 => {
                // 'S' string: UBJSON-encoded length then UTF-8
                let len_val = self.read_any(c)?;
                let len = pack_value_to_usize(len_val)?;
                let s = c.utf8(len)?.to_owned();
                Ok(PackValue::Str(s))
            }
            0x43 => {
                // 'C' char: single UTF-8 code point encoded as 1 byte
                let byte = c.u8()?;
                Ok(PackValue::Str((byte as char).to_string()))
            }
            0x5b => self.read_arr(c), // '['
            0x7b => self.read_obj(c), // '{'
            b => Err(UbjsonError::UnexpectedByte(b, c.pos - 1)),
        }
    }

    fn read_arr(&self, c: &mut Cur) -> Result<PackValue, UbjsonError> {
        // Check for typed array: `[$U#<count>` → binary blob
        if c.data.len() > c.pos + 2
            && c.data[c.pos] == 0x24     // '$'
            && c.data[c.pos + 1] == 0x55 // 'U'
            && c.data[c.pos + 2] == 0x23 // '#'
        {
            c.pos += 3;
            let count_val = self.read_any(c)?;
            let count = pack_value_to_usize(count_val)?;
            let buf = c.buf(count)?.to_vec();
            return Ok(PackValue::Bytes(buf));
        }

        // Check for typed array with optional type then count: `[$<type>#<count>`
        let mut typed: i32 = -1;
        if c.data.len() > c.pos && c.data[c.pos] == 0x24 {
            // '$' type
            c.pos += 1;
            typed = c.u8()? as i32;
        }
        let mut count: i32 = -1;
        if c.data.len() > c.pos && c.data[c.pos] == 0x23 {
            // '#' count
            c.pos += 1;
            let count_val = self.read_any(c)?;
            count = pack_value_to_usize(count_val)? as i32;
        }
        // Second chance for type after count
        if c.data.len() > c.pos && c.data[c.pos] == 0x24 {
            c.pos += 1;
            typed = c.u8()? as i32;
        }

        if count >= 0 {
            // Typed array with count: read `count * word_size` bytes
            let word_size = match typed as u8 {
                0x49 => 2usize,                // 'I' int16
                0x6c | 0x64 => 4usize,         // 'l' int32 or 'd' float32
                0x44 | 0x4c => 8usize,         // 'D' float64 or 'L' int64
                _ => 1usize,
            };
            let total = count as usize * word_size;
            let buf = c.buf(total)?.to_vec();
            Ok(PackValue::Extension(Box::new(JsonPackExtension::new(typed as u64, PackValue::Bytes(buf)))))
        } else {
            // Standard array: read items until ']'
            let mut arr = Vec::new();
            while c.peek()? != 0x5d {
                arr.push(self.read_any(c)?);
            }
            c.pos += 1; // consume ']'
            Ok(PackValue::Array(arr))
        }
    }

    fn read_obj(&self, c: &mut Cur) -> Result<PackValue, UbjsonError> {
        let mut obj = Vec::new();
        while c.peek()? != 0x7d {
            // Key: UBJSON integer (length) + UTF-8 bytes
            let key_len_val = self.read_any(c)?;
            let key_len = pack_value_to_usize(key_len_val)?;
            let key = c.utf8(key_len)?.to_owned();
            if key == "__proto__" {
                return Err(UbjsonError::InvalidKey);
            }
            let val = self.read_any(c)?;
            obj.push((key, val));
        }
        c.pos += 1; // consume '}'
        Ok(PackValue::Object(obj))
    }
}

fn pack_value_to_usize(v: PackValue) -> Result<usize, UbjsonError> {
    match v {
        PackValue::Integer(i) => Ok(i as usize),
        PackValue::UInteger(u) => Ok(u as usize),
        _ => Err(UbjsonError::UnexpectedByte(0, 0)),
    }
}
