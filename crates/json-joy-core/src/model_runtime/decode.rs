use crate::crdt_binary::{read_b1vu56, read_vu57, LogicalClockBase};
use crate::model::ModelError;
use ciborium::value::Value as CborValue;
use json_joy_json_pack::{cbor_to_json_owned, decode_cbor_value_with_consumed};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};

use super::types::{ArrAtom, BinAtom, ConCell, Id, RuntimeNode, StrAtom};

type DecodedRuntimeGraph = (
    HashMap<Id, RuntimeNode>,
    Option<Id>,
    Vec<LogicalClockBase>,
    Option<u64>,
);

pub(super) fn decode_runtime_graph(data: &[u8]) -> Result<DecodedRuntimeGraph, ModelError> {
    if data.is_empty() {
        return Err(ModelError::InvalidModelBinary);
    }
    if (data[0] & 0x80) != 0 {
        let mut pos = 1usize;
        let server_time = read_vu57(data, &mut pos).ok_or(ModelError::InvalidModelBinary)?;
        let mut ctx = DecodeCtx {
            data,
            pos,
            mode: DecodeMode::Server,
            nodes: HashMap::new(),
        };
        let root = decode_root(&mut ctx)?;
        Ok((ctx.nodes, root, Vec::new(), Some(server_time)))
    } else {
        if data.len() < 4 {
            return Err(ModelError::InvalidModelBinary);
        }
        let offset = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
        let root_start = 4usize;
        let root_end = root_start
            .checked_add(offset)
            .ok_or(ModelError::InvalidClockTable)?;
        if root_end > data.len() {
            return Err(ModelError::InvalidClockTable);
        }
        let mut cpos = root_end;
        let len = read_vu57(data, &mut cpos).ok_or(ModelError::InvalidClockTable)? as usize;
        if len == 0 {
            return Err(ModelError::InvalidClockTable);
        }
        let mut table = Vec::with_capacity(len);
        for _ in 0..len {
            let sid = read_vu57(data, &mut cpos).ok_or(ModelError::InvalidClockTable)?;
            let time = read_vu57(data, &mut cpos).ok_or(ModelError::InvalidClockTable)?;
            table.push(LogicalClockBase { sid, time });
        }
        let mut ctx = DecodeCtx {
            data: &data[root_start..root_end],
            pos: 0,
            mode: DecodeMode::Logical(table.clone()),
            nodes: HashMap::new(),
        };
        let root = decode_root(&mut ctx)?;
        if ctx.pos != ctx.data.len() {
            return Err(ModelError::InvalidModelBinary);
        }
        Ok((ctx.nodes, root, table, None))
    }
}

#[derive(Clone)]
enum DecodeMode {
    Logical(Vec<LogicalClockBase>),
    Server,
}

struct DecodeCtx<'a> {
    data: &'a [u8],
    pos: usize,
    mode: DecodeMode,
    nodes: HashMap<Id, RuntimeNode>,
}

fn decode_root(ctx: &mut DecodeCtx<'_>) -> Result<Option<Id>, ModelError> {
    let first = *ctx
        .data
        .get(ctx.pos)
        .ok_or(ModelError::InvalidModelBinary)?;
    if first == 0 {
        ctx.pos += 1;
        return Ok(None);
    }
    let id = decode_node(ctx)?;
    Ok(Some(id))
}

