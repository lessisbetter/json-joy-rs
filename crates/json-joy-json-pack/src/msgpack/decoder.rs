//! `MsgPackDecoder` — full MessagePack decoder with skip and validation.
//!
//! Direct port of `msgpack/MsgPackDecoder.ts` from upstream.

use super::decoder_fast::MsgPackDecoderFast;
use super::error::MsgPackError;
use crate::{JsonPackValue, PackValue};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MsgPackPathSegment<'a> {
    Key(&'a str),
    Index(usize),
}

pub struct MsgPackDecoder {
    pub inner: MsgPackDecoderFast,
}

impl Default for MsgPackDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl MsgPackDecoder {
    pub fn new() -> Self {
        Self {
            inner: MsgPackDecoderFast::new(),
        }
    }

    pub fn decode(&mut self, input: &[u8]) -> Result<PackValue, MsgPackError> {
        self.inner.decode(input)
    }

    /// Reset internal reader state to decode from `input`.
    pub fn reset(&mut self, input: &[u8]) {
        self.inner.data = input.to_vec();
        self.inner.x = 0;
    }

    /// Read one MessagePack value at the current offset.
    pub fn read_any(&mut self) -> Result<PackValue, MsgPackError> {
        self.inner.read_any()
    }

    /// Decode one-level object/array values.
    ///
    /// Top-level maps and arrays are decoded, but nested maps/arrays are returned as
    /// [`PackValue::Blob`] wrappers that preserve pre-encoded bytes.
    pub fn read_level(&mut self, input: &[u8]) -> Result<PackValue, MsgPackError> {
        self.reset(input);
        self.val_one_level()
    }

    /// Skip any MessagePack value and return how many bytes it consumed.
    pub fn skip_any(&mut self) -> Result<usize, MsgPackError> {
        if self.inner.x >= self.inner.data.len() {
            return Err(MsgPackError::UnexpectedEof);
        }
        let start = self.inner.x;
        let byte = self.inner.data[self.inner.x];
        self.inner.x += 1;

        // negative fixint: 0xe0-0xff
        if byte >= 0xe0 {
            return Ok(1);
        }
        // positive fixint: 0x00-0x7f
        if byte <= 0x7f {
            return Ok(1);
        }
        // fixmap: 0x80-0x8f
        if (0x80..=0x8f).contains(&byte) {
            let n = (byte & 0xf) as usize;
            let s = self.skip_obj(n)?;
            return Ok(1 + s);
        }
        // fixarray: 0x90-0x9f
        if (0x90..=0x9f).contains(&byte) {
            let n = (byte & 0xf) as usize;
            let s = self.skip_arr(n)?;
            return Ok(1 + s);
        }
        // fixstr: 0xa0-0xbf
        if (0xa0..=0xbf).contains(&byte) {
            let n = (byte & 0x1f) as usize;
            return self.skip(n).map(|s| 1 + s);
        }

        let _after = match byte {
            0xc0..=0xc3 => 0,
            0xc4 => {
                let n = self.read_u8_size()?;
                self.skip(n)?;
                n + 1
            }
            0xc5 => {
                let n = self.read_u16_size()?;
                self.skip(n)?;
                n + 2
            }
            0xc6 => {
                let n = self.read_u32_size()?;
                self.skip(n)?;
                n + 4
            }
            0xc7 => {
                let n = self.read_u8_size()?;
                self.skip(n + 1)?;
                n + 2
            } // ext8
            0xc8 => {
                let n = self.read_u16_size()?;
                self.skip(n + 1)?;
                n + 3
            } // ext16
            0xc9 => {
                let n = self.read_u32_size()?;
                self.skip(n + 1)?;
                n + 5
            } // ext32
            0xca => self.skip(4)?,  // float32
            0xcb => self.skip(8)?,  // float64
            0xcc => self.skip(1)?,  // uint8
            0xcd => self.skip(2)?,  // uint16
            0xce => self.skip(4)?,  // uint32
            0xcf => self.skip(8)?,  // uint64
            0xd0 => self.skip(1)?,  // int8
            0xd1 => self.skip(2)?,  // int16
            0xd2 => self.skip(4)?,  // int32
            0xd3 => self.skip(8)?,  // int64
            0xd4 => self.skip(2)?,  // fixext1
            0xd5 => self.skip(3)?,  // fixext2
            0xd6 => self.skip(5)?,  // fixext4
            0xd7 => self.skip(9)?,  // fixext8
            0xd8 => self.skip(17)?, // fixext16
            0xd9 => {
                let n = self.read_u8_size()?;
                self.skip(n)?;
                n + 1
            }
            0xda => {
                let n = self.read_u16_size()?;
                self.skip(n)?;
                n + 2
            }
            0xdb => {
                let n = self.read_u32_size()?;
                self.skip(n)?;
                n + 4
            }
            0xdc => {
                let n = self.read_u16_size()?;
                let s = self.skip_arr(n)?;
                s + 2
            }
            0xdd => {
                let n = self.read_u32_size()?;
                let s = self.skip_arr(n)?;
                s + 4
            }
            0xde => {
                let n = self.read_u16_size()?;
                let s = self.skip_obj(n)?;
                s + 2
            }
            0xdf => {
                let n = self.read_u32_size()?;
                let s = self.skip_obj(n)?;
                s + 4
            }
            _ => 0,
        };
        Ok(self.inner.x - start)
    }

