//! SSH 2.0 binary decoder (RFC 4251).
//!
//! Upstream reference: `json-pack/src/ssh/SshDecoder.ts`

use super::error::SshError;
use crate::JsonPackMpint;

/// SSH 2.0 binary decoder.
///
/// Wraps a [`Reader`] and exposes typed read methods for RFC 4251 types.
/// Unlike most decoders, `read_any()` is not meaningful for SSH because the
/// format is schema-driven — use the explicit typed methods instead.
pub struct SshDecoder {
    pub reader: Vec<u8>,
    pub x: usize,
}

impl Default for SshDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl SshDecoder {
    pub fn new() -> Self {
        Self {
            reader: Vec::new(),
            x: 0,
        }
    }

    /// Resets the decoder with a new byte slice to decode from.
    pub fn reset(&mut self, data: &[u8]) {
        self.reader = data.to_vec();
        self.x = 0;
    }

    #[inline]
    fn check(&self, n: usize) -> Result<(), SshError> {
        if self.x + n > self.reader.len() {
            Err(SshError::UnexpectedEof)
        } else {
            Ok(())
        }
    }

    /// Reads an SSH boolean (1 byte; non-zero = true).
    pub fn read_boolean(&mut self) -> Result<bool, SshError> {
        self.check(1)?;
        let val = self.reader[self.x];
        self.x += 1;
        Ok(val != 0)
    }

    /// Reads a single raw byte.
    pub fn read_byte(&mut self) -> Result<u8, SshError> {
        self.check(1)?;
        let val = self.reader[self.x];
        self.x += 1;
        Ok(val)
    }

    /// Reads a big-endian uint32.
    pub fn read_uint32(&mut self) -> Result<u32, SshError> {
        self.check(4)?;
        let val = u32::from_be_bytes([
            self.reader[self.x],
            self.reader[self.x + 1],
            self.reader[self.x + 2],
            self.reader[self.x + 3],
        ]);
        self.x += 4;
        Ok(val)
    }

    /// Reads a big-endian uint64.
    pub fn read_uint64(&mut self) -> Result<u64, SshError> {
        self.check(8)?;
        let val = u64::from_be_bytes([
            self.reader[self.x],
            self.reader[self.x + 1],
            self.reader[self.x + 2],
            self.reader[self.x + 3],
            self.reader[self.x + 4],
            self.reader[self.x + 5],
            self.reader[self.x + 6],
            self.reader[self.x + 7],
        ]);
        self.x += 8;
        Ok(val)
    }

    /// Reads an SSH binary string (uint32 length + raw bytes).
    pub fn read_bin_str(&mut self) -> Result<Vec<u8>, SshError> {
        let length = self.read_uint32()? as usize;
        self.check(length)?;
        let data = self.reader[self.x..self.x + length].to_vec();
        self.x += length;
        Ok(data)
    }

    /// Reads an SSH UTF-8 string (uint32 length + UTF-8 bytes).
    pub fn read_str(&mut self) -> Result<String, SshError> {
        let bytes = self.read_bin_str()?;
        String::from_utf8(bytes).map_err(|_| SshError::InvalidUtf8)
    }

    /// Reads an SSH ASCII string (uint32 length + ASCII bytes).
    pub fn read_ascii_str(&mut self) -> Result<String, SshError> {
        let length = self.read_uint32()? as usize;
        self.check(length)?;
        let s: String = self.reader[self.x..self.x + length]
            .iter()
            .map(|&b| b as char)
            .collect();
        self.x += length;
        Ok(s)
    }

    /// Reads an SSH mpint (uint32 length + two's-complement MSB-first bytes).
    pub fn read_mpint(&mut self) -> Result<JsonPackMpint, SshError> {
        let bytes = self.read_bin_str()?;
        Ok(JsonPackMpint { data: bytes })
    }

    /// Reads an SSH name-list (comma-separated ASCII names).
    pub fn read_name_list(&mut self) -> Result<Vec<String>, SshError> {
        let s = self.read_ascii_str()?;
        if s.is_empty() {
            return Ok(Vec::new());
        }
        Ok(s.split(',').map(|s| s.to_string()).collect())
    }

    /// Reads binary data as an SSH string.
    pub fn read_bin(&mut self) -> Result<Vec<u8>, SshError> {
        self.read_bin_str()
    }
}
