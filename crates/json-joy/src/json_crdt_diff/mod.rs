//! JSON CRDT structural diff — produce a patch that transforms one CRDT state
//! to match a target JSON value.
//!
//! Mirrors `packages/json-joy/src/json-crdt-diff/JsonCrdtDiff.ts`.
//!
//! # Limitations (simplified port)
//!
//! - Array diffing uses delete-all / insert-all (not LCS-based as upstream).
//! - `ConNode` matching uses value equality (no `Timestamp` reference comparison).
//! - `VecNode` diffing is not implemented.

use serde_json::Value;

use crate::json_crdt::constants::ORIGIN;
use crate::json_crdt::nodes::{ArrNode, BinNode, CrdtNode, NodeIndex, ObjNode, StrNode, TsKey, ValNode};
use crate::json_crdt_patch::clock::{Ts, Tss};
use crate::json_crdt_patch::operations::ConValue;
use crate::json_crdt_patch::patch::Patch;
use crate::json_crdt_patch::patch_builder::PatchBuilder;
use crate::util_inner::diff::str as str_diff;
use crate::util_inner::diff::bin as bin_diff;
use json_joy_json_pack::PackValue;

/// Error produced when diffing two incompatible node types.
#[derive(Debug)]
pub struct DiffError(pub &'static str);

impl std::fmt::Display for DiffError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DiffError({})", self.0)
    }
}

impl std::error::Error for DiffError {}

// ── JsonCrdtDiff ──────────────────────────────────────────────────────────

/// Computes a patch that transforms the source CRDT node to look like `dst`.
pub struct JsonCrdtDiff<'a> {
    pub builder: PatchBuilder,
    index: &'a NodeIndex,
}

impl<'a> JsonCrdtDiff<'a> {
    pub fn new(clock_sid: u64, clock_time: u64, index: &'a NodeIndex) -> Self {
        Self { builder: PatchBuilder::new(clock_sid, clock_time), index }
    }

    // ── Str ──────────────────────────────────────────────────────────────

    fn diff_str(&mut self, src: &StrNode, dst: &str) -> Result<(), DiffError> {
        let view = src.view_str();
        if view == dst {
            return Ok(());
        }

        let src_id = src.id;
        let patch = str_diff::diff(&view, dst);

        // Collect insertions and deletions without borrowing `self`.
        let mut inserts: Vec<(Ts, String)> = Vec::new();
        let mut deletes: Vec<Vec<Tss>> = Vec::new();

        str_diff::apply(
            &patch,
            view.chars().count(),
            |pos, text| {
                // Use ORIGIN (ts(0,0)) to insert at the very beginning of the RGA.
                let after = if pos == 0 { ORIGIN } else { src.find(pos - 1).unwrap_or(ORIGIN) };
                inserts.push((after, text.to_string()));
            },
            |pos, len, _| {
                deletes.push(src.find_interval(pos, len));
            },
        );

        for (after, text) in inserts {
            self.builder.ins_str(src_id, after, text);
        }
        for spans in deletes {
            if !spans.is_empty() {
                self.builder.del(src_id, spans);
            }
        }
        Ok(())
    }

    // ── Bin ──────────────────────────────────────────────────────────────

    fn diff_bin(&mut self, src: &BinNode, dst: &[u8]) -> Result<(), DiffError> {
        let view = src.view();
        if view == dst {
            return Ok(());
        }

        let src_id = src.id;
        let patch = bin_diff::diff(&view, dst);

        let mut inserts: Vec<(Ts, Vec<u8>)> = Vec::new();
        let mut deletes: Vec<Vec<Tss>> = Vec::new();

        bin_diff::apply(
            &patch,
            view.len(),
            |pos, bytes| {
                let after = if pos == 0 { ORIGIN } else { find_bin_ts(src, pos - 1).unwrap_or(ORIGIN) };
                inserts.push((after, bytes));
            },
            |pos, len| {
                deletes.push(find_bin_interval(src, pos, len));
            },
        );

        for (after, bytes) in inserts {
            self.builder.ins_bin(src_id, after, bytes);
        }
        for spans in deletes {
            if !spans.is_empty() {
                self.builder.del(src_id, spans);
            }
        }
        Ok(())
    }

    // ── Arr ──────────────────────────────────────────────────────────────

