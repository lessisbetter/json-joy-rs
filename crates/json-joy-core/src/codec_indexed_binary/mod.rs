use std::collections::{BTreeMap, HashMap};
use std::io::Cursor;

use ciborium::value::Value as CborValue;
use serde_json::Value;
use thiserror::Error;

use crate::crdt_binary::{read_b1vu56, read_vu57, write_b1vu56, write_vu57, LogicalClockBase};
use crate::model::ModelError;
use crate::model_runtime::types::{ArrAtom, BinAtom, ConCell, Id, RuntimeNode, StrAtom};
use crate::model_runtime::RuntimeModel;

pub type IndexedFields = BTreeMap<String, Vec<u8>>;

#[derive(Debug, Error)]
pub enum IndexedBinaryCodecError {
    #[error("invalid indexed fields")]
    InvalidFields,
    #[error("invalid indexed node payload")]
    InvalidNode,
    #[error("model runtime failure: {0}")]
    Model(#[from] ModelError),
}

pub fn encode_model_binary_to_fields(
    model_binary: &[u8],
) -> Result<IndexedFields, IndexedBinaryCodecError> {
    let runtime = RuntimeModel::from_model_binary(model_binary)?;
    if runtime.server_clock_time.is_some() {
        return Err(IndexedBinaryCodecError::InvalidFields);
    }
    if runtime.clock_table.is_empty() {
        return Err(IndexedBinaryCodecError::InvalidFields);
    }

    let mut out = IndexedFields::new();
    out.insert("c".to_string(), encode_clock_table(&runtime.clock_table));

    let mut index_by_sid: HashMap<u64, u64> = HashMap::new();
    for (idx, c) in runtime.clock_table.iter().enumerate() {
        index_by_sid.insert(c.sid, idx as u64);
    }

    if let Some(root) = runtime.root {
        if root.sid != 0 {
            out.insert("r".to_string(), encode_indexed_id(root, &index_by_sid)?);
        }
    }

    let mut nodes: Vec<(&Id, &RuntimeNode)> = runtime.nodes.iter().collect();
    nodes.sort_by_key(|(id, _)| (id.sid, id.time));
    for (id, node) in nodes {
        let field = format!(
            "{}_{}",
            to_base36(
                *index_by_sid
                    .get(&id.sid)
                    .ok_or(IndexedBinaryCodecError::InvalidFields)?
            ),
            to_base36(id.time)
        );
        let payload = encode_node_payload(node, &index_by_sid)?;
        out.insert(field, payload);
    }

    Ok(out)
}

pub fn decode_fields_to_model_binary(
    fields: &IndexedFields,
) -> Result<Vec<u8>, IndexedBinaryCodecError> {
    let c = fields
        .get("c")
        .ok_or(IndexedBinaryCodecError::InvalidFields)?;
    let clock_table = decode_clock_table(c)?;
    if clock_table.is_empty() {
        return Err(IndexedBinaryCodecError::InvalidFields);
    }

    let root = match fields.get("r") {
        Some(bytes) => Some(decode_indexed_id(bytes, &clock_table)?),
        None => None,
    };

    let mut nodes: HashMap<Id, RuntimeNode> = HashMap::new();
    for (field, payload) in fields {
        if field == "c" || field == "r" {
            continue;
        }
        let id = parse_field_id(field, &clock_table)?;
        let node = decode_node_payload(payload, &clock_table)?;
        nodes.insert(id, node);
    }

    let runtime = RuntimeModel {
        nodes,
        root,
        clock: Default::default(),
        fallback_view: Value::Null,
        infer_empty_object_root: false,
        clock_table,
        server_clock_time: None,
    };

    runtime
        .to_model_binary_like()
        .map_err(IndexedBinaryCodecError::from)
}

fn encode_clock_table(clock_table: &[LogicalClockBase]) -> Vec<u8> {
    let mut out = Vec::new();
    write_vu57(&mut out, clock_table.len() as u64);
    for c in clock_table {
        write_vu57(&mut out, c.sid);
        write_vu57(&mut out, c.time);
    }
    out
}

fn decode_clock_table(data: &[u8]) -> Result<Vec<LogicalClockBase>, IndexedBinaryCodecError> {
    let mut pos = 0usize;
    let len = read_vu57(data, &mut pos).ok_or(IndexedBinaryCodecError::InvalidFields)? as usize;
    if len == 0 {
        return Err(IndexedBinaryCodecError::InvalidFields);
    }
    let mut out = Vec::with_capacity(len);
    for _ in 0..len {
        let sid = read_vu57(data, &mut pos).ok_or(IndexedBinaryCodecError::InvalidFields)?;
        let time = read_vu57(data, &mut pos).ok_or(IndexedBinaryCodecError::InvalidFields)?;
        out.push(LogicalClockBase { sid, time });
    }
    if pos != data.len() {
        return Err(IndexedBinaryCodecError::InvalidFields);
    }
    Ok(out)
}

fn to_base36(v: u64) -> String {
    if v == 0 {
        return "0".to_string();
    }
    let mut n = v;
    let mut out = Vec::new();
    while n > 0 {
        let d = (n % 36) as u8;
        out.push(if d < 10 {
            (b'0' + d) as char
        } else {
            (b'a' + (d - 10)) as char
        });
        n /= 36;
    }
    out.iter().rev().collect()
}

fn from_base36(s: &str) -> Option<u64> {
    let mut acc = 0u64;
    for b in s.bytes() {
        let v = match b {
            b'0'..=b'9' => (b - b'0') as u64,
            b'a'..=b'z' => 10 + (b - b'a') as u64,
            b'A'..=b'Z' => 10 + (b - b'A') as u64,
            _ => return None,
        };
        acc = acc.checked_mul(36)?.checked_add(v)?;
    }
    Some(acc)
}

fn encode_indexed_id(
    id: Id,
    index_by_sid: &HashMap<u64, u64>,
) -> Result<Vec<u8>, IndexedBinaryCodecError> {
    let x = *index_by_sid
        .get(&id.sid)
        .ok_or(IndexedBinaryCodecError::InvalidFields)?;
    let y = id.time;
    let mut out = Vec::new();
    if x <= 0b111 && y <= 0b1111 {
        out.push(((x as u8) << 4) | (y as u8));
    } else {
        write_b1vu56(&mut out, 1, x);
        write_vu57(&mut out, y);
    }
    Ok(out)
}

fn decode_indexed_id(
    data: &[u8],
    clock_table: &[LogicalClockBase],
) -> Result<Id, IndexedBinaryCodecError> {
    if data.is_empty() {
        return Err(IndexedBinaryCodecError::InvalidFields);
    }
    let first = data[0];
    let (session_index, time) = if first <= 0x7f {
        ((first >> 4) as u64, (first & 0x0f) as u64)
    } else {
        let mut pos = 0usize;
        let (flag, x) =
            read_b1vu56(data, &mut pos).ok_or(IndexedBinaryCodecError::InvalidFields)?;
        if flag != 1 {
            return Err(IndexedBinaryCodecError::InvalidFields);
        }
        let y = read_vu57(data, &mut pos).ok_or(IndexedBinaryCodecError::InvalidFields)?;
        if pos != data.len() {
            return Err(IndexedBinaryCodecError::InvalidFields);
        }
        (x, y)
    };

    let sid = clock_table
        .get(session_index as usize)
        .ok_or(IndexedBinaryCodecError::InvalidFields)?
        .sid;
    Ok(Id { sid, time })
}

fn parse_field_id(
    field: &str,
    clock_table: &[LogicalClockBase],
) -> Result<Id, IndexedBinaryCodecError> {
    let (sid_idx_s, time_s) = field
        .split_once('_')
        .ok_or(IndexedBinaryCodecError::InvalidFields)?;
    let sid_idx = from_base36(sid_idx_s).ok_or(IndexedBinaryCodecError::InvalidFields)? as usize;
    let time = from_base36(time_s).ok_or(IndexedBinaryCodecError::InvalidFields)?;
    let sid = clock_table
        .get(sid_idx)
        .ok_or(IndexedBinaryCodecError::InvalidFields)?
        .sid;
    Ok(Id { sid, time })
}

fn write_type_len(out: &mut Vec<u8>, major: u8, len: u64) {
    if len < 24 {
        out.push((major << 5) | (len as u8));
    } else if len <= 0xff {
        out.push((major << 5) | 24);
        out.push(len as u8);
    } else if len <= 0xffff {
        out.push((major << 5) | 25);
        out.push(((len >> 8) & 0xff) as u8);
        out.push((len & 0xff) as u8);
    } else {
        out.push((major << 5) | 26);
        out.push(((len >> 24) & 0xff) as u8);
        out.push(((len >> 16) & 0xff) as u8);
        out.push(((len >> 8) & 0xff) as u8);
        out.push((len & 0xff) as u8);
    }
}

fn write_cbor_uint(out: &mut Vec<u8>, major: u8, value: u64) {
    if value <= 23 {
        out.push((major << 5) | (value as u8));
    } else if value <= 0xff {
        out.push((major << 5) | 24);
        out.push(value as u8);
    } else if value <= 0xffff {
        out.push((major << 5) | 25);
        out.extend_from_slice(&(value as u16).to_be_bytes());
    } else if value <= 0xffff_ffff {
        out.push((major << 5) | 26);
        out.extend_from_slice(&(value as u32).to_be_bytes());
    } else {
        out.push((major << 5) | 27);
        out.extend_from_slice(&value.to_be_bytes());
    }
}

fn write_json_pack_str(out: &mut Vec<u8>, s: &str) {
    let max_size = s.len().saturating_mul(4);
    let utf8 = s.as_bytes();
    let len = utf8.len();
    if max_size <= 23 {
        out.push(0x60 | (len as u8));
    } else if max_size <= 0xff {
        out.push(0x78);
        out.push(len as u8);
    } else if max_size <= 0xffff {
        out.push(0x79);
        out.extend_from_slice(&(len as u16).to_be_bytes());
    } else {
        out.push(0x7a);
        out.extend_from_slice(&(len as u32).to_be_bytes());
    }
    out.extend_from_slice(utf8);
}

fn write_json_pack_any(out: &mut Vec<u8>, v: &Value) {
    match v {
        Value::Null => out.push(0xf6),
        Value::Bool(false) => out.push(0xf4),
        Value::Bool(true) => out.push(0xf5),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                if i >= 0 {
                    write_cbor_uint(out, 0, i as u64);
                } else {
                    write_cbor_uint(out, 1, (-1 - i) as u64);
                }
            } else if let Some(u) = n.as_u64() {
                write_cbor_uint(out, 0, u);
            } else {
                let f = n.as_f64().unwrap_or(0.0);
                let f32v = f as f32;
                if (f32v as f64) == f {
                    out.push(0xfa);
                    out.extend_from_slice(&f32v.to_be_bytes());
                } else {
                    out.push(0xfb);
                    out.extend_from_slice(&f.to_be_bytes());
                }
            }
        }
        Value::String(s) => write_json_pack_str(out, s),
        Value::Array(arr) => {
            write_cbor_uint(out, 4, arr.len() as u64);
            for item in arr {
                write_json_pack_any(out, item);
            }
        }
        Value::Object(map) => {
            write_cbor_uint(out, 5, map.len() as u64);
            for (k, item) in map {
                write_json_pack_str(out, k);
                write_json_pack_any(out, item);
            }
        }
    }
}

