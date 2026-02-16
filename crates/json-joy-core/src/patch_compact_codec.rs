//! Native compact patch codec port (`json-crdt-patch/codec/compact/*`).

use base64::Engine;
use serde_json::Value;

use crate::patch::{ConValue, DecodedOp, Patch, PatchError, Timespan, Timestamp};
use crate::patch_builder::{encode_patch_from_ops, PatchBuildError};

const SESSION_SERVER: u64 = 1;

#[derive(Debug, thiserror::Error)]
pub enum CompactCodecError {
    #[error("patch must not be empty")]
    EmptyPatch,
    #[error("invalid compact header")]
    InvalidHeader,
    #[error("invalid compact operation")]
    InvalidOperation,
    #[error("unknown compact opcode: {0}")]
    UnknownOpcode(u64),
    #[error("invalid base64 payload")]
    InvalidBase64,
    #[error("patch encode failed: {0}")]
    Build(#[from] PatchBuildError),
    #[error("patch decode failed: {0}")]
    Decode(#[from] PatchError),
}

fn ts_to_compact(base_sid: u64, ts: Timestamp) -> Value {
    if ts.sid == base_sid {
        Value::from(ts.time)
    } else {
        Value::Array(vec![Value::from(ts.sid), Value::from(ts.time)])
    }
}

fn span_to_compact(base_sid: u64, span: Timespan) -> Value {
    if span.sid == base_sid {
        Value::Array(vec![Value::from(span.time), Value::from(span.span)])
    } else {
        Value::Array(vec![
            Value::from(span.sid),
            Value::from(span.time),
            Value::from(span.span),
        ])
    }
}

fn compact_to_ts(base_sid: u64, v: &Value) -> Result<Timestamp, CompactCodecError> {
    if let Some(time) = v.as_u64() {
        return Ok(Timestamp {
            sid: base_sid,
            time,
        });
    }
    let arr = v.as_array().ok_or(CompactCodecError::InvalidOperation)?;
    if arr.len() != 2 {
        return Err(CompactCodecError::InvalidOperation);
    }
    Ok(Timestamp {
        sid: arr[0].as_u64().ok_or(CompactCodecError::InvalidOperation)?,
        time: arr[1].as_u64().ok_or(CompactCodecError::InvalidOperation)?,
    })
}

fn compact_to_span(base_sid: u64, v: &Value) -> Result<Timespan, CompactCodecError> {
    let arr = v.as_array().ok_or(CompactCodecError::InvalidOperation)?;
    match arr.len() {
        2 => Ok(Timespan {
            sid: base_sid,
            time: arr[0].as_u64().ok_or(CompactCodecError::InvalidOperation)?,
            span: arr[1].as_u64().ok_or(CompactCodecError::InvalidOperation)?,
        }),
        3 => Ok(Timespan {
            sid: arr[0].as_u64().ok_or(CompactCodecError::InvalidOperation)?,
            time: arr[1].as_u64().ok_or(CompactCodecError::InvalidOperation)?,
            span: arr[2].as_u64().ok_or(CompactCodecError::InvalidOperation)?,
        }),
        _ => Err(CompactCodecError::InvalidOperation),
    }
}

pub fn encode_patch_compact(patch: &Patch) -> Result<Value, CompactCodecError> {
    let (sid, time) = patch.id().ok_or(CompactCodecError::EmptyPatch)?;
    let mut out = Vec::with_capacity(patch.decoded_ops().len() + 1);
    let header = if sid == SESSION_SERVER {
        Value::Array(vec![Value::from(time)])
    } else {
        Value::Array(vec![Value::Array(vec![
            Value::from(sid),
            Value::from(time),
        ])])
    };
    out.push(header);

    for op in patch.decoded_ops() {
        let row = match op {
            DecodedOp::NewCon { value, .. } => match value {
                ConValue::Undef => Value::Array(vec![Value::from(0u64)]),
                ConValue::Json(v) => Value::Array(vec![Value::from(0u64), v.clone()]),
                ConValue::Ref(ts) => Value::Array(vec![
                    Value::from(0u64),
                    ts_to_compact(sid, *ts),
                    Value::Bool(true),
                ]),
            },
            DecodedOp::NewVal { .. } => Value::Array(vec![Value::from(1u64)]),
            DecodedOp::NewObj { .. } => Value::Array(vec![Value::from(2u64)]),
            DecodedOp::NewVec { .. } => Value::Array(vec![Value::from(3u64)]),
            DecodedOp::NewStr { .. } => Value::Array(vec![Value::from(4u64)]),
            DecodedOp::NewBin { .. } => Value::Array(vec![Value::from(5u64)]),
            DecodedOp::NewArr { .. } => Value::Array(vec![Value::from(6u64)]),
            DecodedOp::InsVal { obj, val, .. } => Value::Array(vec![
                Value::from(9u64),
                ts_to_compact(sid, *obj),
                ts_to_compact(sid, *val),
            ]),
            DecodedOp::InsObj { obj, data, .. } => {
                let tuples = data
                    .iter()
                    .map(|(k, v)| {
                        Value::Array(vec![Value::String(k.clone()), ts_to_compact(sid, *v)])
                    })
                    .collect::<Vec<_>>();
                Value::Array(vec![
                    Value::from(10u64),
                    ts_to_compact(sid, *obj),
                    Value::Array(tuples),
                ])
            }
            DecodedOp::InsVec { obj, data, .. } => {
                let tuples = data
                    .iter()
                    .map(|(k, v)| Value::Array(vec![Value::from(*k), ts_to_compact(sid, *v)]))
                    .collect::<Vec<_>>();
                Value::Array(vec![
                    Value::from(11u64),
                    ts_to_compact(sid, *obj),
                    Value::Array(tuples),
                ])
            }
            DecodedOp::InsStr {
                obj,
                reference,
                data,
                ..
            } => Value::Array(vec![
                Value::from(12u64),
                ts_to_compact(sid, *obj),
                ts_to_compact(sid, *reference),
                Value::String(data.clone()),
            ]),
            DecodedOp::InsBin {
                obj,
                reference,
                data,
                ..
            } => Value::Array(vec![
                Value::from(13u64),
                ts_to_compact(sid, *obj),
                ts_to_compact(sid, *reference),
                Value::String(base64::engine::general_purpose::STANDARD.encode(data)),
            ]),
            DecodedOp::InsArr {
                obj,
                reference,
                data,
                ..
            } => Value::Array(vec![
                Value::from(14u64),
                ts_to_compact(sid, *obj),
                ts_to_compact(sid, *reference),
                Value::Array(data.iter().map(|id| ts_to_compact(sid, *id)).collect()),
            ]),
            DecodedOp::UpdArr {
                obj,
                reference,
                val,
                ..
            } => Value::Array(vec![
                Value::from(15u64),
                ts_to_compact(sid, *obj),
                ts_to_compact(sid, *reference),
                ts_to_compact(sid, *val),
            ]),
            DecodedOp::Del { obj, what, .. } => Value::Array(vec![
                Value::from(16u64),
                ts_to_compact(sid, *obj),
                Value::Array(what.iter().map(|s| span_to_compact(sid, *s)).collect()),
            ]),
            DecodedOp::Nop { len, .. } => {
                if *len > 1 {
                    Value::Array(vec![Value::from(17u64), Value::from(*len)])
                } else {
                    Value::Array(vec![Value::from(17u64)])
                }
            }
        };
        out.push(row);
    }
    Ok(Value::Array(out))
}

pub fn decode_patch_compact(compact: &Value) -> Result<Patch, CompactCodecError> {
    let rows = compact.as_array().ok_or(CompactCodecError::InvalidHeader)?;
    if rows.is_empty() {
        return Err(CompactCodecError::InvalidHeader);
    }
    let header = rows[0].as_array().ok_or(CompactCodecError::InvalidHeader)?;
    if header.is_empty() {
        return Err(CompactCodecError::InvalidHeader);
    }
    let (sid, time) = if let Some(t) = header[0].as_u64() {
        (SESSION_SERVER, t)
    } else {
        let id = header[0]
            .as_array()
            .ok_or(CompactCodecError::InvalidHeader)?;
        if id.len() != 2 {
            return Err(CompactCodecError::InvalidHeader);
        }
        (
            id[0].as_u64().ok_or(CompactCodecError::InvalidHeader)?,
            id[1].as_u64().ok_or(CompactCodecError::InvalidHeader)?,
        )
    };

    let mut ops = Vec::with_capacity(rows.len().saturating_sub(1));
    let mut op_time = time;
    for row in rows.iter().skip(1) {
        let op = row.as_array().ok_or(CompactCodecError::InvalidOperation)?;
        if op.is_empty() {
            return Err(CompactCodecError::InvalidOperation);
        }
        let code = op[0].as_u64().ok_or(CompactCodecError::InvalidOperation)?;
        let id = Timestamp { sid, time: op_time };
        let decoded = match code {
            0 => {
                if op.len() == 1 {
                    DecodedOp::NewCon {
                        id,
                        value: ConValue::Undef,
                    }
                } else if op.get(2).and_then(Value::as_bool) == Some(true) {
                    let ts = compact_to_ts(sid, &op[1])?;
                    DecodedOp::NewCon {
                        id,
                        value: ConValue::Ref(ts),
                    }
                } else {
                    DecodedOp::NewCon {
                        id,
                        value: ConValue::Json(op[1].clone()),
                    }
                }
            }
            1 => DecodedOp::NewVal { id },
            2 => DecodedOp::NewObj { id },
            3 => DecodedOp::NewVec { id },
            4 => DecodedOp::NewStr { id },
            5 => DecodedOp::NewBin { id },
            6 => DecodedOp::NewArr { id },
            9 => DecodedOp::InsVal {
                id,
                obj: compact_to_ts(sid, op.get(1).ok_or(CompactCodecError::InvalidOperation)?)?,
                val: compact_to_ts(sid, op.get(2).ok_or(CompactCodecError::InvalidOperation)?)?,
            },
            10 => {
                let tuples = op
                    .get(2)
                    .and_then(Value::as_array)
                    .ok_or(CompactCodecError::InvalidOperation)?
                    .iter()
                    .map(|t| {
                        let a = t.as_array().ok_or(CompactCodecError::InvalidOperation)?;
                        if a.len() != 2 {
                            return Err(CompactCodecError::InvalidOperation);
                        }
                        Ok((
                            a[0].as_str()
                                .ok_or(CompactCodecError::InvalidOperation)?
                                .to_string(),
                            compact_to_ts(sid, &a[1])?,
                        ))
                    })
                    .collect::<Result<Vec<_>, CompactCodecError>>()?;
                DecodedOp::InsObj {
                    id,
                    obj: compact_to_ts(sid, op.get(1).ok_or(CompactCodecError::InvalidOperation)?)?,
                    data: tuples,
                }
            }
            11 => {
                let tuples = op
                    .get(2)
                    .and_then(Value::as_array)
                    .ok_or(CompactCodecError::InvalidOperation)?
                    .iter()
                    .map(|t| {
                        let a = t.as_array().ok_or(CompactCodecError::InvalidOperation)?;
                        if a.len() != 2 {
                            return Err(CompactCodecError::InvalidOperation);
                        }
                        Ok((
                            a[0].as_u64().ok_or(CompactCodecError::InvalidOperation)?,
                            compact_to_ts(sid, &a[1])?,
                        ))
                    })
                    .collect::<Result<Vec<_>, CompactCodecError>>()?;
                DecodedOp::InsVec {
                    id,
                    obj: compact_to_ts(sid, op.get(1).ok_or(CompactCodecError::InvalidOperation)?)?,
                    data: tuples,
                }
            }
            12 => DecodedOp::InsStr {
                id,
                obj: compact_to_ts(sid, op.get(1).ok_or(CompactCodecError::InvalidOperation)?)?,
                reference: compact_to_ts(
                    sid,
                    op.get(2).ok_or(CompactCodecError::InvalidOperation)?,
                )?,
                data: op
                    .get(3)
                    .and_then(Value::as_str)
                    .ok_or(CompactCodecError::InvalidOperation)?
                    .to_string(),
            },
            13 => DecodedOp::InsBin {
                id,
                obj: compact_to_ts(sid, op.get(1).ok_or(CompactCodecError::InvalidOperation)?)?,
                reference: compact_to_ts(
                    sid,
                    op.get(2).ok_or(CompactCodecError::InvalidOperation)?,
                )?,
                data: base64::engine::general_purpose::STANDARD
                    .decode(
                        op.get(3)
                            .and_then(Value::as_str)
                            .ok_or(CompactCodecError::InvalidOperation)?,
                    )
                    .map_err(|_| CompactCodecError::InvalidBase64)?,
            },
            14 => {
                let data = op
                    .get(3)
                    .and_then(Value::as_array)
                    .ok_or(CompactCodecError::InvalidOperation)?
                    .iter()
                    .map(|v| compact_to_ts(sid, v))
                    .collect::<Result<Vec<_>, _>>()?;
                DecodedOp::InsArr {
                    id,
                    obj: compact_to_ts(sid, op.get(1).ok_or(CompactCodecError::InvalidOperation)?)?,
                    reference: compact_to_ts(
                        sid,
                        op.get(2).ok_or(CompactCodecError::InvalidOperation)?,
                    )?,
                    data,
                }
            }
            15 => DecodedOp::UpdArr {
                id,
                obj: compact_to_ts(sid, op.get(1).ok_or(CompactCodecError::InvalidOperation)?)?,
                reference: compact_to_ts(
                    sid,
                    op.get(2).ok_or(CompactCodecError::InvalidOperation)?,
                )?,
                val: compact_to_ts(sid, op.get(3).ok_or(CompactCodecError::InvalidOperation)?)?,
            },
            16 => {
                let spans = op
                    .get(2)
                    .and_then(Value::as_array)
                    .ok_or(CompactCodecError::InvalidOperation)?
                    .iter()
                    .map(|v| compact_to_span(sid, v))
                    .collect::<Result<Vec<_>, _>>()?;
                DecodedOp::Del {
                    id,
                    obj: compact_to_ts(sid, op.get(1).ok_or(CompactCodecError::InvalidOperation)?)?,
                    what: spans,
                }
            }
            17 => DecodedOp::Nop {
                id,
                len: op.get(1).and_then(Value::as_u64).unwrap_or(1),
            },
            other => return Err(CompactCodecError::UnknownOpcode(other)),
        };
        op_time = op_time.saturating_add(decoded.span());
        ops.push(decoded);
    }

    let bytes = encode_patch_from_ops(sid, time, &ops)?;
    Ok(Patch::from_binary(&bytes)?)
}
