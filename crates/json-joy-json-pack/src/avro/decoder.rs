//! Apache Avro decoder (no schema).
//!
//! Upstream reference: `json-pack/src/avro/AvroDecoder.ts`

/// Avro decoding error.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AvroDecodeError {
    #[error("unexpected end of input")]
    EndOfInput,
    #[error("invalid schema")]
    InvalidSchema,
    #[error("variable-length integer is too long")]
    VarIntTooLong,
    #[error("variable-length long is too long")]
    VarLongTooLong,
    #[error("invalid UTF-8")]
    InvalidUtf8,
    #[error("negative array/map count")]
    NegativeCount,
    #[error("invalid key")]
    InvalidKey,
    #[error("invalid enum index: {0}")]
    InvalidEnumIndex(i32),
    #[error("union index out of range")]
    UnionIndexOutOfRange,
}

/// Apache Avro primitive decoder (schema-free).
pub struct AvroDecoder {
    data: Vec<u8>,
    pos: usize,
}

impl Default for AvroDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl AvroDecoder {
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

    fn read_byte(&mut self) -> Result<u8, AvroDecodeError> {
        if self.pos >= self.data.len() {
            return Err(AvroDecodeError::EndOfInput);
        }
        let b = self.data[self.pos];
        self.pos += 1;
        Ok(b)
    }

    fn read_bytes_raw(&mut self, n: usize) -> Result<Vec<u8>, AvroDecodeError> {
        if self.pos + n > self.data.len() {
            return Err(AvroDecodeError::EndOfInput);
        }
        let bytes = self.data[self.pos..self.pos + n].to_vec();
        self.pos += n;
        Ok(bytes)
    }

    // ---------------------------------------------------------------- varint

    /// Reads a variable-length unsigned integer (max 10 bytes for 64-bit long).
    pub fn read_varint_u64(&mut self) -> Result<u64, AvroDecodeError> {
        let mut result: u64 = 0;
        let mut shift = 0u32;
        for _ in 0..10 {
            let b = self.read_byte()? as u64;
            result |= (b & 0x7f) << shift;
            if b & 0x80 == 0 {
                return Ok(result);
            }
            shift += 7;
        }
        Err(AvroDecodeError::VarLongTooLong)
    }

    /// Reads a variable-length unsigned integer (max 5 bytes for 32-bit int/length).
    pub fn read_varint_u32(&mut self) -> Result<u32, AvroDecodeError> {
        let mut result: u32 = 0;
        let mut shift = 0u32;
        for _ in 0..5 {
            let b = self.read_byte()? as u32;
            result |= (b & 0x7f) << shift;
            if b & 0x80 == 0 {
                return Ok(result);
            }
            shift += 7;
        }
        Err(AvroDecodeError::VarIntTooLong)
    }

    /// Reads a zigzag-decoded signed integer (Avro int).
    pub fn read_int(&mut self) -> Result<i32, AvroDecodeError> {
        let encoded = self.read_varint_u32()?;
        Ok(((encoded >> 1) as i32) ^ -((encoded & 1) as i32))
    }

    /// Reads a zigzag-decoded signed long (Avro long).
    pub fn read_long(&mut self) -> Result<i64, AvroDecodeError> {
        let encoded = self.read_varint_u64()?;
        Ok(((encoded >> 1) as i64) ^ -((encoded & 1) as i64))
    }

    // ---------------------------------------------------------------- primitives

    pub fn read_null(&mut self) {}

    pub fn read_boolean(&mut self) -> Result<bool, AvroDecodeError> {
        Ok(self.read_byte()? != 0)
    }

    pub fn read_float(&mut self) -> Result<f32, AvroDecodeError> {
        let bytes = self.read_bytes_raw(4)?;
        Ok(f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    pub fn read_double(&mut self) -> Result<f64, AvroDecodeError> {
        if self.pos + 8 > self.data.len() {
            return Err(AvroDecodeError::EndOfInput);
        }
        let bytes: [u8; 8] = self.data[self.pos..self.pos + 8]
            .try_into()
            .map_err(|_| AvroDecodeError::EndOfInput)?;
        self.pos += 8;
        Ok(f64::from_le_bytes(bytes))
    }

    pub fn read_bytes(&mut self) -> Result<Vec<u8>, AvroDecodeError> {
        let len = self.read_varint_u32()? as usize;
        self.read_bytes_raw(len)
    }

    pub fn read_string(&mut self) -> Result<String, AvroDecodeError> {
        let bytes = self.read_bytes()?;
        String::from_utf8(bytes).map_err(|_| AvroDecodeError::InvalidUtf8)
    }

    #[inline]
    pub fn read_str(&mut self) -> Result<String, AvroDecodeError> {
        self.read_string()
    }

    pub fn read_enum(&mut self) -> Result<i32, AvroDecodeError> {
        self.read_int()
    }

    pub fn read_fixed(&mut self, size: usize) -> Result<Vec<u8>, AvroDecodeError> {
        self.read_bytes_raw(size)
    }

    /// Reads an array block — returns items in a loop until count = 0.
    pub fn read_array<T, F>(&mut self, mut item_reader: F) -> Result<Vec<T>, AvroDecodeError>
    where
        F: FnMut(&mut Self) -> Result<T, AvroDecodeError>,
    {
        let mut result = Vec::new();
        loop {
            let count = self.read_varint_u32()? as usize;
            if count == 0 {
                break;
            }
            for _ in 0..count {
                result.push(item_reader(self)?);
            }
        }
        Ok(result)
    }

    /// Reads a map block — returns key-value pairs in a loop until count = 0.
    pub fn read_map<T, F>(
        &mut self,
        mut value_reader: F,
    ) -> Result<Vec<(String, T)>, AvroDecodeError>
    where
        F: FnMut(&mut Self) -> Result<T, AvroDecodeError>,
    {
        let mut result = Vec::new();
        loop {
            let count = self.read_varint_u32()? as usize;
            if count == 0 {
                break;
            }
            for _ in 0..count {
                let key = self.read_str()?;
                if key == "__proto__" {
                    return Err(AvroDecodeError::InvalidKey);
                }
                let val = value_reader(self)?;
                result.push((key, val));
            }
        }
        Ok(result)
    }

    /// Reads a union index.
    pub fn read_union_index(&mut self) -> Result<usize, AvroDecodeError> {
        let idx = self.read_int()?;
        if idx < 0 {
            return Err(AvroDecodeError::UnionIndexOutOfRange);
        }
        Ok(idx as usize)
    }
}