    fn skip_arr(&mut self, size: usize) -> Result<usize, MsgPackError> {
        let mut total = 0;
        for _ in 0..size {
            total += self.skip_any()?;
        }
        Ok(total)
    }

    fn skip_obj(&mut self, size: usize) -> Result<usize, MsgPackError> {
        let mut total = 0;
        for _ in 0..size {
            total += self.skip_any()?; // key
            total += self.skip_any()?; // value
        }
        Ok(total)
    }

    fn skip(&mut self, n: usize) -> Result<usize, MsgPackError> {
        if self.inner.x + n > self.inner.data.len() {
            return Err(MsgPackError::UnexpectedEof);
        }
        self.inner.x += n;
        Ok(n)
    }

    fn read_u8_size(&mut self) -> Result<usize, MsgPackError> {
        if self.inner.x >= self.inner.data.len() {
            return Err(MsgPackError::UnexpectedEof);
        }
        let v = self.inner.data[self.inner.x] as usize;
        self.inner.x += 1;
        Ok(v)
    }

    fn read_u16_size(&mut self) -> Result<usize, MsgPackError> {
        if self.inner.x + 2 > self.inner.data.len() {
            return Err(MsgPackError::UnexpectedEof);
        }
        let v = u16::from_be_bytes([
            self.inner.data[self.inner.x],
            self.inner.data[self.inner.x + 1],
        ]) as usize;
        self.inner.x += 2;
        Ok(v)
    }

    fn read_u32_size(&mut self) -> Result<usize, MsgPackError> {
        if self.inner.x + 4 > self.inner.data.len() {
            return Err(MsgPackError::UnexpectedEof);
        }
        let v = u32::from_be_bytes([
            self.inner.data[self.inner.x],
            self.inner.data[self.inner.x + 1],
            self.inner.data[self.inner.x + 2],
            self.inner.data[self.inner.x + 3],
        ]) as usize;
        self.inner.x += 4;
        Ok(v)
    }

    /// Validate that `data[offset..offset+size]` contains exactly one valid MessagePack value.
    pub fn validate(
        &mut self,
        data: &[u8],
        offset: usize,
        size: usize,
    ) -> Result<(), MsgPackError> {
        self.inner.data = data.to_vec();
        self.inner.x = offset;
        let start = offset;
        self.skip_any()?;
        let end = self.inner.x;
        if end - start != size {
            return Err(MsgPackError::InvalidSize);
        }
        Ok(())
    }

    pub fn read_obj_hdr(&mut self) -> Result<usize, MsgPackError> {
        if self.inner.x >= self.inner.data.len() {
            return Err(MsgPackError::UnexpectedEof);
        }
        let byte = self.inner.data[self.inner.x];
        self.inner.x += 1;
        if byte >> 4 == 0b1000 {
            return Ok((byte & 0xf) as usize);
        }
        match byte {
            0xde => self.read_u16_size(),
            0xdf => self.read_u32_size(),
            _ => Err(MsgPackError::NotObj),
        }
    }