    fn diff_arr(&mut self, src: &ArrNode, dst: &[Value]) -> Result<(), DiffError> {
        // Simplified: delete all existing, then insert all new elements.
        let src_size = src.size();
        if src_size > 0 {
            let spans = src.find_interval(0, src_size);
            if !spans.is_empty() {
                self.builder.del(src.id, spans);
            }
        }

        let mut after = src.id;
        for item in dst {
            let new_id = self.build_view(item);
            let ins_id = self.builder.ins_arr(src.id, after, vec![new_id]);
            after = ins_id;
        }
        Ok(())
    }

    // ── Obj ──────────────────────────────────────────────────────────────

    fn diff_obj(
        &mut self,
        src: &ObjNode,
        dst: &serde_json::Map<String, Value>,
    ) -> Result<(), DiffError> {
        let mut inserts: Vec<(String, Ts)> = Vec::new();

        // Keys in src not in dst → set to null (undefined equivalent)
        for key in src.keys.keys() {
            if !dst.contains_key(key) {
                let undef_id = self.builder.con_val(PackValue::Null);
                inserts.push((key.clone(), undef_id));
            }
        }

        // For each key in dst: try recursive diff, fall back to replace
        for (key, dst_val) in dst {
            if let Some(&val_id) = src.keys.get(key) {
                if let Some(src_node) = self.index.get(&TsKey::from(val_id)) {
                    let src_node = src_node.clone();
                    match self.diff_any(&src_node, dst_val) {
                        Ok(()) => continue,
                        Err(_) => {} // fall through to replace
                    }
                }
            }
            let new_id = self.build_con_view(dst_val);
            inserts.push((key.clone(), new_id));
        }

        if !inserts.is_empty() {
            self.builder.ins_obj(src.id, inserts);
        }
        Ok(())
    }

    // ── Val ──────────────────────────────────────────────────────────────

    fn diff_val(&mut self, src: &ValNode, dst: &Value) -> Result<(), DiffError> {
        let child_id = src.val;
        if let Some(child) = self.index.get(&TsKey::from(child_id)) {
            let child = child.clone();
            if self.diff_any(&child, dst).is_ok() {
                return Ok(());
            }
        }
        let new_id = self.build_con_view(dst);
        self.builder.set_val(src.id, new_id);
        Ok(())
    }

    // ── Any ──────────────────────────────────────────────────────────────

    fn diff_any(&mut self, src: &CrdtNode, dst: &Value) -> Result<(), DiffError> {
        match src {
            CrdtNode::Con(node) => {
                let src_val = con_to_json(&node.val);
                if src_val == *dst { Ok(()) } else { Err(DiffError("CON_MISMATCH")) }
            }
            CrdtNode::Str(node) => {
                let node = node.clone();
                match dst {
                    Value::String(s) => self.diff_str(&node, s),
                    _ => Err(DiffError("STR_TYPE_MISMATCH")),
                }
            }
            CrdtNode::Bin(node) => {
                let node = node.clone();
                match dst {
                    Value::Array(arr) => {
                        let bytes: Option<Vec<u8>> = arr
                            .iter()
                            .map(|v| v.as_u64().and_then(|n| u8::try_from(n).ok()))
                            .collect();
                        match bytes {
                            Some(b) => self.diff_bin(&node, &b),
                            None => Err(DiffError("BIN_TYPE_MISMATCH")),
                        }
                    }
                    _ => Err(DiffError("BIN_TYPE_MISMATCH")),
                }
            }
            CrdtNode::Obj(node) => {
                let node = node.clone();
                match dst {
                    Value::Object(map) => self.diff_obj(&node, map),
                    _ => Err(DiffError("OBJ_TYPE_MISMATCH")),
                }
            }
            CrdtNode::Val(node) => {
                let node = node.clone();
                self.diff_val(&node, dst)
            }
            CrdtNode::Arr(node) => {
                let node = node.clone();
                match dst {
                    Value::Array(arr) => self.diff_arr(&node, arr),
                    _ => Err(DiffError("ARR_TYPE_MISMATCH")),
                }
            }
            CrdtNode::Vec(_) => Err(DiffError("VEC_NOT_SUPPORTED")),
        }
    }

    // ── Public API ────────────────────────────────────────────────────────

    /// Compute the diff patch from `src` to `dst`.
    pub fn diff(&mut self, src: &CrdtNode, dst: &Value) -> Patch {
        let _ = self.diff_any(src, dst);
        self.builder.flush()
    }

