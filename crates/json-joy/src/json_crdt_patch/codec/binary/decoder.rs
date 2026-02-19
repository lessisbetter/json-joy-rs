//! Binary codec decoder for JSON CRDT Patches.
//!
//! Mirrors `packages/json-joy/src/json-crdt-patch/codec/binary/Decoder.ts`.

use crate::json_crdt_patch::clock::{interval, ts, ClockVector, ServerClockVector, Ts, Tss};
use crate::json_crdt_patch::enums::{JsonCrdtPatchOpcode, SESSION};
use crate::json_crdt_patch::operations::{ConValue, Op};
use crate::json_crdt_patch::patch::Patch;
use crate::json_crdt_patch::patch_builder::PatchBuilder;
use crate::json_crdt_patch::util::binary::CrdtReader;
use json_joy_json_pack::PackValue;

/// Error type for binary decoding failures.
#[derive(Debug, Clone, PartialEq)]
pub enum DecodeError {
    /// Input was too short.
    UnexpectedEof,
    /// An unknown opcode was encountered.
    UnknownOpcode(u8),
    /// CBOR payload could not be decoded.
    InvalidCbor,
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecodeError::UnexpectedEof => write!(f, "unexpected end of input"),
            DecodeError::UnknownOpcode(op) => write!(f, "unknown opcode: {}", op),
            DecodeError::InvalidCbor => write!(f, "invalid CBOR"),
        }
    }
}

impl std::error::Error for DecodeError {}

/// Binary codec decoder.
pub struct Decoder;

impl Default for Decoder {
    fn default() -> Self {
        Self::new()
    }
}

impl Decoder {
    pub fn new() -> Self {
        Self
    }

