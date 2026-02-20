//! Binary codec encoder for JSON CRDT Patches.
//!
//! Mirrors `packages/json-joy/src/json-crdt-patch/codec/binary/Encoder.ts`.
//!
//! The encoder wraps a [`CrdtWriter`] and also performs inline CBOR
//! encoding (for the meta field and `new_con` values).

use crate::json_crdt_patch::clock::{Ts, Tss};
use crate::json_crdt_patch::enums::OpcodeOverlay;
use crate::json_crdt_patch::operations::{ConValue, Op};
use crate::json_crdt_patch::patch::Patch;
use crate::json_crdt_patch::util::binary::CrdtWriter;
use json_joy_buffers::is_float32;
use json_joy_json_pack::PackValue;

/// Binary codec encoder.
pub struct Encoder {
    pub writer: CrdtWriter,
    /// SID of the patch being encoded; used for compact ID encoding.
    patch_sid: u64,
}

impl Default for Encoder {
    fn default() -> Self {
        Self::new()
    }
}

impl Encoder {
    pub fn new() -> Self {
        Self {
            writer: CrdtWriter::with_alloc_size(4 * 1024),
            patch_sid: 0,
        }
    }

    /// Encodes the patch and returns the binary blob.
    pub fn encode(&mut self, patch: &Patch) -> Vec<u8> {
        self.writer.reset();
        let id = patch.get_id().expect("cannot encode empty patch");
        self.patch_sid = id.sid;
        let w = &mut self.writer;
        w.vu57(id.sid);
        w.vu57(id.time);
        match &patch.meta {
            None => w.u8(0xf7), // CBOR undefined
            Some(val) => {
                // CBOR array of length 1, then the value
                w.u8(0x81);
                Self::write_pack_value(w, val);
            }
        }
        self.encode_operations(patch);
        self.writer.flush()
    }

    fn encode_operations(&mut self, patch: &Patch) {
        let len = patch.ops.len() as u64;
        self.writer.vu57(len);
        // We can't borrow self.writer and self.patch_sid simultaneously via method
        // calls that take &mut self, so we gather what we need and operate.
        for op in &patch.ops {
            self.encode_operation(op);
        }
    }

    fn encode_id(&mut self, id: Ts) {
        let sid = id.sid;
        let time = id.time;
        if sid == self.patch_sid {
            self.writer.b1vu56(0, time);
        } else {
            self.writer.b1vu56(1, time);
            self.writer.vu57(sid);
        }
    }

    fn encode_tss(&mut self, tss: &Tss) {
        self.encode_id(tss.ts());
        self.writer.vu57(tss.span);
    }

    /// Mirrors upstream `Encoder.writeInsStr`:
    /// - writes opcode + logical string length hint
    /// - writes object/ref IDs
    /// - writes UTF-8 payload, returning actual bytes written
    fn write_ins_str(&mut self, length: usize, obj: Ts, after: Ts, data: &str) -> usize {
        if length <= 0b111 {
            self.writer.u8(OpcodeOverlay::INS_STR + length as u8);
        } else {
            self.writer.u8(OpcodeOverlay::INS_STR);
            self.writer.vu57(length as u64);
        }
        self.encode_id(obj);
        self.encode_id(after);
        self.writer.utf8(data)
    }

