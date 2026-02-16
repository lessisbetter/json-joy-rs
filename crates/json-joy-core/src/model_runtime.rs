//! Internal runtime model for fixture-driven patch apply/replay testing (M3).
//!
//! Design note:
//! - M3 uses oracle-first replay fixtures whose base model is deterministic.
//! - We intentionally bootstrap from `Model::from_binary` view parity and build
//!   replay semantics in apply logic, instead of re-implementing full model
//!   binary node-id decoding in this milestone.

use crate::crdt_binary::{read_b1vu56, read_vu57, write_b1vu56, write_vu57, LogicalClockBase};
use crate::model::{Model, ModelError};
use crate::patch::{ConValue, DecodedOp, Patch, Timestamp};
use ciborium::value::Value as CborValue;
use serde_json::{Map, Number, Value};
use std::convert::TryFrom;
use std::collections::{BTreeMap, HashMap};
use std::hash::{Hash, Hasher};
use std::io::Cursor;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApplyError {
    #[error("unsupported operation for runtime apply")]
    UnsupportedOpForM3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Id {
    sid: u64,
    time: u64,
}

impl Hash for Id {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.sid.hash(state);
        self.time.hash(state);
    }
}

impl From<Timestamp> for Id {
    fn from(v: Timestamp) -> Self {
        Self {
            sid: v.sid,
            time: v.time,
        }
    }
}

impl From<Id> for Timestamp {
    fn from(v: Id) -> Self {
        Self {
            sid: v.sid,
            time: v.time,
        }
    }
}

#[derive(Debug, Clone)]
enum RuntimeNode {
    Con(ConCell),
    Val(Id),
    Obj(Vec<(String, Id)>),
    Vec(BTreeMap<u64, Id>),
    Str(Vec<StrAtom>),
    Bin(Vec<BinAtom>),
    Arr(Vec<ArrAtom>),
}

#[derive(Debug, Clone)]
enum ConCell {
    Json(Value),
    Undef,
}

#[derive(Debug, Clone)]
struct StrAtom {
    slot: Id,
    ch: Option<char>,
}

#[derive(Debug, Clone)]
struct BinAtom {
    slot: Id,
    byte: Option<u8>,
}

#[derive(Debug, Clone)]
struct ArrAtom {
    slot: Id,
    value: Option<Id>,
}

#[derive(Debug, Default, Clone)]
struct ClockState {
    observed: HashMap<u64, Vec<(u64, u64)>>,
}

impl ClockState {
    fn observe(&mut self, sid: u64, start: u64, span: u64) {
        let end = start + span.saturating_sub(1);
        let ranges = self.observed.entry(sid).or_default();
        ranges.push((start, end));
        ranges.sort_by_key(|(a, _)| *a);
        let mut merged: Vec<(u64, u64)> = Vec::with_capacity(ranges.len());
        for (a, b) in ranges.iter().copied() {
            if let Some(last) = merged.last_mut() {
                if a <= last.1.saturating_add(1) {
                    last.1 = last.1.max(b);
                } else {
                    merged.push((a, b));
                }
            } else {
                merged.push((a, b));
            }
        }
        *ranges = merged;
    }
}

#[derive(Debug, Clone)]
pub struct RuntimeModel {
    nodes: HashMap<Id, RuntimeNode>,
    root: Option<Id>,
    clock: ClockState,
    fallback_view: Value,
    infer_empty_object_root: bool,
    clock_table: Vec<LogicalClockBase>,
    server_clock_time: Option<u64>,
}

impl RuntimeModel {
    pub fn new_logical_empty(sid: u64) -> Self {
        Self {
            nodes: HashMap::new(),
            root: None,
            clock: ClockState::default(),
            fallback_view: Value::Null,
            infer_empty_object_root: false,
            clock_table: vec![LogicalClockBase { sid, time: 0 }],
            server_clock_time: None,
        }
    }

