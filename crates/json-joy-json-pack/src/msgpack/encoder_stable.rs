//! `MsgPackEncoderStable` — MessagePack encoder with sorted object keys.
//!
//! Direct port of `msgpack/MsgPackEncoderStable.ts` from upstream.

use super::encoder_fast::MsgPackEncoderFast;
use crate::PackValue;

pub struct MsgPackEncoderStable {
    pub inner: MsgPackEncoderFast,
}

impl Default for MsgPackEncoderStable {
    fn default() -> Self {
        Self::new()
    }
}

impl MsgPackEncoderStable {
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
        match value {
            PackValue::Object(obj) => self.write_obj(obj),
            PackValue::Array(arr) => {
                let length = arr.len();
                self.inner.write_arr_hdr(length);
                for item in arr {
                    self.write_any(item);
                }
            }
            other => self.inner.write_any(other),
        }
    }

    /// Write object with keys sorted lexicographically.
    pub fn write_obj(&mut self, obj: &[(String, PackValue)]) {
        let mut indices: Vec<usize> = (0..obj.len()).collect();
        indices.sort_by(|&a, &b| obj[a].0.cmp(&obj[b].0));

        self.inner.write_obj_hdr(obj.len());
        for idx in indices {
            let (key, val) = &obj[idx];
            self.inner.write_str(key);
            self.write_any(val);
        }
    }
}
