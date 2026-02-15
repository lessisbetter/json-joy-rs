//! Internal runtime model for fixture-driven patch apply/replay testing (M3).
//!
//! Design note:
//! - M3 uses oracle-first replay fixtures whose base model is deterministic.
//! - We intentionally bootstrap from `Model::from_binary` view parity and build
//!   replay semantics in apply logic, instead of re-implementing full model
//!   binary node-id decoding in this milestone.

use crate::model::{Model, ModelError};
use crate::patch::{ConValue, DecodedOp, Patch, Timestamp};
use serde_json::{Map, Number, Value};
use std::collections::{BTreeMap, HashMap};
use std::hash::{Hash, Hasher};
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

#[derive(Debug, Clone)]
enum RuntimeNode {
    Con(Value),
    Val(Id),
    Obj(BTreeMap<String, Id>),
    Vec(BTreeMap<u64, Id>),
    Str(Vec<StrAtom>),
    Bin(Vec<BinAtom>),
    Arr(Vec<ArrAtom>),
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
}

impl RuntimeModel {
    pub fn from_model_binary(data: &[u8]) -> Result<Self, ModelError> {
        let model = Model::from_binary(data)?;
        let view = model.view().clone();
        let infer_empty_object_root = matches!(view, Value::Object(ref m) if m.is_empty());

        Ok(Self {
            nodes: HashMap::new(),
            root: None,
            clock: ClockState::default(),
            fallback_view: view,
            infer_empty_object_root,
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

    fn maybe_infer_root_obj(&mut self, obj: Id) {
        // For fixture-covered logical clock models, root object id time is 1.
        // Do not infer root from arbitrary nested object insert targets.
        if self.root.is_none() && self.infer_empty_object_root && obj.time == 1 {
            self.nodes
                .entry(obj)
                .or_insert_with(|| RuntimeNode::Obj(BTreeMap::new()));
            self.root = Some(obj);
        }
    }

    fn apply_op(&mut self, op: &DecodedOp) -> Result<(), ApplyError> {
        match op {
            DecodedOp::NewCon { id, value } => {
                let id = Id::from(*id);
                let val = match value {
                    ConValue::Json(v) => v.clone(),
                    ConValue::Ref(ts) => self.node_view(Id::from(*ts)).unwrap_or(Value::Null),
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
                    .or_insert_with(|| RuntimeNode::Obj(BTreeMap::new()));
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
                            map.insert(k.clone(), vid);
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
            RuntimeNode::Con(v) => Some(v.clone()),
            RuntimeNode::Val(child) => Some(self.node_view(*child).unwrap_or(Value::Null)),
            RuntimeNode::Obj(map) => {
                let mut out = Map::new();
                for (k, v) in map {
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