    /// Decodes a binary blob into a [`Patch`].
    pub fn decode<'a>(&self, data: &'a [u8]) -> Result<Patch, DecodeError> {
        // Minimum encoding: at least 1 byte for SID + 1 for time + 1 for meta.
        if data.len() < 3 {
            return Err(DecodeError::UnexpectedEof);
        }
        let mut r = CrdtReader::new(data);
        self.read_patch(&mut r)
    }

    fn read_patch<'a>(&self, r: &mut CrdtReader<'a>) -> Result<Patch, DecodeError> {
        let sid = r.vu57();
        let time = r.vu57();

        let is_server_clock = sid == SESSION::SERVER;
        let mut builder = if is_server_clock {
            let cv = ServerClockVector::new(time);
            PatchBuilder::from_server_clock(cv)
        } else {
            let cv = ClockVector::new(sid, time);
            PatchBuilder::from_clock_vector(cv)
        };

        let patch_sid = sid;

        // Decode meta: CBOR value (undefined = None, array = Some(arr[0]))
        let meta_byte = r.u8();
        let meta = if meta_byte == 0xF7 {
            // CBOR undefined
            None
        } else if meta_byte == 0x81 {
            // CBOR array of 1
            Some(read_cbor(r)?)
        } else {
            // Unexpected — treat as no meta
            None
        };
        builder.patch.meta = meta;

        // Decode operations
        let op_count = r.vu57() as usize;
        for _ in 0..op_count {
            self.decode_operation(r, &mut builder, patch_sid)?;
        }

        Ok(builder.flush())
    }

    fn decode_id<'a>(&self, r: &mut CrdtReader<'a>, patch_sid: u64) -> Ts {
        let (is_different, time) = r.b1vu56();
        if is_different == 0 {
            ts(patch_sid, time)
        } else {
            let sid = r.vu57();
            ts(sid, time)
        }
    }

    fn decode_tss<'a>(&self, r: &mut CrdtReader<'a>, patch_sid: u64) -> Tss {
        let id = self.decode_id(r, patch_sid);
        let span = r.vu57();
        interval(id, 0, span)
    }

    fn decode_operation<'a>(
        &self,
        r: &mut CrdtReader<'a>,
        builder: &mut PatchBuilder,
        patch_sid: u64,
    ) -> Result<(), DecodeError> {
        let octet = r.u8();
        let opcode = octet >> 3;
        let inline = (octet & 0b111) as u64;

        match JsonCrdtPatchOpcode::from_u8(opcode) {
            Some(JsonCrdtPatchOpcode::NewCon) => {
                if inline == 0 {
                    let val = read_cbor(r)?;
                    builder.con_val(val);
                } else {
                    let id_ref = self.decode_id(r, patch_sid);
                    builder.con_ref(id_ref);
                }
            }
            Some(JsonCrdtPatchOpcode::NewVal) => {
                builder.val();
            }
            Some(JsonCrdtPatchOpcode::NewObj) => {
                builder.obj();
            }
            Some(JsonCrdtPatchOpcode::NewVec) => {
                builder.vec();
            }
            Some(JsonCrdtPatchOpcode::NewStr) => {
                builder.str_node();
            }
            Some(JsonCrdtPatchOpcode::NewBin) => {
                builder.bin();
            }
            Some(JsonCrdtPatchOpcode::NewArr) => {
                builder.arr();
            }
            Some(JsonCrdtPatchOpcode::InsVal) => {
                let obj = self.decode_id(r, patch_sid);
                let val = self.decode_id(r, patch_sid);
                builder.set_val(obj, val);
            }
            Some(JsonCrdtPatchOpcode::InsObj) => {
                let length = if inline == 0 { r.vu57() } else { inline } as usize;
                let obj = self.decode_id(r, patch_sid);
                let mut tuples = Vec::with_capacity(length);
                for _ in 0..length {
                    let key_str = read_cbor_str(r)?;
                    let val_id = self.decode_id(r, patch_sid);
                    tuples.push((key_str, val_id));
                }
                builder.ins_obj(obj, tuples);
            }
            Some(JsonCrdtPatchOpcode::InsVec) => {
                let length = if inline == 0 { r.vu57() } else { inline } as usize;
                let obj = self.decode_id(r, patch_sid);
                let mut tuples = Vec::with_capacity(length);
                for _ in 0..length {
                    let idx = r.u8();
                    let val_id = self.decode_id(r, patch_sid);
                    tuples.push((idx, val_id));
                }
                builder.ins_vec(obj, tuples);
            }
            Some(JsonCrdtPatchOpcode::InsStr) => {
                let length = if inline == 0 { r.vu57() } else { inline } as usize;
                let obj = self.decode_id(r, patch_sid);
                let after = self.decode_id(r, patch_sid);
                let s = r.utf8(length).to_owned();
                builder.ins_str(obj, after, s);
            }
            Some(JsonCrdtPatchOpcode::InsBin) => {
                let length = if inline == 0 { r.vu57() } else { inline } as usize;
                let obj = self.decode_id(r, patch_sid);
                let after = self.decode_id(r, patch_sid);
                let data = r.buf(length).to_vec();
                builder.ins_bin(obj, after, data);
            }
            Some(JsonCrdtPatchOpcode::InsArr) => {
                let length = if inline == 0 { r.vu57() } else { inline } as usize;
                let obj = self.decode_id(r, patch_sid);
                let after = self.decode_id(r, patch_sid);
                let mut elems = Vec::with_capacity(length);
                for _ in 0..length {
                    elems.push(self.decode_id(r, patch_sid));
                }
                builder.ins_arr(obj, after, elems);
            }
            Some(JsonCrdtPatchOpcode::UpdArr) => {
                let obj = self.decode_id(r, patch_sid);
                let after = self.decode_id(r, patch_sid);
                let val = self.decode_id(r, patch_sid);
                builder.upd_arr(obj, after, val);
            }
            Some(JsonCrdtPatchOpcode::Del) => {
                let length = if inline == 0 { r.vu57() } else { inline } as usize;
                let obj = self.decode_id(r, patch_sid);
                let mut what = Vec::with_capacity(length);
                for _ in 0..length {
                    what.push(self.decode_tss(r, patch_sid));
                }
                builder.del(obj, what);
            }
            Some(JsonCrdtPatchOpcode::Nop) => {
                let length = if inline == 0 { r.vu57() } else { inline };
                builder.nop(length);
            }
            _ => return Err(DecodeError::UnknownOpcode(opcode)),
        }
        Ok(())
    }
}

