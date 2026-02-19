//! `JsonEncoderStable` — JSON encoder with deterministic (sorted) key order.
//!
//! Direct port of `json/JsonEncoderStable.ts` from upstream.

use super::encoder::JsonEncoder;
use crate::PackValue;

pub struct JsonEncoderStable {
    pub inner: JsonEncoder,
}

impl Default for JsonEncoderStable {
    fn default() -> Self {
        Self::new()
    }
}

impl JsonEncoderStable {
    pub fn new() -> Self {
        Self {
            inner: JsonEncoder::new(),
        }
    }

    pub fn encode(&mut self, value: &PackValue) -> Vec<u8> {
        self.inner.writer.reset();
        self.write_any(value);
        self.inner.writer.flush()
    }

    pub fn write_any(&mut self, value: &PackValue) {
        match value {
            PackValue::Null => self.inner.write_null(),
            PackValue::Undefined => self.inner.write_undef(),
            PackValue::Bool(b) => self.inner.write_boolean(*b),
            PackValue::Integer(i) => self.inner.write_integer(*i),
            PackValue::UInteger(u) => self.inner.write_u_integer(*u),
            PackValue::Float(f) => self.inner.write_float(*f),
            PackValue::BigInt(i) => self.inner.write_big_int(*i),
            PackValue::Bytes(b) => self.inner.write_bin(b),
            PackValue::Str(s) => self.inner.write_str(s),
            PackValue::Array(arr) => self.write_arr(arr),
            PackValue::Object(obj) => self.write_obj(obj),
            PackValue::Extension(_) | PackValue::Blob(_) => self.inner.write_null(),
        }
    }

    pub fn write_arr(&mut self, arr: &[PackValue]) {
        self.inner.writer.u8(b'[');
        let last = arr.len().saturating_sub(1);
        for (i, item) in arr.iter().enumerate() {
            self.write_any(item);
            if i < last {
                self.inner.writer.u8(b',');
            }
        }
        self.inner.writer.u8(b']');
    }

    /// Write object with keys sorted by length, then lexicographically.
    pub fn write_obj(&mut self, obj: &[(String, PackValue)]) {
        if obj.is_empty() {
            self.inner.writer.u8(b'{');
            self.inner.writer.u8(b'}');
            return;
        }
        // Collect and sort indices by key: length (Unicode scalar count) then lexicographic.
        // Uses chars().count() to mirror upstream JavaScript's string .length (UTF-16 code units)
        // for BMP characters.
        let mut indices: Vec<usize> = (0..obj.len()).collect();
        indices.sort_by(|&a, &b| {
            let ka = &obj[a].0;
            let kb = &obj[b].0;
            let la = ka.chars().count();
            let lb = kb.chars().count();
            if la == lb {
                ka.cmp(kb)
            } else {
                la.cmp(&lb)
            }
        });

        self.inner.writer.u8(b'{');
        let last = indices.len() - 1;
        for (i, &idx) in indices.iter().enumerate() {
            let (key, val) = &obj[idx];
            self.inner.write_str(key);
            self.inner.writer.u8(b':');
            self.write_any(val);
            if i < last {
                self.inner.writer.u8(b',');
            }
        }
        self.inner.writer.u8(b'}');
    }
}
