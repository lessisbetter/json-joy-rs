use crate::crdt_binary::{write_b1vu56, write_vu57, LogicalClockBase};
use crate::model::ModelError;
use ciborium::value::Value as CborValue;
use json_joy_json_pack::{write_cbor_text_like_json_pack, write_json_like_json_pack};
use serde_json::Value;
use std::collections::HashMap;

use super::types::{ArrAtom, BinAtom, ConCell, Id, RuntimeNode, StrAtom};
use super::RuntimeModel;

pub(super) fn encode_logical(model: &RuntimeModel) -> Result<Vec<u8>, ModelError> {
    let mut clock = LogicalClockEncoder::from_model(model)?;

    let mut root = Vec::new();
    let mut enc = EncodeCtx {
        out: &mut root,
        mode: EncodeMode::Logical(&mut clock),
    };
    if let Some(root_id) = model.root {
        encode_node(&mut enc, root_id, model)?;
    } else {
        enc.out.push(0);
    }

    let mut out = Vec::with_capacity(root.len() + 32);
    let root_len = root.len() as u32;
    out.extend_from_slice(&root_len.to_be_bytes());
    out.extend_from_slice(&root);
    let table = clock.table();
    write_vu57(&mut out, table.len() as u64);
    for t in table {
        write_vu57(&mut out, t.sid);
        write_vu57(&mut out, t.time);
    }
    Ok(out)
}

pub(super) fn encode_server(model: &RuntimeModel, server_time: u64) -> Result<Vec<u8>, ModelError> {
    let mut out = Vec::new();
    out.push(0x80);
    write_vu57(&mut out, server_time);
    let mut enc = EncodeCtx {
        out: &mut out,
        mode: EncodeMode::Server,
    };
    if let Some(root_id) = model.root {
        encode_node(&mut enc, root_id, model)?;
    } else {
        out.push(0);
    }
    Ok(out)
}

struct EncodeCtx<'a> {
    out: &'a mut Vec<u8>,
    mode: EncodeMode<'a>,
}

enum EncodeMode<'a> {
    Logical(&'a mut LogicalClockEncoder),
    Server,
}

struct LogicalClockEncoder {
    local_time: u64,
    peers: HashMap<u64, u64>,
    by_sid: HashMap<u64, usize>,
    table: Vec<LogicalClockBase>,
}

impl LogicalClockEncoder {
    fn from_model(model: &RuntimeModel) -> Result<Self, ModelError> {
        let (local_sid, mut local_time, mut peers) = if let Some(first) = model.clock_table.first()
        {
            let mut peers: HashMap<u64, u64> = HashMap::new();
            for p in model.clock_table.iter().skip(1) {
                peers
                    .entry(p.sid)
                    .and_modify(|t| *t = (*t).max(p.time))
                    .or_insert(p.time);
            }
            (first.sid, first.time.saturating_add(1), peers)
        } else {
            return Err(ModelError::InvalidClockTable);
        };

        for (sid, ranges) in &model.clock.observed {
            for (_, end) in ranges {
                if *sid == local_sid {
                    if *end >= local_time {
                        local_time = end.saturating_add(1);
                    }
                } else {
                    peers
                        .entry(*sid)
                        .and_modify(|t| *t = (*t).max(*end))
                        .or_insert(*end);
                    if *end >= local_time {
                        local_time = end.saturating_add(1);
                    }
                }
            }
        }

        let mut by_sid = HashMap::new();
        let mut table = Vec::new();
        let base = local_time.saturating_sub(1);
        by_sid.insert(local_sid, 0);
        table.push(LogicalClockBase {
            sid: local_sid,
            time: base,
        });
        Ok(Self {
            local_time,
            peers,
            by_sid,
            table,
        })
    }

    fn append(&mut self, id: Id) -> Result<(u64, u64), ModelError> {
        if let Some(&idx) = self.by_sid.get(&id.sid) {
            let base = self.table[idx].time;
            let diff = base
                .checked_sub(id.time)
                .ok_or(ModelError::InvalidClockTable)?;
            return Ok(((idx as u64) + 1, diff));
        }
        let base = self
            .peers
            .get(&id.sid)
            .copied()
            .unwrap_or(self.local_time.saturating_sub(1));
        let diff = base
            .checked_sub(id.time)
            .ok_or(ModelError::InvalidClockTable)?;
        let idx = self.table.len();
        self.by_sid.insert(id.sid, idx);
        self.table.push(LogicalClockBase {
            sid: id.sid,
            time: base,
        });
        Ok(((idx as u64) + 1, diff))
    }