fn json_from_cbor(v: &CborValue) -> Result<Value, IndexedBinaryCodecError> {
    json_joy_json_pack::cbor_to_json(v).map_err(|_| IndexedBinaryCodecError::InvalidNode)
}

fn encode_node_payload(
    node: &RuntimeNode,
    index_by_sid: &HashMap<u64, u64>,
) -> Result<Vec<u8>, IndexedBinaryCodecError> {
    let mut out = Vec::new();
    match node {
        RuntimeNode::Con(ConCell::Json(v)) => {
            write_type_len(&mut out, 0, 0);
            write_json_pack_any(&mut out, v);
        }
        RuntimeNode::Con(ConCell::Ref(id)) => {
            write_type_len(&mut out, 0, 1);
            out.extend_from_slice(&encode_indexed_id(*id, index_by_sid)?);
        }
        RuntimeNode::Con(ConCell::Undef) => {
            write_type_len(&mut out, 0, 0);
            out.push(0xf7);
        }
        RuntimeNode::Val(child) => {
            write_type_len(&mut out, 1, 0);
            out.extend_from_slice(&encode_indexed_id(*child, index_by_sid)?);
        }
        RuntimeNode::Obj(entries) => {
            write_type_len(&mut out, 2, entries.len() as u64);
            for (k, v) in entries {
                write_json_pack_str(&mut out, k);
                out.extend_from_slice(&encode_indexed_id(*v, index_by_sid)?);
            }
        }
        RuntimeNode::Vec(elements) => {
            let len = elements.keys().max().map(|v| v + 1).unwrap_or(0);
            write_type_len(&mut out, 3, len);
            for i in 0..len {
                if let Some(id) = elements.get(&i) {
                    out.push(1);
                    out.extend_from_slice(&encode_indexed_id(*id, index_by_sid)?);
                } else {
                    out.push(0);
                }
            }
        }
        RuntimeNode::Str(atoms) => {
            let chunks = group_str_chunks(atoms);
            write_type_len(&mut out, 4, chunks.len() as u64);
            for ch in chunks {
                out.extend_from_slice(&encode_indexed_id(ch.id, index_by_sid)?);
                if let Some(text) = ch.text {
                    write_json_pack_str(&mut out, &text);
                } else {
                    write_cbor_uint(&mut out, 0, ch.span);
                }
            }
        }
        RuntimeNode::Bin(atoms) => {
            let chunks = group_bin_chunks(atoms);
            write_type_len(&mut out, 5, chunks.len() as u64);
            for ch in chunks {
                out.extend_from_slice(&encode_indexed_id(ch.id, index_by_sid)?);
                match ch.bytes {
                    Some(bytes) => {
                        write_b1vu56(&mut out, 0, ch.span);
                        out.extend_from_slice(&bytes);
                    }
                    None => write_b1vu56(&mut out, 1, ch.span),
                }
            }
        }
        RuntimeNode::Arr(atoms) => {
            let chunks = group_arr_chunks(atoms);
            write_type_len(&mut out, 6, chunks.len() as u64);
            for ch in chunks {
                out.extend_from_slice(&encode_indexed_id(ch.id, index_by_sid)?);
                match ch.values {
                    Some(values) => {
                        write_b1vu56(&mut out, 0, ch.span);
                        for v in values {
                            out.extend_from_slice(&encode_indexed_id(v, index_by_sid)?);
                        }
                    }
                    None => write_b1vu56(&mut out, 1, ch.span),
                }
            }
        }
    }
    Ok(out)
}

