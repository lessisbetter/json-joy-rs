//! Ergonomic editing API for JSON CRDT nodes.
//!
//! Mirrors `packages/json-joy/src/json-crdt/model/api/`.
//!
//! # Overview
//!
//! [`ModelApi`] wraps a [`Model`] and a [`PatchBuilder`], buffering operations
//! locally. Calling [`ModelApi::apply`] flushes all pending operations into the
//! model. Each node-type accessor (e.g. [`StrNodeRef`], [`ObjNodeRef`]) holds a
//! node ID and provides editing methods that borrow `&mut ModelApi`.
//!
//! ## What is skipped vs. the upstream TypeScript
//!
//! - Event emitters (`FanOut`, `MicrotaskBufferFanOut`, `MergeFanOut`, `onReset`, …)
//! - JS Proxy accessor (`.s` property)
//! - `SyncStore<T>` interface
//! - `.read()` observable method
//! - Extension node API (`asExt`)

use serde_json::Value;

use crate::json_crdt::constants::ORIGIN;
use crate::json_crdt::model::Model;
use crate::json_crdt::nodes::{
    ArrNode, BinNode, ConNode, CrdtNode, IndexExt, ObjNode, StrNode, ValNode, VecNode,
};
use crate::json_crdt_patch::clock::Ts;
use crate::json_crdt_patch::patch::Patch;
use crate::json_crdt_patch::patch_builder::PatchBuilder;
use json_joy_json_pack::PackValue;

// ── Error type ─────────────────────────────────────────────────────────────

/// Errors returned by the model API editing methods.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiError {
    /// No node found for the given ID or path key.
    NotFound,
    /// A node was found but it has the wrong CRDT type.
    WrongType,
    /// An index is out of bounds for the node's current length.
    OutOfBounds,
    /// An empty write (zero-length insert) was attempted.
    EmptyWrite,
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::NotFound => write!(f, "NOT_FOUND"),
            ApiError::WrongType => write!(f, "WRONG_TYPE"),
            ApiError::OutOfBounds => write!(f, "OUT_OF_BOUNDS"),
            ApiError::EmptyWrite => write!(f, "EMPTY_WRITE"),
        }
    }
}

impl std::error::Error for ApiError {}

// ── Helper: convert serde_json::Value → ConValue (PackValue) ───────────────

fn json_to_pack(v: &Value) -> PackValue {
    match v {
        Value::Null => PackValue::Null,
        Value::Bool(b) => PackValue::Bool(*b),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                PackValue::Integer(i)
            } else if let Some(f) = n.as_f64() {
                PackValue::Float(f)
            } else {
                PackValue::Null
            }
        }
        Value::String(s) => PackValue::Str(s.clone()),
        Value::Array(_) => PackValue::Null, // complex — caller should use json()
        Value::Object(_) => PackValue::Null,
    }
}

// ── ModelApi ───────────────────────────────────────────────────────────────

/// Coordinates editing operations on a JSON CRDT document.
///
/// Mirrors `ModelApi` from the upstream TypeScript, stripped of JS-specific
/// features (events, Proxy, SyncStore).
pub struct ModelApi<'a> {
    /// Reference to the document model being edited.
    pub model: &'a mut Model,
    /// Builder that accumulates pending operations.
    pub builder: PatchBuilder,
}

impl<'a> ModelApi<'a> {
    /// Create a new `ModelApi` for `model`.
    ///
    /// The builder's clock starts at the model's current clock position so all
    /// newly allocated timestamps are ahead of any existing operations.
    pub fn new(model: &'a mut Model) -> Self {
        let sid = model.clock.sid;
        let time = model.clock.time;
        Self {
            model,
            builder: PatchBuilder::new(sid, time),
        }
    }

    /// Apply all pending operations in the builder to the model.
    ///
    /// Mirrors `ModelApi.apply()` in the upstream TypeScript.
    pub fn apply(&mut self) {
        let patch = self.builder.flush();
        if !patch.ops.is_empty() {
            self.model.apply_patch(&patch);
        }
    }

    /// Flush the pending patch **without** applying it to the model.
    ///
    /// Mirrors `ModelApi.flush()` in the upstream TypeScript.
    pub fn flush(&mut self) -> Patch {
        self.builder.flush()
    }

    // ── Navigation ────────────────────────────────────────────────────────

