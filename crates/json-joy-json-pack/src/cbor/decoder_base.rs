//! `CborDecoderBase` — base CBOR decoder.
//!
//! Direct port of `cbor/CborDecoderBase.ts` from upstream.

use json_joy_buffers::decode_f16;

use super::constants::*;
use super::error::CborError;
use crate::{JsonPackExtension, JsonPackValue, PackValue};

/// Internal cursor used during decoding.
pub(crate) struct Cur<'a> {
    pub data: &'a [u8],
    pub pos: usize,
}

impl<'a> Cur<'a> {
    #[inline]
    fn check(&self, n: usize) -> Result<(), CborError> {
        if self.pos + n > self.data.len() {
            Err(CborError::InvalidPayload)
        } else {
            Ok(())
        }
    }

    #[inline]
    pub fn u8(&mut self) -> Result<u8, CborError> {
        self.check(1)?;
        let v = self.data[self.pos];
        self.pos += 1;
        Ok(v)
    }

    #[inline]
    pub fn u16(&mut self) -> Result<u16, CborError> {
        self.check(2)?;
        let v = u16::from_be_bytes([self.data[self.pos], self.data[self.pos + 1]]);
        self.pos += 2;
        Ok(v)
    }

    #[inline]
    pub fn u32(&mut self) -> Result<u32, CborError> {
        self.check(4)?;
        let v = u32::from_be_bytes([
            self.data[self.pos],
            self.data[self.pos + 1],
            self.data[self.pos + 2],
            self.data[self.pos + 3],
        ]);
        self.pos += 4;
        Ok(v)
    }

