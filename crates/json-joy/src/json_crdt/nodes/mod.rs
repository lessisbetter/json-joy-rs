//! JSON CRDT node types.
//!
//! Mirrors `packages/json-joy/src/json-crdt/nodes/`.
//!
//! # Node Types
//!
//! | Rust type      | TypeScript  | Semantics                         |
//! |----------------|-------------|-----------------------------------|
//! | `ConNode`      | `ConNode`   | Immutable constant value          |
//! | `ValNode`      | `ValNode`   | Last-write-wins single register   |
//! | `ObjNode`      | `ObjNode`   | LWW key→value map                 |
//! | `VecNode`      | `VecNode`   | Fixed-length LWW tuple            |
//! | `StrNode`      | `StrNode`   | RGA UTF-16 string                 |
//! | `BinNode`      | `BinNode`   | RGA binary blob                   |
//! | `ArrNode`      | `ArrNode`   | RGA array of node references      |
//! | `RootNode`     | `RootNode`  | Document root (LWW register)      |

pub mod rga;

use std::collections::HashMap;
use serde_json::Value;
use json_joy_json_pack::PackValue;

use crate::json_crdt_patch::clock::{Ts, Tss, compare};
use crate::json_crdt_patch::operations::ConValue;
use super::constants::{ORIGIN, UNDEFINED_TS};
use rga::Rga;

// ── ConNode ───────────────────────────────────────────────────────────────

/// Immutable constant node.  Wraps a static value that never changes.
#[derive(Debug, Clone)]
pub struct ConNode {
    pub id: Ts,
    pub val: ConValue,
}

impl ConNode {
    pub fn new(id: Ts, val: ConValue) -> Self {
        Self { id, val }
    }

    /// Return the JSON view of this constant.
    pub fn view(&self) -> Value {
        match &self.val {
            ConValue::Ref(_) => Value::Null, // reference — caller resolves
            ConValue::Val(pv) => pack_to_json(pv),
        }
    }
}

// ── ValNode ───────────────────────────────────────────────────────────────

/// Last-write-wins single-value register.
///
/// Stores the ID of whichever node currently "wins" the register.
#[derive(Debug, Clone)]
pub struct ValNode {
    pub id: Ts,
    /// The ID of the current value node (starts at UNDEFINED).
    pub val: Ts,
}

impl ValNode {
    pub fn new(id: Ts) -> Self {
        // Use ORIGIN (sid=0, time=0) so any user timestamp wins LWW comparison.
        // Upstream TypeScript uses ORIGIN (not UNDEFINED) as ValNode's initial value.
        Self { id, val: ORIGIN }
    }

    /// Set `new_val` if it has a higher timestamp than the current.
    /// Returns the old value if it was replaced.
    pub fn set(&mut self, new_val: Ts) -> Option<Ts> {
        if compare(new_val, self.val) > 0 {
            let old = self.val;
            self.val = new_val;
            Some(old)
        } else {
            None
        }
    }

    /// View: resolve the pointed-to node from the index.
    pub fn view<'a>(&self, index: &'a NodeIndex) -> Value {
        match index.get(&TsKey::from(self.val)) {
            Some(node) => node.view(index),
            None => Value::Null,
        }
    }
}

// ── ObjNode ───────────────────────────────────────────────────────────────

/// Last-write-wins object (map from string keys to node IDs).
#[derive(Debug, Clone)]
pub struct ObjNode {
    pub id: Ts,
    /// key → winning node ID
    pub keys: HashMap<String, Ts>,
}

impl ObjNode {
    pub fn new(id: Ts) -> Self {
        Self { id, keys: HashMap::new() }
    }

    /// Insert a key, keeping it only if `new_id` is newer than existing.
    /// Returns the old ID if replaced.
    pub fn put(&mut self, key: &str, new_id: Ts) -> Option<Ts> {
        match self.keys.get(key).copied() {
            Some(old) if compare(new_id, old) <= 0 => None,
            old => {
                self.keys.insert(key.to_string(), new_id);
                old
            }
        }
    }

    /// View: build a JSON object by resolving each value from the index.
    pub fn view(&self, index: &NodeIndex) -> Value {
        let mut map = serde_json::Map::new();
        let mut keys: Vec<&String> = self.keys.keys().collect();
        keys.sort();
        for key in keys {
            let id = self.keys[key];
            let val = match index.get(&TsKey::from(id)) {
                Some(node) => node.view(index),
                None => Value::Null,
            };
            map.insert(key.clone(), val);
        }
        Value::Object(map)
    }
}

