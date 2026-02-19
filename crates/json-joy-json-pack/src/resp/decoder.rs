//! RESP3 decoder.
//!
//! Upstream reference: `json-pack/src/resp/RespDecoder.ts`

use super::constants::{Resp, RESP_EXTENSION_ATTRIBUTES, RESP_EXTENSION_PUSH};
use crate::{JsonPackExtension, PackValue};

/// Decode error for RESP3 parsing.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RespDecodeError {
    #[error("unexpected end of input")]
    EndOfInput,
    #[error("unknown RESP type byte: 0x{0:02x}")]
    UnknownType(u8),
    #[error("invalid command frame")]
    InvalidCommand,
    #[error("invalid UTF-8 in RESP payload")]
    InvalidUtf8,
}

/// RESP3 protocol decoder.
///
/// Decodes RESP3 wire format into [`PackValue`].
///
/// Extension mappings:
/// - Push (`>`) → `PackValue::Extension(tag=1, val=Array)`
/// - Attributes (`|`) → `PackValue::Extension(tag=2, val=Object)`
/// - Verbatim string (`=`) with `txt:` encoding → `PackValue::Str`
/// - Verbatim string with other encoding → `PackValue::Bytes`
pub struct RespDecoder {
    data: Vec<u8>,
    pos: usize,
    /// When true, bulk strings (`$`) are decoded as UTF-8 strings if valid.
    pub try_utf8: bool,
}