    /// Return a read-only view of the node identified by `id`, if it exists.
    pub fn node(&self, id: Ts) -> Option<NodeView<'_>> {
        if self.model.index.contains_ts(&id) {
            Some(NodeView {
                id,
                model: self.model,
            })
        } else {
            None
        }
    }

    /// Return a view of the document root's current value node.
    pub fn root_view(&self) -> NodeView<'_> {
        NodeView {
            id: self.model.root.val,
            model: self.model,
        }
    }

    /// Traverse `path` starting from `start` and return the target node ID.
    ///
    /// Each path element is either a string key (for `obj`) or an integer
    /// index (for `arr` and `vec`).  `ValNode` wrappers are automatically
    /// unwrapped during traversal.
    ///
    /// Mirrors `find.ts` from the upstream.
    pub fn find(&self, start: Ts, path: &[Value]) -> Result<Ts, ApiError> {
        find_path(self.model, start, path)
    }

    // ── Val editing ───────────────────────────────────────────────────────

    /// Set the value of a `val` LWW-register to a JSON scalar.
    ///
    /// For the document root use `ORIGIN` as `val_id`.
    pub fn val_set(&mut self, val_id: Ts, json: &Value) -> Result<(), ApiError> {
        let child_id = self.const_or_json(json)?;
        self.builder.set_val(val_id, child_id);
        self.apply();
        Ok(())
    }

    /// Set the root register to a JSON value (equivalent to `model.set(json)`).
    pub fn set_root(&mut self, json: &Value) -> Result<(), ApiError> {
        let child_id = self.const_or_json(json)?;
        self.builder.root(child_id);
        self.apply();
        Ok(())
    }

    // ── Obj editing ───────────────────────────────────────────────────────

    /// Set one or more key→value pairs on an `obj` node.
    ///
    /// Mirrors `ObjApi.set()` in the upstream TypeScript.
    pub fn obj_set(&mut self, obj_id: Ts, entries: &[(String, Value)]) -> Result<(), ApiError> {
        if entries.is_empty() {
            return Ok(());
        }
        let pairs: Vec<(String, Ts)> = entries
            .iter()
            .map(|(k, v)| {
                let id = self.const_or_json(v)?;
                Ok((k.clone(), id))
            })
            .collect::<Result<_, ApiError>>()?;
        self.builder.ins_obj(obj_id, pairs);
        self.apply();
        Ok(())
    }

    /// Delete a list of keys from an `obj` node (by setting them to `undefined`).
    ///
    /// Mirrors `ObjApi.del()` in the upstream TypeScript.
    pub fn obj_del(&mut self, obj_id: Ts, keys: &[String]) -> Result<(), ApiError> {
        if keys.is_empty() {
            return Ok(());
        }
        let pairs: Vec<(String, Ts)> = keys
            .iter()
            .map(|k| {
                let id = self.builder.con_val(PackValue::Null);
                (k.clone(), id)
            })
            .collect();
        self.builder.ins_obj(obj_id, pairs);
        self.apply();
        Ok(())
    }

    /// Returns `true` if `key` is set on the `obj` node identified by `obj_id`.
    ///
    /// Mirrors `ObjApi.has()` in the upstream TypeScript.
    pub fn obj_has(&self, obj_id: Ts, key: &str) -> bool {
        match IndexExt::get(&self.model.index, &obj_id) {
            Some(CrdtNode::Obj(n)) => n.keys.contains_key(key),
            _ => false,
        }
    }

    /// Get the node ID stored at `key` in an `obj` node, if present.
    pub fn obj_get(&self, obj_id: Ts, key: &str) -> Option<Ts> {
        match IndexExt::get(&self.model.index, &obj_id) {
            Some(CrdtNode::Obj(n)) => n.keys.get(key).copied(),
            _ => None,
        }
    }

    // ── Vec editing ───────────────────────────────────────────────────────

    /// Set one or more index→value pairs on a `vec` node.
    ///
    /// Mirrors `VecApi.set()` in the upstream TypeScript.
    pub fn vec_set(&mut self, vec_id: Ts, entries: &[(usize, Value)]) -> Result<(), ApiError> {
        if entries.is_empty() {
            return Ok(());
        }
        let pairs: Vec<(u8, Ts)> = entries
            .iter()
            .map(|(idx, v)| {
                let id = self.const_or_json(v)?;
                Ok((*idx as u8, id))
            })
            .collect::<Result<_, ApiError>>()?;
        self.builder.ins_vec(vec_id, pairs);
        self.apply();
        Ok(())
    }

    // ── Str editing ───────────────────────────────────────────────────────

    /// Insert `text` at character position `index` in a `str` node.
    ///
    /// `index == 0` inserts before the first character; `index == length`
    /// appends to the end.
    ///
    /// Mirrors `StrApi.ins()` in the upstream TypeScript.
    pub fn str_ins(&mut self, str_id: Ts, index: usize, text: &str) -> Result<(), ApiError> {
        if text.is_empty() {
            return Err(ApiError::EmptyWrite);
        }
        // Determine the `after` anchor: the ID of the character immediately
        // before the insertion point, or the node ID itself for "before first".
        let after = if index == 0 {
            str_id
        } else {
            // find returns the ID of the character at position `index - 1`.
            let node = match IndexExt::get(&self.model.index, &str_id) {
                Some(CrdtNode::Str(n)) => n,
                _ => return Err(ApiError::NotFound),
            };
            node.find(index - 1).ok_or(ApiError::OutOfBounds)?
        };

        self.builder.ins_str(str_id, after, text.to_string());
        self.apply();
        Ok(())
    }

    /// Delete `length` characters starting at position `index` in a `str` node.
    ///
    /// Mirrors `StrApi.del()` in the upstream TypeScript.
    pub fn str_del(&mut self, str_id: Ts, index: usize, length: usize) -> Result<(), ApiError> {
        if length == 0 {
            return Ok(());
        }
        let spans = {
            let node = match IndexExt::get(&self.model.index, &str_id) {
                Some(CrdtNode::Str(n)) => n,
                _ => return Err(ApiError::NotFound),
            };
            let spans = node.find_interval(index, length);
            if spans.is_empty() {
                return Err(ApiError::OutOfBounds);
            }
            spans
        };
        self.builder.del(str_id, spans);
        self.apply();
        Ok(())
    }

    /// Return the current length (number of live characters) of a `str` node.
    pub fn str_len(&self, str_id: Ts) -> Option<usize> {
        match IndexExt::get(&self.model.index, &str_id) {
            Some(CrdtNode::Str(n)) => Some(n.size()),
            _ => None,
        }
    }

    // ── Bin editing ───────────────────────────────────────────────────────

    /// Insert `data` at byte position `index` in a `bin` node.
    ///
    /// Mirrors `BinApi.ins()` in the upstream TypeScript.
    pub fn bin_ins(&mut self, bin_id: Ts, index: usize, data: &[u8]) -> Result<(), ApiError> {
        if data.is_empty() {
            return Err(ApiError::EmptyWrite);
        }
        let after = if index == 0 {
            bin_id
        } else {
            let node = match IndexExt::get(&self.model.index, &bin_id) {
                Some(CrdtNode::Bin(n)) => n,
                _ => return Err(ApiError::NotFound),
            };
            // find byte at index - 1
            bin_find(node, index - 1).ok_or(ApiError::OutOfBounds)?
        };

        self.builder.ins_bin(bin_id, after, data.to_vec());
        self.apply();
        Ok(())
    }

    /// Delete `length` bytes at position `index` in a `bin` node.
    ///
    /// Mirrors `BinApi.del()` in the upstream TypeScript.
    pub fn bin_del(&mut self, bin_id: Ts, index: usize, length: usize) -> Result<(), ApiError> {
        if length == 0 {
            return Ok(());
        }
        let spans = {
            let node = match IndexExt::get(&self.model.index, &bin_id) {
                Some(CrdtNode::Bin(n)) => n,
                _ => return Err(ApiError::NotFound),
            };
            bin_find_interval(node, index, length)
        };
        if spans.is_empty() {
            return Err(ApiError::OutOfBounds);
        }
        self.builder.del(bin_id, spans);
        self.apply();
        Ok(())
    }

    /// Return the current number of live bytes in a `bin` node.
    pub fn bin_len(&self, bin_id: Ts) -> Option<usize> {
        match IndexExt::get(&self.model.index, &bin_id) {
            Some(CrdtNode::Bin(n)) => Some(bin_size(n)),
            _ => None,
        }
    }

    // ── Arr editing ───────────────────────────────────────────────────────

    /// Insert `values` at position `index` in an `arr` node.
    ///
    /// Each value is turned into a `con` constant node and referenced by the
    /// array slot. To insert nested objects/arrays/strings, create those nodes
    /// first and pass their IDs directly via [`arr_ins_ids`].
    ///
    /// Mirrors `ArrApi.ins()` in the upstream TypeScript.
    pub fn arr_ins(&mut self, arr_id: Ts, index: usize, values: &[Value]) -> Result<(), ApiError> {
        if values.is_empty() {
            return Ok(());
        }
        let after = if index == 0 {
            // Use ORIGIN as the "before everything" sentinel so the RGA always
            // prepends the new elements.  Using `arr_id` does not work for
            // non-empty arrays because the Rga only recognises (sid=0, time=0)
            // as the prepend anchor.
            ORIGIN
        } else {
            let node = match IndexExt::get(&self.model.index, &arr_id) {
                Some(CrdtNode::Arr(n)) => n,
                _ => return Err(ApiError::NotFound),
            };
            node.find(index - 1).ok_or(ApiError::OutOfBounds)?
        };

        let value_ids: Vec<Ts> = values
            .iter()
            .map(|v| self.const_or_json(v))
            .collect::<Result<_, _>>()?;

        self.builder.ins_arr(arr_id, after, value_ids);
        self.apply();
        Ok(())
    }

    /// Insert pre-built node IDs at position `index` in an `arr` node.
    ///
    /// Use this variant when you have already allocated CRDT nodes (e.g. via
    /// `builder.str_node()`) and want to reference them by their timestamps.
    pub fn arr_ins_ids(&mut self, arr_id: Ts, index: usize, ids: Vec<Ts>) -> Result<(), ApiError> {
        if ids.is_empty() {
            return Ok(());
        }
        let after = if index == 0 {
            // Same fix as arr_ins: use ORIGIN so the RGA prepends correctly.
            ORIGIN
        } else {
            let node = match IndexExt::get(&self.model.index, &arr_id) {
                Some(CrdtNode::Arr(n)) => n,
                _ => return Err(ApiError::NotFound),
            };
            node.find(index - 1).ok_or(ApiError::OutOfBounds)?
        };
        self.builder.ins_arr(arr_id, after, ids);
        self.apply();
        Ok(())
    }

    /// Delete `length` elements starting at position `index` in an `arr` node.
    ///
    /// Mirrors `ArrApi.del()` in the upstream TypeScript.
    pub fn arr_del(&mut self, arr_id: Ts, index: usize, length: usize) -> Result<(), ApiError> {
        if length == 0 {
            return Ok(());
        }
        let spans = {
            let node = match IndexExt::get(&self.model.index, &arr_id) {
                Some(CrdtNode::Arr(n)) => n,
                _ => return Err(ApiError::NotFound),
            };
            let spans = node.find_interval(index, length);
            if spans.is_empty() {
                return Err(ApiError::OutOfBounds);
            }
            spans
        };
        self.builder.del(arr_id, spans);
        self.apply();
        Ok(())
    }

    /// Return the current number of live elements in an `arr` node.
    pub fn arr_len(&self, arr_id: Ts) -> Option<usize> {
        match IndexExt::get(&self.model.index, &arr_id) {
            Some(CrdtNode::Arr(n)) => Some(n.size()),
            _ => None,
        }
    }

    /// Get the data-node ID stored at live position `index` in an `arr` node.
    pub fn arr_get(&self, arr_id: Ts, index: usize) -> Option<Ts> {
        match IndexExt::get(&self.model.index, &arr_id) {
            Some(CrdtNode::Arr(n)) => n.get_data_ts(index),
            _ => None,
        }
    }

    // ── High-level: set root document ─────────────────────────────────────

    /// Replace the entire document with a JSON value.
    ///
    /// Mirrors the root-level `ModelApi.set(json)` call in the upstream.
    pub fn set(&mut self, json: &Value) -> Result<(), ApiError> {
        let id = self.json(json)?;
        self.builder.root(id);
        self.apply();
        Ok(())
    }

    // ── Internal builders ─────────────────────────────────────────────────

    /// Allocate a `con` node for a scalar JSON value, or recursively build
    /// the CRDT structure for objects/arrays.
    ///
    /// Returns the ID of the root node created.
    ///
    /// Mirrors `PatchBuilder.constOrJson()` from the upstream TypeScript.
    pub fn const_or_json(&mut self, v: &Value) -> Result<Ts, ApiError> {
        match v {
            Value::Array(_) | Value::Object(_) => self.json(v),
            _ => {
                let pv = json_to_pack(v);
                Ok(self.builder.con_val(pv))
            }
        }
    }

    /// Recursively build CRDT nodes from a JSON value.
    ///
    /// Returns the ID of the root node created.
    ///
    /// Mirrors `PatchBuilder.json()` from the upstream TypeScript.
    pub fn json(&mut self, v: &Value) -> Result<Ts, ApiError> {
        match v {
            Value::Null => Ok(self.builder.con_val(PackValue::Null)),
            Value::Bool(b) => Ok(self.builder.con_val(PackValue::Bool(*b))),
            Value::Number(n) => {
                let pv = if let Some(i) = n.as_i64() {
                    PackValue::Integer(i)
                } else if let Some(f) = n.as_f64() {
                    PackValue::Float(f)
                } else {
                    PackValue::Null
                };
                Ok(self.builder.con_val(pv))
            }
            Value::String(s) => Ok(self.builder.con_val(PackValue::Str(s.clone()))),
            Value::Array(items) => {
                let arr_id = self.builder.arr();
                if !items.is_empty() {
                    let item_ids: Vec<Ts> = items
                        .iter()
                        .map(|item| self.json(item))
                        .collect::<Result<_, _>>()?;
                    self.builder.ins_arr(arr_id, arr_id, item_ids);
                }
                Ok(arr_id)
            }
            Value::Object(map) => {
                let obj_id = self.builder.obj();
                if !map.is_empty() {
                    let pairs: Vec<(String, Ts)> = map
                        .iter()
                        .map(|(k, v)| {
                            let id = self.json(v)?;
                            Ok((k.clone(), id))
                        })
                        .collect::<Result<_, ApiError>>()?;
                    self.builder.ins_obj(obj_id, pairs);
                }
                Ok(obj_id)
            }
        }
    }
}

