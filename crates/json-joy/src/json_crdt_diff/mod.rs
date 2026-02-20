//! JSON CRDT structural diff — produce a patch that transforms one CRDT state
//! to match a target JSON value.
//!
//! Mirrors `packages/json-joy/src/json-crdt-diff/JsonCrdtDiff.ts`.
//!
//! # Limitations
//!
//! - `ConNode` matching uses value equality (no `Timestamp` reference comparison).
//! - Destination values are plain JSON (`serde_json::Value`), so upstream
//!   NodeBuilder wrapper variants are not represented directly in this API.

use serde_json::Value;
use std::cell::RefCell;

use crate::json_crdt::nodes::{
    ArrNode, BinNode, CrdtNode, NodeIndex, ObjNode, StrNode, TsKey, ValNode, VecNode,
};
use crate::json_crdt_patch::clock::{Ts, Tss};
use crate::json_crdt_patch::operations::ConValue;
use crate::json_crdt_patch::patch::Patch;
use crate::json_crdt_patch::patch_builder::PatchBuilder;
use crate::json_hash::{struct_hash, struct_hash_crdt};
use crate::util_inner::diff::bin as bin_diff;
use crate::util_inner::diff::line as line_diff;
use crate::util_inner::diff::str as str_diff;
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
        Self {
            builder: PatchBuilder::new(clock_sid, clock_time),
            index,
        }
    }

    // ── Str ──────────────────────────────────────────────────────────────

    fn diff_str(&mut self, src: &StrNode, dst: &str) -> Result<(), DiffError> {
        let view = src.view_str();
        if view == dst {
            return Ok(());
        }

        let src_id = src.id;
        let patch = str_diff::diff(&view, dst);

        enum StrEdit {
            Ins(Ts, String),
            Del(Vec<Tss>),
        }
        let edits: RefCell<Vec<StrEdit>> = RefCell::new(Vec::new());

        str_diff::apply(
            &patch,
            view.chars().count(),
            |pos, text| {
                // For pos=0, use the StrNode's own ID as the head sentinel.
                // Mirrors upstream TS: `!pos ? src.id : src.find(pos - 1)!`
                let after = if pos == 0 {
                    src_id
                } else {
                    src.find(pos - 1).unwrap_or(src_id)
                };
                edits
                    .borrow_mut()
                    .push(StrEdit::Ins(after, text.to_string()));
            },
            |pos, len, _| {
                let spans = src.find_interval(pos, len);
                if !spans.is_empty() {
                    edits.borrow_mut().push(StrEdit::Del(spans));
                }
            },
        );

        for edit in edits.into_inner() {
            match edit {
                StrEdit::Ins(after, text) => {
                    self.builder.ins_str(src_id, after, text);
                }
                StrEdit::Del(spans) => {
                    self.builder.del(src_id, spans);
                }
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

        enum BinEdit {
            Ins(Ts, Vec<u8>),
            Del(Vec<Tss>),
        }
        let edits: RefCell<Vec<BinEdit>> = RefCell::new(Vec::new());

        bin_diff::apply(
            &patch,
            view.len(),
            |pos, bytes| {
                let after = if pos == 0 {
                    src_id
                } else {
                    find_bin_ts(src, pos - 1).unwrap_or(src_id)
                };
                edits.borrow_mut().push(BinEdit::Ins(after, bytes));
            },
            |pos, len| {
                let spans = find_bin_interval(src, pos, len);
                if !spans.is_empty() {
                    edits.borrow_mut().push(BinEdit::Del(spans));
                }
            },
        );

        for edit in edits.into_inner() {
            match edit {
                BinEdit::Ins(after, bytes) => {
                    self.builder.ins_bin(src_id, after, bytes);
                }
                BinEdit::Del(spans) => {
                    self.builder.del(src_id, spans);
                }
            }
        }
        Ok(())
    }

    // ── Arr ──────────────────────────────────────────────────────────────

    fn diff_arr(&mut self, src: &ArrNode, dst: &[Value]) -> Result<(), DiffError> {
        let src_size = src.size();
        if src_size == 0 {
            if dst.is_empty() {
                return Ok(());
            }
            let mut after = src.id;
            for view in dst {
                let view_id = self.build_view(view);
                let ins_id = self.builder.ins_arr(src.id, after, vec![view_id]);
                after = ins_id;
            }
            return Ok(());
        } else if dst.is_empty() {
            let mut spans: Vec<Tss> = Vec::new();
            for chunk in src.rga.iter_live() {
                spans.push(Tss::new(chunk.id.sid, chunk.id.time, chunk.span));
            }
            if !spans.is_empty() {
                self.builder.del(src.id, spans);
            }
            return Ok(());
        }

        let mut src_lines: Vec<String> = Vec::with_capacity(src_size);
        for pos in 0..src_size {
            let child = src
                .get_data_ts(pos)
                .and_then(|id| self.index.get(&TsKey::from(id)));
            src_lines.push(struct_hash_crdt(child, self.index));
        }

        let dst_lines: Vec<String> = dst.iter().map(struct_hash).collect();
        let src_line_refs: Vec<&str> = src_lines.iter().map(String::as_str).collect();
        let dst_line_refs: Vec<&str> = dst_lines.iter().map(String::as_str).collect();
        let line_patch = line_diff::diff(&src_line_refs, &dst_line_refs);
        if line_patch.is_empty() {
            return Ok(());
        }

        let mut inserts: Vec<(Ts, Value)> = Vec::new();
        let mut deletes: Vec<Tss> = Vec::new();

        for (op_type, pos_src, pos_dst) in line_patch.iter().rev().copied() {
            match op_type {
                line_diff::LinePatchOpType::Eql => {}
                line_diff::LinePatchOpType::Del => {
                    let span = src.find_interval(pos_src as usize, 1);
                    if span.is_empty() {
                        return Err(DiffError("ARR_DELETE_INTERVAL_MISSING"));
                    }
                    deletes.extend(span);
                }
                line_diff::LinePatchOpType::Ins => {
                    let after = if pos_src >= 0 {
                        src.find(pos_src as usize)
                            .ok_or(DiffError("ARR_INSERT_AFTER_NOT_FOUND"))?
                    } else {
                        src.id
                    };
                    inserts.push((after, dst[pos_dst as usize].clone()));
                }
                line_diff::LinePatchOpType::Mix => {
                    let view = &dst[pos_dst as usize];
                    let src_child_id = src
                        .get_data_ts(pos_src as usize)
                        .ok_or(DiffError("ARR_MIX_SRC_CHILD_NOT_FOUND"))?;
                    let src_child = self
                        .index
                        .get(&TsKey::from(src_child_id))
                        .cloned()
                        .ok_or(DiffError("ARR_MIX_SRC_NODE_NOT_FOUND"))?;
                    if self.diff_any(&src_child, view).is_err() {
                        let span = src.find_interval(pos_src as usize, 1);
                        if span.is_empty() {
                            return Err(DiffError("ARR_MIX_DELETE_INTERVAL_MISSING"));
                        }
                        deletes.extend(span);
                        let after = if pos_src > 0 {
                            src.find((pos_src - 1) as usize)
                                .ok_or(DiffError("ARR_MIX_INSERT_AFTER_NOT_FOUND"))?
                        } else {
                            src.id
                        };
                        inserts.push((after, view.clone()));
                    }
                }
            }
        }

        for (after, view) in inserts {
            let view_id = self.build_view(&view);
            self.builder.ins_arr(src.id, after, vec![view_id]);
        }
        if !deletes.is_empty() {
            self.builder.del(src.id, deletes);
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

        // Mirrors upstream ObjNode.forEach(): only visible (non-tombstoned)
        // keys participate in source-side delete detection.
        for (key, &val_id) in &src.keys {
            let visible = match self.index.get(&TsKey::from(val_id)) {
                Some(CrdtNode::Con(con))
                    if matches!(&con.val, ConValue::Val(PackValue::Undefined)) =>
                {
                    false
                }
                Some(_) => true,
                None => false,
            };
            if !visible {
                continue;
            }
            if !dst.contains_key(key) {
                let undef_id = self.builder.con_val(PackValue::Undefined);
                inserts.push((key.clone(), undef_id));
            }
        }

        // For each key in dst: try recursive diff, fall back to replace
        for (key, dst_val) in dst {
            if let Some(&val_id) = src.keys.get(key) {
                if let Some(src_node) = self.index.get(&TsKey::from(val_id)) {
                    let src_node = src_node.clone();
                    if let Ok(()) = self.diff_any(&src_node, dst_val) {
                        continue;
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

    // ── Vec ──────────────────────────────────────────────────────────────

    fn diff_vec(&mut self, src: &VecNode, dst: &[Value]) -> Result<(), DiffError> {
        let mut edits: Vec<(u8, Ts)> = Vec::new();
        let elements = &src.elements;
        let src_len = elements.len();
        let dst_len = dst.len();
        let min_len = src_len.min(dst_len);

        for (i, elem) in elements.iter().enumerate().take(src_len).skip(dst_len) {
            if i > u8::MAX as usize {
                break;
            }
            let Some(id) = *elem else {
                continue;
            };
            let is_deleted = match self.index.get(&TsKey::from(id)) {
                None => true,
                Some(CrdtNode::Con(con))
                    if matches!(&con.val, ConValue::Val(PackValue::Undefined)) =>
                {
                    true
                }
                _ => false,
            };
            if is_deleted {
                continue;
            }
            edits.push((i as u8, self.builder.con_val(PackValue::Undefined)));
        }

        for (i, value) in dst.iter().enumerate().take(min_len) {
            if i > u8::MAX as usize {
                break;
            }
            let child = elements[i].and_then(|id| self.index.get(&TsKey::from(id)).cloned());
            if let Some(child) = child {
                if self.diff_any(&child, value).is_ok() {
                    continue;
                }
                if matches!(child, CrdtNode::Con(_)) && is_js_non_object(value) {
                    edits.push((i as u8, self.builder.con_val(json_to_pack(value))));
                    continue;
                }
            }
            edits.push((i as u8, self.build_con_view(value)));
        }

        for (i, value) in dst.iter().enumerate().take(dst_len).skip(src_len) {
            if i > u8::MAX as usize {
                break;
            };
            edits.push((i as u8, self.build_con_view(value)));
        }

        if !edits.is_empty() {
            self.builder.ins_vec(src.id, edits);
        }
        Ok(())
    }

    // ── Any ──────────────────────────────────────────────────────────────

    fn diff_any(&mut self, src: &CrdtNode, dst: &Value) -> Result<(), DiffError> {
        match src {
            CrdtNode::Con(node) => {
                if con_equals_dst(&node.val, dst) {
                    Ok(())
                } else {
                    Err(DiffError("CON_MISMATCH"))
                }
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
            CrdtNode::Vec(node) => {
                let node = node.clone();
                match dst {
                    Value::Array(arr) => self.diff_vec(&node, arr),
                    _ => Err(DiffError("VEC_TYPE_MISMATCH")),
                }
            }
        }
    }

    // ── Public API ────────────────────────────────────────────────────────

    /// Compute the diff patch from `src` to `dst`.
    pub fn diff(&mut self, src: &CrdtNode, dst: &Value) -> Patch {
        let _ = self.diff_any(src, dst);
        self.builder.flush()
    }

    // ── Builders ─────────────────────────────────────────────────────────

    fn build_json_val(&mut self, dst: &Value) -> Ts {
        let val_id = self.builder.val();
        let con_id = self.builder.con_val(json_to_pack(dst));
        self.builder.set_val(val_id, con_id);
        val_id
    }

    fn build_view(&mut self, dst: &Value) -> Ts {
        match dst {
            Value::String(s) => {
                let str_id = self.builder.str_node();
                if !s.is_empty() {
                    // Use str_id as ref (node head), matching upstream TS:
                    // `if (str) this.insStr(id, id, str);`
                    self.builder.ins_str(str_id, str_id, s.clone());
                }
                str_id
            }
            Value::Array(arr) => {
                let arr_id = self.builder.arr();
                if !arr.is_empty() {
                    let item_ids: Vec<Ts> = arr.iter().map(|item| self.build_view(item)).collect();
                    self.builder.ins_arr(arr_id, arr_id, item_ids);
                }
                arr_id
            }
            Value::Object(map) => {
                let obj_id = self.builder.obj();
                let inserts: Vec<(String, Ts)> = map
                    .iter()
                    .map(|(k, v)| {
                        let id = match v {
                            Value::Null | Value::Bool(_) | Value::Number(_) => {
                                self.build_con_view(v)
                            }
                            _ => self.build_view(v),
                        };
                        (k.clone(), id)
                    })
                    .collect();
                if !inserts.is_empty() {
                    self.builder.ins_obj(obj_id, inserts);
                }
                obj_id
            }
            Value::Null | Value::Bool(_) | Value::Number(_) => self.build_json_val(dst),
        }
    }

    fn build_con_view(&mut self, dst: &Value) -> Ts {
        match dst {
            Value::Null | Value::Bool(_) | Value::Number(_) => {
                self.builder.con_val(json_to_pack(dst))
            }
            _ => self.build_view(dst),
        }
    }
}

// ── Standalone helpers ─────────────────────────────────────────────────────

fn con_equals_dst(val: &ConValue, dst: &Value) -> bool {
    match val {
        // Fixture destinations are plain JSON values only.
        ConValue::Ref(_) => false,
        // Upstream distinguishes `undefined` tombstones from JSON `null`.
        ConValue::Val(PackValue::Undefined) => false,
        ConValue::Val(pv) => Value::from(pv.clone()) == *dst,
    }
}

fn is_js_non_object(value: &Value) -> bool {
    matches!(value, Value::String(_) | Value::Number(_) | Value::Bool(_))
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
        Value::String(s) => PackValue::Str(s.clone()),
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
                let local_start = pos.saturating_sub(chunk_start);
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
    if patch.ops.is_empty() {
        None
    } else {
        Some(patch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_crdt::constants::ORIGIN;
    use crate::json_crdt::model::Model;
    use crate::json_crdt::nodes::{CrdtNode, TsKey};
    use crate::json_crdt_patch::clock::ts;
    use crate::json_crdt_patch::operations::{ConValue, Op};
    use serde_json::json;

    fn sid() -> u64 {
        123456
    }

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
        let result = diff_node(
            &src_node,
            &model.index,
            model.clock.sid,
            model.clock.time,
            &json!("hello"),
        );
        assert!(result.is_none());
    }

    #[test]
    fn diff_str_change() {
        let (mut model, key) = model_with_str("hello");
        let src_node = model.index.get(&key).unwrap().clone();
        let patch = diff_node(
            &src_node,
            &model.index,
            model.clock.sid,
            model.clock.time,
            &json!("world"),
        )
        .unwrap();
        model.apply_patch(&patch);
        assert_eq!(model.view(), json!("world"));
    }

    #[test]
    fn diff_str_append() {
        let (mut model, key) = model_with_str("hello");
        let src_node = model.index.get(&key).unwrap().clone();
        let patch = diff_node(
            &src_node,
            &model.index,
            model.clock.sid,
            model.clock.time,
            &json!("hello world"),
        )
        .unwrap();
        model.apply_patch(&patch);
        assert_eq!(model.view(), json!("hello world"));
    }

    #[test]
    fn diff_str_delete_prefix() {
        let (mut model, key) = model_with_str("hello world");
        let src_node = model.index.get(&key).unwrap().clone();
        let patch = diff_node(
            &src_node,
            &model.index,
            model.clock.sid,
            model.clock.time,
            &json!("world"),
        )
        .unwrap();
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
            &src_node,
            &model.index,
            model.clock.sid,
            model.clock.time,
            &json!({"a": 1, "b": 2}),
        )
        .unwrap();
        model.apply_patch(&patch);
        assert_eq!(model.view(), json!({"a": 1, "b": 2}));
    }

    #[test]
    fn diff_empty_string() {
        let (mut model, key) = model_with_str("");
        let src_node = model.index.get(&key).unwrap().clone();
        let patch = diff_node(
            &src_node,
            &model.index,
            model.clock.sid,
            model.clock.time,
            &json!("hi"),
        )
        .unwrap();
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
            &src_node,
            &model.index,
            model.clock.sid,
            model.clock.time,
            &json!([10, 20]),
        )
        .unwrap();
        model.apply_patch(&patch);
        assert_eq!(model.view(), json!([10, 20]));
        let elem_id = match model.index.get(&TsKey { sid, time: 1 }) {
            Some(CrdtNode::Arr(arr)) => arr.get_data_ts(0).expect("missing array element"),
            _ => panic!("root should be arr"),
        };
        let con_id = match model.index.get(&TsKey::from(elem_id)) {
            Some(CrdtNode::Val(val)) => val.val,
            _ => panic!("array primitive should be wrapped in val"),
        };
        assert!(matches!(
            model.index.get(&TsKey::from(con_id)),
            Some(CrdtNode::Con(_))
        ));
    }

    #[test]
    fn diff_obj_replaces_string_with_str_node() {
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

        let src_node = model
            .index
            .get(&TsKey { sid, time: 1 })
            .expect("missing obj node")
            .clone();
        let patch = diff_node(
            &src_node,
            &model.index,
            model.clock.sid,
            model.clock.time,
            &json!({"a": "hello"}),
        )
        .unwrap();
        model.apply_patch(&patch);

        let a_id = match model.index.get(&TsKey { sid, time: 1 }) {
            Some(CrdtNode::Obj(obj)) => obj.keys.get("a").copied().expect("missing key"),
            _ => panic!("root should be obj"),
        };
        assert!(matches!(
            model.index.get(&TsKey::from(a_id)),
            Some(CrdtNode::Str(_))
        ));
    }
}
