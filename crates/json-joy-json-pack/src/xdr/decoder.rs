//! XDR primitive decoder.
//!
//! Upstream reference: `json-pack/src/xdr/XdrDecoder.ts`
//! Reference: RFC 4506 — all integers big-endian, 4-byte alignment.

/// XDR decoding error.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum XdrDecodeError {
    #[error("unexpected end of input")]
    EndOfInput,
    #[error("invalid UTF-8")]
    InvalidUtf8,
    #[error("value exceeds maximum allowed size")]
    MaxSizeExceeded,
    #[error("unknown union discriminant")]
    UnknownDiscriminant,
    #[error("unsupported XDR type: {0}")]
    UnsupportedType(&'static str),
}

/// XDR primitive decoder.
pub struct XdrDecoder {
    data: Vec<u8>,
    pos: usize,
}

impl Default for XdrDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl XdrDecoder {
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            pos: 0,
        }
    }

    pub fn reset(&mut self, data: &[u8]) {
        self.data = data.to_vec();
        self.pos = 0;
    }

    // ---------------------------------------------------------------- helpers

    fn read_u32_raw(&mut self) -> Result<u32, XdrDecodeError> {
        if self.pos + 4 > self.data.len() {
            return Err(XdrDecodeError::EndOfInput);
        }
        let b = &self.data[self.pos..self.pos + 4];
        let val = u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
        self.pos += 4;
        Ok(val)
    }

    fn read_bytes(&mut self, n: usize) -> Result<Vec<u8>, XdrDecodeError> {
        if self.pos + n > self.data.len() {
            return Err(XdrDecodeError::EndOfInput);
        }
        let bytes = self.data[self.pos..self.pos + n].to_vec();
        self.pos += n;
        Ok(bytes)
    }

    fn skip_padding(&mut self, data_len: usize) -> Result<(), XdrDecodeError> {
        let rem = data_len % 4;
        if rem != 0 {
            let pad = 4 - rem;
            if self.pos + pad > self.data.len() {
                return Err(XdrDecodeError::EndOfInput);
            }
            self.pos += pad;
        }
        Ok(())
    }

    // ---------------------------------------------------------------- primitives

    pub fn read_void(&mut self) {}

    pub fn read_boolean(&mut self) -> Result<bool, XdrDecodeError> {
        let n = self.read_u32_raw()?;
        Ok(n != 0)
    }

    pub fn read_int(&mut self) -> Result<i32, XdrDecodeError> {
        let n = self.read_u32_raw()?;
        Ok(n as i32)
    }

    pub fn read_unsigned_int(&mut self) -> Result<u32, XdrDecodeError> {
        self.read_u32_raw()
    }

    pub fn read_hyper(&mut self) -> Result<i64, XdrDecodeError> {
        if self.pos + 8 > self.data.len() {
            return Err(XdrDecodeError::EndOfInput);
        }
        let bytes: [u8; 8] = self.data[self.pos..self.pos + 8].try_into().unwrap();
        self.pos += 8;
        Ok(i64::from_be_bytes(bytes))
    }

    pub fn read_unsigned_hyper(&mut self) -> Result<u64, XdrDecodeError> {
        if self.pos + 8 > self.data.len() {
            return Err(XdrDecodeError::EndOfInput);
        }
        let bytes: [u8; 8] = self.data[self.pos..self.pos + 8].try_into().unwrap();
        self.pos += 8;
        Ok(u64::from_be_bytes(bytes))
    }

    pub fn read_float(&mut self) -> Result<f32, XdrDecodeError> {
        let bits = self.read_u32_raw()?;
        Ok(f32::from_bits(bits))
    }

    pub fn read_double(&mut self) -> Result<f64, XdrDecodeError> {
        if self.pos + 8 > self.data.len() {
            return Err(XdrDecodeError::EndOfInput);
        }
        let bytes: [u8; 8] = self.data[self.pos..self.pos + 8].try_into().unwrap();
        self.pos += 8;
        Ok(f64::from_be_bytes(bytes))
    }

    pub fn read_enum(&mut self) -> Result<i32, XdrDecodeError> {
        self.read_int()
    }

    /// Reads fixed-size opaque data with padding.
    pub fn read_opaque(&mut self, size: usize) -> Result<Vec<u8>, XdrDecodeError> {
        let data = self.read_bytes(size)?;
        self.skip_padding(size)?;
        Ok(data)
    }

    /// Reads variable-length opaque: reads length, then opaque(length).
    pub fn read_varlen_opaque(&mut self) -> Result<Vec<u8>, XdrDecodeError> {
        let len = self.read_u32_raw()? as usize;
        self.read_opaque(len)
    }

    /// Reads a string: [length: u32][utf8 bytes][padding].
    pub fn read_string(&mut self) -> Result<String, XdrDecodeError> {
        let len = self.read_u32_raw()? as usize;
        let bytes = self.read_bytes(len)?;
        self.skip_padding(len)?;
        String::from_utf8(bytes).map_err(|_| XdrDecodeError::InvalidUtf8)
    }

    pub fn read_array<T, F>(&mut self, size: usize, mut reader: F) -> Result<Vec<T>, XdrDecodeError>
    where
        F: FnMut(&mut Self) -> Result<T, XdrDecodeError>,
    {
        let mut arr = Vec::with_capacity(size);
        for _ in 0..size {
            arr.push(reader(self)?);
        }
        Ok(arr)
    }

    pub fn read_varlen_array<T, F>(&mut self, reader: F) -> Result<Vec<T>, XdrDecodeError>
    where
        F: FnMut(&mut Self) -> Result<T, XdrDecodeError>,
    {
        let len = self.read_u32_raw()? as usize;
        self.read_array(len, reader)
    }
}
