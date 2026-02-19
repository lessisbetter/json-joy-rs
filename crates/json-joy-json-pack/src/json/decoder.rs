//! `JsonDecoder` — JSON decoder that produces `PackValue`.
//!
//! Direct port of `json/JsonDecoder.ts` from upstream.
//!
//! Handles data URI strings (`data:application/octet-stream;base64,...`) as
//! `PackValue::Bytes` and the CBOR-undefined sentinel as `PackValue::Undefined`.

use json_joy_base64::from_base64_bin;

use super::error::JsonError;
use super::util::find_ending_quote;
use crate::PackValue;

// "data:application/octet-stream;base64," — 37 bytes
const BIN_PREFIX: &[u8] = b"data:application/octet-stream;base64,";
// "data:application/cbor,base64;9w==" — 33 bytes (inside the opening quote)
const UNDEF_INNER: &[u8] = b"ata:application/cbor,base64;9w==\"";

pub struct JsonDecoder {
    pub data: Vec<u8>,
    pub x: usize,
}

impl Default for JsonDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl JsonDecoder {
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            x: 0,
        }
    }

    pub fn decode(&mut self, input: &[u8]) -> Result<PackValue, JsonError> {
        self.data = input.to_vec();
        self.x = 0;
        self.read_any()
    }

    pub fn read_any(&mut self) -> Result<PackValue, JsonError> {
        self.skip_whitespace();
        let data = &self.data;
        let x = self.x;
        if x >= data.len() {
            return Err(JsonError::Invalid(x));
        }
        let ch = data[x];
        match ch {
            b'"' => {
                // Could be a binary data URI or undefined sentinel
                if x + 1 < data.len() && data[x + 1] == b'd' {
                    if let Some(bin) = self.try_read_bin()? {
                        return Ok(PackValue::Bytes(bin));
                    }
                    // Check for undefined sentinel
                    if self.starts_with_undef_inner(x + 2) {
                        self.x = x + 35; // skip `"data:application/cbor,base64;9w=="` (35 bytes)
                        return Ok(PackValue::Undefined);
                    }
                }
                Ok(PackValue::Str(self.read_str()?))
            }
            b'[' => self.read_arr(),
            b'f' => self.read_false(),
            b'n' => self.read_null(),
            b't' => self.read_true(),
            b'{' => self.read_obj(),
            c if (c >= b'0' && c <= b'9') || c == b'-' => self.read_num(),
            _ => Err(JsonError::Invalid(x)),
        }
    }

    pub fn skip_whitespace(&mut self) {
        while self.x < self.data.len() {
            match self.data[self.x] {
                b' ' | b'\t' | b'\n' | b'\r' => self.x += 1,
                _ => break,
            }
        }
    }

    pub fn read_null(&mut self) -> Result<PackValue, JsonError> {
        if self.x + 4 > self.data.len() || &self.data[self.x..self.x + 4] != b"null" {
            return Err(JsonError::Invalid(self.x));
        }
        self.x += 4;
        Ok(PackValue::Null)
    }

    pub fn read_true(&mut self) -> Result<PackValue, JsonError> {
        if self.x + 4 > self.data.len() || &self.data[self.x..self.x + 4] != b"true" {
            return Err(JsonError::Invalid(self.x));
        }
        self.x += 4;
        Ok(PackValue::Bool(true))
    }

    pub fn read_false(&mut self) -> Result<PackValue, JsonError> {
        if self.x + 5 > self.data.len() || &self.data[self.x..self.x + 5] != b"false" {
            return Err(JsonError::Invalid(self.x));
        }
        self.x += 5;
        Ok(PackValue::Bool(false))
    }

    pub fn read_num(&mut self) -> Result<PackValue, JsonError> {
        let start = self.x;
        let data = &self.data;
        let len = data.len();
        let mut x = self.x;

        // Consume sign, digits, decimal, exponent
        if x < len && data[x] == b'-' {
            x += 1;
        }
        while x < len && data[x] >= b'0' && data[x] <= b'9' {
            x += 1;
        }
        let mut is_float = false;
        if x < len && data[x] == b'.' {
            is_float = true;
            x += 1;
            while x < len && data[x] >= b'0' && data[x] <= b'9' {
                x += 1;
            }
        }
        if x < len && (data[x] == b'e' || data[x] == b'E') {
            is_float = true;
            x += 1;
            if x < len && (data[x] == b'+' || data[x] == b'-') {
                x += 1;
            }
            while x < len && data[x] >= b'0' && data[x] <= b'9' {
                x += 1;
            }
        }
        self.x = x;

        let s = std::str::from_utf8(&data[start..x]).map_err(|_| JsonError::InvalidUtf8)?;
        if is_float {
            let f: f64 = s.parse().map_err(|_| JsonError::Invalid(start))?;
            Ok(PackValue::Float(f))
        } else if let Ok(i) = s.parse::<i64>() {
            Ok(PackValue::Integer(i))
        } else if let Ok(u) = s.parse::<u64>() {
            Ok(PackValue::UInteger(u))
        } else if let Ok(i) = s.parse::<i128>() {
            Ok(PackValue::BigInt(i))
        } else {
            Err(JsonError::Invalid(start))
        }
    }

    pub fn read_str(&mut self) -> Result<String, JsonError> {
        let data = &self.data;
        if self.x >= data.len() || data[self.x] != b'"' {
            return Err(JsonError::Invalid(self.x));
        }
        self.x += 1; // skip opening quote
        let x0 = self.x;
        let x1 = find_ending_quote(data, x0)?;
        let slice = &data[x0..x1];
        let s = decode_json_string(slice)?;
        self.x = x1 + 1; // skip closing quote
        Ok(s)
    }

    pub fn try_read_bin(&mut self) -> Result<Option<Vec<u8>>, JsonError> {
        let data = &self.data;
        let x = self.x;
        // Expect opening quote at x
        if x >= data.len() || data[x] != b'"' {
            return Ok(None);
        }
        let content_start = x + 1;
        // Check for "data:application/octet-stream;base64," prefix (37 bytes)
        if content_start + BIN_PREFIX.len() > data.len() {
            return Ok(None);
        }
        if &data[content_start..content_start + BIN_PREFIX.len()] != BIN_PREFIX {
            return Ok(None);
        }
        let b64_start = content_start + BIN_PREFIX.len();
        let b64_end = find_ending_quote(data, b64_start)?;
        let bin = from_base64_bin(data, b64_start, b64_end - b64_start)
            .map_err(|_| JsonError::Invalid(b64_start))?;
        self.x = b64_end + 1; // skip closing quote
        Ok(Some(bin))
    }

    pub fn read_arr(&mut self) -> Result<PackValue, JsonError> {
        if self.x >= self.data.len() || self.data[self.x] != b'[' {
            return Err(JsonError::Invalid(self.x));
        }
        self.x += 1;
        let mut arr = Vec::new();
        let mut first = true;
        loop {
            self.skip_whitespace();
            if self.x >= self.data.len() {
                return Err(JsonError::Invalid(self.x));
            }
            let ch = self.data[self.x];
            if ch == b']' {
                self.x += 1;
                return Ok(PackValue::Array(arr));
            }
            if ch == b',' {
                self.x += 1;
            } else if !first {
                return Err(JsonError::Invalid(self.x));
            }
            self.skip_whitespace();
            arr.push(self.read_any()?);
            first = false;
        }
    }

    pub fn read_obj(&mut self) -> Result<PackValue, JsonError> {
        if self.x >= self.data.len() || self.data[self.x] != b'{' {
            return Err(JsonError::Invalid(self.x));
        }
        self.x += 1;
        let mut obj = Vec::new();
        let mut first = true;
        loop {
            self.skip_whitespace();
            if self.x >= self.data.len() {
                return Err(JsonError::Invalid(self.x));
            }
            let ch = self.data[self.x];
            if ch == b'}' {
                self.x += 1;
                return Ok(PackValue::Object(obj));
            }
            if ch == b',' {
                self.x += 1;
            } else if !first {
                return Err(JsonError::Invalid(self.x));
            }
            self.skip_whitespace();
            // Read key
            if self.x >= self.data.len() || self.data[self.x] != b'"' {
                return Err(JsonError::Invalid(self.x));
            }
            let key = self.read_key()?;
            if key == "__proto__" {
                return Err(JsonError::InvalidKey);
            }
            self.skip_whitespace();
            if self.x >= self.data.len() || self.data[self.x] != b':' {
                return Err(JsonError::Invalid(self.x));
            }
            self.x += 1;
            self.skip_whitespace();
            let val = self.read_any()?;
            obj.push((key, val));
            first = false;
        }
    }

    /// Read a quoted JSON key (without outer quotes in result).
    pub fn read_key(&mut self) -> Result<String, JsonError> {
        self.read_str()
    }

    fn starts_with_undef_inner(&self, x: usize) -> bool {
        let data = &self.data;
        if x + UNDEF_INNER.len() > data.len() {
            return false;
        }
        &data[x..x + UNDEF_INNER.len()] == UNDEF_INNER
    }
}

/// Decode a JSON string body (between the quotes) handling escape sequences.
/// Uses serde_json for correctness.
fn decode_json_string(bytes: &[u8]) -> Result<String, JsonError> {
    // Fast path: no backslash
    if !bytes.contains(&b'\\') {
        return std::str::from_utf8(bytes)
            .map(|s| s.to_string())
            .map_err(|_| JsonError::InvalidUtf8);
    }
    // Wrap in quotes and use serde_json for proper unescaping
    let mut quoted = Vec::with_capacity(bytes.len() + 2);
    quoted.push(b'"');
    quoted.extend_from_slice(bytes);
    quoted.push(b'"');
    let s: String = serde_json::from_slice(&quoted)?;
    Ok(s)
}