/// Read a CBOR value from the reader (minimal subset needed for patch decoding).
fn read_cbor<'a>(r: &mut CrdtReader<'a>) -> Result<PackValue, DecodeError> {
    let b = r.u8();
    let major = b >> 5;
    let info = b & 0x1F;
    match major {
        0 => {
            // Unsigned integer
            let n = read_cbor_uint(r, info)?;
            Ok(PackValue::UInteger(n))
        }
        1 => {
            // Negative integer
            let n = read_cbor_uint(r, info)?;
            Ok(PackValue::Integer(-1 - n as i64))
        }
        2 => {
            // Byte string
            let len = read_cbor_uint(r, info)? as usize;
            Ok(PackValue::Bytes(r.buf(len).to_vec()))
        }
        3 => {
            // Text string
            let len = read_cbor_uint(r, info)? as usize;
            Ok(PackValue::Str(r.utf8(len).to_owned()))
        }
        4 => {
            // Array
            let len = read_cbor_uint(r, info)? as usize;
            let mut arr = Vec::with_capacity(len);
            for _ in 0..len {
                arr.push(read_cbor(r)?);
            }
            Ok(PackValue::Array(arr))
        }
        5 => {
            // Map
            let len = read_cbor_uint(r, info)? as usize;
            let mut map = Vec::with_capacity(len);
            for _ in 0..len {
                let key = match read_cbor(r)? {
                    PackValue::Str(s) => s,
                    _ => return Err(DecodeError::InvalidCbor),
                };
                let val = read_cbor(r)?;
                map.push((key, val));
            }
            Ok(PackValue::Object(map))
        }
        7 => {
            // Float / simple
            match info {
                20 => Ok(PackValue::Bool(false)),
                21 => Ok(PackValue::Bool(true)),
                22 => Ok(PackValue::Null),
                23 => Ok(PackValue::Undefined),
                25 => {
                    let bytes = r.buf(2);
                    let bits = u16::from_be_bytes([bytes[0], bytes[1]]);
                    Ok(PackValue::Float(json_joy_buffers::decode_f16(bits) as f64))
                }
                26 => {
                    let bytes = r.buf(4);
                    Ok(PackValue::Float(
                        f32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as f64,
                    ))
                }
                27 => {
                    let bytes = r.buf(8);
                    Ok(PackValue::Float(f64::from_be_bytes([
                        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6],
                        bytes[7],
                    ])))
                }
                _ => Err(DecodeError::InvalidCbor),
            }
        }
        _ => Err(DecodeError::InvalidCbor),
    }
}

fn read_cbor_uint<'a>(r: &mut CrdtReader<'a>, info: u8) -> Result<u64, DecodeError> {
    match info {
        0..=23 => Ok(info as u64),
        24 => Ok(r.u8() as u64),
        25 => {
            let b = r.buf(2);
            Ok(u16::from_be_bytes([b[0], b[1]]) as u64)
        }
        26 => {
            let b = r.buf(4);
            Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as u64)
        }
        27 => {
            let b = r.buf(8);
            Ok(u64::from_be_bytes([
                b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
            ]))
        }
        _ => Err(DecodeError::InvalidCbor),
    }
}

fn read_cbor_str<'a>(r: &mut CrdtReader<'a>) -> Result<String, DecodeError> {
    match read_cbor(r)? {
        PackValue::Str(s) => Ok(s),
        _ => Err(DecodeError::InvalidCbor),
    }
}