// ── VecNode ───────────────────────────────────────────────────────────────

/// Fixed-length LWW tuple (vector).
#[derive(Debug, Clone)]
pub struct VecNode {
    pub id: Ts,
    /// Indexed by position → node ID (None = unset).
    pub elements: Vec<Option<Ts>>,
}

impl VecNode {
    pub fn new(id: Ts) -> Self {
        Self { id, elements: Vec::new() }
    }

    /// Set element at `index`, keeping it only if `new_id` is newer.
    /// Returns old ID if replaced.
    pub fn put(&mut self, index: usize, new_id: Ts) -> Option<Ts> {
        if index >= self.elements.len() {
            self.elements.resize(index + 1, None);
        }
        match self.elements[index] {
            Some(old) if compare(new_id, old) <= 0 => None,
            old => {
                self.elements[index] = Some(new_id);
                old  // old: Option<Ts>, already the right type
            }
        }
    }

    /// View: build a JSON array by resolving each element.
    pub fn view(&self, index: &NodeIndex) -> Value {
        let items: Vec<Value> = self.elements.iter().map(|e| match e {
            Some(id) => match index.get(&TsKey::from(*id)) {
                Some(node) => node.view(index),
                None => Value::Null,
            },
            None => Value::Null,
        }).collect();
        Value::Array(items)
    }
}

// ── StrNode ───────────────────────────────────────────────────────────────

/// RGA string node (UTF-16 chunks, as in the upstream).
#[derive(Debug, Clone)]
pub struct StrNode {
    pub id: Ts,
    pub rga: Rga<String>,
}

impl StrNode {
    pub fn new(id: Ts) -> Self {
        Self { id, rga: Rga::new() }
    }

    pub fn ins(&mut self, after: Ts, id: Ts, data: String) {
        let span = data.chars().count() as u64; // approximate span as char count
        self.rga.insert(after, id, span, data);
    }

    pub fn delete(&mut self, spans: &[Tss]) {
        self.rga.delete(spans);
    }

    pub fn view(&self) -> Value {
        let s: String = self.rga.iter_live().filter_map(|c| c.data.as_deref()).collect();
        Value::String(s)
    }

    /// Return the string content as a plain `String`.
    pub fn view_str(&self) -> String {
        self.rga.iter_live().filter_map(|c| c.data.as_deref()).collect()
    }

    /// Number of live characters in this string.
    pub fn size(&self) -> usize {
        self.rga.iter_live()
            .filter_map(|c| c.data.as_deref())
            .map(|s| s.chars().count())
            .sum()
    }

    /// Find the chunk-ID timestamp of the character at live position `pos`.
    ///
    /// Returns `None` if `pos >= self.size()`.
    pub fn find(&self, pos: usize) -> Option<Ts> {
        let mut count = 0usize;
        for chunk in self.rga.iter_live() {
            if let Some(data) = &chunk.data {
                let chunk_len = data.chars().count();
                if pos < count + chunk_len {
                    let offset = pos - count;
                    return Some(Ts::new(chunk.id.sid, chunk.id.time + offset as u64));
                }
                count += chunk_len;
            }
        }
        None
    }