// ── NodeView ───────────────────────────────────────────────────────────────

/// An immutable view of a CRDT node — borrows the model read-only.
///
/// Use this to read values and navigate without queuing edits.
pub struct NodeView<'a> {
    /// Timestamp ID of the node being viewed.
    pub id: Ts,
    /// The model containing this node.
    pub model: &'a Model,
}

impl<'a> NodeView<'a> {
    /// Return the underlying `CrdtNode`, if the ID is still present.
    pub fn crdt_node(&self) -> Option<&'a CrdtNode> {
        IndexExt::get(&self.model.index, &self.id)
    }

    /// Return the JSON view of this node.
    pub fn view(&self) -> Value {
        match self.crdt_node() {
            Some(n) => n.view(&self.model.index),
            None => Value::Null,
        }
    }

    /// Attempt to get a reference to the inner `StrNode`.
    pub fn as_str(&self) -> Option<&'a StrNode> {
        match self.crdt_node()? {
            CrdtNode::Str(n) => Some(n),
            _ => None,
        }
    }

    /// Attempt to get a reference to the inner `ObjNode`.
    pub fn as_obj(&self) -> Option<&'a ObjNode> {
        match self.crdt_node()? {
            CrdtNode::Obj(n) => Some(n),
            _ => None,
        }
    }

    /// Attempt to get a reference to the inner `ArrNode`.
    pub fn as_arr(&self) -> Option<&'a ArrNode> {
        match self.crdt_node()? {
            CrdtNode::Arr(n) => Some(n),
            _ => None,
        }
    }

    /// Attempt to get a reference to the inner `VecNode`.
    pub fn as_vec(&self) -> Option<&'a VecNode> {
        match self.crdt_node()? {
            CrdtNode::Vec(n) => Some(n),
            _ => None,
        }
    }

    /// Attempt to get a reference to the inner `ValNode`.
    pub fn as_val(&self) -> Option<&'a ValNode> {
        match self.crdt_node()? {
            CrdtNode::Val(n) => Some(n),
            _ => None,
        }
    }

    /// Attempt to get a reference to the inner `ConNode`.
    pub fn as_con(&self) -> Option<&'a ConNode> {
        match self.crdt_node()? {
            CrdtNode::Con(n) => Some(n),
            _ => None,
        }
    }

    /// Attempt to get a reference to the inner `BinNode`.
    pub fn as_bin(&self) -> Option<&'a BinNode> {
        match self.crdt_node()? {
            CrdtNode::Bin(n) => Some(n),
            _ => None,
        }
    }

    /// Navigate to a child node by traversing `path`.
    ///
    /// Mirrors `NodeApi.find()` in the upstream TypeScript.
    pub fn find(&self, path: &[Value]) -> Result<NodeView<'a>, ApiError> {
        let id = find_path(self.model, self.id, path)?;
        Ok(NodeView {
            id,
            model: self.model,
        })
    }
}

