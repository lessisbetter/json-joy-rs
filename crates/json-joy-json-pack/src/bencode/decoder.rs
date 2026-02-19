//! `BencodeDecoder` — BitTorrent Bencode decoder.
//!
//! Direct port of `bencode/BencodeDecoder.ts` from upstream.

use super::error::BencodeError;
use crate::PackValue;

/// Internal cursor used during decoding.
struct Cur<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cur<'a> {
    #[inline]
    fn peek(&self) -> u8 {
        self.data[self.pos]
    }

    #[inline]
    fn u8(&mut self) -> u8 {
        let v = self.data[self.pos];
        self.pos += 1;
        v
    }
}

/// Stateless Bencode decoder.
#[derive(Default)]
pub struct BencodeDecoder;

impl BencodeDecoder {
    pub fn new() -> Self {
        Self
    }

    pub fn decode(&self, input: &[u8]) -> Result<PackValue, BencodeError> {
        let mut c = Cur {
            data: input,
            pos: 0,
        };
        self.read_any(&mut c)
    }

    fn read_any(&self, c: &mut Cur) -> Result<PackValue, BencodeError> {
        let ch = c.peek();
        match ch {
            b'i' => self.read_num(c),
            b'd' => self.read_obj(c),
            b'l' => self.read_arr(c),
            b't' => {
                c.pos += 1;
                Ok(PackValue::Bool(true))
            }
            b'f' => {
                c.pos += 1;
                Ok(PackValue::Bool(false))
            }
            b'n' => {
                c.pos += 1;
                Ok(PackValue::Null)
            }
            b'u' => {
                c.pos += 1;
                Ok(PackValue::Undefined)
            }
            b'0'..=b'9' => self.read_bin_as_value(c),
            _ => Err(BencodeError::InvalidByte(c.pos)),
        }
    }

    /// Read a bencode integer (`i<decimal>e`) as a PackValue.
    fn read_num(&self, c: &mut Cur) -> Result<PackValue, BencodeError> {
        if c.u8() != b'i' {
            return Err(BencodeError::InvalidByte(c.pos - 1));
        }
        let mut num_str = String::new();
        let mut i = 0usize;
        loop {
            let ch = c.u8();
            if ch == b'e' {
                break;
            }
            num_str.push(ch as char);
            i += 1;
            if i > 25 {
                return Err(BencodeError::IntegerOverflow);
            }
        }
        if num_str.is_empty() {
            return Err(BencodeError::InvalidByte(c.pos));
        }
        // Try i64 first, fall back to i128
        if let Ok(n) = num_str.parse::<i64>() {
            Ok(PackValue::Integer(n))
        } else if let Ok(n) = num_str.parse::<i128>() {
            Ok(PackValue::BigInt(n))
        } else {
            Err(BencodeError::IntegerOverflow)
        }
    }

    /// Read a bencode string (`<len>:<bytes>`) and return as `PackValue::Bytes`.
    fn read_bin_as_value(&self, c: &mut Cur) -> Result<PackValue, BencodeError> {
        let buf = self.read_bin(c)?;
        Ok(PackValue::Bytes(buf))
    }

    /// Read a bencode string (`<len>:<bytes>`) as raw bytes.
    fn read_bin(&self, c: &mut Cur) -> Result<Vec<u8>, BencodeError> {
        let mut len_str = String::new();
        let mut i = 0usize;
        loop {
            let ch = c.u8();
            if ch == b':' {
                break;
            }
            if ch < b'0' || ch > b'9' {
                return Err(BencodeError::InvalidByte(c.pos - 1));
            }
            len_str.push(ch as char);
            i += 1;
            if i > 10 {
                return Err(BencodeError::IntegerOverflow);
            }
        }
        let len: usize = len_str.parse().map_err(|_| BencodeError::IntegerOverflow)?;
        let buf = c.data[c.pos..c.pos + len].to_vec();
        c.pos += len;
        Ok(buf)
    }

    /// Read a bencode string and decode as UTF-8.
    fn read_str(&self, c: &mut Cur) -> Result<String, BencodeError> {
        let bin = self.read_bin(c)?;
        String::from_utf8(bin).map_err(|_| BencodeError::InvalidUtf8)
    }

    fn read_arr(&self, c: &mut Cur) -> Result<PackValue, BencodeError> {
        if c.u8() != b'l' {
            return Err(BencodeError::InvalidByte(c.pos - 1));
        }
        let mut arr = Vec::new();
        while c.peek() != b'e' {
            arr.push(self.read_any(c)?);
        }
        c.pos += 1; // consume 'e'
        Ok(PackValue::Array(arr))
    }

    fn read_obj(&self, c: &mut Cur) -> Result<PackValue, BencodeError> {
        if c.u8() != b'd' {
            return Err(BencodeError::InvalidByte(c.pos - 1));
        }
        let mut obj = Vec::new();
        while c.peek() != b'e' {
            let key = self.read_str(c)?;
            if key == "__proto__" {
                return Err(BencodeError::InvalidKey);
            }
            let val = self.read_any(c)?;
            obj.push((key, val));
        }
        c.pos += 1; // consume 'e'
        Ok(PackValue::Object(obj))
    }
}