    pub fn read_arr_hdr(&mut self) -> Result<usize, MsgPackError> {
        if self.inner.x >= self.inner.data.len() {
            return Err(MsgPackError::UnexpectedEof);
        }
        let byte = self.inner.data[self.inner.x];
        self.inner.x += 1;
        if byte >> 4 == 0b1001 {
            return Ok((byte & 0xf) as usize);
        }
        match byte {
            0xdc => self.read_u16_size(),
            0xdd => self.read_u32_size(),
            _ => Err(MsgPackError::NotArr),
        }
    }

    pub fn read_str_hdr(&mut self) -> Result<usize, MsgPackError> {
        if self.inner.x >= self.inner.data.len() {
            return Err(MsgPackError::UnexpectedEof);
        }
        let byte = self.inner.data[self.inner.x];
        self.inner.x += 1;
        if byte >> 5 == 0b101 {
            return Ok((byte & 0x1f) as usize);
        }
        match byte {
            0xd9 => self.read_u8_size(),
            0xda => self.read_u16_size(),
            0xdb => self.read_u32_size(),
            _ => Err(MsgPackError::NotStr),
        }
    }

    pub fn find_key(&mut self, key: &str) -> Result<&mut Self, MsgPackError> {
        let size = self.read_obj_hdr()?;
        for _ in 0..size {
            let current = self.inner.read_key()?;
            if current == key {
                return Ok(self);
            }
            self.skip_any()?;
        }
        Err(MsgPackError::KeyNotFound)
    }

    pub fn find_index(&mut self, index: usize) -> Result<&mut Self, MsgPackError> {
        let size = self.read_arr_hdr()?;
        if index >= size {
            return Err(MsgPackError::IndexOutOfBounds);
        }
        for _ in 0..index {
            self.skip_any()?;
        }
        Ok(self)
    }

    pub fn find_path<'a>(
        &mut self,
        path: &[MsgPackPathSegment<'a>],
    ) -> Result<&mut Self, MsgPackError> {
        for segment in path {
            match segment {
                MsgPackPathSegment::Key(key) => {
                    self.find_key(key)?;
                }
                MsgPackPathSegment::Index(index) => {
                    self.find_index(*index)?;
                }
            }
        }
        Ok(self)
    }

    fn val_one_level(&mut self) -> Result<PackValue, MsgPackError> {
        if self.inner.x >= self.inner.data.len() {
            return Err(MsgPackError::UnexpectedEof);
        }

        let byte = self.inner.data[self.inner.x];
        let is_map = matches!(byte, 0xde | 0xdf) || (byte >> 4 == 0b1000);
        if is_map {
            let size = self.read_obj_hdr()?;
            let mut obj = Vec::with_capacity(size);
            for _ in 0..size {
                let key = self.inner.read_key()?;
                let value = self.primitive()?;
                obj.push((key, value));
            }
            return Ok(PackValue::Object(obj));
        }

        let is_array = matches!(byte, 0xdc | 0xdd) || (byte >> 4 == 0b1001);
        if is_array {
            let size = self.read_arr_hdr()?;
            let mut arr = Vec::with_capacity(size);
            for _ in 0..size {
                arr.push(self.primitive()?);
            }
            return Ok(PackValue::Array(arr));
        }

        self.inner.read_any()
    }

    fn primitive(&mut self) -> Result<PackValue, MsgPackError> {
        if self.inner.x >= self.inner.data.len() {
            return Err(MsgPackError::UnexpectedEof);
        }

        let byte = self.inner.data[self.inner.x];
        let is_map = matches!(byte, 0xde | 0xdf) || (byte >> 4 == 0b1000);
        let is_array = matches!(byte, 0xdc | 0xdd) || (byte >> 4 == 0b1001);
        if is_map || is_array {
            let start = self.inner.x;
            let length = self.skip_any()?;
            let end = start + length;
            let value = self.inner.data[start..end].to_vec();
            return Ok(PackValue::Blob(JsonPackValue::new(value)));
        }

        self.inner.read_any()
    }
}