    #[inline]
    pub fn u64(&mut self) -> Result<u64, CborError> {
        self.check(8)?;
        let v = u64::from_be_bytes([
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
    pub fn f32(&mut self) -> Result<f32, CborError> {
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
    pub fn f64(&mut self) -> Result<f64, CborError> {
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
    pub fn peek(&self) -> Result<u8, CborError> {
        self.check(1)?;
        Ok(self.data[self.pos])
    }

    #[inline]
    pub fn utf8(&mut self, len: usize) -> Result<&'a str, CborError> {
        self.check(len)?;
        let s = std::str::from_utf8(&self.data[self.pos..self.pos + len])
            .map_err(|_| CborError::InvalidPayload)?;
        self.pos += len;
        Ok(s)
    }

    #[inline]
    pub fn buf(&mut self, len: usize) -> Result<&'a [u8], CborError> {
        self.check(len)?;
        let s = &self.data[self.pos..self.pos + len];
        self.pos += len;
        Ok(s)
    }

    #[inline]
    pub fn skip(&mut self, n: usize) -> Result<(), CborError> {
        self.check(n)?;
        self.pos += n;
        Ok(())
    }
}

/// Base CBOR decoder. Stateless — instantiate once and reuse.
#[derive(Default)]
pub struct CborDecoderBase;

impl CborDecoderBase {
    pub fn new() -> Self {
        Self
    }

    /// Decode CBOR bytes into a [`PackValue`].
    pub fn decode(&self, input: &[u8]) -> Result<PackValue, CborError> {
        let mut cur = Cur { data: input, pos: 0 };
        self.read_any(&mut cur)
    }

    /// Decode CBOR bytes, returning value and number of bytes consumed.
    pub fn decode_with_consumed(&self, input: &[u8]) -> Result<(PackValue, usize), CborError> {
        let mut cur = Cur { data: input, pos: 0 };
        let v = self.read_any(&mut cur)?;
        Ok((v, cur.pos))
    }

    pub fn read_any(&self, c: &mut Cur) -> Result<PackValue, CborError> {
        if c.pos >= c.data.len() {
            return Err(CborError::InvalidPayload);
        }
        let octet = c.u8()?;
        self.read_any_raw(c, octet)
    }

    pub fn read_any_raw(&self, c: &mut Cur, octet: u8) -> Result<PackValue, CborError> {
        let major = octet >> 5;
        let minor = octet & MINOR_MASK;
        match major {
            MAJOR_UIN => {
                let u = self.read_uint(c, minor)?;
                // Return as Integer when it fits
                if u <= i64::MAX as u64 {
                    Ok(PackValue::Integer(u as i64))
                } else {
                    Ok(PackValue::UInteger(u))
                }
            }
            MAJOR_NIN => self.read_nint(c, minor),
            MAJOR_BIN => self.read_bin(c, minor).map(PackValue::Bytes),
            MAJOR_STR => self.read_str(c, minor).map(PackValue::Str),
            MAJOR_ARR => self.read_arr(c, minor).map(PackValue::Array),
            MAJOR_MAP => self.read_obj(c, minor).map(PackValue::Object),
            MAJOR_TAG => self.read_tag(c, minor),
            MAJOR_TKN => self.read_tkn(c, minor),
            _ => Err(CborError::UnexpectedMajor),
        }
    }

    pub fn read_minor_len(&self, c: &mut Cur, minor: u8) -> Result<i64, CborError> {
        if minor < 24 {
            return Ok(minor as i64);
        }
        match minor {
            24 => Ok(c.u8()? as i64),
            25 => Ok(c.u16()? as i64),
            26 => Ok(c.u32()? as i64),
            27 => Ok(c.u64()? as i64),
            31 => Ok(-1), // indefinite length
            _ => Err(CborError::UnexpectedMinor),
        }
    }

    // ---- Unsigned int ----

    pub fn read_uint(&self, c: &mut Cur, minor: u8) -> Result<u64, CborError> {
        if minor < 24 {
            return Ok(minor as u64);
        }
        match minor {
            24 => Ok(c.u8()? as u64),
            25 => Ok(c.u16()? as u64),
            26 => Ok(c.u32()? as u64),
            27 => c.u64(),
            _ => Err(CborError::UnexpectedMinor),
        }
    }

    // ---- Negative int ----

    pub fn read_nint(&self, c: &mut Cur, minor: u8) -> Result<PackValue, CborError> {
        let uint = self.read_uint(c, minor)?;
        let neg = -1i128 - uint as i128;
        if neg >= i64::MIN as i128 {
            Ok(PackValue::Integer(neg as i64))
        } else {
            Ok(PackValue::BigInt(neg))
        }
    }

    // ---- Binary ----

    pub fn read_bin(&self, c: &mut Cur, minor: u8) -> Result<Vec<u8>, CborError> {
        match minor {
            0..=23 => Ok(c.buf(minor as usize)?.to_vec()),
            24 => {
                let len = c.u8()? as usize;
                Ok(c.buf(len)?.to_vec())
            }
            25 => {
                let len = c.u16()? as usize;
                Ok(c.buf(len)?.to_vec())
            }
            26 => {
                let len = c.u32()? as usize;
                Ok(c.buf(len)?.to_vec())
            }
            27 => {
                let len = c.u64()? as usize;
                Ok(c.buf(len)?.to_vec())
            }
            31 => {
                let mut result = Vec::new();
                while c.peek()? != CBOR_END {
                    let chunk = self.read_bin_chunk(c)?;
                    result.extend_from_slice(&chunk);
                }
                c.pos += 1;
                Ok(result)
            }
            _ => Err(CborError::UnexpectedMinor),
        }
    }

    pub fn read_bin_chunk(&self, c: &mut Cur) -> Result<Vec<u8>, CborError> {
        let octet = c.u8()?;
        let major = octet >> 5;
        let minor = octet & MINOR_MASK;
        if major != MAJOR_BIN {
            return Err(CborError::UnexpectedBinChunkMajor);
        }
        if minor > 27 {
            return Err(CborError::UnexpectedBinChunkMinor);
        }
        self.read_bin(c, minor)
    }

    // ---- String ----

    pub fn read_str(&self, c: &mut Cur, minor: u8) -> Result<String, CborError> {
        match minor {
            0..=23 => Ok(c.utf8(minor as usize)?.to_owned()),
            24 => {
                let len = c.u8()? as usize;
                Ok(c.utf8(len)?.to_owned())
            }
            25 => {
                let len = c.u16()? as usize;
                Ok(c.utf8(len)?.to_owned())
            }
            26 => {
                let len = c.u32()? as usize;
                Ok(c.utf8(len)?.to_owned())
            }
            27 => {
                let len = c.u64()? as usize;
                Ok(c.utf8(len)?.to_owned())
            }
            31 => {
                let mut result = String::new();
                while c.peek()? != CBOR_END {
                    let chunk = self.read_str_chunk(c)?;
                    result.push_str(&chunk);
                }
                c.pos += 1;
                Ok(result)
            }
            _ => Err(CborError::UnexpectedMinor),
        }
    }

    pub fn read_str_len(&self, c: &mut Cur, minor: u8) -> Result<usize, CborError> {
        match minor {
            0..=23 => Ok(minor as usize),
            24 => Ok(c.u8()? as usize),
            25 => Ok(c.u16()? as usize),
            26 => Ok(c.u32()? as usize),
            27 => Ok(c.u64()? as usize),
            _ => Err(CborError::UnexpectedMinor),
        }
    }

    pub fn read_str_chunk(&self, c: &mut Cur) -> Result<String, CborError> {
        let octet = c.u8()?;
        let major = octet >> 5;
        let minor = octet & MINOR_MASK;
        if major != MAJOR_STR {
            return Err(CborError::UnexpectedStrChunkMajor);
        }
        if minor > 27 {
            return Err(CborError::UnexpectedStrChunkMinor);
        }
        self.read_str(c, minor)
    }

    // ---- Array ----

    pub fn read_arr(&self, c: &mut Cur, minor: u8) -> Result<Vec<PackValue>, CborError> {
        let length = self.read_minor_len(c, minor)?;
        if length >= 0 {
            self.read_arr_raw(c, length as usize)
        } else {
            self.read_arr_indef(c)
        }
    }

    pub fn read_arr_raw(&self, c: &mut Cur, length: usize) -> Result<Vec<PackValue>, CborError> {
        let mut arr = Vec::with_capacity(length);
        for _ in 0..length {
            arr.push(self.read_any(c)?);
        }
        Ok(arr)
    }

    pub fn read_arr_indef(&self, c: &mut Cur) -> Result<Vec<PackValue>, CborError> {
        let mut arr = Vec::new();
        while c.peek()? != CBOR_END {
            arr.push(self.read_any(c)?);
        }
        c.pos += 1;
        Ok(arr)
    }

    // ---- Object ----

    pub fn read_obj(&self, c: &mut Cur, minor: u8) -> Result<Vec<(String, PackValue)>, CborError> {
        let length = self.read_minor_len(c, minor)?;
        if length >= 0 {
            self.read_obj_raw(c, length as usize)
        } else {
            self.read_obj_indef(c)
        }
    }

    pub fn read_obj_raw(
        &self,
        c: &mut Cur,
        length: usize,
    ) -> Result<Vec<(String, PackValue)>, CborError> {
        let mut obj = Vec::with_capacity(length);
        for _ in 0..length {
            let key = self.read_key(c)?;
            if key == "__proto__" {
                return Err(CborError::UnexpectedObjKey);
            }
            let value = self.read_any(c)?;
            obj.push((key, value));
        }
        Ok(obj)
    }

    pub fn read_obj_indef(&self, c: &mut Cur) -> Result<Vec<(String, PackValue)>, CborError> {
        let mut obj = Vec::new();
        while c.peek()? != CBOR_END {
            let key = self.read_key(c)?;
            if key == "__proto__" {
                return Err(CborError::UnexpectedObjKey);
            }
            if c.peek()? == CBOR_END {
                return Err(CborError::UnexpectedObjBreak);
            }
            let value = self.read_any(c)?;
            obj.push((key, value));
        }
        c.pos += 1;
        Ok(obj)
    }

    /// Read object key (always returns a string).
    pub fn read_key(&self, c: &mut Cur) -> Result<String, CborError> {
        let octet = c.u8()?;
        let major = octet >> 5;
        let minor = octet & MINOR_MASK;
        if major != MAJOR_STR {
            // Non-string key: convert to string representation
            let v = self.read_any_raw(c, octet)?;
            return Ok(pack_value_to_key_string(v));
        }
        let len = self.read_str_len(c, minor)?;
        Ok(c.utf8(len)?.to_owned())
    }

    // ---- Tag ----

    pub fn read_tag(&self, c: &mut Cur, minor: u8) -> Result<PackValue, CborError> {
        let tag = self.read_uint(c, minor)?;
        self.read_tag_raw(c, tag)
    }

    pub fn read_tag_raw(&self, c: &mut Cur, tag: u64) -> Result<PackValue, CborError> {
        let val = self.read_any(c)?;
        Ok(PackValue::Extension(Box::new(JsonPackExtension::new(tag, val))))
    }

    // ---- Token ----

    pub fn read_tkn(&self, c: &mut Cur, minor: u8) -> Result<PackValue, CborError> {
        match minor {
            20 => Ok(PackValue::Bool(false)),  // 0xf4 & 0x1f
            21 => Ok(PackValue::Bool(true)),   // 0xf5 & 0x1f
            22 => Ok(PackValue::Null),          // 0xf6 & 0x1f
            23 => Ok(PackValue::Undefined),     // 0xf7 & 0x1f
            24 => {
                let v = c.u8()?;
                Ok(PackValue::Blob(JsonPackValue::new(vec![v])))
            }
            25 => {
                // f16
                let raw = c.u16()?;
                Ok(PackValue::Float(decode_f16(raw)))
            }
            26 => Ok(PackValue::Float(c.f32()? as f64)),
            27 => Ok(PackValue::Float(c.f64()?)),
            v if v <= 19 => Ok(PackValue::Blob(JsonPackValue::new(vec![v]))),
            _ => Err(CborError::UnexpectedMinor),
        }
    }

    // ---- Skip (for CborDecoder) ----

    pub fn skip_any(&self, c: &mut Cur) -> Result<(), CborError> {
        let octet = c.u8()?;
        self.skip_any_raw(c, octet)
    }

    pub fn skip_any_raw(&self, c: &mut Cur, octet: u8) -> Result<(), CborError> {
        let major = octet >> 5;
        let minor = octet & MINOR_MASK;
        match major {
            MAJOR_UIN | MAJOR_NIN => self.skip_uint(c, minor),
            MAJOR_BIN => self.skip_bin(c, minor),
            MAJOR_STR => self.skip_str(c, minor),
            MAJOR_ARR => self.skip_arr(c, minor),
            MAJOR_MAP => self.skip_obj(c, minor),
            MAJOR_TAG => self.skip_tag(c, minor),
            MAJOR_TKN => self.skip_tkn(c, minor),
            _ => Err(CborError::UnexpectedMajor),
        }
    }

    pub fn skip_uint(&self, c: &mut Cur, minor: u8) -> Result<(), CborError> {
        if minor <= 23 {
            return Ok(());
        }
        match minor {
            24 => c.skip(1),
            25 => c.skip(2),
            26 => c.skip(4),
            27 => c.skip(8),
            _ => Err(CborError::UnexpectedMinor),
        }
    }

    pub fn skip_bin(&self, c: &mut Cur, minor: u8) -> Result<(), CborError> {
        let len = self.read_minor_len(c, minor)?;
        if len >= 0 {
            c.skip(len as usize)?;
            Ok(())
        } else {
            while c.peek()? != CBOR_END {
                self.skip_bin_chunk(c)?;
            }
            c.pos += 1;
            Ok(())
        }
    }

    pub fn skip_bin_chunk(&self, c: &mut Cur) -> Result<(), CborError> {
        let octet = c.u8()?;
        let major = octet >> 5;
        let minor = octet & MINOR_MASK;
        if major != MAJOR_BIN {
            return Err(CborError::UnexpectedBinChunkMajor);
        }
        self.skip_bin(c, minor)
    }

    pub fn skip_str(&self, c: &mut Cur, minor: u8) -> Result<(), CborError> {
        let len = self.read_minor_len(c, minor)?;
        if len >= 0 {
            c.skip(len as usize)?;
            Ok(())
        } else {
            while c.peek()? != CBOR_END {
                self.skip_str_chunk(c)?;
            }
            c.pos += 1;
            Ok(())
        }
    }

    pub fn skip_str_chunk(&self, c: &mut Cur) -> Result<(), CborError> {
        let octet = c.u8()?;
        let major = octet >> 5;
        let minor = octet & MINOR_MASK;
        if major != MAJOR_STR {
            return Err(CborError::UnexpectedStrChunkMajor);
        }
        self.skip_str(c, minor)
    }

    pub fn skip_arr(&self, c: &mut Cur, minor: u8) -> Result<(), CborError> {
        let len = self.read_minor_len(c, minor)?;
        if len >= 0 {
            for _ in 0..len {
                self.skip_any(c)?;
            }
            Ok(())
        } else {
            while c.peek()? != CBOR_END {
                self.skip_any(c)?;
            }
            c.pos += 1;
            Ok(())
        }
    }

    pub fn skip_obj(&self, c: &mut Cur, minor: u8) -> Result<(), CborError> {
        let len = self.read_minor_len(c, minor)?;
        if len >= 0 {
            for _ in 0..len * 2 {
                self.skip_any(c)?;
            }
            Ok(())
        } else {
            while c.peek()? != CBOR_END {
                self.skip_any(c)?;
                if c.peek()? == CBOR_END {
                    return Err(CborError::UnexpectedObjBreak);
                }
                self.skip_any(c)?;
            }
            c.pos += 1;
            Ok(())
        }
    }

    pub fn skip_tag(&self, c: &mut Cur, minor: u8) -> Result<(), CborError> {
        let _tag = self.read_uint(c, minor)?;
        self.skip_any(c)
    }

    pub fn skip_tkn(&self, c: &mut Cur, minor: u8) -> Result<(), CborError> {
        match minor {
            24 => c.skip(1),
            25 => c.skip(2),
            26 => c.skip(4),
            27 => c.skip(8),
            v if v <= 23 => Ok(()),
            _ => Err(CborError::UnexpectedMinor),
        }
    }

    /// Validate CBOR at offset, checking exact size match.
    pub fn validate(&self, data: &[u8], offset: usize, size: usize) -> Result<(), CborError> {
        let mut c = Cur { data, pos: offset };
        let start = offset;
        self.skip_any(&mut c)?;
        let end = c.pos;
        if end - start != size {
            Err(CborError::InvalidSize)
        } else {
            Ok(())
        }
    }
}

fn pack_value_to_key_string(v: PackValue) -> String {
    match v {
        PackValue::Str(s) => s,
        PackValue::Integer(i) => i.to_string(),
        PackValue::UInteger(u) => u.to_string(),
        PackValue::Float(f) => f.to_string(),
        PackValue::Bool(b) => b.to_string(),
        PackValue::Null => "null".to_string(),
        _ => String::new(),
    }
}