    // ── Builders ─────────────────────────────────────────────────────────

    fn build_view(&mut self, dst: &Value) -> Ts {
        match dst {
            Value::String(s) => {
                let str_id = self.builder.str_node();
                if !s.is_empty() {
                    // ORIGIN (ts(0,0)) means "insert at the start of this string RGA"
                    self.builder.ins_str(str_id, ORIGIN, s.clone());
                }
                str_id
            }
            Value::Array(arr) => {
                let arr_id = self.builder.arr();
                let mut after = arr_id;
                for item in arr {
                    let item_id = self.build_view(item);
                    let ins_id = self.builder.ins_arr(arr_id, after, vec![item_id]);
                    after = ins_id;
                }
                arr_id
            }
            Value::Object(map) => {
                let obj_id = self.builder.obj();
                let inserts: Vec<(String, Ts)> = map
                    .iter()
                    .map(|(k, v)| (k.clone(), self.build_con_view(v)))
                    .collect();
                if !inserts.is_empty() {
                    self.builder.ins_obj(obj_id, inserts);
                }
                obj_id
            }
            _ => self.build_con_view(dst),
        }
    }

    fn build_con_view(&mut self, dst: &Value) -> Ts {
        self.builder.con_val(json_to_pack(dst))
    }
}

// ── Standalone helpers ─────────────────────────────────────────────────────

fn con_to_json(val: &ConValue) -> Value {
    match val {
        ConValue::Ref(_) => Value::Null,
        ConValue::Val(pv) => Value::from(pv.clone()),
    }
}

fn json_to_pack(val: &Value) -> PackValue {
    match val {
        Value::Null => PackValue::Null,
        Value::Bool(b) => PackValue::Bool(*b),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                PackValue::Integer(i)
            } else {
                PackValue::Float(n.as_f64().unwrap_or(0.0))
            }
        }
        Value::String(s) => PackValue::Str(s.clone().into()),
        _ => PackValue::Null,
    }
}

fn find_bin_ts(src: &BinNode, pos: usize) -> Option<Ts> {
    let mut count = 0usize;
    for chunk in src.rga.iter_live() {
        if let Some(data) = &chunk.data {
            let chunk_len = data.len();
            if pos < count + chunk_len {
                return Some(Ts::new(chunk.id.sid, chunk.id.time + (pos - count) as u64));
            }
            count += chunk_len;
        }
    }
    None
}

fn find_bin_interval(src: &BinNode, pos: usize, len: usize) -> Vec<Tss> {
    let mut result = Vec::new();
    let mut count = 0usize;
    let end = pos + len;
    for chunk in src.rga.iter_live() {
        if let Some(data) = &chunk.data {
            let chunk_len = data.len();
            let chunk_start = count;
            let chunk_end = count + chunk_len;
            if chunk_end > pos && chunk_start < end {
                let local_start = if chunk_start >= pos { 0 } else { pos - chunk_start };
                let local_end = (end - chunk_start).min(chunk_len);
                result.push(Tss::new(
                    chunk.id.sid,
                    chunk.id.time + local_start as u64,
                    (local_end - local_start) as u64,
                ));
            }
            count = chunk_end;
        }
    }
    result
}

// ── Public entry point ────────────────────────────────────────────────────