struct DecodeCursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> DecodeCursor<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn u8(&mut self) -> Result<u8, IndexedBinaryCodecError> {
        let b = *self
            .data
            .get(self.pos)
            .ok_or(IndexedBinaryCodecError::InvalidNode)?;
        self.pos += 1;
        Ok(b)
    }

    fn read_indexed_id(
        &mut self,
        table: &[LogicalClockBase],
    ) -> Result<Id, IndexedBinaryCodecError> {
        let first = self.u8()?;
        let (x, y) = if first <= 0x7f {
            ((first >> 4) as u64, (first & 0x0f) as u64)
        } else {
            self.pos -= 1;
            let (flag, x) = read_b1vu56(self.data, &mut self.pos)
                .ok_or(IndexedBinaryCodecError::InvalidNode)?;
            if flag != 1 {
                return Err(IndexedBinaryCodecError::InvalidNode);
            }
            let y =
                read_vu57(self.data, &mut self.pos).ok_or(IndexedBinaryCodecError::InvalidNode)?;
            (x, y)
        };
        let sid = table
            .get(x as usize)
            .ok_or(IndexedBinaryCodecError::InvalidNode)?
            .sid;
        Ok(Id { sid, time: y })
    }

    fn read_len(&mut self, minor: u8) -> Result<u64, IndexedBinaryCodecError> {
        match minor {
            0..=23 => Ok(minor as u64),
            24 => Ok(self.u8()? as u64),
            25 => {
                let a = self.u8()? as u64;
                let b = self.u8()? as u64;
                Ok((a << 8) | b)
            }
            26 => {
                let a = self.u8()? as u64;
                let b = self.u8()? as u64;
                let c = self.u8()? as u64;
                let d = self.u8()? as u64;
                Ok((a << 24) | (b << 16) | (c << 8) | d)
            }
            _ => Err(IndexedBinaryCodecError::InvalidNode),
        }
    }

    fn read_one_cbor(&mut self) -> Result<CborValue, IndexedBinaryCodecError> {
        let start = self.pos;
        let mut cursor = Cursor::new(&self.data[start..]);
        let value: CborValue = ciborium::de::from_reader(&mut cursor)
            .map_err(|_| IndexedBinaryCodecError::InvalidNode)?;
        let consumed = cursor.position() as usize;
        self.pos += consumed;
        Ok(value)
    }

    fn is_eof(&self) -> bool {
        self.pos == self.data.len()
    }
}