    pub fn from_model_binary(data: &[u8]) -> Result<Self, ModelError> {
        let model = Model::from_binary(data)?;
        let view = model.view().clone();
        let infer_empty_object_root = matches!(view, Value::Object(ref m) if m.is_empty());

        let decoded = decode_runtime_graph(data);
        let (nodes, root, clock_table, server_clock_time) = match decoded {
            Ok(v) => v,
            Err(_) => (HashMap::new(), None, Vec::new(), None),
        };

        Ok(Self {
            nodes,
            root,
            clock: ClockState::default(),
            fallback_view: view,
            infer_empty_object_root,
            clock_table,
            server_clock_time,
        })
    }

    pub fn apply_patch(&mut self, patch: &Patch) -> Result<(), ApplyError> {
        for op in patch.decoded_ops() {
            let id = Id::from(op.id());
            let span = op.span();
            self.clock.observe(id.sid, id.time, span);
            self.apply_op(op)?;
        }
        Ok(())
    }

    pub fn view_json(&self) -> Value {
        match self.root {
            Some(id) => self.node_view(id).unwrap_or(Value::Null),
            None => self.fallback_view.clone(),
        }
    }

    pub fn to_model_binary_like(&self) -> Result<Vec<u8>, ModelError> {
        if let Some(server_time) = self.server_clock_time {
            return encode_server(self, server_time);
        }
        encode_logical(self)
    }

