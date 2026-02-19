//! BSON document decoder.
//!
//! Upstream reference: `json-pack/src/bson/BsonDecoder.ts`
//!
//! BSON is a little-endian binary format.

use super::error::BsonError;
use super::values::{
    BsonBinary, BsonDbPointer, BsonDecimal128, BsonJavascriptCode, BsonJavascriptCodeWithScope,
    BsonObjectId, BsonSymbol, BsonTimestamp, BsonValue,
};

/// BSON document decoder.
pub struct BsonDecoder {
    data: Vec<u8>,
    x: usize,
}

impl Default for BsonDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl BsonDecoder {
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            x: 0,
        }
    }

    /// Decodes a BSON document from bytes, returning an error on malformed input.
    pub fn decode(&mut self, data: &[u8]) -> Result<Vec<(String, BsonValue)>, BsonError> {
        self.data = data.to_vec();
        self.x = 0;
        self.read_document()
    }

    #[inline]
    fn check(&self, n: usize) -> Result<(), BsonError> {
        if self.x + n > self.data.len() {
            Err(BsonError::UnexpectedEof)
        } else {
            Ok(())
        }
    }

    fn u8(&mut self) -> Result<u8, BsonError> {
        self.check(1)?;
        let val = self.data[self.x];
        self.x += 1;
        Ok(val)
    }

    fn i32_le(&mut self) -> Result<i32, BsonError> {
        self.check(4)?;
        let val = i32::from_le_bytes([
            self.data[self.x],
            self.data[self.x + 1],
            self.data[self.x + 2],
            self.data[self.x + 3],
        ]);
        self.x += 4;
        Ok(val)
    }

    fn i64_le(&mut self) -> Result<i64, BsonError> {
        self.check(8)?;
        let val = i64::from_le_bytes([
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
        Ok(val)
    }

    fn f64_le(&mut self) -> Result<f64, BsonError> {
        self.check(8)?;
        let val = f64::from_le_bytes([
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
        Ok(val)
    }

    fn buf(&mut self, n: usize) -> Result<Vec<u8>, BsonError> {
        self.check(n)?;
        let data = self.data[self.x..self.x + n].to_vec();
        self.x += n;
        Ok(data)
    }

    fn utf8(&mut self, n: usize) -> Result<String, BsonError> {
        let bytes = self.buf(n)?;
        String::from_utf8(bytes).map_err(|_| BsonError::InvalidUtf8)
    }

    fn read_document(&mut self) -> Result<Vec<(String, BsonValue)>, BsonError> {
        let document_size = self.i32_le()? as usize;
        let start_pos = self.x; // position after the 4-byte size field
                                // Validate the stated document size fits within the buffer
        if start_pos + document_size.saturating_sub(4) > self.data.len() {
            return Err(BsonError::UnexpectedEof);
        }
        let end_pos = start_pos + document_size - 4 - 1; // before terminating null
        let mut fields: Vec<(String, BsonValue)> = Vec::new();

        while self.x < end_pos {
            let element_type = self.u8()?;
            if element_type == 0 {
                break;
            }
            let key = self.read_cstring()?;
            let value = self.read_element_value(element_type)?;
            fields.push((key, value));
        }

        // Skip to end of document (including terminating null)
        if self.x <= end_pos {
            self.x = start_pos + document_size - 4;
        }

        Ok(fields)
    }

    fn read_cstring(&mut self) -> Result<String, BsonError> {
        let start = self.x;
        while self.x < self.data.len() && self.data[self.x] != 0 {
            self.x += 1;
        }
        if self.x >= self.data.len() {
            return Err(BsonError::UnexpectedEof);
        }
        let s = String::from_utf8(self.data[start..self.x].to_vec())
            .map_err(|_| BsonError::InvalidUtf8)?;
        self.x += 1; // skip null terminator
        Ok(s)
    }

    fn read_string(&mut self) -> Result<String, BsonError> {
        let length = self.i32_le()? as usize;
        if length == 0 {
            return Ok(String::new());
        }
        let s = self.utf8(length - 1)?; // -1: length includes null terminator
        self.x += 1; // skip null terminator
        Ok(s)
    }

    fn read_element_value(&mut self, typ: u8) -> Result<BsonValue, BsonError> {
        match typ {
            0x01 => Ok(BsonValue::Float(self.f64_le()?)),
            0x02 => Ok(BsonValue::Str(self.read_string()?)),
            0x03 => Ok(BsonValue::Document(self.read_document()?)),
            0x04 => Ok(BsonValue::Array(self.read_array()?)),
            0x05 => self.read_binary(),
            0x06 => Ok(BsonValue::Undefined),
            0x07 => Ok(BsonValue::ObjectId(self.read_object_id()?)),
            0x08 => Ok(BsonValue::Boolean(self.u8()? == 1)),
            0x09 => Ok(BsonValue::DateTime(self.i64_le()?)),
            0x0a => Ok(BsonValue::Null),
            0x0b => self.read_regex(),
            0x0c => self.read_db_pointer(),
            0x0d => Ok(BsonValue::JavaScriptCode(BsonJavascriptCode {
                code: self.read_string()?,
            })),
            0x0e => Ok(BsonValue::Symbol(BsonSymbol {
                symbol: self.read_string()?,
            })),
            0x0f => self.read_code_with_scope(),
            0x10 => Ok(BsonValue::Int32(self.i32_le()?)),
            0x11 => self.read_timestamp(),
            0x12 => Ok(BsonValue::Int64(self.i64_le()?)),
            0x13 => Ok(BsonValue::Decimal128(BsonDecimal128 {
                data: self.buf(16)?,
            })),
            0xff => Ok(BsonValue::MinKey),
            0x7f => Ok(BsonValue::MaxKey),
            t => Err(BsonError::UnsupportedType(t)),
        }
    }

    fn read_array(&mut self) -> Result<Vec<BsonValue>, BsonError> {
        let fields = self.read_document()?;
        // Sort by numeric key and extract values
        let mut indexed: Vec<(usize, BsonValue)> = fields
            .into_iter()
            .map(|(k, v)| (k.parse::<usize>().unwrap_or(0), v))
            .collect();
        indexed.sort_by_key(|(i, _)| *i);
        Ok(indexed.into_iter().map(|(_, v)| v).collect())
    }

    fn read_binary(&mut self) -> Result<BsonValue, BsonError> {
        let length = self.i32_le()? as usize;
        let subtype = self.u8()?;
        let data = self.buf(length)?;
        Ok(BsonValue::Binary(BsonBinary { subtype, data }))
    }

    fn read_object_id(&mut self) -> Result<BsonObjectId, BsonError> {
        let bytes = self.buf(12)?;
        // Timestamp: 4 bytes big-endian
        let timestamp = ((bytes[0] as u32) << 24)
            | ((bytes[1] as u32) << 16)
            | ((bytes[2] as u32) << 8)
            | (bytes[3] as u32);
        // Process: 5 bytes (4 LE + 1 high)
        let lo32 = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as u64;
        let hi8 = bytes[8] as u64;
        let process = lo32 | (hi8 << 32);
        // Counter: 3 bytes big-endian
        let counter = ((bytes[9] as u32) << 16) | ((bytes[10] as u32) << 8) | (bytes[11] as u32);
        Ok(BsonObjectId {
            timestamp,
            process,
            counter,
        })
    }

    fn read_regex(&mut self) -> Result<BsonValue, BsonError> {
        let pattern = self.read_cstring()?;
        let flags = self.read_cstring()?;
        Ok(BsonValue::Regex(pattern, flags))
    }

    fn read_db_pointer(&mut self) -> Result<BsonValue, BsonError> {
        let name = self.read_string()?;
        let id = self.read_object_id()?;
        Ok(BsonValue::DbPointer(BsonDbPointer { name, id }))
    }

    fn read_code_with_scope(&mut self) -> Result<BsonValue, BsonError> {
        let _total_len = self.i32_le()?; // skip total length
        let code = self.read_string()?;
        let scope = self.read_document()?;
        Ok(BsonValue::JavaScriptCodeWithScope(
            BsonJavascriptCodeWithScope { code, scope },
        ))
    }

    fn read_timestamp(&mut self) -> Result<BsonValue, BsonError> {
        let increment = self.i32_le()?;
        let timestamp = self.i32_le()?;
        Ok(BsonValue::Timestamp(BsonTimestamp {
            increment,
            timestamp,
        }))
    }
}