    fn encode_operation(&mut self, op: &Op) {
        match op {
            Op::NewCon { val, .. } => match val {
                ConValue::Ref(ts_ref) => {
                    self.writer.u8(OpcodeOverlay::NEW_CON + 1);
                    let ts_ref = *ts_ref;
                    self.encode_id(ts_ref);
                }
                ConValue::Val(pack_val) => {
                    self.writer.u8(OpcodeOverlay::NEW_CON);
                    let v = pack_val.clone();
                    Self::write_pack_value(&mut self.writer, &v);
                }
            },
            Op::NewVal { .. } => self.writer.u8(OpcodeOverlay::NEW_VAL),
            Op::NewObj { .. } => self.writer.u8(OpcodeOverlay::NEW_OBJ),
            Op::NewVec { .. } => self.writer.u8(OpcodeOverlay::NEW_VEC),
            Op::NewStr { .. } => self.writer.u8(OpcodeOverlay::NEW_STR),
            Op::NewBin { .. } => self.writer.u8(OpcodeOverlay::NEW_BIN),
            Op::NewArr { .. } => self.writer.u8(OpcodeOverlay::NEW_ARR),
            Op::InsVal { obj, val, .. } => {
                self.writer.u8(OpcodeOverlay::INS_VAL);
                let obj = *obj;
                let val = *val;
                self.encode_id(obj);
                self.encode_id(val);
            }
            Op::InsObj { obj, data, .. } => {
                let length = data.len();
                if length <= 0b111 {
                    self.writer.u8(OpcodeOverlay::INS_OBJ + length as u8);
                } else {
                    self.writer.u8(OpcodeOverlay::INS_OBJ);
                    self.writer.vu57(length as u64);
                }
                let obj = *obj;
                self.encode_id(obj);
                // Encode data as pairs of CBOR string + encoded ID
                let data: Vec<_> = data.clone();
                for (key, val_id) in data {
                    Self::write_cbor_str(&mut self.writer, &key);
                    self.encode_id(val_id);
                }
            }
            Op::InsVec { obj, data, .. } => {
                let length = data.len();
                if length <= 0b111 {
                    self.writer.u8(OpcodeOverlay::INS_VEC + length as u8);
                } else {
                    self.writer.u8(OpcodeOverlay::INS_VEC);
                    self.writer.vu57(length as u64);
                }
                let obj = *obj;
                self.encode_id(obj);
                let data: Vec<_> = data.clone();
                for (idx, val_id) in data {
                    self.writer.u8(idx);
                    self.encode_id(val_id);
                }
            }
            Op::InsStr {
                obj, after, data, ..
            } => {
                let obj = *obj;
                let after = *after;
                let data = data.as_str();
                // Upstream uses JS `string.length` (UTF-16 code units) for len1.
                let len1 = data.encode_utf16().count();
                self.writer.ensure_capacity(24 + len1 * 4);
                let x = self.writer.inner.x;
                let len2 = self.write_ins_str(len1, obj, after, data);
                if len1 != len2 {
                    self.writer.inner.x = x;
                    self.write_ins_str(len2, obj, after, data);
                }
            }
            Op::InsBin {
                obj, after, data, ..
            } => {
                let length = data.len();
                if length <= 0b111 {
                    self.writer.u8(OpcodeOverlay::INS_BIN + length as u8);
                } else {
                    self.writer.u8(OpcodeOverlay::INS_BIN);
                    self.writer.vu57(length as u64);
                }
                let obj = *obj;
                let after = *after;
                let data = data.clone();
                self.encode_id(obj);
                self.encode_id(after);
                self.writer.buf(&data);
            }
            Op::InsArr {
                obj, after, data, ..
            } => {
                let length = data.len();
                if length <= 0b111 {
                    self.writer.u8(OpcodeOverlay::INS_ARR + length as u8);
                } else {
                    self.writer.u8(OpcodeOverlay::INS_ARR);
                    self.writer.vu57(length as u64);
                }
                let obj = *obj;
                let after = *after;
                let data: Vec<_> = data.clone();
                self.encode_id(obj);
                self.encode_id(after);
                for elem in data {
                    self.encode_id(elem);
                }
            }
            Op::UpdArr {
                obj, after, val, ..
            } => {
                self.writer.u8(OpcodeOverlay::UPD_ARR);
                let obj = *obj;
                let after = *after;
                let val = *val;
                self.encode_id(obj);
                self.encode_id(after);
                self.encode_id(val);
            }
            Op::Del { obj, what, .. } => {
                let length = what.len();
                if length <= 0b111 {
                    self.writer.u8(OpcodeOverlay::DEL + length as u8);
                } else {
                    self.writer.u8(OpcodeOverlay::DEL);
                    self.writer.vu57(length as u64);
                }
                let obj = *obj;
                let what: Vec<_> = what.clone();
                self.encode_id(obj);
                for tss in &what {
                    self.encode_tss(tss);
                }
            }
            Op::Nop { len, .. } => {
                let length = *len;
                if length <= 0b111 {
                    self.writer.u8(OpcodeOverlay::NOP + length as u8);
                } else {
                    self.writer.u8(OpcodeOverlay::NOP);
                    self.writer.vu57(length);
                }
            }
        }
    }

    // ── Inline CBOR encoding ───────────────────────────────────────────────

    /// Writes a CBOR text string exactly like upstream CborEncoderFast.writeStr.
    fn write_cbor_str(w: &mut CrdtWriter, s: &str) {
        let logical_len = s.encode_utf16().count();
        let max_size = logical_len * 4;
        w.ensure_capacity(5 + s.len());

        let length_offset: usize;
        if max_size <= 23 {
            length_offset = w.inner.x;
            w.inner.x += 1;
        } else if max_size <= 0xFF {
            w.inner.uint8[w.inner.x] = 0x78;
            w.inner.x += 1;
            length_offset = w.inner.x;
            w.inner.x += 1;
        } else if max_size <= 0xFFFF {
            w.inner.uint8[w.inner.x] = 0x79;
            w.inner.x += 1;
            length_offset = w.inner.x;
            w.inner.x += 2;
        } else {
            w.inner.uint8[w.inner.x] = 0x7a;
            w.inner.x += 1;
            length_offset = w.inner.x;
            w.inner.x += 4;
        }

        let bytes_written = w.utf8(s);
        if max_size <= 23 {
            w.inner.uint8[length_offset] = 0x60 | bytes_written as u8;
        } else if max_size <= 0xFF {
            w.inner.uint8[length_offset] = bytes_written as u8;
        } else if max_size <= 0xFFFF {
            let b = (bytes_written as u16).to_be_bytes();
            w.inner.uint8[length_offset] = b[0];
            w.inner.uint8[length_offset + 1] = b[1];
        } else {
            let b = (bytes_written as u32).to_be_bytes();
            w.inner.uint8[length_offset..length_offset + 4].copy_from_slice(&b);
        }
    }

