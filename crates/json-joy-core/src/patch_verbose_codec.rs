//! Native verbose patch codec port (`json-crdt-patch/codec/verbose/*`).

use base64::Engine;
use serde_json::{Map, Value};

use crate::patch::{ConValue, DecodedOp, Patch, PatchError, Timespan, Timestamp};
use crate::patch_builder::{encode_patch_from_ops, PatchBuildError};

const SESSION_SERVER: u64 = 1;

#[derive(Debug, thiserror::Error)]
pub enum VerboseCodecError {
    #[error("patch must not be empty")]
    EmptyPatch,
    #[error("invalid verbose patch payload")]
    InvalidPayload,
    #[error("invalid verbose operation")]
    InvalidOperation,
    #[error("unknown verbose operation: {0}")]
    UnknownOperation(String),
    #[error("invalid base64 payload")]
    InvalidBase64,
    #[error("patch encode failed: {0}")]
    Build(#[from] PatchBuildError),
    #[error("patch decode failed: {0}")]
    Decode(#[from] PatchError),
}

fn ts_to_verbose(ts: Timestamp) -> Value {
    if ts.sid == SESSION_SERVER {
        Value::from(ts.time)
    } else {
        Value::Array(vec![Value::from(ts.sid), Value::from(ts.time)])
    }
}

fn verbose_to_ts(v: &Value) -> Result<Timestamp, VerboseCodecError> {
    if let Some(time) = v.as_u64() {
        return Ok(Timestamp {
            sid: SESSION_SERVER,
            time,
        });
    }
    let a = v.as_array().ok_or(VerboseCodecError::InvalidOperation)?;
    if a.len() != 2 {
        return Err(VerboseCodecError::InvalidOperation);
    }
    Ok(Timestamp {
        sid: a[0].as_u64().ok_or(VerboseCodecError::InvalidOperation)?,
        time: a[1].as_u64().ok_or(VerboseCodecError::InvalidOperation)?,
    })
}

fn verbose_to_span(v: &Value) -> Result<Timespan, VerboseCodecError> {
    let a = v.as_array().ok_or(VerboseCodecError::InvalidOperation)?;
    if a.len() != 3 {
        return Err(VerboseCodecError::InvalidOperation);
    }
    Ok(Timespan {
        sid: a[0].as_u64().ok_or(VerboseCodecError::InvalidOperation)?,
        time: a[1].as_u64().ok_or(VerboseCodecError::InvalidOperation)?,
        span: a[2].as_u64().ok_or(VerboseCodecError::InvalidOperation)?,
    })
}

pub fn encode_patch_verbose(patch: &Patch) -> Result<Value, VerboseCodecError> {
    let (sid, time) = patch.id().ok_or(VerboseCodecError::EmptyPatch)?;
    let mut root = Map::new();
    root.insert(
        "id".to_string(),
        Value::Array(vec![Value::from(sid), Value::from(time)]),
    );
    let mut ops_out = Vec::with_capacity(patch.decoded_ops().len());

    for op in patch.decoded_ops() {
        let mut row = Map::new();
        match op {
            DecodedOp::NewCon { value, .. } => {
                row.insert("op".to_string(), Value::String("new_con".to_string()));
                match value {
                    ConValue::Undef => {}
                    ConValue::Json(v) => {
                        row.insert("value".to_string(), v.clone());
                    }
                    ConValue::Ref(ts) => {
                        row.insert("timestamp".to_string(), Value::Bool(true));
                        row.insert("value".to_string(), ts_to_verbose(*ts));
                    }
                }
            }
            DecodedOp::NewVal { .. } => {
                row.insert("op".to_string(), Value::String("new_val".to_string()));
            }
            DecodedOp::NewObj { .. } => {
                row.insert("op".to_string(), Value::String("new_obj".to_string()));
            }
            DecodedOp::NewVec { .. } => {
                row.insert("op".to_string(), Value::String("new_vec".to_string()));
            }
            DecodedOp::NewStr { .. } => {
                row.insert("op".to_string(), Value::String("new_str".to_string()));
            }
            DecodedOp::NewBin { .. } => {
                row.insert("op".to_string(), Value::String("new_bin".to_string()));
            }
            DecodedOp::NewArr { .. } => {
                row.insert("op".to_string(), Value::String("new_arr".to_string()));
            }
            DecodedOp::InsVal { obj, val, .. } => {
                row.insert("op".to_string(), Value::String("ins_val".to_string()));
                row.insert("obj".to_string(), ts_to_verbose(*obj));
                row.insert("value".to_string(), ts_to_verbose(*val));
            }
            DecodedOp::InsObj { obj, data, .. } => {
                row.insert("op".to_string(), Value::String("ins_obj".to_string()));
                row.insert("obj".to_string(), ts_to_verbose(*obj));
                row.insert(
                    "value".to_string(),
                    Value::Array(
                        data.iter()
                            .map(|(k, v)| {
                                Value::Array(vec![Value::String(k.clone()), ts_to_verbose(*v)])
                            })
                            .collect(),
                    ),
                );
            }
            DecodedOp::InsVec { obj, data, .. } => {
                row.insert("op".to_string(), Value::String("ins_vec".to_string()));
                row.insert("obj".to_string(), ts_to_verbose(*obj));
                row.insert(
                    "value".to_string(),
                    Value::Array(
                        data.iter()
                            .map(|(k, v)| Value::Array(vec![Value::from(*k), ts_to_verbose(*v)]))
                            .collect(),
                    ),
                );
            }
            DecodedOp::InsStr {
                obj,
                reference,
                data,
                ..
            } => {
                row.insert("op".to_string(), Value::String("ins_str".to_string()));
                row.insert("obj".to_string(), ts_to_verbose(*obj));
                row.insert("after".to_string(), ts_to_verbose(*reference));
                row.insert("value".to_string(), Value::String(data.clone()));
            }
            DecodedOp::InsBin {
                obj,
                reference,
                data,
                ..
            } => {
                row.insert("op".to_string(), Value::String("ins_bin".to_string()));
                row.insert("obj".to_string(), ts_to_verbose(*obj));
                row.insert("after".to_string(), ts_to_verbose(*reference));
                row.insert(
                    "value".to_string(),
                    Value::String(base64::engine::general_purpose::STANDARD.encode(data)),
                );
            }
            DecodedOp::InsArr {
                obj,
                reference,
                data,
                ..
            } => {
                row.insert("op".to_string(), Value::String("ins_arr".to_string()));
                row.insert("obj".to_string(), ts_to_verbose(*obj));
                row.insert("after".to_string(), ts_to_verbose(*reference));
                row.insert(
                    "values".to_string(),
                    Value::Array(data.iter().map(|v| ts_to_verbose(*v)).collect()),
                );
            }
            DecodedOp::UpdArr {
                obj,
                reference,
                val,
                ..
            } => {
                row.insert("op".to_string(), Value::String("upd_arr".to_string()));
                row.insert("obj".to_string(), ts_to_verbose(*obj));
                row.insert("ref".to_string(), ts_to_verbose(*reference));
                row.insert("value".to_string(), ts_to_verbose(*val));
            }
            DecodedOp::Del { obj, what, .. } => {
                row.insert("op".to_string(), Value::String("del".to_string()));
                row.insert("obj".to_string(), ts_to_verbose(*obj));
                row.insert(
                    "what".to_string(),
                    Value::Array(
                        what.iter()
                            .map(|span| {
                                Value::Array(vec![
                                    Value::from(span.sid),
                                    Value::from(span.time),
                                    Value::from(span.span),
                                ])
                            })
                            .collect(),
                    ),
                );
            }
            DecodedOp::Nop { len, .. } => {
                row.insert("op".to_string(), Value::String("nop".to_string()));
                if *len > 1 {
                    row.insert("len".to_string(), Value::from(*len));
                }
            }
        }
        ops_out.push(Value::Object(row));
    }

    root.insert("ops".to_string(), Value::Array(ops_out));
    Ok(Value::Object(root))
}

pub fn decode_patch_verbose(verbose: &Value) -> Result<Patch, VerboseCodecError> {
    let root = verbose
        .as_object()
        .ok_or(VerboseCodecError::InvalidPayload)?;
    let id_v = root.get("id").ok_or(VerboseCodecError::InvalidPayload)?;
    let id_a = id_v.as_array().ok_or(VerboseCodecError::InvalidPayload)?;
    if id_a.len() != 2 {
        return Err(VerboseCodecError::InvalidPayload);
    }
    let sid = id_a[0].as_u64().ok_or(VerboseCodecError::InvalidPayload)?;
    let time = id_a[1].as_u64().ok_or(VerboseCodecError::InvalidPayload)?;
    let rows = root
        .get("ops")
        .and_then(Value::as_array)
        .ok_or(VerboseCodecError::InvalidPayload)?;

    let mut ops = Vec::with_capacity(rows.len());
    let mut op_time = time;
    for row in rows {
        let obj = row.as_object().ok_or(VerboseCodecError::InvalidOperation)?;
        let name = obj
            .get("op")
            .and_then(Value::as_str)
            .ok_or(VerboseCodecError::InvalidOperation)?;
        let id = Timestamp { sid, time: op_time };
        let op = match name {
            "new_con" => {
                if obj.get("timestamp").and_then(Value::as_bool) == Some(true) {
                    DecodedOp::NewCon {
                        id,
                        value: ConValue::Ref(verbose_to_ts(
                            obj.get("value")
                                .ok_or(VerboseCodecError::InvalidOperation)?,
                        )?),
                    }
                } else {
                    match obj.get("value") {
                        Some(v) => DecodedOp::NewCon {
                            id,
                            value: ConValue::Json(v.clone()),
                        },
                        None => DecodedOp::NewCon {
                            id,
                            value: ConValue::Undef,
                        },
                    }
                }
            }
            "new_val" => DecodedOp::NewVal { id },
            "new_obj" => DecodedOp::NewObj { id },
            "new_vec" => DecodedOp::NewVec { id },
            "new_str" => DecodedOp::NewStr { id },
            "new_bin" => DecodedOp::NewBin { id },
            "new_arr" => DecodedOp::NewArr { id },
            "ins_val" => DecodedOp::InsVal {
                id,
                obj: verbose_to_ts(obj.get("obj").ok_or(VerboseCodecError::InvalidOperation)?)?,
                val: verbose_to_ts(
                    obj.get("value")
                        .ok_or(VerboseCodecError::InvalidOperation)?,
                )?,
            },
            "ins_obj" => {
                let data = obj
                    .get("value")
                    .and_then(Value::as_array)
                    .ok_or(VerboseCodecError::InvalidOperation)?
                    .iter()
                    .map(|t| {
                        let a = t.as_array().ok_or(VerboseCodecError::InvalidOperation)?;
                        if a.len() != 2 {
                            return Err(VerboseCodecError::InvalidOperation);
                        }
                        Ok((
                            a[0].as_str()
                                .ok_or(VerboseCodecError::InvalidOperation)?
                                .to_string(),
                            verbose_to_ts(&a[1])?,
                        ))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                DecodedOp::InsObj {
                    id,
                    obj: verbose_to_ts(obj.get("obj").ok_or(VerboseCodecError::InvalidOperation)?)?,
                    data,
                }
            }
            "ins_vec" => {
                let data = obj
                    .get("value")
                    .and_then(Value::as_array)
                    .ok_or(VerboseCodecError::InvalidOperation)?
                    .iter()
                    .map(|t| {
                        let a = t.as_array().ok_or(VerboseCodecError::InvalidOperation)?;
                        if a.len() != 2 {
                            return Err(VerboseCodecError::InvalidOperation);
                        }
                        Ok((
                            a[0].as_u64().ok_or(VerboseCodecError::InvalidOperation)?,
                            verbose_to_ts(&a[1])?,
                        ))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                DecodedOp::InsVec {
                    id,
                    obj: verbose_to_ts(obj.get("obj").ok_or(VerboseCodecError::InvalidOperation)?)?,
                    data,
                }
            }
            "ins_str" => DecodedOp::InsStr {
                id,
                obj: verbose_to_ts(obj.get("obj").ok_or(VerboseCodecError::InvalidOperation)?)?,
                reference: verbose_to_ts(
                    obj.get("after")
                        .or_else(|| obj.get("obj"))
                        .ok_or(VerboseCodecError::InvalidOperation)?,
                )?,
                data: obj
                    .get("value")
                    .and_then(Value::as_str)
                    .ok_or(VerboseCodecError::InvalidOperation)?
                    .to_string(),
            },
            "ins_bin" => DecodedOp::InsBin {
                id,
                obj: verbose_to_ts(obj.get("obj").ok_or(VerboseCodecError::InvalidOperation)?)?,
                reference: verbose_to_ts(
                    obj.get("after")
                        .or_else(|| obj.get("obj"))
                        .ok_or(VerboseCodecError::InvalidOperation)?,
                )?,
                data: base64::engine::general_purpose::STANDARD
                    .decode(
                        obj.get("value")
                            .and_then(Value::as_str)
                            .ok_or(VerboseCodecError::InvalidOperation)?,
                    )
                    .map_err(|_| VerboseCodecError::InvalidBase64)?,
            },
            "ins_arr" => DecodedOp::InsArr {
                id,
                obj: verbose_to_ts(obj.get("obj").ok_or(VerboseCodecError::InvalidOperation)?)?,
                reference: verbose_to_ts(
                    obj.get("after")
                        .or_else(|| obj.get("obj"))
                        .ok_or(VerboseCodecError::InvalidOperation)?,
                )?,
                data: obj
                    .get("values")
                    .and_then(Value::as_array)
                    .ok_or(VerboseCodecError::InvalidOperation)?
                    .iter()
                    .map(verbose_to_ts)
                    .collect::<Result<Vec<_>, _>>()?,
            },
            "upd_arr" => DecodedOp::UpdArr {
                id,
                obj: verbose_to_ts(obj.get("obj").ok_or(VerboseCodecError::InvalidOperation)?)?,
                reference: verbose_to_ts(
                    obj.get("ref").ok_or(VerboseCodecError::InvalidOperation)?,
                )?,
                val: verbose_to_ts(
                    obj.get("value")
                        .ok_or(VerboseCodecError::InvalidOperation)?,
                )?,
            },
            "del" => DecodedOp::Del {
                id,
                obj: verbose_to_ts(obj.get("obj").ok_or(VerboseCodecError::InvalidOperation)?)?,
                what: obj
                    .get("what")
                    .and_then(Value::as_array)
                    .ok_or(VerboseCodecError::InvalidOperation)?
                    .iter()
                    .map(verbose_to_span)
                    .collect::<Result<Vec<_>, _>>()?,
            },
            "nop" => DecodedOp::Nop {
                id,
                len: obj.get("len").and_then(Value::as_u64).unwrap_or(1),
            },
            other => return Err(VerboseCodecError::UnknownOperation(other.to_string())),
        };
        op_time = op_time.saturating_add(op.span());
        ops.push(op);
    }

    let bytes = encode_patch_from_ops(sid, time, &ops)?;
    Ok(Patch::from_binary(&bytes)?)
}
