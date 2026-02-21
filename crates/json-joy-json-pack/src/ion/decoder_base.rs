//! Ion binary decoder base implementation.
//!
//! Upstream reference: `json-pack/src/ion/IonDecoderBase.ts`

use super::constants::{Type, ION_BVM};
use super::symbols::IonSymbols;
use crate::PackValue;

/// Ion decoding error.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IonDecodeError {
    #[error("unexpected end of input")]
    EndOfInput,
    #[error("invalid Ion Binary Version Marker")]
    InvalidBvm,
    #[error("unknown symbol ID: {0}")]
    UnknownSymbol(u32),
    #[error("invalid UTF-8")]
    InvalidUtf8,
    #[error("unsupported float length: {0}")]
    UnsupportedFloatLen(u8),
    #[error("negative zero integer is illegal")]
    NegativeZero,
    #[error("invalid bool length: {0}")]
    InvalidBoolLen(u8),
    #[error("unknown Ion type: 0x{0:01x}")]
    UnknownType(u8),
    #[error("annotation wrapper must have at least 3 bytes")]
    AnnotationTooShort(u8),
    #[error("list parsing error: incorrect length")]
    ListLengthMismatch,
    #[error("struct parsing error: incorrect length")]
    StructLengthMismatch,
}

/// Base decoder shared by Ion decoder wrappers.
pub struct IonDecoderBase {
    data: Vec<u8>,
    pos: usize,
    symbols: IonSymbols,
}

impl Default for IonDecoderBase {
    fn default() -> Self {
        Self::new()
    }
}