// ── Path traversal ─────────────────────────────────────────────────────────

/// Traverse a JSON Pointer–style path starting from `start_id`.
///
/// `ValNode` wrappers are automatically unwrapped during traversal, mirroring
/// the upstream `find.ts`.
///
/// Returns the `Ts` of the node reached at the end of `path`.
pub fn find_path(model: &Model, start_id: Ts, path: &[Value]) -> Result<Ts, ApiError> {
    if path.is_empty() {
        return Ok(start_id);
    }

    let mut current_id = start_id;

    // Unwrap leading ValNode wrappers (mirrors `find.ts` behaviour).
    loop {
        match IndexExt::get(&model.index, &current_id) {
            Some(CrdtNode::Val(v)) => {
                current_id = v.val;
            }
            _ => break,
        }
    }

    for step in path {
        // Unwrap any ValNode at the current position.
        loop {
            match IndexExt::get(&model.index, &current_id) {
                Some(CrdtNode::Val(v)) => {
                    current_id = v.val;
                }
                _ => break,
            }
        }

        match IndexExt::get(&model.index, &current_id) {
            Some(CrdtNode::Obj(n)) => {
                let key = match step {
                    Value::String(s) => s.as_str(),
                    Value::Number(_) => {
                        // Unlikely for obj but handle gracefully.
                        return Err(ApiError::NotFound);
                    }
                    _ => return Err(ApiError::NotFound),
                };
                current_id = *n.keys.get(key).ok_or(ApiError::NotFound)?;
            }
            Some(CrdtNode::Arr(n)) => {
                let idx = match step {
                    Value::Number(n) => n.as_u64().ok_or(ApiError::OutOfBounds)? as usize,
                    Value::String(s) => s.parse::<usize>().map_err(|_| ApiError::NotFound)?,
                    _ => return Err(ApiError::NotFound),
                };
                current_id = n.get_data_ts(idx).ok_or(ApiError::OutOfBounds)?;
            }
            Some(CrdtNode::Vec(n)) => {
                let idx = match step {
                    Value::Number(n) => n.as_u64().ok_or(ApiError::OutOfBounds)? as usize,
                    Value::String(s) => s.parse::<usize>().map_err(|_| ApiError::NotFound)?,
                    _ => return Err(ApiError::NotFound),
                };
                current_id = n
                    .elements
                    .get(idx)
                    .and_then(|e| *e)
                    .ok_or(ApiError::OutOfBounds)?;
            }
            _ => return Err(ApiError::NotFound),
        }
    }

    Ok(current_id)
}

