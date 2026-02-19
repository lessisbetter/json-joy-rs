//! `JsonEncoderDag` — DAG-JSON encoder (IPLD spec).
//!
//! Direct port of `json/JsonEncoderDag.ts` from upstream.
//!
//! Binary is encoded as `{"/":{"bytes":"<base64-no-padding>"}}`.
//! CIDs are encoded as `{"/":"<cid>"}`.

use json_joy_base64::to_base64_bin;

use super::encoder_stable::JsonEncoderStable;
use crate::PackValue;

// "{"/":{"bytes":""}}" = 18 bytes
const OBJ_BASE_LEN: usize = 18;
// "{"/":""}" = 8 bytes
const CID_BASE_LEN: usize = 8;

pub struct JsonEncoderDag {
    pub inner: JsonEncoderStable,
}

impl Default for JsonEncoderDag {
    fn default() -> Self {
        Self::new()
    }
}

impl JsonEncoderDag {
    pub fn new() -> Self {
        Self {
            inner: JsonEncoderStable::new(),
        }
    }

    pub fn encode(&mut self, value: &PackValue) -> Vec<u8> {
        self.inner.inner.writer.reset();
        self.write_any(value);
        self.inner.inner.writer.flush()
    }

    pub fn write_any(&mut self, value: &PackValue) {
        match value {
            PackValue::Bytes(b) => self.write_bin(b),
            PackValue::Array(arr) => self.write_arr(arr),
            PackValue::Object(obj) => self.write_obj(obj),
            other => self.inner.write_any(other),
        }
    }

    /// Write binary data as DAG-JSON: `{"/":{"bytes":"<base64-no-pad>"}}`
    pub fn write_bin(&mut self, buf: &[u8]) {
        let writer = &mut self.inner.inner.writer;
        let length = buf.len();
        // max base64 size without padding: ceil(len * 4/3)
        let max_b64 = (length * 4 / 3) + 4;
        writer.ensure_capacity(OBJ_BASE_LEN + max_b64);

        // {"/"
        writer.u8(b'{');
        writer.u8(b'"');
        writer.u8(b'/');
        writer.u8(b'"');
        // :{"bytes":"
        writer.u8(b':');
        writer.u8(b'{');
        writer.u8(b'"');
        writer.u8(b'b');
        writer.u8(b'y');
        writer.u8(b't');
        writer.u8(b'e');
        writer.u8(b's');
        writer.u8(b'"');
        writer.u8(b':');
        writer.u8(b'"');
        // base64 without padding — write to a temp buf then append
        let mut tmp = vec![0u8; max_b64 + 4];
        let written = to_base64_bin(buf, 0, length, &mut tmp, 0);
        // Strip padding '=' chars from the end
        let mut end = written;
        while end > 0 && tmp[end - 1] == b'=' {
            end -= 1;
        }
        writer.buf(&tmp[..end]);
        // "}}
        writer.u8(b'"');
        writer.u8(b'}');
        writer.u8(b'}');
    }

    pub fn write_arr(&mut self, arr: &[PackValue]) {
        self.inner.inner.writer.u8(b'[');
        let last = arr.len().saturating_sub(1);
        for (i, item) in arr.iter().enumerate() {
            self.write_any(item);
            if i < last {
                self.inner.inner.writer.u8(b',');
            }
        }
        self.inner.inner.writer.u8(b']');
    }

    pub fn write_obj(&mut self, obj: &[(String, PackValue)]) {
        if obj.is_empty() {
            self.inner.inner.writer.u8(b'{');
            self.inner.inner.writer.u8(b'}');
            return;
        }
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

        self.inner.inner.writer.u8(b'{');
        let last = indices.len() - 1;
        for (i, &idx) in indices.iter().enumerate() {
            let (key, val) = &obj[idx];
            self.inner.inner.write_str(key);
            self.inner.inner.writer.u8(b':');
            self.write_any(val);
            if i < last {
                self.inner.inner.writer.u8(b',');
            }
        }
        self.inner.inner.writer.u8(b'}');
    }

    /// Write a CID as DAG-JSON: `{"/":"<cid>"}`
    pub fn write_cid(&mut self, cid: &str) {
        let writer = &mut self.inner.inner.writer;
        writer.ensure_capacity(CID_BASE_LEN + cid.len());
        // {"/"
        writer.u8(b'{');
        writer.u8(b'"');
        writer.u8(b'/');
        writer.u8(b'"');
        writer.u8(b':');
        writer.u8(b'"');
        writer.ascii(cid);
        // "}
        writer.u8(b'"');
        writer.u8(b'}');
    }
}