impl Default for RespDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl RespDecoder {
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            pos: 0,
            try_utf8: false,
        }
    }

    /// Decodes a RESP3 value from `data`.
    pub fn decode(&mut self, data: &[u8]) -> Result<PackValue, RespDecodeError> {
        self.data = data.to_vec();
        self.pos = 0;
        self.read_any()
    }

    // ---------------------------------------------------------------- helpers

    fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }

    fn u8(&mut self) -> Result<u8, RespDecodeError> {
        if self.pos >= self.data.len() {
            return Err(RespDecodeError::EndOfInput);
        }
        let b = self.data[self.pos];
        self.pos += 1;
        Ok(b)
    }

    fn peek(&self) -> Result<u8, RespDecodeError> {
        if self.pos >= self.data.len() {
            return Err(RespDecodeError::EndOfInput);
        }
        Ok(self.data[self.pos])
    }

    fn skip(&mut self, n: usize) -> Result<(), RespDecodeError> {
        if self.pos + n > self.data.len() {
            return Err(RespDecodeError::EndOfInput);
        }
        self.pos += n;
        Ok(())
    }

    fn buf(&mut self, n: usize) -> Result<Vec<u8>, RespDecodeError> {
        if self.pos + n > self.data.len() {
            return Err(RespDecodeError::EndOfInput);
        }
        let bytes = self.data[self.pos..self.pos + n].to_vec();
        self.pos += n;
        Ok(bytes)
    }

    fn u32_be(&mut self) -> Result<u32, RespDecodeError> {
        let bytes = self.buf(4)?;
        Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn utf8(&mut self, n: usize) -> Result<String, RespDecodeError> {
        let bytes = self.buf(n)?;
        String::from_utf8(bytes).map_err(|_| RespDecodeError::InvalidUtf8)
    }

    fn ascii_str(&mut self, n: usize) -> Result<String, RespDecodeError> {
        let bytes = self.buf(n)?;
        Ok(bytes.into_iter().map(|b| b as char).collect())
    }

    /// Reads ASCII decimal digits up to and including `\r\n`. Returns the number.
    fn read_length(&mut self) -> Result<usize, RespDecodeError> {
        let mut n: usize = 0;
        loop {
            let c = self.u8()?;
            if c == Resp::R {
                self.skip(1)?; // skip \n
                return Ok(n);
            }
            n = n * 10 + (c - 48) as usize;
        }
    }

    // ---------------------------------------------------------------- readers

    pub fn read_any(&mut self) -> Result<PackValue, RespDecodeError> {
        let typ = self.u8()?;
        match typ {
            t if t == Resp::INT => self.read_int(),
            t if t == Resp::FLOAT => self.read_float(),
            t if t == Resp::STR_SIMPLE => self.read_str_simple(),
            t if t == Resp::STR_BULK => self.read_str_bulk(),
            t if t == Resp::BOOL => self.read_bool(),
            t if t == Resp::NULL => {
                self.skip(2)?; // \r\n
                Ok(PackValue::Null)
            }
            t if t == Resp::OBJ => self.read_obj(),
            t if t == Resp::ARR => self.read_arr_value(),
            t if t == Resp::STR_VERBATIM => self.read_str_verbatim(),
            t if t == Resp::PUSH => {
                let arr = self.read_arr_inner()?;
                Ok(PackValue::Extension(Box::new(JsonPackExtension::new(
                    RESP_EXTENSION_PUSH,
                    PackValue::Array(arr),
                ))))
            }
            t if t == Resp::BIG => self.read_bigint(),
            t if t == Resp::SET => self.read_set(),
            t if t == Resp::ERR_SIMPLE => self.read_err_simple(),
            t if t == Resp::ERR_BULK => self.read_err_bulk(),
            t if t == Resp::ATTR => {
                let fields = self.read_obj_inner()?;
                Ok(PackValue::Extension(Box::new(JsonPackExtension::new(
                    RESP_EXTENSION_ATTRIBUTES,
                    PackValue::Object(fields),
                ))))
            }
            other => Err(RespDecodeError::UnknownType(other)),
        }
    }

    fn read_bool(&mut self) -> Result<PackValue, RespDecodeError> {
        let c = self.u8()?;
        self.skip(2)?; // \r\n
        Ok(PackValue::Bool(c == b't'))
    }

    fn read_int(&mut self) -> Result<PackValue, RespDecodeError> {
        let mut negative = false;
        let mut c = self.u8()?;
        let mut n: i64 = 0;
        if c == Resp::MINUS {
            negative = true;
        } else if c != Resp::PLUS {
            n = (c - 48) as i64;
        }
        loop {
            c = self.u8()?;
            if c == Resp::R {
                self.skip(1)?; // \n
                return Ok(PackValue::Integer(if negative { -n } else { n }));
            }
            n = n * 10 + (c - 48) as i64;
        }
    }

    fn read_float(&mut self) -> Result<PackValue, RespDecodeError> {
        let start = self.pos;
        loop {
            let c = self.u8()?;
            if c != Resp::R {
                continue;
            }
            let len = self.pos - start - 1;
            let s = self.ascii_str_at(start, len)?;
            self.skip(1)?; // \n
            let f = match s.as_str() {
                "inf" => f64::INFINITY,
                "-inf" => f64::NEG_INFINITY,
                "nan" => f64::NAN,
                other => other
                    .parse::<f64>()
                    .map_err(|_| RespDecodeError::InvalidUtf8)?,
            };
            return Ok(PackValue::Float(f));
        }
    }

    fn ascii_str_at(&self, start: usize, len: usize) -> Result<String, RespDecodeError> {
        if start + len > self.data.len() {
            return Err(RespDecodeError::EndOfInput);
        }
        Ok(self.data[start..start + len]
            .iter()
            .map(|&b| b as char)
            .collect())
    }

    fn read_bigint(&mut self) -> Result<PackValue, RespDecodeError> {
        let start = self.pos;
        loop {
            let c = self.u8()?;
            if c != Resp::R {
                continue;
            }
            let len = self.pos - start - 1;
            let s = self.ascii_str_at(start, len)?;
            self.skip(1)?; // \n
            let n: i128 = s.parse().map_err(|_| RespDecodeError::InvalidUtf8)?;
            return Ok(PackValue::BigInt(n));
        }
    }

    fn read_str_simple(&mut self) -> Result<PackValue, RespDecodeError> {
        let start = self.pos;
        loop {
            let c = self.u8()?;
            if c != Resp::R {
                continue;
            }
            let size = self.pos - start - 1;
            let s = String::from_utf8(self.data[start..start + size].to_vec())
                .map_err(|_| RespDecodeError::InvalidUtf8)?;
            self.skip(1)?; // \n
            return Ok(PackValue::Str(s));
        }
    }

    fn read_str_bulk(&mut self) -> Result<PackValue, RespDecodeError> {
        if self.peek()? == Resp::MINUS {
            self.skip(4)?; // -1\r\n
            return Ok(PackValue::Null);
        }
        let length = self.read_length()?;
        let bytes = self.buf(length)?;
        self.skip(2)?; // \r\n
        if self.try_utf8 {
            if let Ok(s) = String::from_utf8(bytes.clone()) {
                return Ok(PackValue::Str(s));
            }
        }
        Ok(PackValue::Bytes(bytes))
    }

    fn read_str_verbatim(&mut self) -> Result<PackValue, RespDecodeError> {
        let length = self.read_length()?;
        let prefix = self.u32_be()?;
        // "txt:" = 0x7478743a
        const TXT_COLON: u32 = u32::from_be_bytes([b't', b'x', b't', b':']);
        if prefix == TXT_COLON {
            let s = self.utf8(length - 4)?;
            self.skip(2)?; // \r\n
            Ok(PackValue::Str(s))
        } else {
            let bytes = self.buf(length - 4)?;
            self.skip(2)?; // \r\n
            Ok(PackValue::Bytes(bytes))
        }
    }

    fn read_err_simple(&mut self) -> Result<PackValue, RespDecodeError> {
        // Return errors as simple strings (lose error/value distinction)
        self.read_str_simple()
    }

    fn read_err_bulk(&mut self) -> Result<PackValue, RespDecodeError> {
        let length = self.read_length()?;
        let s = self.utf8(length)?;
        self.skip(2)?; // \r\n
        Ok(PackValue::Str(s))
    }

    fn read_arr_inner(&mut self) -> Result<Vec<PackValue>, RespDecodeError> {
        if self.peek()? == Resp::MINUS {
            self.skip(4)?; // -1\r\n
            return Ok(Vec::new());
        }
        let length = self.read_length()?;
        let mut arr = Vec::with_capacity(length);
        for _ in 0..length {
            arr.push(self.read_any()?);
        }
        Ok(arr)
    }

    fn read_arr_value(&mut self) -> Result<PackValue, RespDecodeError> {
        if self.peek()? == Resp::MINUS {
            self.skip(4)?;
            return Ok(PackValue::Null);
        }
        let arr = self.read_arr_inner()?;
        Ok(PackValue::Array(arr))
    }

    fn read_set(&mut self) -> Result<PackValue, RespDecodeError> {
        let length = self.read_length()?;
        let mut arr = Vec::with_capacity(length);
        for _ in 0..length {
            arr.push(self.read_any()?);
        }
        // RESP sets are encoded as arrays (no native Set in PackValue)
        Ok(PackValue::Array(arr))
    }

    fn read_obj_inner(&mut self) -> Result<Vec<(String, PackValue)>, RespDecodeError> {
        let length = self.read_length()?;
        let mut fields = Vec::with_capacity(length);
        for _ in 0..length {
            let key_val = self.read_any()?;
            let key = match key_val {
                PackValue::Str(s) => s,
                other => format!("{:?}", other),
            };
            let value = self.read_any()?;
            fields.push((key, value));
        }
        Ok(fields)
    }

    fn read_obj(&mut self) -> Result<PackValue, RespDecodeError> {
        let fields = self.read_obj_inner()?;
        Ok(PackValue::Object(fields))
    }

    // ---------------------------------------------------------------- skip

    pub fn skip_any(&mut self) -> Result<(), RespDecodeError> {
        let typ = self.u8()?;
        match typ {
            t if t == Resp::INT => self.skip_line(),
            t if t == Resp::FLOAT => self.skip_line(),
            t if t == Resp::STR_SIMPLE => self.skip_line(),
            t if t == Resp::STR_BULK => self.skip_str_bulk(),
            t if t == Resp::BOOL => self.skip(3),
            t if t == Resp::NULL => self.skip(2),
            t if t == Resp::OBJ => self.skip_obj(),
            t if t == Resp::ARR => self.skip_arr(),
            t if t == Resp::STR_VERBATIM => self.skip_str_verbatim(),
            t if t == Resp::PUSH => self.skip_arr(),
            t if t == Resp::BIG => self.skip_line(),
            t if t == Resp::SET => self.skip_set(),
            t if t == Resp::ERR_SIMPLE => self.skip_line(),
            t if t == Resp::ERR_BULK => self.skip_str_bulk(),
            t if t == Resp::ATTR => self.skip_obj(),
            other => Err(RespDecodeError::UnknownType(other)),
        }
    }

    fn skip_line(&mut self) -> Result<(), RespDecodeError> {
        loop {
            if self.u8()? == Resp::R {
                self.skip(1)?;
                return Ok(());
            }
        }
    }

    fn skip_str_bulk(&mut self) -> Result<(), RespDecodeError> {
        if self.peek()? == Resp::MINUS {
            return self.skip(4); // -1\r\n
        }
        let length = self.read_length()?;
        self.skip(length + 2) // content + \r\n
    }

    fn skip_str_verbatim(&mut self) -> Result<(), RespDecodeError> {
        let length = self.read_length()?;
        self.skip(length + 2)
    }

    fn skip_arr(&mut self) -> Result<(), RespDecodeError> {
        if self.peek()? == Resp::MINUS {
            return self.skip(4);
        }
        let length = self.read_length()?;
        for _ in 0..length {
            self.skip_any()?;
        }
        Ok(())
    }

    fn skip_set(&mut self) -> Result<(), RespDecodeError> {
        let length = self.read_length()?;
        for _ in 0..length {
            self.skip_any()?;
        }
        Ok(())
    }

    fn skip_obj(&mut self) -> Result<(), RespDecodeError> {
        let length = self.read_length()?;
        for _ in 0..length {
            self.skip_any()?;
            self.skip_any()?;
        }
        Ok(())
    }
}