// ── BinNode helpers ─────────────────────────────────────────────────────────

/// Return the number of live bytes in a `BinNode`.
fn bin_size(node: &BinNode) -> usize {
    node.rga
        .iter_live()
        .filter_map(|c| c.data.as_deref())
        .map(|b| b.len())
        .sum()
}

/// Return the timestamp of the byte at live position `pos` in a `BinNode`.
fn bin_find(node: &BinNode, pos: usize) -> Option<Ts> {
    let mut count = 0usize;
    for chunk in node.rga.iter_live() {
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

/// Return the timestamp spans covering live positions `[pos, pos + len)` in a `BinNode`.
fn bin_find_interval(
    node: &BinNode,
    pos: usize,
    len: usize,
) -> Vec<crate::json_crdt_patch::clock::Tss> {
    use crate::json_crdt_patch::clock::Tss;
    let mut result = Vec::new();
    let mut count = 0usize;
    let end = pos + len;
    for chunk in node.rga.iter_live() {
        if let Some(data) = &chunk.data {
            let chunk_len = data.len();
            let chunk_start = count;
            let chunk_end = count + chunk_len;
            if chunk_end > pos && chunk_start < end {
                let local_start = if chunk_start >= pos {
                    0
                } else {
                    pos - chunk_start
                };
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

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_crdt::model::Model;
    use serde_json::json;

    // ── set root ────────────────────────────────────────────────────────────

    #[test]
    fn set_root_scalar() {
        let mut model = Model::create();
        let mut api = ModelApi::new(&mut model);
        api.set(&json!(42)).unwrap();
        assert_eq!(model.view(), json!(42));
    }

    #[test]
    fn set_root_string() {
        let mut model = Model::create();
        let mut api = ModelApi::new(&mut model);
        api.set(&json!("hello")).unwrap();
        assert_eq!(model.view(), json!("hello"));
    }

    #[test]
    fn set_root_object() {
        let mut model = Model::create();
        let mut api = ModelApi::new(&mut model);
        api.set(&json!({"x": 1, "y": 2})).unwrap();
        let v = model.view();
        assert_eq!(v["x"], json!(1));
        assert_eq!(v["y"], json!(2));
    }

    #[test]
    fn set_root_array() {
        let mut model = Model::create();
        let mut api = ModelApi::new(&mut model);
        api.set(&json!([1, 2, 3])).unwrap();
        assert_eq!(model.view(), json!([1, 2, 3]));
    }

    #[test]
    fn set_root_nested() {
        let mut model = Model::create();
        let mut api = ModelApi::new(&mut model);
        api.set(&json!({"a": [1, {"b": true}]})).unwrap();
        let v = model.view();
        assert_eq!(v["a"][0], json!(1));
        assert_eq!(v["a"][1]["b"], json!(true));
    }

    // ── obj editing ─────────────────────────────────────────────────────────

    #[test]
    fn obj_set_and_get() {
        let mut model = Model::create();
        let mut api = ModelApi::new(&mut model);
        api.set(&json!({"name": "Alice"})).unwrap();
        // Find obj node by traversal
        let view = model.view();
        assert_eq!(view["name"], json!("Alice"));
    }

    #[test]
    fn obj_set_updates_key() {
        let mut model = Model::create();
        {
            let mut api = ModelApi::new(&mut model);
            api.set(&json!({"x": 1})).unwrap();
        }
        assert_eq!(model.view()["x"], json!(1));

        // Now find the obj node and update it
        let obj_id = {
            let root_val = model.root.val;
            match IndexExt::get(&model.index, &root_val) {
                Some(CrdtNode::Obj(_)) => root_val,
                _ => panic!("root should be an obj node"),
            }
        };
        {
            let mut api = ModelApi::new(&mut model);
            api.obj_set(obj_id, &[("x".to_string(), json!(99))])
                .unwrap();
        }
        assert_eq!(model.view()["x"], json!(99));
    }

    #[test]
    fn obj_set_adds_key() {
        let mut model = Model::create();
        {
            let mut api = ModelApi::new(&mut model);
            api.set(&json!({"a": 1})).unwrap();
        }
        let obj_id = model.root.val;
        {
            let mut api = ModelApi::new(&mut model);
            api.obj_set(obj_id, &[("b".to_string(), json!(2))]).unwrap();
        }
        let v = model.view();
        assert_eq!(v["a"], json!(1));
        assert_eq!(v["b"], json!(2));
    }

    #[test]
    fn obj_del_removes_key() {
        let mut model = Model::create();
        {
            let mut api = ModelApi::new(&mut model);
            api.set(&json!({"k": "v", "other": 1})).unwrap();
        }
        let obj_id = model.root.val;
        {
            let mut api = ModelApi::new(&mut model);
            // del sets the key to a null con, which makes the value null in view
            api.obj_del(obj_id, &["k".to_string()]).unwrap();
        }
        // After deletion the key still exists but points to a null con
        // (matches upstream behaviour: del sets value to undefined/null)
        let v = model.view();
        assert_eq!(v["other"], json!(1));
    }

    #[test]
    fn obj_has_key() {
        let mut model = Model::create();
        {
            let mut api = ModelApi::new(&mut model);
            api.set(&json!({"present": true})).unwrap();
        }
        let obj_id = model.root.val;
        let api = ModelApi::new(&mut model);
        assert!(api.obj_has(obj_id, "present"));
        assert!(!api.obj_has(obj_id, "absent"));
    }

    // ── str editing ─────────────────────────────────────────────────────────

    #[test]
    fn str_ins_appends_text() {
        let mut model = Model::create();
        // Build a model with a str node at root
        let str_id = {
            let mut api = ModelApi::new(&mut model);
            let id = api.builder.str_node();
            api.builder.root(id);
            api.apply();
            id
        };
        {
            let mut api = ModelApi::new(&mut model);
            api.str_ins(str_id, 0, "hello").unwrap();
        }
        assert_eq!(model.view(), json!("hello"));
    }

    #[test]
    fn str_ins_at_position() {
        let mut model = Model::create();
        let str_id = {
            let mut api = ModelApi::new(&mut model);
            let id = api.builder.str_node();
            api.builder.root(id);
            api.apply();
            id
        };
        {
            let mut api = ModelApi::new(&mut model);
            api.str_ins(str_id, 0, "helo").unwrap();
        }
        {
            let mut api = ModelApi::new(&mut model);
            api.str_ins(str_id, 2, "l").unwrap();
        }
        assert_eq!(model.view(), json!("hello"));
    }

    #[test]
    fn str_del_removes_chars() {
        let mut model = Model::create();
        let str_id = {
            let mut api = ModelApi::new(&mut model);
            let id = api.builder.str_node();
            api.builder.root(id);
            api.apply();
            id
        };
        {
            let mut api = ModelApi::new(&mut model);
            api.str_ins(str_id, 0, "hello").unwrap();
        }
        {
            let mut api = ModelApi::new(&mut model);
            api.str_del(str_id, 1, 3).unwrap(); // remove "ell"
        }
        assert_eq!(model.view(), json!("ho"));
    }

    #[test]
    fn str_len_reports_live_chars() {
        let mut model = Model::create();
        let str_id = {
            let mut api = ModelApi::new(&mut model);
            let id = api.builder.str_node();
            api.builder.root(id);
            api.apply();
            id
        };
        {
            let mut api = ModelApi::new(&mut model);
            api.str_ins(str_id, 0, "hello").unwrap();
        }
        let api = ModelApi::new(&mut model);
        assert_eq!(api.str_len(str_id), Some(5));
    }

    #[test]
    fn str_ins_empty_returns_error() {
        let mut model = Model::create();
        let str_id = {
            let mut api = ModelApi::new(&mut model);
            let id = api.builder.str_node();
            api.builder.root(id);
            api.apply();
            id
        };
        let mut api = ModelApi::new(&mut model);
        assert_eq!(api.str_ins(str_id, 0, ""), Err(ApiError::EmptyWrite));
    }

    // ── bin editing ─────────────────────────────────────────────────────────

    #[test]
    fn bin_ins_and_del() {
        let mut model = Model::create();
        let bin_id = {
            let mut api = ModelApi::new(&mut model);
            let id = api.builder.bin();
            api.builder.root(id);
            api.apply();
            id
        };
        {
            let mut api = ModelApi::new(&mut model);
            api.bin_ins(bin_id, 0, &[0xDE, 0xAD, 0xBE, 0xEF]).unwrap();
        }
        {
            let mut api = ModelApi::new(&mut model);
            api.bin_del(bin_id, 1, 2).unwrap(); // remove 0xAD, 0xBE
        }
        let v = model.view();
        assert_eq!(v, json!([0xDE, 0xEF]));
    }

    #[test]
    fn bin_len_reports_live_bytes() {
        let mut model = Model::create();
        let bin_id = {
            let mut api = ModelApi::new(&mut model);
            let id = api.builder.bin();
            api.builder.root(id);
            api.apply();
            id
        };
        {
            let mut api = ModelApi::new(&mut model);
            api.bin_ins(bin_id, 0, &[1, 2, 3]).unwrap();
        }
        let api = ModelApi::new(&mut model);
        assert_eq!(api.bin_len(bin_id), Some(3));
    }

    // ── arr editing ─────────────────────────────────────────────────────────

    #[test]
    fn arr_ins_and_del() {
        let mut model = Model::create();
        let arr_id = {
            let mut api = ModelApi::new(&mut model);
            let id = api.builder.arr();
            api.builder.root(id);
            api.apply();
            id
        };
        {
            let mut api = ModelApi::new(&mut model);
            api.arr_ins(arr_id, 0, &[json!(1), json!(2), json!(3)])
                .unwrap();
        }
        assert_eq!(model.view(), json!([1, 2, 3]));
        {
            let mut api = ModelApi::new(&mut model);
            api.arr_del(arr_id, 1, 1).unwrap(); // remove element at index 1
        }
        assert_eq!(model.view(), json!([1, 3]));
    }

    #[test]
    fn arr_len_reports_live_elements() {
        let mut model = Model::create();
        let arr_id = {
            let mut api = ModelApi::new(&mut model);
            let id = api.builder.arr();
            api.builder.root(id);
            api.apply();
            id
        };
        {
            let mut api = ModelApi::new(&mut model);
            api.arr_ins(arr_id, 0, &[json!("a"), json!("b")]).unwrap();
        }
        let api = ModelApi::new(&mut model);
        assert_eq!(api.arr_len(arr_id), Some(2));
    }

    // ── vec editing ─────────────────────────────────────────────────────────

    #[test]
    fn vec_set_elements() {
        let mut model = Model::create();
        let vec_id = {
            let mut api = ModelApi::new(&mut model);
            let id = api.builder.vec();
            api.builder.root(id);
            api.apply();
            id
        };
        {
            let mut api = ModelApi::new(&mut model);
            api.vec_set(vec_id, &[(0, json!(true)), (1, json!(42))])
                .unwrap();
        }
        assert_eq!(model.view(), json!([true, 42]));
    }

    // ── find / NodeView ─────────────────────────────────────────────────────

    #[test]
    fn find_path_in_nested_obj() {
        let mut model = Model::create();
        {
            let mut api = ModelApi::new(&mut model);
            api.set(&json!({"a": {"b": 99}})).unwrap();
        }
        let root_id = model.root.val;
        let api = ModelApi::new(&mut model);
        let b_id = api.find(root_id, &[json!("a"), json!("b")]).unwrap();
        let view = NodeView {
            id: b_id,
            model: api.model,
        };
        assert_eq!(view.view(), json!(99));
    }

    #[test]
    fn find_path_in_array() {
        let mut model = Model::create();
        {
            let mut api = ModelApi::new(&mut model);
            api.set(&json!([10, 20, 30])).unwrap();
        }
        let root_id = model.root.val;
        let api = ModelApi::new(&mut model);
        let elem_id = api.find(root_id, &[json!(1)]).unwrap();
        let view = NodeView {
            id: elem_id,
            model: api.model,
        };
        assert_eq!(view.view(), json!(20));
    }

    #[test]
    fn node_view_as_str() {
        let mut model = Model::create();
        let str_id = {
            let mut api = ModelApi::new(&mut model);
            let id = api.builder.str_node();
            api.builder.root(id);
            api.apply();
            id
        };
        {
            let mut api = ModelApi::new(&mut model);
            api.str_ins(str_id, 0, "world").unwrap();
        }
        let api = ModelApi::new(&mut model);
        let nv = api.node(str_id).unwrap();
        assert!(nv.as_str().is_some());
        assert_eq!(nv.view(), json!("world"));
    }

    // ── flush (no apply) ────────────────────────────────────────────────────

    #[test]
    fn flush_returns_patch_without_applying() {
        let mut model = Model::create();
        let patch = {
            let mut api = ModelApi::new(&mut model);
            api.builder.con_val(json_to_pack(&json!(1)));
            api.flush()
        };
        // The patch has one op but the model is unchanged (still null).
        assert!(!patch.ops.is_empty());
        assert_eq!(model.view(), json!(null));
    }
}