impl IonDecoderBase {
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            pos: 0,
            symbols: IonSymbols::new(),
        }
    }

    pub(crate) fn reset(&mut self, data: &[u8]) {
        self.data.clear();
        self.data.extend_from_slice(data);
        self.pos = 0;
        self.symbols = IonSymbols::new();
    }

    pub(crate) fn validate_bvm(&mut self) -> Result<(), IonDecodeError> {
        if self.pos + 4 > self.data.len() {
            return Err(IonDecodeError::EndOfInput);
        }
        let marker = &self.data[self.pos..self.pos + 4];
        if marker != ION_BVM {
            return Err(IonDecodeError::InvalidBvm);
        }
        self.pos += 4;
        Ok(())
    }

    pub(crate) fn has_remaining(&self) -> bool {
        self.pos < self.data.len()
    }

    pub(crate) fn peek_type_id(&self) -> Result<u8, IonDecodeError> {
        let descriptor = self.peek_byte()?;
        Ok((descriptor >> 4) & 0x0f)
    }

    pub(crate) fn symbols_mut(&mut self) -> &mut IonSymbols {
        &mut self.symbols
    }

    pub fn val(&mut self) -> Result<PackValue, IonDecodeError> {
        let typedesc = self.read_byte()?;
        let type_id = (typedesc >> 4) & 0x0f;
        let length = typedesc & 0x0f;

        match type_id {
            t if t == Type::NULL => self.read_null(length),
            t if t == Type::BOOL => self.read_bool(length),
            t if t == Type::UINT => self.read_uint(length),
            t if t == Type::NINT => self.read_nint(length),
            t if t == Type::FLOT => self.read_float(length),
            t if t == Type::STRI => self.read_string(length),
            t if t == Type::BINA => self.read_binary(length),
            t if t == Type::LIST => self.read_list(length),
            t if t == Type::STRU => self.read_struct(length),
            t if t == Type::ANNO => self.read_annotation(length),
            _ => Err(IonDecodeError::UnknownType(type_id)),
        }
    }

    fn read_null(&mut self, length: u8) -> Result<PackValue, IonDecodeError> {
        if length == 15 {
            return Ok(PackValue::Null);
        }
        let pad_len = if length == 14 {
            self.read_vuint()? as usize
        } else {
            length as usize
        };
        self.skip(pad_len)?;
        let _ = self.val()?;
        Ok(PackValue::Null)
    }

    fn read_bool(&mut self, length: u8) -> Result<PackValue, IonDecodeError> {
        match length {
            15 => Ok(PackValue::Null),
            0 => Ok(PackValue::Bool(false)),
            1 => Ok(PackValue::Bool(true)),
            _ => Err(IonDecodeError::InvalidBoolLen(length)),
        }
    }

    fn read_uint(&mut self, length: u8) -> Result<PackValue, IonDecodeError> {
        if length == 15 {
            return Ok(PackValue::Null);
        }
        if length == 0 {
            return Ok(PackValue::UInteger(0));
        }

        let bytes = self.read_bytes(length as usize)?;
        let mut value: u64 = 0;
        for b in bytes {
            value = (value << 8) | b as u64;
        }
        Ok(PackValue::UInteger(value))
    }

    fn read_nint(&mut self, length: u8) -> Result<PackValue, IonDecodeError> {
        if length == 15 {
            return Ok(PackValue::Null);
        }
        if length == 0 {
            return Err(IonDecodeError::NegativeZero);
        }

        let bytes = self.read_bytes(length as usize)?;
        let mut value: i64 = 0;
        for b in bytes {
            value = (value << 8) | b as i64;
        }
        Ok(PackValue::Integer(-value))
    }

    fn read_float(&mut self, length: u8) -> Result<PackValue, IonDecodeError> {
        if length == 15 {
            return Ok(PackValue::Null);
        }
        if length == 0 {
            return Ok(PackValue::Float(0.0));
        }

        match length {
            4 => {
                let bytes: [u8; 4] = self
                    .read_bytes(4)?
                    .try_into()
                    .map_err(|_| IonDecodeError::EndOfInput)?;
                Ok(PackValue::Float(f32::from_le_bytes(bytes) as f64))
            }
            8 => {
                let bytes: [u8; 8] = self
                    .read_bytes(8)?
                    .try_into()
                    .map_err(|_| IonDecodeError::EndOfInput)?;
                Ok(PackValue::Float(f64::from_le_bytes(bytes)))
            }
            _ => Err(IonDecodeError::UnsupportedFloatLen(length)),
        }
    }

    fn read_string(&mut self, length: u8) -> Result<PackValue, IonDecodeError> {
        if length == 15 {
            return Ok(PackValue::Null);
        }

        let actual_length = if length == 14 {
            self.read_vuint()? as usize
        } else {
            length as usize
        };

        let bytes = self.read_bytes(actual_length)?;
        let text = String::from_utf8(bytes).map_err(|_| IonDecodeError::InvalidUtf8)?;
        Ok(PackValue::Str(text))
    }

    fn read_binary(&mut self, length: u8) -> Result<PackValue, IonDecodeError> {
        if length == 15 {
            return Ok(PackValue::Null);
        }

        let actual_length = if length == 14 {
            self.read_vuint()? as usize
        } else {
            length as usize
        };

        Ok(PackValue::Bytes(self.read_bytes(actual_length)?))
    }

    fn read_list(&mut self, length: u8) -> Result<PackValue, IonDecodeError> {
        if length == 15 {
            return Ok(PackValue::Null);
        }

        let actual_length = if length == 14 {
            self.read_vuint()? as usize
        } else {
            length as usize
        };
        let end_pos = self
            .pos
            .checked_add(actual_length)
            .ok_or(IonDecodeError::EndOfInput)?;

        if end_pos > self.data.len() {
            return Err(IonDecodeError::EndOfInput);
        }

        let mut list = Vec::new();
        while self.pos < end_pos {
            list.push(self.val()?);
        }

        if self.pos != end_pos {
            return Err(IonDecodeError::ListLengthMismatch);
        }

        Ok(PackValue::Array(list))
    }

    fn read_struct(&mut self, length: u8) -> Result<PackValue, IonDecodeError> {
        if length == 15 {
            return Ok(PackValue::Null);
        }

        let actual_length = if length == 14 {
            self.read_vuint()? as usize
        } else {
            length as usize
        };
        let end_pos = self
            .pos
            .checked_add(actual_length)
            .ok_or(IonDecodeError::EndOfInput)?;

        if end_pos > self.data.len() {
            return Err(IonDecodeError::EndOfInput);
        }

        let mut fields = Vec::new();
        while self.pos < end_pos {
            let field_sid = self.read_vuint()?;
            let field_name = self
                .symbols
                .get_text(field_sid)
                .ok_or(IonDecodeError::UnknownSymbol(field_sid))?
                .to_string();
            let field_value = self.val()?;
            fields.push((field_name, field_value));
        }

        if self.pos != end_pos {
            return Err(IonDecodeError::StructLengthMismatch);
        }

        Ok(PackValue::Object(fields))
    }

    fn read_annotation(&mut self, length: u8) -> Result<PackValue, IonDecodeError> {
        if length < 3 {
            return Err(IonDecodeError::AnnotationTooShort(length));
        }

        let _actual_length = if length == 14 {
            self.read_vuint()? as usize
        } else {
            length as usize
        };

        let annot_length = self.read_vuint()? as usize;
        let end_annot_pos = self
            .pos
            .checked_add(annot_length)
            .ok_or(IonDecodeError::EndOfInput)?;

        while self.pos < end_annot_pos {
            let _ = self.read_vuint()?;
        }

        if self.pos != end_annot_pos {
            return Err(IonDecodeError::EndOfInput);
        }

        self.val()
    }

    /// Reads a VUint (max 5 bytes for a 32-bit value).
    ///
    /// Ion VUint: each byte has 7 data bits; MSB=1 signals this is the last byte.
    fn read_vuint(&mut self) -> Result<u32, IonDecodeError> {
        let mut result: u32 = 0;
        for _ in 0..5 {
            let b = self.read_byte()? as u32;
            result = (result << 7) | (b & 0x7f);
            if b & 0x80 != 0 {
                return Ok(result);
            }
        }
        Err(IonDecodeError::EndOfInput)
    }

    fn read_byte(&mut self) -> Result<u8, IonDecodeError> {
        if self.pos >= self.data.len() {
            return Err(IonDecodeError::EndOfInput);
        }
        let b = self.data[self.pos];
        self.pos += 1;
        Ok(b)
    }

    fn peek_byte(&self) -> Result<u8, IonDecodeError> {
        if self.pos >= self.data.len() {
            return Err(IonDecodeError::EndOfInput);
        }
        Ok(self.data[self.pos])
    }

    fn read_bytes(&mut self, n: usize) -> Result<Vec<u8>, IonDecodeError> {
        if self.pos + n > self.data.len() {
            return Err(IonDecodeError::EndOfInput);
        }
        let bytes = self.data[self.pos..self.pos + n].to_vec();
        self.pos += n;
        Ok(bytes)
    }

    fn skip(&mut self, n: usize) -> Result<(), IonDecodeError> {
        if self.pos + n > self.data.len() {
            return Err(IonDecodeError::EndOfInput);
        }
        self.pos += n;
        Ok(())
    }
}
