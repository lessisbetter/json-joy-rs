//! `MsgPackEncoder` — full MessagePack encoder (handles all PackValue types).
//!
//! Direct port of `msgpack/MsgPackEncoder.ts` from upstream.

use super::encoder_fast::MsgPackEncoderFast;
use crate::PackValue;

pub struct MsgPackEncoder {
    pub inner: MsgPackEncoderFast,
}

impl Default for MsgPackEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl MsgPackEncoder {
    pub fn new() -> Self {
        Self {
            inner: MsgPackEncoderFast::new(),
        }
    }

    pub fn encode(&mut self, value: &PackValue) -> Vec<u8> {
        self.inner.writer.reset();
        self.write_any(value);
        self.inner.writer.flush()
    }

    pub fn write_any(&mut self, value: &PackValue) {
        // MsgPackEncoder handles all PackValue variants explicitly
        self.inner.write_any(value);
    }
}
