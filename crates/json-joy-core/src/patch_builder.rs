//! Native patch construction and binary encoding helpers.
//!
//! M6 note:
//! - This is the production counterpart to earlier test-local canonical encoders.
//! - It supports constructing canonical patch bytes from semantic decoded ops so
//!   diff/apply runtime paths can remain fully native.

use ciborium::value::{Integer, Value as CborValue};

use crate::patch::{ConValue, DecodedOp, Patch, PatchError, Timestamp};

#[derive(Debug, thiserror::Error)]
pub enum PatchBuildError {
    #[error("operation id must match patch timeline at index {index}: expected ({expected_sid},{expected_time}) got ({actual_sid},{actual_time})")]
    NonCanonicalId {
        index: usize,
        expected_sid: u64,
        expected_time: u64,
        actual_sid: u64,
        actual_time: u64,
    },
    #[error("ins_vec index must fit in u8")]
    VecIndexOutOfRange,
    #[error("binary patch decode failed after encode: {0}")]
    EncodedPatchDecode(#[from] PatchError),
}

#[derive(Debug, Default)]
pub struct PatchBuilder {
    sid: u64,
    time: u64,
    ops: Vec<DecodedOp>,
}

impl PatchBuilder {
    pub fn new(sid: u64, time: u64) -> Self {
        Self {
            sid,
            time,
            ops: Vec::new(),
        }
    }

    pub fn sid(&self) -> u64 {
        self.sid
    }

    pub fn time(&self) -> u64 {
        self.time
    }

    pub fn ops(&self) -> &[DecodedOp] {
        &self.ops
    }

    pub fn push_op(&mut self, op: DecodedOp) {
        self.ops.push(op);
    }

    pub fn into_bytes(self) -> Result<Vec<u8>, PatchBuildError> {
        encode_patch_from_ops(self.sid, self.time, &self.ops)
    }

    pub fn into_patch(self) -> Result<Patch, PatchBuildError> {
        let bytes = self.into_bytes()?;
        Ok(Patch::from_binary(&bytes)?)
    }
}

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

    // metadata: CBOR undefined, which is the default used by upstream patch
    // builders unless explicit metadata is attached.
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
                    self.push_cbor(&json_to_cbor(json));
                }
                ConValue::Ref(ts) => {
                    self.write_op_len(0, 1);
                    self.encode_id(*ts);
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
                    self.push_cbor(&CborValue::Text(k.clone()));
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
                self.write_op_len(12, data.as_bytes().len() as u64);
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

    fn push_cbor(&mut self, value: &CborValue) {
        ciborium::ser::into_writer(value, &mut self.bytes).expect("CBOR encode must succeed");
    }

    fn vu57(&mut self, mut value: u64) {
        for _ in 0..7 {
            let mut b = (value & 0x7f) as u8;
            value >>= 7;
            if value == 0 {
                self.bytes.push(b);
                return;
            }
            b |= 0x80;
            self.bytes.push(b);
        }
        self.bytes.push((value & 0xff) as u8);
    }

    fn b1vu56(&mut self, flag: u8, mut value: u64) {
        let low6 = (value & 0x3f) as u8;
        value >>= 6;
        let mut first = (flag << 7) | low6;
        if value == 0 {
            self.bytes.push(first);
            return;
        }
        first |= 0x40;
        self.bytes.push(first);

        for _ in 0..6 {
            let mut b = (value & 0x7f) as u8;
            value >>= 7;
            if value == 0 {
                self.bytes.push(b);
                return;
            }
            b |= 0x80;
            self.bytes.push(b);
        }
        self.bytes.push((value & 0xff) as u8);
    }
}

fn json_to_cbor(v: &serde_json::Value) -> CborValue {
    match v {
        serde_json::Value::Null => CborValue::Null,
        serde_json::Value::Bool(b) => CborValue::Bool(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                CborValue::Integer(Integer::from(i))
            } else if let Some(u) = n.as_u64() {
                CborValue::Integer(Integer::from(u))
            } else {
                CborValue::Float(n.as_f64().expect("finite f64"))
            }
        }
        serde_json::Value::String(s) => CborValue::Text(s.clone()),
        serde_json::Value::Array(items) => CborValue::Array(items.iter().map(json_to_cbor).collect()),
        serde_json::Value::Object(map) => {
            let mut out = Vec::with_capacity(map.len());
            for (k, v) in map {
                out.push((CborValue::Text(k.clone()), json_to_cbor(v)));
            }
            CborValue::Map(out)
        }
    }
}