/// Compute a patch that makes `src` look like `dst`.
///
/// Returns `None` if no changes are needed.
pub fn diff_node(
    src: &CrdtNode,
    index: &NodeIndex,
    clock_sid: u64,
    clock_time: u64,
    dst: &Value,
) -> Option<Patch> {
    let mut d = JsonCrdtDiff::new(clock_sid, clock_time, index);
    let patch = d.diff(src, dst);
    if patch.ops.is_empty() { None } else { Some(patch) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_crdt::model::Model;
    use crate::json_crdt::nodes::TsKey;
    use crate::json_crdt::constants::ORIGIN;
    use crate::json_crdt_patch::operations::{ConValue, Op};
    use crate::json_crdt_patch::clock::ts;
    use serde_json::json;

    fn sid() -> u64 { 123456 }

    fn model_with_str(s: &str) -> (Model, TsKey) {
        let sid = sid();
        let mut model = Model::new(sid);
        model.apply_operation(&Op::NewStr { id: ts(sid, 1) });
        if !s.is_empty() {
            model.apply_operation(&Op::InsStr {
                id: ts(sid, 2),
                obj: ts(sid, 1),
                after: ORIGIN,
                data: s.to_string(),
            });
        }
        let next = 2 + s.chars().count() as u64;
        model.apply_operation(&Op::InsVal {
            id: ts(sid, next),
            obj: ORIGIN,
            val: ts(sid, 1),
        });
        let key = TsKey { sid, time: 1 };
        (model, key)
    }

    #[test]
    fn diff_str_no_change() {
        let (model, key) = model_with_str("hello");
        let src_node = model.index.get(&key).unwrap().clone();
        let result = diff_node(&src_node, &model.index, model.clock.sid, model.clock.time, &json!("hello"));
        assert!(result.is_none());
    }

    #[test]
    fn diff_str_change() {
        let (mut model, key) = model_with_str("hello");
        let src_node = model.index.get(&key).unwrap().clone();
        let patch = diff_node(&src_node, &model.index, model.clock.sid, model.clock.time, &json!("world")).unwrap();
        model.apply_patch(&patch);
        assert_eq!(model.view(), json!("world"));
    }

    #[test]
    fn diff_str_append() {
        let (mut model, key) = model_with_str("hello");
        let src_node = model.index.get(&key).unwrap().clone();
        let patch = diff_node(&src_node, &model.index, model.clock.sid, model.clock.time, &json!("hello world")).unwrap();
        model.apply_patch(&patch);
        assert_eq!(model.view(), json!("hello world"));
    }

    #[test]
    fn diff_str_delete_prefix() {
        let (mut model, key) = model_with_str("hello world");
        let src_node = model.index.get(&key).unwrap().clone();
        let patch = diff_node(&src_node, &model.index, model.clock.sid, model.clock.time, &json!("world")).unwrap();
        model.apply_patch(&patch);
        assert_eq!(model.view(), json!("world"));
    }

    #[test]
    fn diff_obj_add_key() {
        let sid = sid();
        let mut model = Model::new(sid);
        model.apply_operation(&Op::NewObj { id: ts(sid, 1) });
        model.apply_operation(&Op::NewCon {
            id: ts(sid, 2),
            val: ConValue::Val(PackValue::Integer(1)),
        });
        model.apply_operation(&Op::InsObj {
            id: ts(sid, 3),
            obj: ts(sid, 1),
            data: vec![("a".to_string(), ts(sid, 2))],
        });
        model.apply_operation(&Op::InsVal {
            id: ts(sid, 4),
            obj: ORIGIN,
            val: ts(sid, 1),
        });

        let key = TsKey { sid, time: 1 };
        let src_node = model.index.get(&key).unwrap().clone();
        let patch = diff_node(
            &src_node, &model.index, model.clock.sid, model.clock.time,
            &json!({"a": 1, "b": 2}),
        ).unwrap();
        model.apply_patch(&patch);
        assert_eq!(model.view(), json!({"a": 1, "b": 2}));
    }

    #[test]
    fn diff_empty_string() {
        let (mut model, key) = model_with_str("");
        let src_node = model.index.get(&key).unwrap().clone();
        let patch = diff_node(&src_node, &model.index, model.clock.sid, model.clock.time, &json!("hi")).unwrap();
        model.apply_patch(&patch);
        assert_eq!(model.view(), json!("hi"));
    }

    #[test]
    fn diff_arr_change() {
        let sid = sid();
        let mut model = Model::new(sid);
        model.apply_operation(&Op::NewArr { id: ts(sid, 1) });
        model.apply_operation(&Op::NewCon {
            id: ts(sid, 2),
            val: ConValue::Val(PackValue::Integer(1)),
        });
        model.apply_operation(&Op::InsArr {
            id: ts(sid, 3),
            obj: ts(sid, 1),
            after: ORIGIN,
            data: vec![ts(sid, 2)],
        });
        model.apply_operation(&Op::InsVal {
            id: ts(sid, 4),
            obj: ORIGIN,
            val: ts(sid, 1),
        });

        let key = TsKey { sid, time: 1 };
        let src_node = model.index.get(&key).unwrap().clone();
        let patch = diff_node(
            &src_node, &model.index, model.clock.sid, model.clock.time,
            &json!([10, 20]),
        ).unwrap();
        model.apply_patch(&patch);
        assert_eq!(model.view(), json!([10, 20]));
    }
}
