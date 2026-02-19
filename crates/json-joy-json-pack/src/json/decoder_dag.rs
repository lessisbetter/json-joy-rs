//! `JsonDecoderDag` — DAG-JSON decoder (IPLD spec).
//!
//! Direct port of `json/JsonDecoderDag.ts` from upstream.
//!
//! Extends `JsonDecoder` to recognise `{"/":{"bytes":"..."}}` as binary and
//! `{"/":"..."}` as CID strings.

use json_joy_base64::from_base64_bin;

use super::decoder::JsonDecoder;
use super::error::JsonError;
use super::util::find_ending_quote;
use crate::PackValue;

pub struct JsonDecoderDag {
    pub inner: JsonDecoder,
}

impl Default for JsonDecoderDag {
    fn default() -> Self {
        Self::new()
    }
}

impl JsonDecoderDag {
    pub fn new() -> Self {
        Self {
            inner: JsonDecoder::new(),
        }
    }

    pub fn decode(&mut self, input: &[u8]) -> Result<PackValue, JsonError> {
        self.inner.data = input.to_vec();
        self.inner.x = 0;
        self.read_any()
    }

    pub fn read_any(&mut self) -> Result<PackValue, JsonError> {
        self.inner.skip_whitespace();
        let x = self.inner.x;
        let data = &self.inner.data;
        if x >= data.len() {
            return Err(JsonError::Invalid(x));
        }
        let ch = data[x];
        if ch == b'{' {
            // Try DAG-JSON patterns first
            if let Some(v) = self.try_read_bytes()? {
                return Ok(PackValue::Bytes(v));
            }
            if let Some(cid) = self.try_read_cid()? {
                return Ok(PackValue::Str(cid));
            }
            return self.inner.read_obj();
        }
        // Delegate to base decoder for all other types
        self.inner.read_any()
    }

    /// Try to read `{"/":{"bytes":"<b64>"}}`.
    fn try_read_bytes(&mut self) -> Result<Option<Vec<u8>>, JsonError> {
        let saved = self.inner.x;
        macro_rules! backtrack {
            () => {{
                self.inner.x = saved;
                return Ok(None);
            }};
        }

        let data = &self.inner.data;
        if self.inner.x >= data.len() || data[self.inner.x] != b'{' {
            backtrack!()
        }
        self.inner.x += 1;
        self.inner.skip_whitespace();

        // "/"
        if !self.consume_literal(b'"') || !self.consume_literal(b'/') || !self.consume_literal(b'"')
        {
            backtrack!()
        }
        self.inner.skip_whitespace();
        if !self.consume_literal(b':') {
            backtrack!()
        }
        self.inner.skip_whitespace();
        // {"bytes":
        if !self.consume_literal(b'{') {
            backtrack!()
        }
        self.inner.skip_whitespace();
        // "bytes"
        if !self.consume_slice(b"\"bytes\"") {
            backtrack!()
        }
        self.inner.skip_whitespace();
        if !self.consume_literal(b':') {
            backtrack!()
        }
        self.inner.skip_whitespace();
        if !self.consume_literal(b'"') {
            backtrack!()
        }

        let buf_start = self.inner.x;
        let buf_end = match find_ending_quote(&self.inner.data, buf_start) {
            Ok(e) => e,
            Err(_) => backtrack!(),
        };
        self.inner.x = buf_end + 1; // skip closing quote

        self.inner.skip_whitespace();
        if !self.consume_literal(b'}') {
            backtrack!()
        }
        self.inner.skip_whitespace();
        if !self.consume_literal(b'}') {
            backtrack!()
        }

        let bin = from_base64_bin(&self.inner.data, buf_start, buf_end - buf_start)
            .map_err(|_| JsonError::Invalid(buf_start))?;
        Ok(Some(bin))
    }

    /// Try to read `{"/":"<cid>"}` and return the CID string.
    fn try_read_cid(&mut self) -> Result<Option<String>, JsonError> {
        let saved = self.inner.x;
        macro_rules! backtrack {
            () => {{
                self.inner.x = saved;
                return Ok(None);
            }};
        }

        let data = &self.inner.data;
        if self.inner.x >= data.len() || data[self.inner.x] != b'{' {
            backtrack!()
        }
        self.inner.x += 1;
        self.inner.skip_whitespace();

        // "/"
        if !self.consume_literal(b'"') || !self.consume_literal(b'/') || !self.consume_literal(b'"')
        {
            backtrack!()
        }
        self.inner.skip_whitespace();
        if !self.consume_literal(b':') {
            backtrack!()
        }
        self.inner.skip_whitespace();
        if !self.consume_literal(b'"') {
            backtrack!()
        }

        let buf_start = self.inner.x;
        let buf_end = match find_ending_quote(&self.inner.data, buf_start) {
            Ok(e) => e,
            Err(_) => backtrack!(),
        };
        self.inner.x = buf_end + 1; // skip closing quote

        self.inner.skip_whitespace();
        if !self.consume_literal(b'}') {
            backtrack!()
        }

        let cid = std::str::from_utf8(&self.inner.data[buf_start..buf_end])
            .map(|s| s.to_string())
            .map_err(|_| JsonError::InvalidUtf8)?;
        Ok(Some(cid))
    }

    fn consume_literal(&mut self, expected: u8) -> bool {
        if self.inner.x < self.inner.data.len() && self.inner.data[self.inner.x] == expected {
            self.inner.x += 1;
            true
        } else {
            false
        }
    }

    fn consume_slice(&mut self, expected: &[u8]) -> bool {
        let end = self.inner.x + expected.len();
        if end <= self.inner.data.len() && &self.inner.data[self.inner.x..end] == expected {
            self.inner.x = end;
            true
        } else {
            false
        }
    }
}
