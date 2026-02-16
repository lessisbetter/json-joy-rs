// Native patch binary encoding shared by patch builder and runtime callers.
//
// Upstream reference:
// - json-crdt-patch codec/binary encoder paths in json-joy@17.67.0.
// - String headers intentionally follow json-pack writeStr behavior
//   (`0x78/0x79/0x7a` selected by max UTF-8 size, not shortest canonical).

use crate::patch_builder::PatchBuildError;

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

    fn push_cbor_text_like_json_pack(&mut self, value: &str) {
        let utf8 = value.as_bytes();
        let bytes_len = utf8.len();
        let max_size = value.chars().count().saturating_mul(4);

        if max_size <= 23 {
            self.bytes.push(0x60u8.saturating_add(bytes_len as u8));
        } else if max_size <= 0xff {
            self.bytes.push(0x78);
            self.bytes.push(bytes_len as u8);
        } else if max_size <= 0xffff {
            self.bytes.push(0x79);
            self.bytes
                .extend_from_slice(&(bytes_len as u16).to_be_bytes());
        } else {
            self.bytes.push(0x7a);
            self.bytes
                .extend_from_slice(&(bytes_len as u32).to_be_bytes());
        }

        self.bytes.extend_from_slice(utf8);
    }

    fn push_json_like_json_pack(&mut self, value: &serde_json::Value) {
        match value {
            serde_json::Value::Null => self.bytes.push(0xf6),
            serde_json::Value::Bool(b) => self.bytes.push(if *b { 0xf5 } else { 0xf4 }),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    self.write_signed(i);
                } else if let Some(u) = n.as_u64() {
                    self.write_unsigned_major(0, u);
                } else {
                    let f = n.as_f64().expect("finite f64");
                    if is_f32_roundtrip(f) {
                        self.bytes.push(0xfa);
                        self.bytes.extend_from_slice(&(f as f32).to_bits().to_be_bytes());
                    } else {
                        self.bytes.push(0xfb);
                        self.bytes.extend_from_slice(&f.to_bits().to_be_bytes());
                    }
                }
            }
            serde_json::Value::String(s) => self.push_cbor_text_like_json_pack(s),
            serde_json::Value::Array(arr) => {
                self.write_unsigned_major(4, arr.len() as u64);
                for item in arr {
                    self.push_json_like_json_pack(item);
                }
            }
            serde_json::Value::Object(map) => {
                self.write_unsigned_major(5, map.len() as u64);
                for (k, v) in map {
                    self.push_cbor_text_like_json_pack(k);
                    self.push_json_like_json_pack(v);
                }
            }
        }
    }

    fn write_signed(&mut self, n: i64) {
        if n >= 0 {
            self.write_unsigned_major(0, n as u64);
        } else {
            let encoded = (-1i128 - n as i128) as u64;
            self.write_unsigned_major(1, encoded);
        }
    }

    fn write_unsigned_major(&mut self, major: u8, n: u64) {
        let major_bits = major << 5;
        if n <= 23 {
            self.bytes.push(major_bits | (n as u8));
        } else if n <= 0xff {
            self.bytes.push(major_bits | 24);
            self.bytes.push(n as u8);
        } else if n <= 0xffff {
            self.bytes.push(major_bits | 25);
            self.bytes.extend_from_slice(&(n as u16).to_be_bytes());
        } else if n <= 0xffff_ffff {
            self.bytes.push(major_bits | 26);
            self.bytes.extend_from_slice(&(n as u32).to_be_bytes());
        } else {
            self.bytes.push(major_bits | 27);
            self.bytes.extend_from_slice(&n.to_be_bytes());
        }
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

fn is_f32_roundtrip(v: f64) -> bool {
    if !v.is_finite() {
        return false;
    }
    (v as f32) as f64 == v
}
