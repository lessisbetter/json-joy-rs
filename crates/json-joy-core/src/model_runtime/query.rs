use crate::patch::{Timespan, Timestamp};
use serde_json::Value;

use super::types::{ConCell, Id, RuntimeNode};
use super::RuntimeModel;

impl RuntimeModel {
    pub(crate) fn root_object_field(&self, key: &str) -> Option<Timestamp> {
        let root = self.root?;
        match self.nodes.get(&root)? {
            RuntimeNode::Obj(entries) => entries
                .iter()
                .rev()
                .find(|(k, _)| k == key)
                .map(|(_, id)| (*id).into()),
            RuntimeNode::Val(child) => match self.nodes.get(child)? {
                RuntimeNode::Obj(entries) => entries
                    .iter()
                    .rev()
                    .find(|(k, _)| k == key)
                    .map(|(_, id)| (*id).into()),
                _ => None,
            },
            _ => None,
        }
    }

    pub(crate) fn root_id(&self) -> Option<Timestamp> {
        self.root.map(Into::into)
    }

    pub(crate) fn object_field(&self, obj: Timestamp, key: &str) -> Option<Timestamp> {
        match self.nodes.get(&Id::from(obj))? {
            RuntimeNode::Obj(entries) => entries
                .iter()
                .rev()
                .find(|(k, _)| k == key)
                .map(|(_, id)| (*id).into()),
            RuntimeNode::Val(child) => match self.nodes.get(child)? {
                RuntimeNode::Obj(entries) => entries
                    .iter()
                    .rev()
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

    pub(crate) fn node_is_bin(&self, id: Timestamp) -> bool {
        matches!(self.nodes.get(&Id::from(id)), Some(RuntimeNode::Bin(_)))
    }

    pub(crate) fn node_is_object(&self, id: Timestamp) -> bool {
        matches!(self.nodes.get(&Id::from(id)), Some(RuntimeNode::Obj(_)))
    }

    pub(crate) fn node_is_vec(&self, id: Timestamp) -> bool {
        matches!(self.nodes.get(&Id::from(id)), Some(RuntimeNode::Vec(_)))
    }

    pub(crate) fn node_is_val(&self, id: Timestamp) -> bool {
        matches!(self.nodes.get(&Id::from(id)), Some(RuntimeNode::Val(_)))
    }

    pub(crate) fn val_child(&self, id: Timestamp) -> Option<Timestamp> {
        match self.nodes.get(&Id::from(id))? {
            RuntimeNode::Val(child) => Some((*child).into()),
            _ => None,
        }
    }

    pub(crate) fn resolve_string_node(&self, id: Timestamp) -> Option<Timestamp> {
        if self.node_is_string(id) {
            return Some(id);
        }
        let child = self.val_child(id)?;
        self.node_is_string(child).then_some(child)
    }

    pub(crate) fn find_string_node_by_value(&self, expected: &str) -> Option<Timestamp> {
        let mut found: Option<Id> = None;
        for (id, node) in &self.nodes {
            let RuntimeNode::Str(atoms) = node else {
                continue;
            };
            let mut s = String::new();
            for atom in atoms {
                if let Some(ch) = atom.ch {
                    s.push(ch);
                }
            }
            if s == expected {
                if found.is_some() {
                    return None;
                }
                found = Some(*id);
            }
        }
        found.map(Into::into)
    }

    pub(crate) fn resolve_bin_node(&self, id: Timestamp) -> Option<Timestamp> {
        if self.node_is_bin(id) {
            return Some(id);
        }
        let child = self.val_child(id)?;
        self.node_is_bin(child).then_some(child)
    }

    pub(crate) fn resolve_array_node(&self, id: Timestamp) -> Option<Timestamp> {
        if self.node_is_array(id) {
            return Some(id);
        }
        let child = self.val_child(id)?;
        self.node_is_array(child).then_some(child)
    }

    pub(crate) fn resolve_vec_node(&self, id: Timestamp) -> Option<Timestamp> {
        if self.node_is_vec(id) {
            return Some(id);
        }
        let child = self.val_child(id)?;
        self.node_is_vec(child).then_some(child)
    }

    pub(crate) fn resolve_object_node(&self, id: Timestamp) -> Option<Timestamp> {
        if self.node_is_object(id) {
            return Some(id);
        }
        let child = self.val_child(id)?;
        self.node_is_object(child).then_some(child)
    }

    pub(crate) fn vec_index_value(&self, id: Timestamp, index: u64) -> Option<Timestamp> {
        let node = self.nodes.get(&Id::from(id))?;
        if let RuntimeNode::Vec(map) = node {
            map.get(&index).copied().map(Into::into)
        } else {
            None
        }
    }

    pub(crate) fn vec_max_index(&self, id: Timestamp) -> Option<u64> {
        let node = self.nodes.get(&Id::from(id))?;
        if let RuntimeNode::Vec(map) = node {
            map.keys().copied().max()
        } else {
            None
        }
    }

    pub(crate) fn node_json_value(&self, id: Timestamp) -> Option<Value> {
        self.node_view(Id::from(id))
    }

    pub(crate) fn node_is_deleted_or_missing(&self, id: Timestamp) -> bool {
        let key = Id::from(id);
        matches!(
            self.nodes.get(&key),
            None | Some(RuntimeNode::Con(ConCell::Undef))
        )
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

    pub(crate) fn bin_visible_slots(&self, id: Timestamp) -> Option<Vec<Timestamp>> {
        let node = self.nodes.get(&Id::from(id))?;
        if let RuntimeNode::Bin(atoms) = node {
            let mut out = Vec::new();
            for atom in atoms {
                if atom.byte.is_some() {
                    out.push(atom.slot.into());
                }
            }
            Some(out)
        } else {
            None
        }
    }

    #[allow(dead_code)]
    pub(crate) fn array_find(&self, id: Timestamp, index: usize) -> Option<Timestamp> {
        let node = self.nodes.get(&Id::from(id))?;
        let RuntimeNode::Arr(atoms) = node else {
            return None;
        };
        let mut visible = 0usize;
        for atom in atoms {
            if atom.value.is_none() {
                continue;
            }
            if visible == index {
                return Some(atom.slot.into());
            }
            visible += 1;
        }
        None
    }

    #[allow(dead_code)]
    pub(crate) fn array_find_interval(
        &self,
        id: Timestamp,
        index: usize,
        len: usize,
    ) -> Vec<Timespan> {
        let node = match self.nodes.get(&Id::from(id)) {
            Some(RuntimeNode::Arr(atoms)) => atoms,
            _ => return Vec::new(),
        };
        collect_visible_interval_spans(
            node.iter().filter_map(|a| a.value.as_ref().map(|_| a.slot)),
            index,
            len,
        )
    }

    #[allow(dead_code)]
    pub(crate) fn bin_find(&self, id: Timestamp, index: usize) -> Option<Timestamp> {
        let node = self.nodes.get(&Id::from(id))?;
        let RuntimeNode::Bin(atoms) = node else {
            return None;
        };
        let mut visible = 0usize;
        for atom in atoms {
            if atom.byte.is_none() {
                continue;
            }
            if visible == index {
                return Some(atom.slot.into());
            }
            visible += 1;
        }
        None
    }

    #[allow(dead_code)]
    pub(crate) fn bin_find_interval(
        &self,
        id: Timestamp,
        index: usize,
        len: usize,
    ) -> Vec<Timespan> {
        let node = match self.nodes.get(&Id::from(id)) {
            Some(RuntimeNode::Bin(atoms)) => atoms,
            _ => return Vec::new(),
        };
        collect_visible_interval_spans(
            node.iter().filter_map(|a| a.byte.as_ref().map(|_| a.slot)),
            index,
            len,
        )
    }

    #[allow(dead_code)]
    pub(crate) fn string_find(&self, id: Timestamp, index: usize) -> Option<Timestamp> {
        let node = self.nodes.get(&Id::from(id))?;
        let RuntimeNode::Str(atoms) = node else {
            return None;
        };
        let mut visible = 0usize;
        for atom in atoms {
            if atom.ch.is_none() {
                continue;
            }
            if visible == index {
                return Some(atom.slot.into());
            }
            visible += 1;
        }
        None
    }

    #[allow(dead_code)]
    pub(crate) fn string_find_interval(
        &self,
        id: Timestamp,
        index: usize,
        len: usize,
    ) -> Vec<Timespan> {
        let node = match self.nodes.get(&Id::from(id)) {
            Some(RuntimeNode::Str(atoms)) => atoms,
            _ => return Vec::new(),
        };
        collect_visible_interval_spans(
            node.iter().filter_map(|a| a.ch.as_ref().map(|_| a.slot)),
            index,
            len,
        )
    }
}

#[allow(dead_code)]
fn collect_visible_interval_spans(
    slots: impl Iterator<Item = Id>,
    index: usize,
    len: usize,
) -> Vec<Timespan> {
    if len == 0 {
        return Vec::new();
    }
    let mut out: Vec<Timespan> = Vec::new();
    let mut visible = 0usize;
    let mut remaining = len;
    for slot in slots {
        if visible < index {
            visible += 1;
            continue;
        }
        if remaining == 0 {
            break;
        }
        if let Some(last) = out.last_mut() {
            if last.sid == slot.sid && last.time + last.span == slot.time {
                last.span += 1;
            } else {
                out.push(Timespan {
                    sid: slot.sid,
                    time: slot.time,
                    span: 1,
                });
            }
        } else {
            out.push(Timespan {
                sid: slot.sid,
                time: slot.time,
                span: 1,
            });
        }
        visible += 1;
        remaining -= 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patch::{ConValue, DecodedOp, Patch};
    use crate::patch_builder::encode_patch_from_ops;

    fn ts(sid: u64, time: u64) -> Timestamp {
        Timestamp { sid, time }
    }

    fn apply_ops(runtime: &mut RuntimeModel, sid: u64, time: u64, ops: &[DecodedOp]) {
        let bytes = encode_patch_from_ops(sid, time, ops).expect("encode patch");
        let patch = Patch::from_binary(&bytes).expect("decode patch");
        runtime.apply_patch(&patch).expect("apply patch");
    }

    #[test]
    fn upstream_port_arr_find_and_interval_matrix() {
        // Upstream reference:
        // /Users/nchapman/Code/json-joy/packages/json-joy/src/json-crdt/model/__tests__/Model.array.spec.ts
        let sid = 99210;
        let mut runtime = RuntimeModel::new_logical_empty(sid);
        let arr = ts(sid, 1);
        let t = ts(sid, 3);
        let f = ts(sid, 4);
        let n = ts(sid, 5);
        apply_ops(
            &mut runtime,
            sid,
            1,
            &[
                DecodedOp::NewArr { id: arr },
                DecodedOp::InsVal {
                    id: ts(sid, 2),
                    obj: ts(0, 0),
                    val: arr,
                },
                DecodedOp::NewCon {
                    id: t,
                    value: ConValue::Json(Value::Bool(true)),
                },
                DecodedOp::NewCon {
                    id: f,
                    value: ConValue::Json(Value::Bool(false)),
                },
                DecodedOp::NewCon {
                    id: n,
                    value: ConValue::Json(Value::Null),
                },
                DecodedOp::InsArr {
                    id: ts(sid, 6),
                    obj: arr,
                    reference: arr,
                    data: vec![f, t, t],
                },
                DecodedOp::InsArr {
                    id: ts(sid, 9),
                    obj: arr,
                    reference: ts(sid, 8),
                    data: vec![f, t, t],
                },
                DecodedOp::InsArr {
                    id: ts(sid, 12),
                    obj: arr,
                    reference: ts(sid, 11),
                    data: vec![f, t, n],
                },
            ],
        );

        assert_eq!(runtime.array_find(arr, 0), Some(ts(sid, 6)));
        assert_eq!(runtime.array_find(arr, 5), Some(ts(sid, 11)));
        assert_eq!(runtime.array_find(arr, 8), Some(ts(sid, 14)));
        assert_eq!(runtime.array_find(arr, 9), None);

        let spans = runtime.array_find_interval(arr, 2, 5);
        assert_eq!(
            spans,
            vec![Timespan {
                sid,
                time: 8,
                span: 5
            }]
        );
    }

    #[test]
    fn upstream_port_bin_find_and_interval_matrix() {
        // Upstream reference:
        // /Users/nchapman/Code/json-joy/packages/json-joy/src/json-crdt/model/__tests__/Model.binary.spec.ts
        let sid = 99211;
        let mut runtime = RuntimeModel::new_logical_empty(sid);
        let bin = ts(sid, 1);
        apply_ops(
            &mut runtime,
            sid,
            1,
            &[
                DecodedOp::NewBin { id: bin },
                DecodedOp::InsVal {
                    id: ts(sid, 2),
                    obj: ts(0, 0),
                    val: bin,
                },
                DecodedOp::InsBin {
                    id: ts(sid, 3),
                    obj: bin,
                    reference: bin,
                    data: vec![1, 2, 3],
                },
                DecodedOp::Nop {
                    id: ts(sid, 6),
                    len: 123,
                },
                DecodedOp::InsBin {
                    id: ts(sid, 129),
                    obj: bin,
                    reference: ts(sid, 5),
                    data: vec![4, 5, 6],
                },
                DecodedOp::Nop {
                    id: ts(sid, 132),
                    len: 10,
                },
                DecodedOp::InsBin {
                    id: ts(sid, 142),
                    obj: bin,
                    reference: ts(sid, 131),
                    data: vec![7, 8, 9],
                },
            ],
        );

        assert_eq!(runtime.bin_find(bin, 2), Some(ts(sid, 5)));
        assert_eq!(runtime.bin_find(bin, 6), Some(ts(sid, 142)));
        assert_eq!(runtime.bin_find(bin, 9), None);

        let spans = runtime.bin_find_interval(bin, 1, 7);
        assert_eq!(
            spans,
            vec![
                Timespan {
                    sid,
                    time: 4,
                    span: 2
                },
                Timespan {
                    sid,
                    time: 129,
                    span: 3
                },
                Timespan {
                    sid,
                    time: 142,
                    span: 2
                }
            ]
        );
    }

    #[test]
    fn upstream_port_string_find_and_interval_matrix() {
        // Upstream reference:
        // /Users/nchapman/Code/json-joy/packages/json-joy/src/json-crdt/model/__tests__/Model.string.spec.ts
        let sid = 99212;
        let mut runtime = RuntimeModel::new_logical_empty(sid);
        let str_id = ts(sid, 1);
        apply_ops(
            &mut runtime,
            sid,
            1,
            &[
                DecodedOp::NewStr { id: str_id },
                DecodedOp::InsVal {
                    id: ts(sid, 2),
                    obj: ts(0, 0),
                    val: str_id,
                },
                DecodedOp::InsStr {
                    id: ts(sid, 3),
                    obj: str_id,
                    reference: str_id,
                    data: "abc".to_string(),
                },
                DecodedOp::Nop {
                    id: ts(sid, 6),
                    len: 123,
                },
                DecodedOp::InsStr {
                    id: ts(sid, 129),
                    obj: str_id,
                    reference: ts(sid, 5),
                    data: "def".to_string(),
                },
                DecodedOp::Nop {
                    id: ts(sid, 132),
                    len: 10,
                },
                DecodedOp::InsStr {
                    id: ts(sid, 142),
                    obj: str_id,
                    reference: ts(sid, 131),
                    data: "ghi".to_string(),
                },
            ],
        );

        assert_eq!(runtime.string_find(str_id, 2), Some(ts(sid, 5)));
        assert_eq!(runtime.string_find(str_id, 6), Some(ts(sid, 142)));
        assert_eq!(runtime.string_find(str_id, 9), None);

        let spans = runtime.string_find_interval(str_id, 1, 7);
        assert_eq!(
            spans,
            vec![
                Timespan {
                    sid,
                    time: 4,
                    span: 2
                },
                Timespan {
                    sid,
                    time: 129,
                    span: 3
                },
                Timespan {
                    sid,
                    time: 142,
                    span: 2
                }
            ]
        );
    }
}