    fn table(&self) -> &[LogicalClockBase] {
        &self.table
    }
}

fn encode_id(enc: &mut EncodeCtx<'_>, id: Id) -> Result<(), ModelError> {
    let (session_index, diff) = match &mut enc.mode {
        EncodeMode::Logical(clock) => clock.append(id)?,
        EncodeMode::Server => {
            write_vu57(enc.out, id.time);
            return Ok(());
        }
    };
    if session_index <= 0b111 && diff <= 0b1111 {
        enc.out.push(((session_index as u8) << 4) | (diff as u8));
    } else {
        write_b1vu56(enc.out, 1, session_index);
        write_vu57(enc.out, diff);
    }
    Ok(())
}

fn encode_node(enc: &mut EncodeCtx<'_>, id: Id, model: &RuntimeModel) -> Result<(), ModelError> {
    let node = model.nodes.get(&id).ok_or(ModelError::InvalidModelBinary)?;
    encode_id(enc, id)?;
    match node {
        RuntimeNode::Con(ConCell::Json(v)) => {
            enc.out.push(0);
            json_to_cbor_bytes(v, enc.out)?;
        }
        RuntimeNode::Con(ConCell::Ref(rid)) => {
            enc.out.push(1);
            encode_id(enc, *rid)?;
        }
        RuntimeNode::Con(ConCell::Undef) => {
            enc.out.push(0);
            enc.out.push(0xf7);
        }
        RuntimeNode::Val(child) => {
            enc.out.push(0b0010_0000);
            encode_node(enc, *child, model)?;
        }
        RuntimeNode::Obj(entries) => {
            write_type_len(enc.out, 2, entries.len() as u64);
            for (k, v) in entries {
                cbor_text_bytes(k, enc.out)?;
                encode_node(enc, *v, model)?;
            }
        }
        RuntimeNode::Vec(map) => {
            let max = map.keys().copied().max().unwrap_or(0);
            let len = if map.is_empty() { 0 } else { max + 1 };
            write_type_len(enc.out, 3, len);
            for i in 0..len {
                if let Some(id) = map.get(&i) {
                    encode_node(enc, *id, model)?;
                } else {
                    enc.out.push(0);
                }
            }
        }
        RuntimeNode::Str(atoms) => {
            let chunks = group_str_chunks(atoms);
            write_type_len(enc.out, 4, chunks.len() as u64);
            for chunk in chunks {
                encode_id(enc, chunk.id)?;
                if let Some(text) = chunk.text {
                    cbor_text_bytes(&text, enc.out)?;
                } else {
                    let cbor = CborValue::Integer(ciborium::value::Integer::from(chunk.span));
                    ciborium::ser::into_writer(&cbor, &mut *enc.out)
                        .map_err(|_| ModelError::InvalidModelBinary)?;
                }
            }
        }
        RuntimeNode::Bin(atoms) => {
            let chunks = group_bin_chunks(atoms);
            write_type_len(enc.out, 5, chunks.len() as u64);
            for chunk in chunks {
                encode_id(enc, chunk.id)?;
                if let Some(bytes) = chunk.bytes {
                    write_b1vu56(enc.out, 0, chunk.span);
                    enc.out.extend_from_slice(&bytes);
                } else {
                    write_b1vu56(enc.out, 1, chunk.span);
                }
            }
        }
        RuntimeNode::Arr(atoms) => {
            let chunks = group_arr_chunks(atoms);
            write_type_len(enc.out, 6, chunks.len() as u64);
            for chunk in chunks {
                encode_id(enc, chunk.id)?;
                if let Some(values) = chunk.values {
                    write_b1vu56(enc.out, 0, chunk.span);
                    for v in values {
                        encode_node(enc, v, model)?;
                    }
                } else {
                    write_b1vu56(enc.out, 1, chunk.span);
                }
            }
        }
    }
    Ok(())
}

struct StrChunkEnc {
    id: Id,
    span: u64,
    text: Option<String>,
}

struct BinChunkEnc {
    id: Id,
    span: u64,
    bytes: Option<Vec<u8>>,
}

struct ArrChunkEnc {
    id: Id,
    span: u64,
    values: Option<Vec<Id>>,
}

