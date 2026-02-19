//! `JsonDecoderPartial` — fault-tolerant JSON decoder.
//!
//! Direct port of `json/JsonDecoderPartial.ts` from upstream.
//!
//! Parses JSON that may be incomplete or missing closing brackets/braces.
//! Returns the portion of the value that was successfully parsed.
//!
//! Key behavioral parity with upstream:
//! - When a nested structure (array/object) is incomplete, returns the partial structure.
//! - When a child element is completely corrupt/invalid, drops it and returns the parent.
//! - When input ends unexpectedly, returns what was collected so far.

use super::decoder::JsonDecoder;
use super::error::JsonError;
use crate::PackValue;

/// Carries a partially-decoded value up the call stack.
/// `None` means the element was completely invalid (drop it); `Some(v)` means partial but usable.
#[derive(Debug)]
struct FinishError(Option<PackValue>);

pub struct JsonDecoderPartial {
    pub inner: JsonDecoder,
}

impl Default for JsonDecoderPartial {
    fn default() -> Self {
        Self::new()
    }
}

impl JsonDecoderPartial {
    pub fn new() -> Self {
        Self {
            inner: JsonDecoder::new(),
        }
    }

    pub fn decode(&mut self, input: &[u8]) -> Result<PackValue, JsonError> {
        self.inner.data = input.to_vec();
        self.inner.x = 0;
        match self.read_any_partial() {
            Ok(v) => Ok(v),
            Err(FinishError(Some(v))) => Ok(v),
            Err(FinishError(None)) => Err(JsonError::Invalid(self.inner.x)),
        }
    }

    /// Read any value using partial (fault-tolerant) parsing.
    /// Dispatches nested arrays/objects to partial readers instead of the base decoder.
    fn read_any_partial(&mut self) -> Result<PackValue, FinishError> {
        self.inner.skip_whitespace();
        if self.inner.x >= self.inner.data.len() {
            return Err(FinishError(None));
        }
        let ch = self.inner.data[self.inner.x];
        match ch {
            b'[' => self.read_arr().map_err(|_| FinishError(None)),
            b'{' => self.read_obj().map_err(|_| FinishError(None)),
            _ => self.inner.read_any().map_err(|_| FinishError(None)),
        }
    }

    pub fn read_arr(&mut self) -> Result<PackValue, JsonError> {
        if self.inner.x >= self.inner.data.len() || self.inner.data[self.inner.x] != b'[' {
            return Err(JsonError::Invalid(self.inner.x));
        }
        self.inner.x += 1;
        let mut arr: Vec<PackValue> = Vec::new();
        let mut first = true;
        loop {
            self.inner.skip_whitespace();
            if self.inner.x >= self.inner.data.len() {
                // End of input — return what we have
                return Ok(PackValue::Array(arr));
            }
            let ch = self.inner.data[self.inner.x];
            if ch == b']' {
                self.inner.x += 1;
                return Ok(PackValue::Array(arr));
            }
            if ch == b',' {
                self.inner.x += 1;
            } else if !first {
                // Not a comma and not `]` — no valid separator → return what we have
                return Ok(PackValue::Array(arr));
            }
            self.inner.skip_whitespace();
            match self.read_any_partial() {
                Ok(v) => arr.push(v),
                Err(FinishError(Some(v))) => {
                    // Partial nested structure — include it and return
                    arr.push(v);
                    return Ok(PackValue::Array(arr));
                }
                Err(FinishError(None)) => {
                    // Element completely invalid — drop it and return what we have
                    return Ok(PackValue::Array(arr));
                }
            }
            first = false;
        }
    }

    pub fn read_obj(&mut self) -> Result<PackValue, JsonError> {
        if self.inner.x >= self.inner.data.len() || self.inner.data[self.inner.x] != b'{' {
            return Err(JsonError::Invalid(self.inner.x));
        }
        self.inner.x += 1;
        let mut obj: Vec<(String, PackValue)> = Vec::new();
        loop {
            self.inner.skip_whitespace();
            if self.inner.x >= self.inner.data.len() {
                return Ok(PackValue::Object(obj));
            }
            let ch = self.inner.data[self.inner.x];
            if ch == b'}' {
                self.inner.x += 1;
                return Ok(PackValue::Object(obj));
            }
            if ch == b',' {
                self.inner.x += 1;
                continue;
            }
            // Read key
            if ch != b'"' {
                return Ok(PackValue::Object(obj));
            }
            let key = match self.inner.read_key() {
                Ok(k) => k,
                Err(_) => return Ok(PackValue::Object(obj)),
            };
            if key == "__proto__" {
                return Err(JsonError::InvalidKey);
            }
            self.inner.skip_whitespace();
            if self.inner.x >= self.inner.data.len() || self.inner.data[self.inner.x] != b':' {
                // Key with no value — drop this entry and return what we have
                return Ok(PackValue::Object(obj));
            }
            self.inner.x += 1;
            self.inner.skip_whitespace();
            match self.read_any_partial() {
                Ok(v) => obj.push((key, v)),
                Err(FinishError(Some(v))) => {
                    // Partial nested structure — include it and return
                    obj.push((key, v));
                    return Ok(PackValue::Object(obj));
                }
                Err(FinishError(None)) => {
                    // Value completely invalid — drop this key-value and return
                    return Ok(PackValue::Object(obj));
                }
            }
        }
    }
}