fn decode_node_payload(
    payload: &[u8],
    clock_table: &[LogicalClockBase],
) -> Result<RuntimeNode, IndexedBinaryCodecError> {
    let mut r = DecodeCursor::new(payload);
    let octet = r.u8()?;
    let major = octet >> 5;
    let len = r.read_len(octet & 0x1f)?;
    let node = match major {
        0 => {
            if len == 0 {
                if r.data.get(r.pos) == Some(&0xf7) {
                    r.pos += 1;
                    RuntimeNode::Con(ConCell::Undef)
                } else {
                    let cbor = r.read_one_cbor()?;
                    RuntimeNode::Con(ConCell::Json(json_from_cbor(&cbor)?))
                }
            } else {
                let id = r.read_indexed_id(clock_table)?;
                RuntimeNode::Con(ConCell::Ref(id))
            }
        }
        1 => {
            let child = r.read_indexed_id(clock_table)?;
            RuntimeNode::Val(child)
        }
        2 => {
            let mut entries = Vec::with_capacity(len as usize);
            for _ in 0..len {
                let key = match r.read_one_cbor()? {
                    CborValue::Text(s) => s,
                    _ => return Err(IndexedBinaryCodecError::InvalidNode),
                };
                let id = r.read_indexed_id(clock_table)?;
                entries.push((key, id));
            }
            RuntimeNode::Obj(entries)
        }
        3 => {
            let mut map = std::collections::BTreeMap::new();
            for i in 0..len {
                let flag = r.u8()?;
                if flag != 0 {
                    let id = r.read_indexed_id(clock_table)?;
                    map.insert(i, id);
                }
            }
            RuntimeNode::Vec(map)
        }
        4 => {
            let mut atoms = Vec::new();
            for _ in 0..len {
                let id = r.read_indexed_id(clock_table)?;
                match r.read_one_cbor()? {
                    CborValue::Text(s) => {
                        let chars: Vec<char> = s.chars().collect();
                        for (i, ch) in chars.iter().enumerate() {
                            atoms.push(StrAtom {
                                slot: Id {
                                    sid: id.sid,
                                    time: id.time + i as u64,
                                },
                                ch: Some(*ch),
                            });
                        }
                    }
                    CborValue::Integer(n) => {
                        let span =
                            u64::try_from(n).map_err(|_| IndexedBinaryCodecError::InvalidNode)?;
                        for i in 0..span {
                            atoms.push(StrAtom {
                                slot: Id {
                                    sid: id.sid,
                                    time: id.time + i,
                                },
                                ch: None,
                            });
                        }
                    }
                    _ => return Err(IndexedBinaryCodecError::InvalidNode),
                }
            }
            RuntimeNode::Str(atoms)
        }
        5 => {
            let mut atoms = Vec::new();
            for _ in 0..len {
                let id = r.read_indexed_id(clock_table)?;
                let (deleted, span) =
                    read_b1vu56(r.data, &mut r.pos).ok_or(IndexedBinaryCodecError::InvalidNode)?;
                if deleted == 1 {
                    for i in 0..span {
                        atoms.push(BinAtom {
                            slot: Id {
                                sid: id.sid,
                                time: id.time + i,
                            },
                            byte: None,
                        });
                    }
                } else {
                    for i in 0..span {
                        let b = r.u8()?;
                        atoms.push(BinAtom {
                            slot: Id {
                                sid: id.sid,
                                time: id.time + i,
                            },
                            byte: Some(b),
                        });
                    }
                }
            }
            RuntimeNode::Bin(atoms)
        }
        6 => {
            let mut atoms = Vec::new();
            for _ in 0..len {
                let id = r.read_indexed_id(clock_table)?;
                let (deleted, span) =
                    read_b1vu56(r.data, &mut r.pos).ok_or(IndexedBinaryCodecError::InvalidNode)?;
                if deleted == 1 {
                    for i in 0..span {
                        atoms.push(ArrAtom {
                            slot: Id {
                                sid: id.sid,
                                time: id.time + i,
                            },
                            value: None,
                        });
                    }
                } else {
                    for i in 0..span {
                        let value = r.read_indexed_id(clock_table)?;
                        atoms.push(ArrAtom {
                            slot: Id {
                                sid: id.sid,
                                time: id.time + i,
                            },
                            value: Some(value),
                        });
                    }
                }
            }
            RuntimeNode::Arr(atoms)
        }
        _ => return Err(IndexedBinaryCodecError::InvalidNode),
    };
    if !r.is_eof() {
        return Err(IndexedBinaryCodecError::InvalidNode);
    }
    Ok(node)
}