fn decode_node(ctx: &mut DecodeCtx<'_>) -> Result<Id, ModelError> {
    let id = decode_id(ctx)?;
    let oct = read_u8(ctx)?;
    let major = oct >> 5;
    let minor = (oct & 0b1_1111) as u64;
    match major {
        0 => {
            let node = if minor == 0 {
                RuntimeNode::Con(ConCell::Json(cbor_to_json(read_one_cbor(ctx)?)?))
            } else {
                let rid = decode_id(ctx)?;
                RuntimeNode::Con(ConCell::Ref(rid))
            };
            ctx.nodes.insert(id, node);
        }
        1 => {
            let child = decode_node(ctx)?;
            ctx.nodes.insert(id, RuntimeNode::Val(child));
        }
        2 => {
            let len = if minor != 31 {
                minor
            } else {
                read_vu57_ctx(ctx)?
            };
            let mut entries = Vec::new();
            for _ in 0..len {
                let key = match read_one_cbor(ctx)? {
                    CborValue::Text(s) => s,
                    _ => return Err(ModelError::InvalidModelBinary),
                };
                let child = decode_node(ctx)?;
                entries.push((key, child));
            }
            ctx.nodes.insert(id, RuntimeNode::Obj(entries));
        }
        3 => {
            let len = if minor != 31 {
                minor
            } else {
                read_vu57_ctx(ctx)?
            };
            let mut map = BTreeMap::new();
            for i in 0..len {
                if peek_u8(ctx)? == 0 {
                    ctx.pos += 1;
                } else {
                    let child = decode_node(ctx)?;
                    map.insert(i, child);
                }
            }
            ctx.nodes.insert(id, RuntimeNode::Vec(map));
        }
        4 => {
            let len = if minor != 31 {
                minor
            } else {
                read_vu57_ctx(ctx)?
            };
            let mut atoms = Vec::new();
            for _ in 0..len {
                let chunk_id = decode_id(ctx)?;
                let val = read_one_cbor(ctx)?;
                match val {
                    CborValue::Text(s) => {
                        for (i, ch) in s.chars().enumerate() {
                            atoms.push(StrAtom {
                                slot: Id {
                                    sid: chunk_id.sid,
                                    time: chunk_id.time + i as u64,
                                },
                                ch: Some(ch),
                            });
                        }
                    }
                    CborValue::Integer(i) => {
                        let span: u64 = i.try_into().map_err(|_| ModelError::InvalidModelBinary)?;
                        for i in 0..span {
                            atoms.push(StrAtom {
                                slot: Id {
                                    sid: chunk_id.sid,
                                    time: chunk_id.time + i,
                                },
                                ch: None,
                            });
                        }
                    }
                    _ => return Err(ModelError::InvalidModelBinary),
                }
            }
            ctx.nodes.insert(id, RuntimeNode::Str(atoms));
        }
        5 => {
            let len = if minor != 31 {
                minor
            } else {
                read_vu57_ctx(ctx)?
            };
            let mut atoms = Vec::new();
            for _ in 0..len {
                let chunk_id = decode_id(ctx)?;
                let (deleted, span) =
                    read_b1vu56(ctx.data, &mut ctx.pos).ok_or(ModelError::InvalidModelBinary)?;
                if deleted == 1 {
                    for i in 0..span {
                        atoms.push(BinAtom {
                            slot: Id {
                                sid: chunk_id.sid,
                                time: chunk_id.time + i,
                            },
                            byte: None,
                        });
                    }
                } else {
                    let bytes = read_buf(ctx, span as usize)?;
                    for (i, b) in bytes.iter().enumerate() {
                        atoms.push(BinAtom {
                            slot: Id {
                                sid: chunk_id.sid,
                                time: chunk_id.time + i as u64,
                            },
                            byte: Some(*b),
                        });
                    }
                }
            }
            ctx.nodes.insert(id, RuntimeNode::Bin(atoms));
        }
        6 => {
            let len = if minor != 31 {
                minor
            } else {
                read_vu57_ctx(ctx)?
            };
            let mut atoms = Vec::new();
            for _ in 0..len {
                let chunk_id = decode_id(ctx)?;
                let (deleted, span) =
                    read_b1vu56(ctx.data, &mut ctx.pos).ok_or(ModelError::InvalidModelBinary)?;
                if deleted == 1 {
                    for i in 0..span {
                        atoms.push(ArrAtom {
                            slot: Id {
                                sid: chunk_id.sid,
                                time: chunk_id.time + i,
                            },
                            value: None,
                        });
                    }
                } else {
                    for i in 0..span {
                        let child = decode_node(ctx)?;
                        atoms.push(ArrAtom {
                            slot: Id {
                                sid: chunk_id.sid,
                                time: chunk_id.time + i,
                            },
                            value: Some(child),
                        });
                    }
                }
            }
            ctx.nodes.insert(id, RuntimeNode::Arr(atoms));
        }
        _ => return Err(ModelError::InvalidModelBinary),
    }
    Ok(id)
}

pub(super) fn bootstrap_graph_from_view(
    view: &Value,
    sid: u64,
) -> (HashMap<Id, RuntimeNode>, Option<Id>, Vec<LogicalClockBase>) {
    let mut b = ViewBootstrap::new(sid);
    let root = Some(b.build_node(view));
    let table = vec![LogicalClockBase {
        sid,
        time: b.next_time.saturating_sub(1),
    }];
    (b.nodes, root, table)
}

struct ViewBootstrap {
    sid: u64,
    next_time: u64,
    nodes: HashMap<Id, RuntimeNode>,
}

impl ViewBootstrap {
    fn new(sid: u64) -> Self {
        Self {
            sid,
            next_time: 1,
            nodes: HashMap::new(),
        }
    }

    fn alloc_id(&mut self) -> Id {
        let id = Id {
            sid: self.sid,
            time: self.next_time,
        };
        self.next_time = self.next_time.saturating_add(1);
        id
    }

    fn alloc_slots(&mut self, len: usize) -> Id {
        let base = Id {
            sid: self.sid,
            time: self.next_time,
        };
        self.next_time = self.next_time.saturating_add(len as u64);
        base
    }