    /// Return the timestamp spans covering live positions `[pos, pos + len)`.
    pub fn find_interval(&self, pos: usize, len: usize) -> Vec<Tss> {
        let mut result = Vec::new();
        let mut count = 0usize;
        let end = pos + len;
        for chunk in self.rga.iter_live() {
            if let Some(data) = &chunk.data {
                let chunk_len = data.chars().count();
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
}

// ── BinNode ───────────────────────────────────────────────────────────────

/// RGA binary node.
#[derive(Debug, Clone)]
pub struct BinNode {
    pub id: Ts,
    pub rga: Rga<Vec<u8>>,
}

impl BinNode {
    pub fn new(id: Ts) -> Self {
        Self { id, rga: Rga::new() }
    }

    pub fn ins(&mut self, after: Ts, id: Ts, data: Vec<u8>) {
        let span = data.len() as u64;
        self.rga.insert(after, id, span, data);
    }

    pub fn delete(&mut self, spans: &[Tss]) {
        self.rga.delete(spans);
    }

    pub fn view(&self) -> Vec<u8> {
        self.rga.iter_live()
            .flat_map(|c| c.data.as_deref().unwrap_or(&[]))
            .copied()
            .collect()
    }

    /// View as a JSON array of byte values.
    pub fn view_json(&self) -> Value {
        let bytes = self.view();
        Value::Array(bytes.into_iter().map(|b| Value::Number(b.into())).collect())
    }
}

// ── ArrNode ───────────────────────────────────────────────────────────────

/// RGA array of node-ID references.
#[derive(Debug, Clone)]
pub struct ArrNode {
    pub id: Ts,
    pub rga: Rga<Vec<Ts>>,
}

impl ArrNode {
    pub fn new(id: Ts) -> Self {
        Self { id, rga: Rga::new() }
    }

    /// Insert node IDs after `after`.
    pub fn ins(&mut self, after: Ts, id: Ts, data: Vec<Ts>) {
        let span = data.len() as u64;
        self.rga.insert(after, id, span, data);
    }

    /// Get the node ID at the given absolute position.
    pub fn get_by_id(&self, target: Ts) -> Option<Ts> {
        for chunk in &self.rga.chunks {
            if chunk.id.sid == target.sid
                && chunk.id.time <= target.time
                && target.time < chunk.id.time + chunk.span
            {
                if let Some(data) = &chunk.data {
                    let offset = (target.time - chunk.id.time) as usize;
                    return data.get(offset).copied();
                }
            }
        }
        None
    }

    /// Update (replace) an existing element at the slot identified by `ref_id`.
    ///
    /// Mirrors `ArrNode.upd` in the upstream TypeScript.
    /// Only replaces the current value if `val` has a higher timestamp.
    pub fn upd(&mut self, ref_id: Ts, val: Ts) -> Option<Ts> {
        for chunk in &mut self.rga.chunks {
            if chunk.id.sid == ref_id.sid
                && chunk.id.time <= ref_id.time
                && ref_id.time < chunk.id.time + chunk.span
            {
                if let Some(data) = &mut chunk.data {
                    let offset = (ref_id.time - chunk.id.time) as usize;
                    if let Some(current) = data.get(offset).copied() {
                        use crate::json_crdt_patch::clock::compare;
                        if compare(current, val) >= 0 {
                            return None; // existing is same or newer
                        }
                        let old = data[offset];
                        data[offset] = val;
                        return Some(old);
                    }
                }
            }
        }
        None
    }

    pub fn delete(&mut self, spans: &[Tss]) {
        self.rga.delete(spans);
    }

    /// Number of live elements in this array.
    pub fn size(&self) -> usize {
        self.rga.iter_live().filter_map(|c| c.data.as_ref()).map(|v| v.len()).sum()
    }

    /// Return the slot-ID timestamp of the element at live position `pos`.
    ///
    /// The slot-ID is the timestamp assigned to the **slot** in the RGA
    /// (not the data node stored at that slot).
    pub fn find(&self, pos: usize) -> Option<Ts> {
        let mut count = 0usize;
        for chunk in self.rga.iter_live() {
            if let Some(data) = &chunk.data {
                let chunk_len = data.len();
                if pos < count + chunk_len {
                    let offset = pos - count;
                    return Some(Ts::new(chunk.id.sid, chunk.id.time + offset as u64));
                }
                count += chunk_len;
            }
        }
        None
    }

    /// Return the data-node timestamps of live elements from position `pos` for `len` items.
    pub fn find_data_at(&self, pos: usize, len: usize) -> Vec<Ts> {
        let mut result = Vec::new();
        let mut count = 0usize;
        let end = pos + len;
        for chunk in self.rga.iter_live() {
            if let Some(data) = &chunk.data {
                let chunk_len = data.len();
                let chunk_start = count;
                let chunk_end = count + chunk_len;
                if chunk_end > pos && chunk_start < end {
                    let local_start = if chunk_start >= pos { 0 } else { pos - chunk_start };
                    let local_end = (end - chunk_start).min(chunk_len);
                    result.extend_from_slice(&data[local_start..local_end]);
                }
                count = chunk_end;
            }
        }
        result
    }

    /// Return the slot-ID spans covering live positions `[pos, pos + len)`.
    pub fn find_interval(&self, pos: usize, len: usize) -> Vec<Tss> {
        let mut result = Vec::new();
        let mut count = 0usize;
        let end = pos + len;
        for chunk in self.rga.iter_live() {
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

    /// Get the data-node timestamp (what the slot points to) at live position `pos`.
    pub fn get_data_ts(&self, pos: usize) -> Option<Ts> {
        let mut count = 0usize;
        for chunk in self.rga.iter_live() {
            if let Some(data) = &chunk.data {
                let chunk_len = data.len();
                if pos < count + chunk_len {
                    return Some(data[pos - count]);
                }
                count += chunk_len;
            }
        }
        None
    }

    /// View: resolve all non-deleted element IDs from the index.
    pub fn view(&self, index: &NodeIndex) -> Value {
        let mut items = Vec::new();
        for chunk in self.rga.iter_live() {
            if let Some(ids) = &chunk.data {
                for id in ids {
                    let val = match index.get(&TsKey::from(*id)) {
                        Some(node) => node.view(index),
                        None => Value::Null,
                    };
                    items.push(val);
                }
            }
        }
        Value::Array(items)
    }
}

// ── RootNode ──────────────────────────────────────────────────────────────

/// Document root — a LWW register pointing to the root JSON node.
#[derive(Debug, Clone)]
pub struct RootNode {
    pub val: Ts,
}

impl RootNode {
    pub fn new() -> Self {
        Self { val: UNDEFINED_TS }
    }

    pub fn set(&mut self, new_val: Ts) -> Option<Ts> {
        if compare(new_val, self.val) > 0 {
            let old = self.val;
            self.val = new_val;
            Some(old)
        } else {
            None
        }
    }

    pub fn view(&self, index: &NodeIndex) -> Value {
        match index.get(&TsKey::from(self.val)) {
            Some(node) => node.view(index),
            None => Value::Null,
        }
    }
}

impl Default for RootNode {
    fn default() -> Self { Self::new() }
}

// ── CrdtNode enum ─────────────────────────────────────────────────────────

/// All possible CRDT node types.
#[derive(Debug, Clone)]
pub enum CrdtNode {
    Con(ConNode),
    Val(ValNode),
    Obj(ObjNode),
    Vec(VecNode),
    Str(StrNode),
    Bin(BinNode),
    Arr(ArrNode),
}

impl CrdtNode {
    pub fn id(&self) -> Ts {
        match self {
            Self::Con(n) => n.id,
            Self::Val(n) => n.id,
            Self::Obj(n) => n.id,
            Self::Vec(n) => n.id,
            Self::Str(n) => n.id,
            Self::Bin(n) => n.id,
            Self::Arr(n) => n.id,
        }
    }

    pub fn view(&self, index: &NodeIndex) -> Value {
        match self {
            Self::Con(n) => n.view(),
            Self::Val(n) => n.view(index),
            Self::Obj(n) => n.view(index),
            Self::Vec(n) => n.view(index),
            Self::Str(n) => n.view(),
            Self::Bin(n) => n.view_json(),
            Self::Arr(n) => n.view(index),
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Con(_) => "con",
            Self::Val(_) => "val",
            Self::Obj(_) => "obj",
            Self::Vec(_) => "vec",
            Self::Str(_) => "str",
            Self::Bin(_) => "bin",
            Self::Arr(_) => "arr",
        }
    }
}

// ── NodeIndex ─────────────────────────────────────────────────────────────

/// Map from timestamp ID to CRDT node.
pub type NodeIndex = HashMap<TsKey, CrdtNode>;

/// Hashable key for Ts (since Ts doesn't implement Hash by default).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TsKey {
    pub sid: u64,
    pub time: u64,
}

impl From<Ts> for TsKey {
    fn from(ts: Ts) -> Self { Self { sid: ts.sid, time: ts.time } }
}

/// Convenience trait to look up nodes using `&Ts`.
pub trait IndexExt {
    fn get(&self, ts: &Ts) -> Option<&CrdtNode>;
    fn get_mut_ts(&mut self, ts: &Ts) -> Option<&mut CrdtNode>;
    fn insert_node(&mut self, ts: Ts, node: CrdtNode);
    fn remove_node(&mut self, ts: &Ts) -> Option<CrdtNode>;
    fn contains_ts(&self, ts: &Ts) -> bool;
}

impl IndexExt for NodeIndex {
    fn get(&self, ts: &Ts) -> Option<&CrdtNode> {
        self.get(&TsKey::from(*ts))
    }

    fn get_mut_ts(&mut self, ts: &Ts) -> Option<&mut CrdtNode> {
        self.get_mut(&TsKey::from(*ts))
    }

    fn insert_node(&mut self, ts: Ts, node: CrdtNode) {
        self.insert(TsKey::from(ts), node);
    }

    fn remove_node(&mut self, ts: &Ts) -> Option<CrdtNode> {
        self.remove(&TsKey::from(*ts))
    }

    fn contains_ts(&self, ts: &Ts) -> bool {
        self.contains_key(&TsKey::from(*ts))
    }
}

// ── Helper: PackValue → serde_json::Value ─────────────────────────────────

pub fn pack_to_json(pv: &PackValue) -> Value {
    Value::from(pv.clone())
}

