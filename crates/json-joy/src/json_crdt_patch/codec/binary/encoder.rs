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
                let byte_len = data.len(); // UTF-8 byte count
                let char_len = data.chars().count();
                let obj = *obj;
                let after = *after;
                let data = data.clone();
                // First pass: write using char_len as the inline length hint
                if char_len <= 0b111 {
                    self.writer.u8(OpcodeOverlay::INS_STR + char_len as u8);
                } else {
                    self.writer.u8(OpcodeOverlay::INS_STR);
                    self.writer.vu57(char_len as u64);
                }
                self.encode_id(obj);
                self.encode_id(after);
                // Write the actual UTF-8 bytes
                // If char_len != byte_len we need to rewrite the header with byte_len
                // (mimics the upstream two-pass approach for multi-byte chars)
                let saved_x = self.writer.inner.x;
                let actual_bytes = self.writer.utf8(&data);
                if char_len != actual_bytes {
                    // Rewind and rewrite from scratch with the correct byte length
                    self.writer.inner.x = saved_x
                        - 1
                        - encode_id_byte_count(obj, self.patch_sid)
                        - encode_id_byte_count(after, self.patch_sid)
                        - if char_len <= 0b111 {
                            1
                        } else {
                            1 + vu57_byte_count(char_len as u64)
                        };
                    if actual_bytes <= 0b111 {
                        self.writer.u8(OpcodeOverlay::INS_STR + actual_bytes as u8);
                    } else {
                        self.writer.u8(OpcodeOverlay::INS_STR);
                        self.writer.vu57(actual_bytes as u64);
                    }
                    self.encode_id(obj);
                    self.encode_id(after);
                    self.writer.utf8(&data);
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

    /// Writes a CBOR text string (major type 3).
    fn write_cbor_str(w: &mut CrdtWriter, s: &str) {
        let len = s.len();
        Self::write_cbor_str_hdr(w, len);
        w.buf(s.as_bytes());
    }

    fn write_cbor_str_hdr(w: &mut CrdtWriter, len: usize) {
        if len <= 23 {
            w.u8(0x60 | len as u8);
        } else if len <= 0xFF {
            w.u8(0x78);
            w.u8(len as u8);
        } else if len <= 0xFFFF {
            w.u8(0x79);
            w.buf(&(len as u16).to_be_bytes());
        } else {
            w.u8(0x7A);
            w.buf(&(len as u32).to_be_bytes());
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
                w.u8(0xFB);
                w.buf(&f.to_be_bytes());
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
                let tag = ext.tag as u64;
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

/// Returns the number of bytes that `b1vu56` would encode for the given `Ts`
/// relative to `patch_sid`. Used for the InsStr rewind logic.
fn encode_id_byte_count(id: Ts, patch_sid: u64) -> usize {
    if id.sid == patch_sid {
        b1vu56_byte_count(id.time)
    } else {
        b1vu56_byte_count(id.time) + vu57_byte_count(id.sid)
    }
}

fn b1vu56_byte_count(n: u64) -> usize {
    if n <= 0x3F {
        1
    } else if n <= 0x1FFF {
        2
    } else if n <= 0xFF_FFFF {
        3
    } else if n <= 0x7FFF_FFFF {
        4
    } else if n <= 0x3F_FFFF_FFFF {
        5
    } else if n <= 0x1FFF_FFFF_FFFF {
        6
    } else if n <= 0xFF_FFFF_FFFF_FFFF {
        7
    } else {
        8
    }
}

fn vu57_byte_count(n: u64) -> usize {
    if n <= 0x7F {
        1
    } else if n <= 0x3FFF {
        2
    } else if n <= 0x1F_FFFF {
        3
    } else if n <= 0xFFF_FFFF {
        4
    } else if n <= 0x7_FFFF_FFFF {
        5
    } else if n <= 0x3FF_FFFF_FFFF {
        6
    } else if n <= 0x1_FFFF_FFFF_FFFF {
        7
    } else {
        8
    }
}