    fn build_node(&mut self, value: &Value) -> Id {
        match value {
            Value::Null | Value::Bool(_) | Value::Number(_) => {
                let id = self.alloc_id();
                self.nodes
                    .insert(id, RuntimeNode::Con(ConCell::Json(value.clone())));
                id
            }
            Value::String(s) => {
                let id = self.alloc_id();
                let base = self.alloc_slots(s.chars().count());
                let mut atoms = Vec::new();
                for (i, ch) in s.chars().enumerate() {
                    atoms.push(StrAtom {
                        slot: Id {
                            sid: self.sid,
                            time: base.time + i as u64,
                        },
                        ch: Some(ch),
                    });
                }
                self.nodes.insert(id, RuntimeNode::Str(atoms));
                id
            }
            Value::Array(items) => {
                let id = self.alloc_id();
                let mut child_ids = Vec::with_capacity(items.len());
                for item in items {
                    child_ids.push(self.build_node(item));
                }
                let mut atoms = Vec::with_capacity(child_ids.len());
                if !child_ids.is_empty() {
                    let base = self.alloc_slots(child_ids.len());
                    for (i, child_id) in child_ids.into_iter().enumerate() {
                        atoms.push(ArrAtom {
                            slot: Id {
                                sid: self.sid,
                                time: base.time + i as u64,
                            },
                            value: Some(child_id),
                        });
                    }
                }
                self.nodes.insert(id, RuntimeNode::Arr(atoms));
                id
            }
            Value::Object(map) => {
                let id = self.alloc_id();
                let mut entries = Vec::with_capacity(map.len());
                for (k, v) in map {
                    let child = self.build_node(v);
                    entries.push((k.clone(), child));
                }
                self.nodes.insert(id, RuntimeNode::Obj(entries));
                id
            }
        }
    }
}

fn cbor_to_json(v: CborValue) -> Result<Value, ModelError> {
    cbor_to_json_owned(v).map_err(|_| ModelError::InvalidModelBinary)
}

fn decode_id(ctx: &mut DecodeCtx<'_>) -> Result<Id, ModelError> {
    match ctx.mode.clone() {
        DecodeMode::Server => {
            let time = read_vu57_ctx(ctx)?;
            Ok(Id { sid: 1, time })
        }
        DecodeMode::Logical(table) => {
            let first = read_u8(ctx)?;
            if first <= 0x7f {
                let session_index = (first >> 4) as usize;
                let diff = (first & 0x0f) as u64;
                decode_rel_id(&table, session_index, diff)
            } else {
                ctx.pos -= 1;
                let (_flag, session_index) =
                    read_b1vu56(ctx.data, &mut ctx.pos).ok_or(ModelError::InvalidModelBinary)?;
                let diff =
                    read_vu57(ctx.data, &mut ctx.pos).ok_or(ModelError::InvalidModelBinary)?;
                decode_rel_id(&table, session_index as usize, diff)
            }
        }
    }
}

fn decode_rel_id(
    table: &[LogicalClockBase],
    session_index: usize,
    diff: u64,
) -> Result<Id, ModelError> {
    if session_index == 0 {
        return Ok(Id { sid: 0, time: diff });
    }
    let base = table
        .get(session_index - 1)
        .ok_or(ModelError::InvalidClockTable)?;
    let time = base
        .time
        .checked_sub(diff)
        .ok_or(ModelError::InvalidClockTable)?;
    Ok(Id {
        sid: base.sid,
        time,
    })
}

fn read_u8(ctx: &mut DecodeCtx<'_>) -> Result<u8, ModelError> {
    let b = *ctx
        .data
        .get(ctx.pos)
        .ok_or(ModelError::InvalidModelBinary)?;
    ctx.pos += 1;
    Ok(b)
}

fn peek_u8(ctx: &DecodeCtx<'_>) -> Result<u8, ModelError> {
    ctx.data
        .get(ctx.pos)
        .copied()
        .ok_or(ModelError::InvalidModelBinary)
}

fn read_buf<'a>(ctx: &mut DecodeCtx<'a>, n: usize) -> Result<&'a [u8], ModelError> {
    if ctx.pos + n > ctx.data.len() {
        return Err(ModelError::InvalidModelBinary);
    }
    let start = ctx.pos;
    ctx.pos += n;
    Ok(&ctx.data[start..start + n])
}

fn read_vu57_ctx(ctx: &mut DecodeCtx<'_>) -> Result<u64, ModelError> {
    read_vu57(ctx.data, &mut ctx.pos).ok_or(ModelError::InvalidModelBinary)
}

fn read_one_cbor(ctx: &mut DecodeCtx<'_>) -> Result<CborValue, ModelError> {
    let slice = &ctx.data[ctx.pos..];
    let (val, consumed) =
        decode_cbor_value_with_consumed(slice).map_err(|_| ModelError::InvalidModelBinary)?;
    ctx.pos += consumed;
    Ok(val)
}