    /// Writes a [`PackValue`] as CBOR inline into the writer.
    fn write_pack_value(w: &mut CrdtWriter, val: &PackValue) {
        match val {
            PackValue::Null => w.u8(0xF6),
            PackValue::Undefined => w.u8(0xF7),
            PackValue::Bool(b) => w.u8(if *b { 0xF5 } else { 0xF4 }),
            PackValue::Integer(i) => Self::write_int(w, *i),
            PackValue::UInteger(u) => Self::write_uint(w, *u),
            PackValue::Float(f) => {
                if is_float32(*f) {
                    w.u8(0xFA);
                    w.buf(&(*f as f32).to_be_bytes());
                } else {
                    w.u8(0xFB);
                    w.buf(&f.to_be_bytes());
                }
            }
            PackValue::BigInt(i) => {
                if *i >= 0 && (*i as u128) <= u64::MAX as u128 {
                    Self::write_uint(w, *i as u64);
                } else if *i >= i64::MIN as i128 {
                    Self::write_int(w, *i as i64);
                } else {
                    w.u8(0xFB);
                    w.buf(&(*i as f64).to_be_bytes()); // fallback
                }
            }
            PackValue::Str(s) => Self::write_cbor_str(w, s),
            PackValue::Bytes(b) => {
                let len = b.len();
                if len <= 23 {
                    w.u8(0x40 | len as u8);
                } else if len <= 0xFF {
                    w.u8(0x58);
                    w.u8(len as u8);
                } else if len <= 0xFFFF {
                    w.u8(0x59);
                    w.buf(&(len as u16).to_be_bytes());
                } else {
                    w.u8(0x5A);
                    w.buf(&(len as u32).to_be_bytes());
                }
                w.buf(b);
            }
            PackValue::Array(arr) => {
                let len = arr.len();
                if len <= 23 {
                    w.u8(0x80 | len as u8);
                } else if len <= 0xFF {
                    w.u8(0x98);
                    w.u8(len as u8);
                } else {
                    w.u8(0x99);
                    w.buf(&(len as u16).to_be_bytes());
                }
                for item in arr {
                    Self::write_pack_value(w, item);
                }
            }
            PackValue::Object(obj) => {
                let len = obj.len();
                if len <= 23 {
                    w.u8(0xA0 | len as u8);
                } else if len <= 0xFF {
                    w.u8(0xB8);
                    w.u8(len as u8);
                } else {
                    w.u8(0xB9);
                    w.buf(&(len as u16).to_be_bytes());
                }
                for (k, v) in obj {
                    Self::write_cbor_str(w, k);
                    Self::write_pack_value(w, v);
                }
            }
            PackValue::Blob(b) => w.buf(&b.val),
            PackValue::Extension(ext) => {
                // CBOR tag
                let tag = ext.tag;
                if tag <= 23 {
                    w.u8(0xC0 | tag as u8);
                } else if tag <= 0xFF {
                    w.u8(0xD8);
                    w.u8(tag as u8);
                } else {
                    w.u8(0xD9);
                    w.buf(&(tag as u16).to_be_bytes());
                }
                Self::write_pack_value(w, &ext.val);
            }
        }
    }

    fn write_uint(w: &mut CrdtWriter, u: u64) {
        if u <= 23 {
            w.u8(u as u8);
        } else if u <= 0xFF {
            w.u8(0x18);
            w.u8(u as u8);
        } else if u <= 0xFFFF {
            w.u8(0x19);
            w.buf(&(u as u16).to_be_bytes());
        } else if u <= 0xFFFF_FFFF {
            w.u8(0x1A);
            w.buf(&(u as u32).to_be_bytes());
        } else {
            w.u8(0x1B);
            w.buf(&u.to_be_bytes());
        }
    }

    fn write_int(w: &mut CrdtWriter, i: i64) {
        if i >= 0 {
            Self::write_uint(w, i as u64);
        } else {
            let u = ((-1i64).wrapping_sub(i)) as u64;
            if u < 24 {
                w.u8(0x20 | u as u8);
            } else if u <= 0xFF {
                w.u8(0x38);
                w.u8(u as u8);
            } else if u <= 0xFFFF {
                w.u8(0x39);
                w.buf(&(u as u16).to_be_bytes());
            } else if u <= 0xFFFF_FFFF {
                w.u8(0x3A);
                w.buf(&(u as u32).to_be_bytes());
            } else {
                w.u8(0x3B);
                w.buf(&u.to_be_bytes());
            }
        }
    }
}