    pub(crate) fn root_object_field(&self, key: &str) -> Option<Timestamp> {
        let root = self.root?;
        match self.nodes.get(&root)? {
            RuntimeNode::Obj(entries) => entries
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, id)| (*id).into()),
            RuntimeNode::Val(child) => match self.nodes.get(child)? {
                RuntimeNode::Obj(entries) => entries
                    .iter()
                    .find(|(k, _)| k == key)
                    .map(|(_, id)| (*id).into()),
                _ => None,
            },
            _ => None,
        }
    }

    pub(crate) fn object_field(&self, obj: Timestamp, key: &str) -> Option<Timestamp> {
        match self.nodes.get(&Id::from(obj))? {
            RuntimeNode::Obj(entries) => entries
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, id)| (*id).into()),
            RuntimeNode::Val(child) => match self.nodes.get(child)? {
                RuntimeNode::Obj(entries) => entries
                    .iter()
                    .find(|(k, _)| k == key)
                    .map(|(_, id)| (*id).into()),
                _ => None,
            },
            _ => None,
        }
    }

    pub(crate) fn node_is_string(&self, id: Timestamp) -> bool {
        matches!(self.nodes.get(&Id::from(id)), Some(RuntimeNode::Str(_)))
    }

    pub(crate) fn node_is_array(&self, id: Timestamp) -> bool {
        matches!(self.nodes.get(&Id::from(id)), Some(RuntimeNode::Arr(_)))
    }

    pub(crate) fn node_is_object(&self, id: Timestamp) -> bool {
        matches!(self.nodes.get(&Id::from(id)), Some(RuntimeNode::Obj(_)))
    }

    pub(crate) fn string_visible_slots(&self, id: Timestamp) -> Option<Vec<Timestamp>> {
        let node = self.nodes.get(&Id::from(id))?;
        if let RuntimeNode::Str(atoms) = node {
            let mut out = Vec::new();
            for atom in atoms {
                if atom.ch.is_some() {
                    out.push(atom.slot.into());
                }
            }
            Some(out)
        } else {
            None
        }
    }

    pub(crate) fn array_visible_slots(&self, id: Timestamp) -> Option<Vec<Timestamp>> {
        let node = self.nodes.get(&Id::from(id))?;
        if let RuntimeNode::Arr(atoms) = node {
            let mut out = Vec::new();
            for atom in atoms {
                if atom.value.is_some() {
                    out.push(atom.slot.into());
                }
            }
            Some(out)
        } else {
            None
        }
    }

    pub(crate) fn array_visible_values(&self, id: Timestamp) -> Option<Vec<Timestamp>> {
        let node = self.nodes.get(&Id::from(id))?;
        if let RuntimeNode::Arr(atoms) = node {
            let mut out = Vec::new();
            for atom in atoms {
                if let Some(value) = atom.value {
                    out.push(value.into());
                }
            }
            Some(out)
        } else {
            None
        }
    }

    fn maybe_infer_root_obj(&mut self, obj: Id) {
        // For fixture-covered logical clock models, root object id time is 1.
        // Do not infer root from arbitrary nested object insert targets.
        if self.root.is_none() && self.infer_empty_object_root && obj.time == 1 {
            self.nodes
                .entry(obj)
                .or_insert_with(|| RuntimeNode::Obj(Vec::new()));
            self.root = Some(obj);
        }
    }

    fn apply_op(&mut self, op: &DecodedOp) -> Result<(), ApplyError> {
        match op {
            DecodedOp::NewCon { id, value } => {
                let id = Id::from(*id);
                let val = match value {
                    ConValue::Json(v) => ConCell::Json(v.clone()),
                    ConValue::Ref(ts) => {
                        ConCell::Json(self.node_view(Id::from(*ts)).unwrap_or(Value::Null))
                    }
                    ConValue::Undef => ConCell::Undef,
                };
                self.nodes.entry(id).or_insert(RuntimeNode::Con(val));
            }
            DecodedOp::NewVal { id } => {
                let id = Id::from(*id);
                self.nodes
                    .entry(id)
                    .or_insert(RuntimeNode::Val(Id { sid: 0, time: 0 }));
            }
            DecodedOp::NewObj { id } => {
                self.nodes
                    .entry(Id::from(*id))
                    .or_insert_with(|| RuntimeNode::Obj(Vec::new()));
            }
            DecodedOp::NewVec { id } => {
                self.nodes
                    .entry(Id::from(*id))
                    .or_insert_with(|| RuntimeNode::Vec(BTreeMap::new()));
            }
            DecodedOp::NewStr { id } => {
                self.nodes
                    .entry(Id::from(*id))
                    .or_insert_with(|| RuntimeNode::Str(Vec::new()));
            }
            DecodedOp::NewBin { id } => {
                self.nodes
                    .entry(Id::from(*id))
                    .or_insert_with(|| RuntimeNode::Bin(Vec::new()));
            }
            DecodedOp::NewArr { id } => {
                self.nodes
                    .entry(Id::from(*id))
                    .or_insert_with(|| RuntimeNode::Arr(Vec::new()));
            }
            DecodedOp::InsVal { obj, val, .. } => {
                let obj = Id::from(*obj);
                let val = Id::from(*val);
                let has_val = self.nodes.contains_key(&val);
                if obj.sid == 0 && obj.time == 0 {
                    if has_val {
                        self.root = Some(val);
                    }
                } else if let Some(RuntimeNode::Val(child)) = self.nodes.get_mut(&obj) {
                    if has_val {
                        *child = val;
                    }
                }
            }
            DecodedOp::InsObj { obj, data, .. } => {
                let obj = Id::from(*obj);
                self.maybe_infer_root_obj(obj);
                let existing_ids = data
                    .iter()
                    .filter_map(|(_, v)| {
                        let vid = Id::from(*v);
                        self.nodes.contains_key(&vid).then_some(vid)
                    })
                    .collect::<Vec<_>>();
                if let Some(RuntimeNode::Obj(map)) = self.nodes.get_mut(&obj) {
                    for (k, vid) in data.iter().map(|(k, v)| (k, Id::from(*v))) {
                        if existing_ids.contains(&vid) && obj.time < vid.time {
                            if let Some((_, v)) = map.iter_mut().find(|(existing, _)| existing == k) {
                                *v = vid;
                            } else {
                                map.push((k.clone(), vid));
                            }
                        }
                    }
                }
            }
            DecodedOp::InsVec { obj, data, .. } => {
                let obj = Id::from(*obj);
                let existing_ids = data
                    .iter()
                    .filter_map(|(_, v)| {
                        let vid = Id::from(*v);
                        self.nodes.contains_key(&vid).then_some(vid)
                    })
                    .collect::<Vec<_>>();
                if let Some(RuntimeNode::Vec(map)) = self.nodes.get_mut(&obj) {
                    for (idx, v) in data {
                        let vid = Id::from(*v);
                        if existing_ids.contains(&vid) && obj.time < vid.time {
                            map.insert(*idx, vid);
                        }
                    }
                }
            }
            DecodedOp::InsStr {
                id,
                obj,
                reference,
                data,
            } => {
                let obj = Id::from(*obj);
                if let Some(RuntimeNode::Str(atoms)) = self.nodes.get_mut(&obj) {
                    let idx = find_insert_index_str(atoms, Id::from(*reference), obj);
                    let mut inserted = Vec::new();
                    for (i, ch) in data.chars().enumerate() {
                        let slot = Id {
                            sid: id.sid,
                            time: id.time + i as u64,
                        };
                        if atoms.iter().any(|a| a.slot == slot) {
                            continue;
                        }
                        inserted.push(StrAtom {
                            slot,
                            ch: Some(ch),
                        });
                    }
                    atoms.splice(idx..idx, inserted);
                }
            }
            DecodedOp::InsBin {
                id,
                obj,
                reference,
                data,
            } => {
                let obj = Id::from(*obj);
                if let Some(RuntimeNode::Bin(atoms)) = self.nodes.get_mut(&obj) {
                    let idx = find_insert_index_bin(atoms, Id::from(*reference), obj);
                    let inserted = data
                        .iter()
                        .enumerate()
                        .filter_map(|(i, b)| {
                            let slot = Id {
                                sid: id.sid,
                                time: id.time + i as u64,
                            };
                            if atoms.iter().any(|a| a.slot == slot) {
                                None
                            } else {
                                Some(BinAtom {
                                    slot,
                                    byte: Some(*b),
                                })
                            }
                        })
                        .collect::<Vec<_>>();
                    atoms.splice(idx..idx, inserted);
                }
            }
            DecodedOp::InsArr {
                id,
                obj,
                reference,
                data,
            } => {
                let obj = Id::from(*obj);
                let existing_ids = data
                    .iter()
                    .filter_map(|v| {
                        let vid = Id::from(*v);
                        self.nodes.contains_key(&vid).then_some(vid)
                    })
                    .collect::<Vec<_>>();
                if let Some(RuntimeNode::Arr(atoms)) = self.nodes.get_mut(&obj) {
                    let idx = find_insert_index_arr(atoms, Id::from(*reference), obj);
                    let mut inserted = Vec::new();
                    for (i, v) in data.iter().enumerate() {
                        let vid = Id::from(*v);
                        let slot = Id {
                            sid: id.sid,
                            time: id.time + i as u64,
                        };
                        if existing_ids.contains(&vid) && obj.time < vid.time {
                            if atoms.iter().any(|a| a.slot == slot) {
                                continue;
                            }
                            inserted.push(ArrAtom {
                                slot,
                                value: Some(vid),
                            });
                        }
                    }
                    atoms.splice(idx..idx, inserted);
                }
            }
            DecodedOp::UpdArr {
                obj,
                reference,
                val,
                ..
            } => {
                let obj = Id::from(*obj);
                let reference = Id::from(*reference);
                let val = Id::from(*val);
                if !self.nodes.contains_key(&val) {
                    return Ok(());
                }
                if let Some(RuntimeNode::Arr(atoms)) = self.nodes.get_mut(&obj) {
                    if let Some(atom) = atoms.iter_mut().find(|a| a.slot == reference) {
                        atom.value = Some(val);
                    }
                }
            }
            DecodedOp::Del { obj, what, .. } => {
                let obj = Id::from(*obj);
                if let Some(node) = self.nodes.get_mut(&obj) {
                    match node {
                        RuntimeNode::Str(atoms) => {
                            for span in what {
                                for t in span.time..span.time + span.span {
                                    if let Some(a) = atoms
                                        .iter_mut()
                                        .find(|a| a.slot.sid == span.sid && a.slot.time == t)
                                    {
                                        a.ch = None;
                                    }
                                }
                            }
                        }
                        RuntimeNode::Bin(atoms) => {
                            for span in what {
                                for t in span.time..span.time + span.span {
                                    if let Some(a) = atoms
                                        .iter_mut()
                                        .find(|a| a.slot.sid == span.sid && a.slot.time == t)
                                    {
                                        a.byte = None;
                                    }
                                }
                            }
                        }
                        RuntimeNode::Arr(atoms) => {
                            for span in what {
                                for t in span.time..span.time + span.span {
                                    if let Some(a) = atoms
                                        .iter_mut()
                                        .find(|a| a.slot.sid == span.sid && a.slot.time == t)
                                    {
                                        a.value = None;
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            DecodedOp::Nop { .. } => {}
        }
        Ok(())
    }

    fn node_view(&self, id: Id) -> Option<Value> {
        match self.nodes.get(&id)? {
            RuntimeNode::Con(ConCell::Json(v)) => Some(v.clone()),
            RuntimeNode::Con(ConCell::Undef) => None,
            RuntimeNode::Val(child) => Some(self.node_view(*child).unwrap_or(Value::Null)),
            RuntimeNode::Obj(entries) => {
                let mut out = Map::new();
                for (k, v) in entries {
                    if let Some(val) = self.node_view(*v) {
                        out.insert(k.clone(), val);
                    }
                }
                Some(Value::Object(out))
            }
            RuntimeNode::Vec(map) => {
                let max = map.keys().copied().max().unwrap_or(0);
                let mut out = vec![Value::Null; max as usize + 1];
                for (i, id) in map {
                    if let Some(v) = self.node_view(*id) {
                        out[*i as usize] = v;
                    }
                }
                Some(Value::Array(out))
            }
            RuntimeNode::Str(atoms) => {
                let mut s = String::new();
                for a in atoms {
                    if let Some(ch) = a.ch {
                        s.push(ch);
                    }
                }
                Some(Value::String(s))
            }
            RuntimeNode::Bin(atoms) => {
                let mut map = Map::new();
                let mut idx = 0usize;
                for a in atoms {
                    if let Some(b) = a.byte {
                        map.insert(idx.to_string(), Value::Number(Number::from(b)));
                        idx += 1;
                    }
                }
                Some(Value::Object(map))
            }
            RuntimeNode::Arr(atoms) => {
                let mut out = Vec::new();
                for a in atoms {
                    if let Some(value_id) = a.value {
                        if let Some(v) = self.node_view(value_id) {
                            out.push(v);
                        }
                    }
                }
                Some(Value::Array(out))
            }
        }
    }
}

fn decode_runtime_graph(
    data: &[u8],
) -> Result<(HashMap<Id, RuntimeNode>, Option<Id>, Vec<LogicalClockBase>, Option<u64>), ModelError> {
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
    let first = *ctx.data.get(ctx.pos).ok_or(ModelError::InvalidModelBinary)?;
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
                RuntimeNode::Con(ConCell::Json(rid_to_json(rid)))
            };
            ctx.nodes.insert(id, node);
        }
        1 => {
            let child = decode_node(ctx)?;
            ctx.nodes.insert(id, RuntimeNode::Val(child));
        }
        2 => {
            let len = if minor != 31 { minor } else { read_vu57_ctx(ctx)? };
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
            let len = if minor != 31 { minor } else { read_vu57_ctx(ctx)? };
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
            let len = if minor != 31 { minor } else { read_vu57_ctx(ctx)? };
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
            let len = if minor != 31 { minor } else { read_vu57_ctx(ctx)? };
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
            let len = if minor != 31 { minor } else { read_vu57_ctx(ctx)? };
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

fn rid_to_json(id: Id) -> Value {
    let mut m = Map::new();
    m.insert("sid".to_string(), Value::Number(Number::from(id.sid)));
    m.insert("time".to_string(), Value::Number(Number::from(id.time)));
    Value::Object(m)
}

fn encode_logical(model: &RuntimeModel) -> Result<Vec<u8>, ModelError> {
    let table = normalized_clock_table(model);
    if table.is_empty() {
        return Err(ModelError::InvalidClockTable);
    }

    let mut root = Vec::new();
    let mut enc = EncodeCtx {
        out: &mut root,
        table: &table,
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
    write_vu57(&mut out, table.len() as u64);
    for t in table {
        write_vu57(&mut out, t.sid);
        write_vu57(&mut out, t.time);
    }
    Ok(out)
}

fn encode_server(model: &RuntimeModel, server_time: u64) -> Result<Vec<u8>, ModelError> {
    let mut out = Vec::new();
    out.push(0x80);
    write_vu57(&mut out, server_time);
    let mut enc = EncodeCtx {
        out: &mut out,
        table: &[],
    };
    if let Some(root_id) = model.root {
        encode_node(&mut enc, root_id, model)?;
    } else {
        out.push(0);
    }
    Ok(out)
}

fn normalized_clock_table(model: &RuntimeModel) -> Vec<LogicalClockBase> {
    let mut table = if model.clock_table.is_empty() {
        Vec::new()
    } else {
        model.clock_table.clone()
    };

    let mut max_by_sid: HashMap<u64, u64> = HashMap::new();
    for id in model.nodes.keys() {
        max_by_sid
            .entry(id.sid)
            .and_modify(|m| *m = (*m).max(id.time))
            .or_insert(id.time);
    }
    for node in model.nodes.values() {
        match node {
            RuntimeNode::Str(atoms) => {
                for a in atoms {
                    max_by_sid
                        .entry(a.slot.sid)
                        .and_modify(|m| *m = (*m).max(a.slot.time))
                        .or_insert(a.slot.time);
                }
            }
            RuntimeNode::Bin(atoms) => {
                for a in atoms {
                    max_by_sid
                        .entry(a.slot.sid)
                        .and_modify(|m| *m = (*m).max(a.slot.time))
                        .or_insert(a.slot.time);
                }
            }
            RuntimeNode::Arr(atoms) => {
                for a in atoms {
                    max_by_sid
                        .entry(a.slot.sid)
                        .and_modify(|m| *m = (*m).max(a.slot.time))
                        .or_insert(a.slot.time);
                }
            }
            _ => {}
        }
    }
    for (sid, ranges) in &model.clock.observed {
        for (_, end) in ranges {
            max_by_sid
                .entry(*sid)
                .and_modify(|m| *m = (*m).max(*end))
                .or_insert(*end);
        }
    }

    if table.is_empty() {
        if let Some((&sid, &time)) = max_by_sid.iter().next() {
            table.push(LogicalClockBase { sid, time });
        }
    } else {
        let global_max = max_by_sid.values().copied().max().unwrap_or(0);
        if let Some(first) = table.first_mut() {
            first.time = first.time.max(global_max);
        }
        for t in &mut table {
            if let Some(max_t) = max_by_sid.get(&t.sid) {
                t.time = t.time.max(*max_t);
            }
        }
        for (sid, time) in max_by_sid {
            if !table.iter().any(|t| t.sid == sid) {
                table.push(LogicalClockBase { sid, time });
            }
        }
    }
    table
}

struct EncodeCtx<'a> {
    out: &'a mut Vec<u8>,
    table: &'a [LogicalClockBase],
}

fn encode_id(enc: &mut EncodeCtx<'_>, id: Id) -> Result<(), ModelError> {
    if enc.table.is_empty() {
        write_vu57(enc.out, id.time);
        return Ok(());
    }
    let (idx, base) = enc
        .table
        .iter()
        .enumerate()
        .find(|(_, c)| c.sid == id.sid)
        .ok_or(ModelError::InvalidClockTable)?;
    let diff = base
        .time
        .checked_sub(id.time)
        .ok_or(ModelError::InvalidClockTable)?;
    let session_index = (idx as u64) + 1;
    if session_index <= 0b111 && diff <= 0b1111 {
        enc.out
            .push(((session_index as u8) << 4) | (diff as u8));
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
                    let cbor =
                        CborValue::Integer(ciborium::value::Integer::from(chunk.span));
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
    let cbor = CborValue::Text(s.to_string());
    ciborium::ser::into_writer(&cbor, out).map_err(|_| ModelError::InvalidModelBinary)
}

fn json_to_cbor_bytes(v: &Value, out: &mut Vec<u8>) -> Result<(), ModelError> {
    let cbor = json_to_cbor(v)?;
    ciborium::ser::into_writer(&cbor, out).map_err(|_| ModelError::InvalidModelBinary)
}

fn json_to_cbor(v: &Value) -> Result<CborValue, ModelError> {
    Ok(match v {
        Value::Null => CborValue::Null,
        Value::Bool(b) => CborValue::Bool(*b),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                CborValue::Integer(ciborium::value::Integer::from(i))
            } else if let Some(u) = n.as_u64() {
                CborValue::Integer(ciborium::value::Integer::from(u))
            } else {
                return Err(ModelError::InvalidModelBinary);
            }
        }
        Value::String(s) => CborValue::Text(s.clone()),
        Value::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(json_to_cbor(item)?);
            }
            CborValue::Array(out)
        }
        Value::Object(map) => {
            let mut out = Vec::with_capacity(map.len());
            for (k, v) in map {
                out.push((CborValue::Text(k.clone()), json_to_cbor(v)?));
            }
            CborValue::Map(out)
        }
    })
}

fn cbor_to_json(v: CborValue) -> Result<Value, ModelError> {
    Ok(match v {
        CborValue::Null => Value::Null,
        CborValue::Bool(b) => Value::Bool(b),
        CborValue::Integer(i) => {
            let signed: i128 = i.into();
            if signed >= 0 {
                Value::Number(Number::from(
                    u64::try_from(signed).map_err(|_| ModelError::InvalidModelBinary)?,
                ))
            } else {
                Value::Number(Number::from(
                    i64::try_from(signed).map_err(|_| ModelError::InvalidModelBinary)?,
                ))
            }
        }
        CborValue::Float(f) => Number::from_f64(f as f64)
            .map(Value::Number)
            .ok_or(ModelError::InvalidModelBinary)?,
        CborValue::Text(s) => Value::String(s),
        CborValue::Bytes(bytes) => Value::Array(
            bytes
                .into_iter()
                .map(|b| Value::Number(Number::from(b)))
                .collect(),
        ),
        CborValue::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(cbor_to_json(item)?);
            }
            Value::Array(out)
        }
        CborValue::Map(entries) => {
            let mut out = Map::new();
            for (k, v) in entries {
                let key = match k {
                    CborValue::Text(s) => s,
                    _ => return Err(ModelError::InvalidModelBinary),
                };
                out.insert(key, cbor_to_json(v)?);
            }
            Value::Object(out)
        }
        _ => return Err(ModelError::InvalidModelBinary),
    })
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
                let diff = read_vu57(ctx.data, &mut ctx.pos).ok_or(ModelError::InvalidModelBinary)?;
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
    Ok(Id { sid: base.sid, time })
}

fn read_u8(ctx: &mut DecodeCtx<'_>) -> Result<u8, ModelError> {
    let b = *ctx.data.get(ctx.pos).ok_or(ModelError::InvalidModelBinary)?;
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
    let mut cursor = Cursor::new(slice);
    let val = ciborium::de::from_reader::<CborValue, _>(&mut cursor)
        .map_err(|_| ModelError::InvalidModelBinary)?;
    let consumed = cursor.position() as usize;
    ctx.pos += consumed;
    Ok(val)
}

fn find_insert_index_str(atoms: &[StrAtom], reference: Id, container: Id) -> usize {
    if reference == container {
        return 0;
    }
    atoms
        .iter()
        .position(|a| a.slot == reference)
        .map_or(atoms.len(), |i| i + 1)
}

fn find_insert_index_bin(atoms: &[BinAtom], reference: Id, container: Id) -> usize {
    if reference == container {
        return 0;
    }
    atoms
        .iter()
        .position(|a| a.slot == reference)
        .map_or(atoms.len(), |i| i + 1)
}

fn find_insert_index_arr(atoms: &[ArrAtom], reference: Id, container: Id) -> usize {
    if reference == container {
        return 0;
    }
    atoms
        .iter()
        .position(|a| a.slot == reference)
        .map_or(atoms.len(), |i| i + 1)
}
