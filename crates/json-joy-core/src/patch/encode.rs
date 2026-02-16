// Native patch binary encoding shared by patch builder and runtime callers.
//
// Upstream reference:
// - json-crdt-patch codec/binary encoder paths in json-joy@17.67.0.
// - String headers intentionally follow json-pack writeStr behavior
//   (`0x78/0x79/0x7a` selected by max UTF-8 size, not shortest canonical).

use crate::patch_builder::PatchBuildError;
use crate::{crdt_binary::write_b1vu56, crdt_binary::write_vu57};
use json_joy_json_pack::{write_cbor_text_like_json_pack, write_json_like_json_pack};

pub fn encode_patch_from_ops(
    sid: u64,
    time: u64,
    ops: &[DecodedOp],
) -> Result<Vec<u8>, PatchBuildError> {
    validate_canonical_ids(sid, time, ops)?;

    let mut w = Writer {
        bytes: Vec::with_capacity(64),
        patch_sid: sid,
    };

    w.vu57(sid);
    w.vu57(time);

    // metadata: CBOR undefined, matching upstream default PatchBuilder output.
    w.bytes.push(0xf7);

    w.vu57(ops.len() as u64);
    for op in ops {
        w.encode_op(op)?;
    }
    Ok(w.bytes)
}

fn validate_canonical_ids(sid: u64, time: u64, ops: &[DecodedOp]) -> Result<(), PatchBuildError> {
    let mut op_time = time;
    for (index, op) in ops.iter().enumerate() {
        let id = op.id();
        if id.sid != sid || id.time != op_time {
            return Err(PatchBuildError::NonCanonicalId {
                index,
                expected_sid: sid,
                expected_time: op_time,
                actual_sid: id.sid,
                actual_time: id.time,
            });
        }
        op_time = op_time.saturating_add(op.span());
    }
    Ok(())
}

struct Writer {
    bytes: Vec<u8>,
    patch_sid: u64,
}

impl Writer {
    fn encode_op(&mut self, op: &DecodedOp) -> Result<(), PatchBuildError> {
        match op {
            DecodedOp::NewCon { value, .. } => match value {
                ConValue::Json(json) => {
                    self.bytes.push(0 << 3);
                    self.push_json_like_json_pack(json);
                }
                ConValue::Ref(ts) => {
                    self.write_op_len(0, 1);
                    self.encode_id(*ts);
                }
                ConValue::Undef => {
                    self.bytes.push(0 << 3);
                    self.bytes.push(0xf7);
                }
            },
            DecodedOp::NewVal { .. } => self.bytes.push(1 << 3),
            DecodedOp::NewObj { .. } => self.bytes.push(2 << 3),
            DecodedOp::NewVec { .. } => self.bytes.push(3 << 3),
            DecodedOp::NewStr { .. } => self.bytes.push(4 << 3),
            DecodedOp::NewBin { .. } => self.bytes.push(5 << 3),
            DecodedOp::NewArr { .. } => self.bytes.push(6 << 3),
            DecodedOp::InsVal { obj, val, .. } => {
                self.bytes.push(9 << 3);
                self.encode_id(*obj);
                self.encode_id(*val);
            }
            DecodedOp::InsObj { obj, data, .. } => {
                self.write_op_len(10, data.len() as u64);
                self.encode_id(*obj);
                for (k, id) in data {
                    self.push_cbor_text_like_json_pack(k);
                    self.encode_id(*id);
                }
            }
            DecodedOp::InsVec { obj, data, .. } => {
                self.write_op_len(11, data.len() as u64);
                self.encode_id(*obj);
                for (idx, id) in data {
                    let idx = u8::try_from(*idx).map_err(|_| PatchBuildError::VecIndexOutOfRange)?;
                    self.bytes.push(idx);
                    self.encode_id(*id);
                }
            }
            DecodedOp::InsStr {
                obj,
                reference,
                data,
                ..
            } => {
                self.write_op_len(12, data.len() as u64);
                self.encode_id(*obj);
                self.encode_id(*reference);
                self.bytes.extend_from_slice(data.as_bytes());
            }
            DecodedOp::InsBin {
                obj,
                reference,
                data,
                ..
            } => {
                self.write_op_len(13, data.len() as u64);
                self.encode_id(*obj);
                self.encode_id(*reference);
                self.bytes.extend_from_slice(data);
            }
            DecodedOp::InsArr {
                obj,
                reference,
                data,
                ..
            } => {
                self.write_op_len(14, data.len() as u64);
                self.encode_id(*obj);
                self.encode_id(*reference);
                for id in data {
                    self.encode_id(*id);
                }
            }
            DecodedOp::UpdArr {
                obj,
                reference,
                val,
                ..
            } => {
                self.bytes.push(15 << 3);
                self.encode_id(*obj);
                self.encode_id(*reference);
                self.encode_id(*val);
            }
            DecodedOp::Del { obj, what, .. } => {
                self.write_op_len(16, what.len() as u64);
                self.encode_id(*obj);
                for span in what {
                    self.encode_id(Timestamp {
                        sid: span.sid,
                        time: span.time,
                    });
                    self.vu57(span.span);
                }
            }
            DecodedOp::Nop { len, .. } => self.write_op_len(17, *len),
        }
        Ok(())
    }

    fn encode_id(&mut self, ts: Timestamp) {
        if ts.sid == self.patch_sid {
            self.b1vu56(0, ts.time);
        } else {
            self.b1vu56(1, ts.time);
            self.vu57(ts.sid);
        }
    }

    fn write_op_len(&mut self, opcode: u8, len: u64) {
        if len <= 0b111 {
            self.bytes.push((opcode << 3) | (len as u8));
        } else {
            self.bytes.push(opcode << 3);
            self.vu57(len);
        }
    }

    fn push_cbor_text_like_json_pack(&mut self, value: &str) {
        write_cbor_text_like_json_pack(&mut self.bytes, value);
    }

    fn push_json_like_json_pack(&mut self, value: &serde_json::Value) {
        write_json_like_json_pack(&mut self.bytes, value)
            .expect("json-pack CBOR encode must succeed for serde_json::Value");
    }

    fn vu57(&mut self, value: u64) {
        write_vu57(&mut self.bytes, value);
    }

    fn b1vu56(&mut self, flag: u8, value: u64) {
        write_b1vu56(&mut self.bytes, flag, value);
    }
}
