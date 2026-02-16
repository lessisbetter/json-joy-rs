use std::collections::BTreeMap;

use crate::patch::{ConValue, DecodedOp};

use super::rga::{find_insert_index_arr, find_insert_index_bin, find_insert_index_str};
use super::types::{cmp_id_time_sid, ArrAtom, BinAtom, ConCell, Id, RuntimeNode, StrAtom};
use super::{ApplyError, RuntimeModel};

impl RuntimeModel {
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

    // Ported from upstream Model._gcTree behavior: when a container/register
    // is overwritten or an array value is deleted, recursively drop the old
    // subtree from the runtime index.
    fn gc_tree(&mut self, value: Id) {
        if value.sid == 0 && value.time == 0 {
            return;
        }
        let Some(node) = self.nodes.remove(&value) else {
            return;
        };
        match node {
            RuntimeNode::Con(_) => {}
            RuntimeNode::Val(child) => self.gc_tree(child),
            RuntimeNode::Obj(entries) => {
                for (_, child) in entries {
                    self.gc_tree(child);
                }
            }
            RuntimeNode::Vec(map) => {
                for (_, child) in map {
                    self.gc_tree(child);
                }
            }
            RuntimeNode::Str(_) => {}
            RuntimeNode::Bin(_) => {}
            RuntimeNode::Arr(atoms) => {
                for atom in atoms {
                    if let Some(child) = atom.value {
                        self.gc_tree(child);
                    }
                }
            }
        }
    }

    pub(super) fn apply_op(&mut self, op: &DecodedOp) -> Result<(), ApplyError> {
        match op {
            DecodedOp::NewCon { id, value } => {
                let id = Id::from(*id);
                let val = match value {
                    ConValue::Json(v) => ConCell::Json(v.clone()),
                    ConValue::Ref(ts) => ConCell::Ref(Id::from(*ts)),
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
                        let old = self.root;
                        let can_set = match old {
                            Some(current) => cmp_id_time_sid(val, current).is_gt(),
                            None => true,
                        };
                        if can_set {
                            self.root = Some(val);
                            if let Some(old) = old {
                                if old != val {
                                    self.gc_tree(old);
                                }
                            }
                        }
                    }
                } else if let Some(RuntimeNode::Val(child)) = self.nodes.get_mut(&obj) {
                    if has_val {
                        let old = *child;
                        // Port of upstream ValNode.set semantics:
                        // - ignore if new <= current and current is non-system
                        // - ignore if new <= register id
                        let current_non_system = old.sid != 0;
                        if current_non_system && !cmp_id_time_sid(val, old).is_gt() {
                            return Ok(());
                        }
                        if !cmp_id_time_sid(val, obj).is_gt() {
                            return Ok(());
                        }
                        *child = val;
                        if old != val {
                            self.gc_tree(old);
                        }
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
                let mut gc = Vec::new();
                if let Some(RuntimeNode::Obj(map)) = self.nodes.get_mut(&obj) {
                    for (k, vid) in data.iter().map(|(k, v)| (k, Id::from(*v))) {
                        if existing_ids.contains(&vid) && obj.time < vid.time {
                            if let Some((_, v)) = map.iter_mut().find(|(existing, _)| existing == k)
                            {
                                let old = *v;
                                if cmp_id_time_sid(old, vid).is_lt() {
                                    *v = vid;
                                    gc.push(old);
                                }
                            } else {
                                map.push((k.clone(), vid));
                            }
                        }
                    }
                }
                for id in gc {
                    self.gc_tree(id);
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
                let mut gc = Vec::new();
                if let Some(RuntimeNode::Vec(map)) = self.nodes.get_mut(&obj) {
                    for (idx, v) in data {
                        let vid = Id::from(*v);
                        if existing_ids.contains(&vid) && obj.time < vid.time {
                            if let Some(old) = map.get(idx).copied() {
                                if cmp_id_time_sid(old, vid).is_lt() {
                                    let old = map.insert(*idx, vid).unwrap_or(old);
                                    gc.push(old);
                                }
                            } else if let Some(old) = map.insert(*idx, vid) {
                                gc.push(old);
                            }
                        }
                    }
                }
                for id in gc {
                    self.gc_tree(id);
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
                    let insert_id = Id {
                        sid: id.sid,
                        time: id.time,
                    };
                    let Some(idx) =
                        find_insert_index_str(atoms, Id::from(*reference), obj, insert_id)
                    else {
                        return Ok(());
                    };
                    let mut inserted = Vec::new();
                    for (i, ch) in data.chars().enumerate() {
                        let slot = Id {
                            sid: id.sid,
                            time: id.time + i as u64,
                        };
                        if atoms.iter().any(|a| a.slot == slot) {
                            continue;
                        }
                        inserted.push(StrAtom { slot, ch: Some(ch) });
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
                    let insert_id = Id {
                        sid: id.sid,
                        time: id.time,
                    };
                    let Some(idx) =
                        find_insert_index_bin(atoms, Id::from(*reference), obj, insert_id)
                    else {
                        return Ok(());
                    };
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
                    let insert_id = Id {
                        sid: id.sid,
                        time: id.time,
                    };
                    let Some(idx) =
                        find_insert_index_arr(atoms, Id::from(*reference), obj, insert_id)
                    else {
                        return Ok(());
                    };
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
                        let old = atom.value;
                        if let Some(old) = old {
                            if cmp_id_time_sid(old, val).is_lt() {
                                atom.value = Some(val);
                                self.gc_tree(old);
                            }
                        } else {
                            atom.value = Some(val);
                        }
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
                            let mut gc = Vec::new();
                            for span in what {
                                for t in span.time..span.time + span.span {
                                    if let Some(a) = atoms
                                        .iter_mut()
                                        .find(|a| a.slot.sid == span.sid && a.slot.time == t)
                                    {
                                        if let Some(old) = a.value {
                                            gc.push(old);
                                        }
                                        a.value = None;
                                    }
                                }
                            }
                            for id in gc {
                                self.gc_tree(id);
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
}