struct StrChunk {
    id: Id,
    span: u64,
    text: Option<String>,
}

struct BinChunk {
    id: Id,
    span: u64,
    bytes: Option<Vec<u8>>,
}

struct ArrChunk {
    id: Id,
    span: u64,
    values: Option<Vec<Id>>,
}

fn group_str_chunks(atoms: &[StrAtom]) -> Vec<StrChunk> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < atoms.len() {
        let start = &atoms[i];
        let mut j = i + 1;
        while j < atoms.len()
            && atoms[j - 1].slot.sid == atoms[j].slot.sid
            && atoms[j - 1].ch.is_some() == atoms[j].ch.is_some()
            && atoms[j].slot.time == atoms[j - 1].slot.time + 1
        {
            j += 1;
        }
        if start.ch.is_some() {
            let mut s = String::new();
            for a in &atoms[i..j] {
                if let Some(ch) = a.ch {
                    s.push(ch);
                }
            }
            out.push(StrChunk {
                id: start.slot,
                span: (j - i) as u64,
                text: Some(s),
            });
        } else {
            out.push(StrChunk {
                id: start.slot,
                span: (j - i) as u64,
                text: None,
            });
        }
        i = j;
    }
    out
}

fn group_bin_chunks(atoms: &[BinAtom]) -> Vec<BinChunk> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < atoms.len() {
        let start = &atoms[i];
        let mut j = i + 1;
        while j < atoms.len()
            && atoms[j - 1].slot.sid == atoms[j].slot.sid
            && atoms[j - 1].byte.is_some() == atoms[j].byte.is_some()
            && atoms[j].slot.time == atoms[j - 1].slot.time + 1
        {
            j += 1;
        }
        if start.byte.is_some() {
            let mut bytes = Vec::new();
            for a in &atoms[i..j] {
                if let Some(b) = a.byte {
                    bytes.push(b);
                }
            }
            out.push(BinChunk {
                id: start.slot,
                span: (j - i) as u64,
                bytes: Some(bytes),
            });
        } else {
            out.push(BinChunk {
                id: start.slot,
                span: (j - i) as u64,
                bytes: None,
            });
        }
        i = j;
    }
    out
}

fn group_arr_chunks(atoms: &[ArrAtom]) -> Vec<ArrChunk> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < atoms.len() {
        let start = &atoms[i];
        let mut j = i + 1;
        while j < atoms.len()
            && atoms[j - 1].slot.sid == atoms[j].slot.sid
            && atoms[j - 1].value.is_some() == atoms[j].value.is_some()
            && atoms[j].slot.time == atoms[j - 1].slot.time + 1
        {
            j += 1;
        }
        if start.value.is_some() {
            let mut values = Vec::new();
            for a in &atoms[i..j] {
                if let Some(v) = a.value {
                    values.push(v);
                }
            }
            out.push(ArrChunk {
                id: start.slot,
                span: (j - i) as u64,
                values: Some(values),
            });
        } else {
            out.push(ArrChunk {
                id: start.slot,
                span: (j - i) as u64,
                values: None,
            });
        }
        i = j;
    }
    out
}