fn group_str_chunks(atoms: &[StrAtom]) -> Vec<StrChunkEnc> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < atoms.len() {
        let start = &atoms[i];
        let mut j = i + 1;
        let mut dir: i8 = 0;
        while j < atoms.len() {
            let prev = &atoms[j - 1];
            let cur = &atoms[j];
            if prev.slot.sid != cur.slot.sid {
                break;
            }
            if prev.ch.is_some() != cur.ch.is_some() {
                break;
            }
            let step = if cur.slot.time == prev.slot.time.saturating_add(1) {
                1
            } else if cur.slot.time.saturating_add(1) == prev.slot.time {
                -1
            } else {
                0
            };
            if step == 0 || (dir != 0 && step != dir) {
                break;
            }
            dir = step;
            j += 1;
        }
        let span = (j - i) as u64;
        if start.ch.is_some() {
            let mut text = String::new();
            for atom in &atoms[i..j] {
                if let Some(ch) = atom.ch {
                    text.push(ch);
                }
            }
            out.push(StrChunkEnc {
                id: start.slot,
                span,
                text: Some(text),
            });
        } else {
            out.push(StrChunkEnc {
                id: start.slot,
                span,
                text: None,
            });
        }
        i = j;
    }
    out
}

fn group_bin_chunks(atoms: &[BinAtom]) -> Vec<BinChunkEnc> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < atoms.len() {
        let start = &atoms[i];
        let mut j = i + 1;
        let mut dir: i8 = 0;
        while j < atoms.len() {
            let prev = &atoms[j - 1];
            let cur = &atoms[j];
            if prev.slot.sid != cur.slot.sid {
                break;
            }
            if prev.byte.is_some() != cur.byte.is_some() {
                break;
            }
            let step = if cur.slot.time == prev.slot.time.saturating_add(1) {
                1
            } else if cur.slot.time.saturating_add(1) == prev.slot.time {
                -1
            } else {
                0
            };
            if step == 0 || (dir != 0 && step != dir) {
                break;
            }
            dir = step;
            j += 1;
        }
        let span = (j - i) as u64;
        if start.byte.is_some() {
            let mut bytes = Vec::with_capacity(j - i);
            for atom in &atoms[i..j] {
                if let Some(b) = atom.byte {
                    bytes.push(b);
                }
            }
            out.push(BinChunkEnc {
                id: start.slot,
                span,
                bytes: Some(bytes),
            });
        } else {
            out.push(BinChunkEnc {
                id: start.slot,
                span,
                bytes: None,
            });
        }
        i = j;
    }
    out
}

fn group_arr_chunks(atoms: &[ArrAtom]) -> Vec<ArrChunkEnc> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < atoms.len() {
        let start = &atoms[i];
        let mut j = i + 1;
        let mut dir: i8 = 0;
        while j < atoms.len() {
            let prev = &atoms[j - 1];
            let cur = &atoms[j];
            if prev.slot.sid != cur.slot.sid {
                break;
            }
            if prev.value.is_some() != cur.value.is_some() {
                break;
            }
            let step = if cur.slot.time == prev.slot.time.saturating_add(1) {
                1
            } else if cur.slot.time.saturating_add(1) == prev.slot.time {
                -1
            } else {
                0
            };
            if step == 0 || (dir != 0 && step != dir) {
                break;
            }
            dir = step;
            j += 1;
        }
        let span = (j - i) as u64;
        if start.value.is_some() {
            let mut values = Vec::with_capacity(j - i);
            for atom in &atoms[i..j] {
                if let Some(v) = atom.value {
                    values.push(v);
                }
            }
            out.push(ArrChunkEnc {
                id: start.slot,
                span,
                values: Some(values),
            });
        } else {
            out.push(ArrChunkEnc {
                id: start.slot,
                span,
                values: None,
            });
        }
        i = j;
    }
    out
}

fn write_type_len(out: &mut Vec<u8>, major: u8, len: u64) {
    if len < 31 {
        out.push((major << 5) | (len as u8));
    } else {
        out.push((major << 5) | 31);
        write_vu57(out, len);
    }
}

fn cbor_text_bytes(s: &str, out: &mut Vec<u8>) -> Result<(), ModelError> {
    write_cbor_text_like_json_pack(out, s);
    Ok(())
}

fn json_to_cbor_bytes(v: &Value, out: &mut Vec<u8>) -> Result<(), ModelError> {
    write_json_like_json_pack(out, v).map_err(|_| ModelError::InvalidModelBinary)
}
