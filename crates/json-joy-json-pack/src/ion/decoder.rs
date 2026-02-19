//! Ion binary decoder.
//!
//! Upstream reference: `json-pack/src/ion/IonDecoder.ts`, `IonDecoderBase.ts`

use super::constants::{Type, ION_BVM, SID_ION_SYMBOL_TABLE, SID_SYMBOLS};
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
}

/// Ion binary decoder.
pub struct IonDecoder {
    data: Vec<u8>,
    pos: usize,
    symbols: IonSymbols,
}

impl Default for IonDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl IonDecoder {
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            pos: 0,
            symbols: IonSymbols::new(),
        }
    }

    pub fn decode(&mut self, data: &[u8]) -> Result<PackValue, IonDecodeError> {
        self.data = data.to_vec();
        self.pos = 0;
        self.symbols = IonSymbols::new();
        self.validate_bvm()?;
        self.maybe_read_symbol_table()?;
        self.read_val()
    }

    // ---------------------------------------------------------------- helpers

    fn remaining(&self) -> usize {
        self.data.len() - self.pos
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

    fn validate_bvm(&mut self) -> Result<(), IonDecodeError> {
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

    // ---------------------------------------------------------------- type dispatch

    fn read_val(&mut self) -> Result<PackValue, IonDecodeError> {
        let descriptor = self.read_byte()?;
        let type_id = (descriptor >> 4) & 0x0f;
        let length = descriptor & 0x0f;
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
            _ => Err(IonDecodeError::EndOfInput),
        }
    }

    fn read_null(&mut self, length: u8) -> Result<PackValue, IonDecodeError> {
        if length == 15 {
            return Ok(PackValue::Null);
        }
        // NOP padding — skip it and read next value.
        let pad_len = if length == 14 {
            self.read_vuint()? as usize
        } else {
            length as usize
        };
        self.skip(pad_len)?;
        self.read_val()
    }

    fn read_bool(&mut self, length: u8) -> Result<PackValue, IonDecodeError> {
        if length == 15 {
            return Ok(PackValue::Null);
        }
        Ok(PackValue::Bool(length == 1))
    }

    fn read_uint(&mut self, length: u8) -> Result<PackValue, IonDecodeError> {
        if length == 15 {
            return Ok(PackValue::Null);
        }
        if length == 0 {
            return Ok(PackValue::UInteger(0));
        }
        let len = if length == 14 {
            self.read_vuint()? as usize
        } else {
            length as usize
        };
        let bytes = self.read_bytes(len)?;
        let mut n: u64 = 0;
        for &b in &bytes {
            n = (n << 8) | b as u64;
        }
        Ok(PackValue::UInteger(n))
    }

    fn read_nint(&mut self, length: u8) -> Result<PackValue, IonDecodeError> {
        if length == 15 {
            return Ok(PackValue::Null);
        }
        if length == 0 {
            return Err(IonDecodeError::NegativeZero);
        }
        let len = if length == 14 {
            self.read_vuint()? as usize
        } else {
            length as usize
        };
        let bytes = self.read_bytes(len)?;
        let mut n: i64 = 0;
        for &b in &bytes {
            n = (n << 8) | b as i64;
        }
        Ok(PackValue::Integer(-n))
    }

    fn read_float(&mut self, length: u8) -> Result<PackValue, IonDecodeError> {
        if length == 15 {
            return Ok(PackValue::Null);
        }
        if length == 0 {
            return Ok(PackValue::Float(0.0));
        }
        if length == 4 {
            if self.pos + 4 > self.data.len() {
                return Err(IonDecodeError::EndOfInput);
            }
            let bytes: [u8; 4] = self.data[self.pos..self.pos + 4]
                .try_into()
                .map_err(|_| IonDecodeError::EndOfInput)?;
            self.pos += 4;
            // NOTE: upstream uses LE bytes to match the TypeScript buffers library;
            // standard Ion binary spec requires big-endian. We match upstream behavior.
            let f = f32::from_le_bytes(bytes);
            return Ok(PackValue::Float(f as f64));
        }
        if length == 8 {
            if self.pos + 8 > self.data.len() {
                return Err(IonDecodeError::EndOfInput);
            }
            let bytes: [u8; 8] = self.data[self.pos..self.pos + 8]
                .try_into()
                .map_err(|_| IonDecodeError::EndOfInput)?;
            self.pos += 8;
            // NOTE: upstream uses LE bytes; standard Ion binary spec requires big-endian.
            let f = f64::from_le_bytes(bytes);
            return Ok(PackValue::Float(f));
        }
        Err(IonDecodeError::UnsupportedFloatLen(length))
    }

    fn read_string(&mut self, length: u8) -> Result<PackValue, IonDecodeError> {
        if length == 15 {
            return Ok(PackValue::Null);
        }
        let len = if length == 14 {
            self.read_vuint()? as usize
        } else {
            length as usize
        };
        let bytes = self.read_bytes(len)?;
        let s = String::from_utf8(bytes).map_err(|_| IonDecodeError::InvalidUtf8)?;
        Ok(PackValue::Str(s))
    }

    fn read_binary(&mut self, length: u8) -> Result<PackValue, IonDecodeError> {
        if length == 15 {
            return Ok(PackValue::Null);
        }
        let len = if length == 14 {
            self.read_vuint()? as usize
        } else {
            length as usize
        };
        let bytes = self.read_bytes(len)?;
        Ok(PackValue::Bytes(bytes))
    }

    fn read_list(&mut self, length: u8) -> Result<PackValue, IonDecodeError> {
        if length == 15 {
            return Ok(PackValue::Null);
        }
        let content_len = if length == 14 {
            self.read_vuint()? as usize
        } else {
            length as usize
        };
        if content_len == 0 {
            return Ok(PackValue::Array(Vec::new()));
        }
        let end = self.pos + content_len;
        let mut arr = Vec::new();
        while self.pos < end {
            arr.push(self.read_val()?);
        }
        Ok(PackValue::Array(arr))
    }

    fn read_struct(&mut self, length: u8) -> Result<PackValue, IonDecodeError> {
        if length == 15 {
            return Ok(PackValue::Null);
        }
        let content_len = if length == 14 {
            self.read_vuint()? as usize
        } else {
            length as usize
        };
        if content_len == 0 {
            return Ok(PackValue::Object(Vec::new()));
        }
        let end = self.pos + content_len;
        let mut obj = Vec::new();
        while self.pos < end {
            let sid = self.read_vuint()?;
            let text = self
                .symbols
                .get_text(sid)
                .map(|s| s.to_string())
                .ok_or(IonDecodeError::UnknownSymbol(sid))?;
            let val = self.read_val()?;
            obj.push((text, val));
        }
        Ok(PackValue::Object(obj))
    }

    fn read_annotation(&mut self, length: u8) -> Result<PackValue, IonDecodeError> {
        let content_len = if length == 14 {
            self.read_vuint()? as usize
        } else {
            length as usize
        };
        let end = self.pos + content_len;
        // Read annotation sequence length.
        let anno_seq_len = self.read_vuint()? as usize;
        let anno_end = self.pos + anno_seq_len;
        // Read annotation symbol IDs.
        let mut anno_sids = Vec::new();
        while self.pos < anno_end {
            anno_sids.push(self.read_vuint()?);
        }
        // Check if this is a symbol table annotation.
        if anno_sids.first() == Some(&SID_ION_SYMBOL_TABLE) {
            // Try to read as symbol table struct.
            let saved_pos = self.pos;
            if let Ok(sym_table) = self.try_read_symbol_table_struct(end) {
                // Add symbols.
                for sym in sym_table {
                    self.symbols.add(&sym);
                }
                return self.read_val();
            }
            self.pos = saved_pos;
        }
        // Otherwise, read the annotated value and ignore annotations.
        let val = self.read_val()?;
        self.pos = end; // ensure we consumed all annotation content
        Ok(val)
    }

    fn try_read_symbol_table_struct(&mut self, end: usize) -> Result<Vec<String>, IonDecodeError> {
        let descriptor = self.read_byte()?;
        let type_id = (descriptor >> 4) & 0x0f;
        let length = descriptor & 0x0f;
        if type_id != Type::STRU {
            return Err(IonDecodeError::EndOfInput);
        }
        let struct_len = if length == 14 {
            self.read_vuint()? as usize
        } else {
            length as usize
        };
        let struct_end = self.pos + struct_len;
        let mut symbols = Vec::new();
        while self.pos < struct_end {
            let sid = self.read_vuint()?;
            if sid == SID_SYMBOLS {
                // Read the symbols list.
                symbols = self.read_symbols_list()?;
            } else {
                // Skip the value.
                self.read_val()?;
            }
        }
        self.pos = end;
        Ok(symbols)
    }

    fn read_symbols_list(&mut self) -> Result<Vec<String>, IonDecodeError> {
        let descriptor = self.read_byte()?;
        let type_id = (descriptor >> 4) & 0x0f;
        let length = descriptor & 0x0f;
        if type_id != Type::LIST {
            return Err(IonDecodeError::EndOfInput);
        }
        let list_len = if length == 14 {
            self.read_vuint()? as usize
        } else {
            length as usize
        };
        let end = self.pos + list_len;
        let mut syms = Vec::new();
        while self.pos < end {
            if let PackValue::Str(s) = self.read_val()? {
                syms.push(s);
            }
        }
        Ok(syms)
    }

    fn maybe_read_symbol_table(&mut self) -> Result<(), IonDecodeError> {
        // Peek at next byte — if it's an annotation, it might be a symbol table.
        if self.remaining() < 1 {
            return Ok(());
        }
        let next = self.peek_byte()?;
        let type_id = (next >> 4) & 0x0f;
        if type_id == Type::ANNO {
            let saved_pos = self.pos;
            let descriptor = self.read_byte()?;
            let length = descriptor & 0x0f;
            let content_len = if length == 14 {
                self.read_vuint()? as usize
            } else {
                length as usize
            };
            let end = self.pos + content_len;
            let anno_seq_len = self.read_vuint()? as usize;
            let anno_end = self.pos + anno_seq_len;
            let mut anno_sids = Vec::new();
            while self.pos < anno_end {
                anno_sids.push(self.read_vuint()?);
            }
            if anno_sids.first() == Some(&SID_ION_SYMBOL_TABLE) {
                if let Ok(sym_table) = self.try_read_symbol_table_struct(end) {
                    for sym in sym_table {
                        self.symbols.add(&sym);
                    }
                    return Ok(());
                }
            }
            // Not a symbol table — restore position.
            self.pos = saved_pos;
        }
        Ok(())
    }
}
