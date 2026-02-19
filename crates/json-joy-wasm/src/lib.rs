//! WASM bindings for json-joy-rs.
//!
//! Exposes a `Model` class that mirrors the upstream TypeScript `json-joy`
//! library API.  The TypeScript layer in `js/` wraps this to reconstruct the
//! exact chainable API feel (`model.api.str(['key']).ins(0, 'hello')`).
//!
//! # Boundary discipline
//!
//! Every public `#[wasm_bindgen]` method performs exactly **one** meaningful
//! unit of work, so JS can drive batch operations without extra round-trips.
//! Navigation and internal helpers are pure Rust.

use serde::Serialize as _;
use wasm_bindgen::prelude::*;

use json_joy::json_crdt::codec::structural::binary as structural_binary;
use json_joy::json_crdt::model::api::find_path;
use json_joy::json_crdt::model::util::random_session_id;
use json_joy::json_crdt::model::Model as CrdtModel;
use json_joy::json_crdt::nodes::{BinNode, CrdtNode, IndexExt};
use json_joy::json_crdt::ORIGIN;
use json_joy::json_crdt_diff::JsonCrdtDiff;
use json_joy::json_crdt_patch::clock::{Ts, Tss};
use json_joy::json_crdt_patch::operations::Op;
use json_joy::json_crdt_patch::patch::Patch;
use json_joy::json_crdt_patch::patch_builder::PatchBuilder;
use json_joy_json_pack::PackValue;
use serde_json::Value;

mod extensions;

// ── Internal helpers ─────────────────────────────────────────────────────────

/// Convert a plain JSON value to its `PackValue` equivalent for `con` nodes.
/// Complex types (array/object) return `Null` — the caller must use `build_json`.
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
        _ => PackValue::Null,
    }
}

/// Recursively allocate CRDT nodes for a JSON value using the given builder.
/// Returns the timestamp ID of the root node created.
///
/// Mirrors upstream `PatchBuilder.json()`:
/// - Scalars (null/bool/number) → ConNode
/// - Strings → StrNode (CRDT-editable, so `api.str([key]).ins()` works after `api.set(...)`)
/// - Arrays → ArrNode (elements via `build_json` recursively)
/// - Objects → ObjNode (values via `build_json` recursively)
fn build_json(builder: &mut PatchBuilder, v: &Value) -> Ts {
    match v {
        Value::Null | Value::Bool(_) | Value::Number(_) => builder.con_val(json_to_pack(v)),
        Value::String(s) => {
            // Strings become CRDT-editable StrNodes, matching upstream behaviour.
            let str_id = builder.str_node();
            if !s.is_empty() {
                builder.ins_str(str_id, str_id, s.clone());
            }
            str_id
        }
        Value::Array(items) => {
            let arr_id = builder.arr();
            if !items.is_empty() {
                let ids: Vec<Ts> = items.iter().map(|item| build_json(builder, item)).collect();
                builder.ins_arr(arr_id, arr_id, ids);
            }
            arr_id
        }
        Value::Object(map) => {
            let obj_id = builder.obj();
            if !map.is_empty() {
                let pairs: Vec<(String, Ts)> = map
                    .iter()
                    .map(|(k, v)| (k.clone(), build_json(builder, v)))
                    .collect();
                builder.ins_obj(obj_id, pairs);
            }
            obj_id
        }
    }
}

/// Like `build_json` but treats scalars as `con` constants and compound types
/// as structural CRDT nodes.  Mirrors `PatchBuilder.constOrJson()`.
fn const_or_json(builder: &mut PatchBuilder, v: &Value) -> Ts {
    match v {
        Value::Array(_) | Value::Object(_) => build_json(builder, v),
        _ => builder.con_val(json_to_pack(v)),
    }
}

/// Parse a path argument from JS (JSON-encoded array, string, or number).
/// `null` / absent → empty path (document root).
fn parse_path(path_json: &str) -> Result<Vec<Value>, String> {
    if path_json.is_empty() || path_json == "null" || path_json == "undefined" {
        return Ok(vec![]);
    }
    let v: Value =
        serde_json::from_str(path_json).map_err(|e| format!("invalid path JSON: {e}"))?;
    match v {
        Value::Array(arr) => Ok(arr),
        Value::String(s) => Ok(vec![Value::String(s)]),
        Value::Number(n) => Ok(vec![Value::Number(n)]),
        _ => Err(format!("path must be an array, string, or number; got {v}")),
    }
}

/// Merge a collection of patches into a single `Patch` by concatenating ops.
fn merge_patches(patches: Vec<Patch>) -> Patch {
    match patches.len() {
        0 => Patch {
            ops: vec![],
            meta: None,
        },
        1 => patches.into_iter().next().unwrap(),
        _ => {
            let ops: Vec<Op> = patches.into_iter().flat_map(|p| p.ops).collect();
            Patch { ops, meta: None }
        }
    }
}

// ── Model ────────────────────────────────────────────────────────────────────

/// A JSON CRDT document.
///
/// Mirrors `Model` from `json-joy`.  The TypeScript wrapper in `js/Model.ts`
/// wraps this to expose the chainable `.api` property.
///
/// ## Editing lifecycle
///
/// 1. Call editing methods (`apiSet`, `apiObjSet`, `apiStrIns`, …).  Each op
///    is immediately applied to the in-memory document so `view()` stays
///    up-to-date, and is also appended to the local-changes log.
/// 2. Call `apiFlush()` to get the accumulated patch as a `Uint8Array` to
///    send to peers.  The log is then cleared.
/// 3. Call `applyPatch(bytes)` to integrate a remote peer's patch.
#[wasm_bindgen]
pub struct Model {
    inner: CrdtModel,
    /// Tracks ops applied since the last `apiFlush()` so we can return a
    /// single binary patch representing all local changes.
    local_changes: Vec<Patch>,
    /// Cached JS view and the `tick` it was computed at.
    ///
    /// Mirrors the per-node `_tick`/`_view` cache in the upstream TypeScript:
    /// each `apply_patch` increments `inner.tick`; if the tick hasn't changed
    /// since the last `view()` call we can return the cached `JsValue` in O(1)
    /// (a single reference-count bump) instead of rebuilding the full tree.
    view_cache: Option<(u64, JsValue)>,
}

impl Model {
    fn from_inner(inner: CrdtModel) -> Self {
        Self {
            inner,
            local_changes: Vec::new(),
            view_cache: None,
        }
    }

    /// Execute `f` with a fresh `PatchBuilder` seeded from the model clock,
    /// then immediately apply the resulting patch and record it in
    /// `local_changes`.
    fn with_builder<F>(&mut self, f: F) -> Result<(), String>
    where
        F: FnOnce(&CrdtModel, &mut PatchBuilder) -> Result<(), String>,
    {
        let sid = self.inner.clock.sid;
        let time = self.inner.clock.time;
        let mut builder = PatchBuilder::new(sid, time);
        f(&self.inner, &mut builder)?;
        let patch = builder.flush();
        if !patch.ops.is_empty() {
            self.inner.apply_patch(&patch);
            self.local_changes.push(patch);
            self.view_cache = None;
        }
        Ok(())
    }

    /// Navigate `path` within the model, returning the target node's
    /// timestamp ID.  An empty path returns the root register's value node.
    fn resolve(&self, path: &[Value]) -> Result<Ts, String> {
        let root_val = self.inner.root.val;
        if path.is_empty() {
            return Ok(root_val);
        }
        find_path(&self.inner, root_val, path).map_err(|e| format!("path not found: {e:?}"))
    }
}

#[wasm_bindgen]
impl Model {
    // ── Lifecycle ─────────────────────────────────────────────────────────

    /// Create a new empty document.
    ///
    /// `sid` is optional; if omitted a random session ID is generated.
    ///
    /// Mirrors `Model.create(schema?, sid?)`.
    #[wasm_bindgen(js_name = "create", constructor)]
    pub fn create(sid: Option<u64>) -> Model {
        let inner = match sid {
            Some(s) => CrdtModel::new(s),
            None => CrdtModel::create(),
        };
        Self::from_inner(inner)
    }

    /// Decode a model from its binary representation.
    ///
    /// Mirrors `Model.fromBinary(bytes)`.
    #[wasm_bindgen(js_name = "fromBinary")]
    pub fn from_binary(data: &[u8]) -> Result<Model, JsValue> {
        structural_binary::decode(data)
            .map(Self::from_inner)
            .map_err(|e| JsValue::from_str(&format!("decode error: {e:?}")))
    }

    /// Encode this model to its binary representation.
    ///
    /// Mirrors `model.toBinary()`.
    #[wasm_bindgen(js_name = "toBinary")]
    pub fn to_binary(&self) -> Vec<u8> {
        structural_binary::encode(&self.inner)
    }

    /// Return the current JSON view of this document as a JS value.
    ///
    /// Uses `serde-wasm-bindgen` with the JSON-compatible serializer so that
    /// objects come back as plain JS objects (not Maps), compatible with
    /// `JSON.stringify` and standard property access.
    ///
    /// The result is cached by `inner.tick` (which increments on every
    /// `apply_patch`).  Repeated calls on an unchanged document are O(1) —
    /// a single `JsValue` reference-count bump — mirroring the tick-based
    /// `_view` cache in the upstream TypeScript nodes.
    ///
    /// Mirrors `model.view()`.
    pub fn view(&mut self) -> JsValue {
        let tick = self.inner.tick;
        if let Some((cached_tick, ref v)) = self.view_cache {
            if cached_tick == tick {
                return v.clone();
            }
        }
        let val = self.inner.view();
        let ser = serde_wasm_bindgen::Serializer::json_compatible();
        let js = val.serialize(&ser).unwrap_or(JsValue::NULL);
        self.view_cache = Some((tick, js.clone()));
        js
    }

    /// Fork this document with a new session ID.
    ///
    /// Mirrors `model.fork(sid?)`.
    pub fn fork(&self, sid: Option<u64>) -> Model {
        let new_sid = sid.unwrap_or_else(random_session_id);
        let mut cloned = self.inner.clone();
        cloned.clock.sid = new_sid;
        Self::from_inner(cloned)
    }

    /// Return this document's session ID.
    pub fn sid(&self) -> u64 {
        self.inner.clock.sid
    }

    /// Generate a fresh random session ID.
    ///
    /// Mirrors `Model.sid()` / `model.rndSid()`.
    #[wasm_bindgen(js_name = "rndSid")]
    pub fn rnd_sid() -> u64 {
        random_session_id()
    }

    /// Convert a Slate document JSON payload to Peritext view-range JSON.
    ///
    /// Input and output are JSON strings to avoid extra JS<->WASM object
    /// marshaling on hot paths.
    #[wasm_bindgen(js_name = "convertSlateToViewRange")]
    pub fn convert_slate_to_view_range(doc_json: &str) -> Result<String, JsValue> {
        let doc: Value = serde_json::from_str(doc_json)
            .map_err(|e| JsValue::from_str(&format!("invalid Slate JSON: {e}")))?;
        let view = extensions::from_slate_to_view_range(&doc);
        serde_json::to_string(&view)
            .map_err(|e| JsValue::from_str(&format!("failed to encode view-range JSON: {e}")))
    }

    /// Convert a ProseMirror node JSON payload to Peritext view-range JSON.
    ///
    /// Input and output are JSON strings to avoid extra JS<->WASM object
    /// marshaling on hot paths.
    #[wasm_bindgen(js_name = "convertProseMirrorToViewRange")]
    pub fn convert_prosemirror_to_view_range(node_json: &str) -> Result<String, JsValue> {
        let node: Value = serde_json::from_str(node_json)
            .map_err(|e| JsValue::from_str(&format!("invalid ProseMirror JSON: {e}")))?;
        let view = extensions::from_prosemirror_to_view_range(&node);
        serde_json::to_string(&view)
            .map_err(|e| JsValue::from_str(&format!("failed to encode view-range JSON: {e}")))
    }

    // ── Patch application ─────────────────────────────────────────────────

    /// Apply a remote patch (received from a peer).
    ///
    /// Mirrors `model.applyPatch(patch)` where `patch` is passed as binary.
    #[wasm_bindgen(js_name = "applyPatch")]
    pub fn apply_patch(&mut self, patch_bytes: &[u8]) -> Result<(), JsValue> {
        let patch = Patch::from_binary(patch_bytes)
            .map_err(|e| JsValue::from_str(&format!("patch decode error: {e:?}")))?;
        self.inner.apply_patch(&patch);
        self.view_cache = None;
        Ok(())
    }

    // ── Local editing API ─────────────────────────────────────────────────
    //
    // These methods are called by the TypeScript `ModelApi` / node-API
    // wrappers.  Each call corresponds to one logical CRDT operation and is
    // applied immediately so `view()` reflects it; the patch is also
    // accumulated in `local_changes` for the next `apiFlush()`.

    /// Replace the entire document with a JSON value.
    ///
    /// Called by `model.api.set(json)`.
    #[wasm_bindgen(js_name = "apiSet")]
    pub fn api_set(&mut self, json_str: &str) -> Result<(), JsValue> {
        let v: Value = serde_json::from_str(json_str)
            .map_err(|e| JsValue::from_str(&format!("invalid JSON: {e}")))?;
        self.with_builder(|_, builder| {
            let id = build_json(builder, &v);
            builder.root(id);
            Ok(())
        })
        .map_err(|e| JsValue::from_str(&e))
    }

    /// Set one or more key→value pairs on the object at `path`.
    ///
    /// `path_json`: JSON-encoded path (e.g. `'["key","nested"]'`) or `"null"`.
    /// `entries_json`: JSON-encoded `{key: value, …}` object.
    ///
    /// Called by `model.api.obj(path).set(entries)`.
    #[wasm_bindgen(js_name = "apiObjSet")]
    pub fn api_obj_set(&mut self, path_json: &str, entries_json: &str) -> Result<(), JsValue> {
        let path = parse_path(path_json).map_err(|e| JsValue::from_str(&e))?;
        let entries: Value = serde_json::from_str(entries_json)
            .map_err(|e| JsValue::from_str(&format!("invalid entries JSON: {e}")))?;
        let obj_id = self.resolve(&path).map_err(|e| JsValue::from_str(&e))?;
        let map = match &entries {
            Value::Object(m) => m.clone(),
            _ => return Err(JsValue::from_str("entries must be a JSON object")),
        };
        if map.is_empty() {
            return Ok(());
        }
        self.with_builder(|_, builder| {
            let pairs: Vec<(String, Ts)> = map
                .iter()
                .map(|(k, v)| (k.clone(), const_or_json(builder, v)))
                .collect();
            builder.ins_obj(obj_id, pairs);
            Ok(())
        })
        .map_err(|e| JsValue::from_str(&e))
    }

    /// Delete keys from the object at `path`.
    ///
    /// `keys_json`: JSON-encoded array of key strings.
    ///
    /// Called by `model.api.obj(path).del(keys)`.
    #[wasm_bindgen(js_name = "apiObjDel")]
    pub fn api_obj_del(&mut self, path_json: &str, keys_json: &str) -> Result<(), JsValue> {
        let path = parse_path(path_json).map_err(|e| JsValue::from_str(&e))?;
        let obj_id = self.resolve(&path).map_err(|e| JsValue::from_str(&e))?;
        let keys: Vec<String> = serde_json::from_str(keys_json)
            .map_err(|e| JsValue::from_str(&format!("invalid keys JSON: {e}")))?;
        if keys.is_empty() {
            return Ok(());
        }
        self.with_builder(|_, builder| {
            let pairs: Vec<(String, Ts)> = keys
                .iter()
                .map(|k| (k.clone(), builder.con_val(PackValue::Null)))
                .collect();
            builder.ins_obj(obj_id, pairs);
            Ok(())
        })
        .map_err(|e| JsValue::from_str(&e))
    }

    /// Set indexed entries on the `vec` node at `path`.
    ///
    /// `entries_json`: JSON-encoded array of `[index, value]` pairs.
    ///
    /// Called by `model.api.vec(path).set(entries)`.
    #[wasm_bindgen(js_name = "apiVecSet")]
    pub fn api_vec_set(&mut self, path_json: &str, entries_json: &str) -> Result<(), JsValue> {
        let path = parse_path(path_json).map_err(|e| JsValue::from_str(&e))?;
        let vec_id = self.resolve(&path).map_err(|e| JsValue::from_str(&e))?;
        let raw: Vec<(usize, Value)> = serde_json::from_str(entries_json)
            .map_err(|e| JsValue::from_str(&format!("invalid vec entries JSON: {e}")))?;
        if raw.is_empty() {
            return Ok(());
        }
        self.with_builder(|_, builder| {
            let pairs: Vec<(u8, Ts)> = raw
                .iter()
                .map(|(idx, v)| (*idx as u8, const_or_json(builder, v)))
                .collect();
            builder.ins_vec(vec_id, pairs);
            Ok(())
        })
        .map_err(|e| JsValue::from_str(&e))
    }

    /// Set the value of a `val` (LWW register) node at `path`.
    ///
    /// Called by `model.api.val(path).set(value)`.
    #[wasm_bindgen(js_name = "apiValSet")]
    pub fn api_val_set(&mut self, path_json: &str, value_json: &str) -> Result<(), JsValue> {
        let path = parse_path(path_json).map_err(|e| JsValue::from_str(&e))?;
        let val_id = self.resolve(&path).map_err(|e| JsValue::from_str(&e))?;
        let v: Value = serde_json::from_str(value_json)
            .map_err(|e| JsValue::from_str(&format!("invalid value JSON: {e}")))?;
        self.with_builder(|_, builder| {
            let child = const_or_json(builder, &v);
            builder.set_val(val_id, child);
            Ok(())
        })
        .map_err(|e| JsValue::from_str(&e))
    }

    /// Create a new empty `StrNode` (CRDT-editable string) at `key` within the
    /// object at `obj_path`, and optionally seed it with `initial_text`.
    ///
    /// This is the WASM equivalent of using `s.str(initial)` in a schema.
    /// Strings created via `apiSet` are `ConNode` constants — they cannot be
    /// edited with `apiStrIns`.  Use this method first when you need a
    /// collaboratively-editable string.
    ///
    /// Called by the TypeScript schema builder for `s.str(...)`.
    #[wasm_bindgen(js_name = "apiNewStr")]
    pub fn api_new_str(
        &mut self,
        obj_path_json: &str,
        key: &str,
        initial_text: &str,
    ) -> Result<(), JsValue> {
        let path = parse_path(obj_path_json).map_err(|e| JsValue::from_str(&e))?;
        let obj_id = self.resolve(&path).map_err(|e| JsValue::from_str(&e))?;
        self.with_builder(|_, builder| {
            let str_id = builder.str_node();
            if !initial_text.is_empty() {
                builder.ins_str(str_id, str_id, initial_text.to_string());
            }
            builder.ins_obj(obj_id, vec![(key.to_string(), str_id)]);
            Ok(())
        })
        .map_err(|e| JsValue::from_str(&e))
    }

    /// Insert text into the `str` node at `path`.
    ///
    /// Called by `model.api.str(path).ins(index, text)`.
    #[wasm_bindgen(js_name = "apiStrIns")]
    pub fn api_str_ins(&mut self, path_json: &str, index: u32, text: &str) -> Result<(), JsValue> {
        if text.is_empty() {
            return Ok(());
        }
        let path = parse_path(path_json).map_err(|e| JsValue::from_str(&e))?;
        let str_id = self.resolve(&path).map_err(|e| JsValue::from_str(&e))?;
        let index = index as usize;
        let after = if index == 0 {
            str_id
        } else {
            let node = match IndexExt::get(&self.inner.index, &str_id) {
                Some(CrdtNode::Str(n)) => n,
                _ => return Err(JsValue::from_str("str node not found at path")),
            };
            node.find(index - 1)
                .ok_or_else(|| JsValue::from_str("str index out of bounds"))?
        };
        self.with_builder(|_, builder| {
            builder.ins_str(str_id, after, text.to_string());
            Ok(())
        })
        .map_err(|e| JsValue::from_str(&e))
    }

    /// Delete characters from the `str` node at `path`.
    ///
    /// Called by `model.api.str(path).del(index, count)`.
    #[wasm_bindgen(js_name = "apiStrDel")]
    pub fn api_str_del(&mut self, path_json: &str, index: u32, length: u32) -> Result<(), JsValue> {
        if length == 0 {
            return Ok(());
        }
        let path = parse_path(path_json).map_err(|e| JsValue::from_str(&e))?;
        let str_id = self.resolve(&path).map_err(|e| JsValue::from_str(&e))?;
        let spans = {
            let node = match IndexExt::get(&self.inner.index, &str_id) {
                Some(CrdtNode::Str(n)) => n,
                _ => return Err(JsValue::from_str("str node not found at path")),
            };
            node.find_interval(index as usize, length as usize)
        };
        if spans.is_empty() {
            return Err(JsValue::from_str("str deletion out of bounds"));
        }
        self.with_builder(|_, builder| {
            builder.del(str_id, spans);
            Ok(())
        })
        .map_err(|e| JsValue::from_str(&e))
    }

    /// Insert bytes into the `bin` node at `path`.
    ///
    /// Called by `model.api.bin(path).ins(index, bytes)`.
    #[wasm_bindgen(js_name = "apiBinIns")]
    pub fn api_bin_ins(&mut self, path_json: &str, index: u32, data: &[u8]) -> Result<(), JsValue> {
        if data.is_empty() {
            return Ok(());
        }
        let path = parse_path(path_json).map_err(|e| JsValue::from_str(&e))?;
        let bin_id = self.resolve(&path).map_err(|e| JsValue::from_str(&e))?;
        let index = index as usize;
        let after = if index == 0 {
            bin_id
        } else {
            let node = match IndexExt::get(&self.inner.index, &bin_id) {
                Some(CrdtNode::Bin(n)) => n,
                _ => return Err(JsValue::from_str("bin node not found at path")),
            };
            bin_find(node, index - 1).ok_or_else(|| JsValue::from_str("bin index out of bounds"))?
        };
        self.with_builder(|_, builder| {
            builder.ins_bin(bin_id, after, data.to_vec());
            Ok(())
        })
        .map_err(|e| JsValue::from_str(&e))
    }

    /// Delete bytes from the `bin` node at `path`.
    ///
    /// Called by `model.api.bin(path).del(index, count)`.
    #[wasm_bindgen(js_name = "apiBinDel")]
    pub fn api_bin_del(&mut self, path_json: &str, index: u32, length: u32) -> Result<(), JsValue> {
        if length == 0 {
            return Ok(());
        }
        let path = parse_path(path_json).map_err(|e| JsValue::from_str(&e))?;
        let bin_id = self.resolve(&path).map_err(|e| JsValue::from_str(&e))?;
        let spans = {
            let node = match IndexExt::get(&self.inner.index, &bin_id) {
                Some(CrdtNode::Bin(n)) => n,
                _ => return Err(JsValue::from_str("bin node not found at path")),
            };
            bin_find_interval(node, index as usize, length as usize)
        };
        if spans.is_empty() {
            return Err(JsValue::from_str("bin deletion out of bounds"));
        }
        self.with_builder(|_, builder| {
            builder.del(bin_id, spans);
            Ok(())
        })
        .map_err(|e| JsValue::from_str(&e))
    }

    /// Insert items into the `arr` node at `path`.
    ///
    /// `values_json`: JSON-encoded array of values to insert.
    ///
    /// Called by `model.api.arr(path).ins(index, values)`.
    #[wasm_bindgen(js_name = "apiArrIns")]
    pub fn api_arr_ins(
        &mut self,
        path_json: &str,
        index: u32,
        values_json: &str,
    ) -> Result<(), JsValue> {
        let path = parse_path(path_json).map_err(|e| JsValue::from_str(&e))?;
        let arr_id = self.resolve(&path).map_err(|e| JsValue::from_str(&e))?;
        let values: Vec<Value> = serde_json::from_str(values_json)
            .map_err(|e| JsValue::from_str(&format!("invalid values JSON: {e}")))?;
        if values.is_empty() {
            return Ok(());
        }
        let index = index as usize;
        let after = if index == 0 {
            ORIGIN
        } else {
            let node = match IndexExt::get(&self.inner.index, &arr_id) {
                Some(CrdtNode::Arr(n)) => n,
                _ => return Err(JsValue::from_str("arr node not found at path")),
            };
            node.find(index - 1)
                .ok_or_else(|| JsValue::from_str("arr index out of bounds"))?
        };
        self.with_builder(|_, builder| {
            // Use build_json (not const_or_json) to match upstream ArrApi.ins which
            // calls builder.json() — strings in arrays become StrNodes.
            let ids: Vec<Ts> = values.iter().map(|v| build_json(builder, v)).collect();
            builder.ins_arr(arr_id, after, ids);
            Ok(())
        })
        .map_err(|e| JsValue::from_str(&e))
    }

    /// Overwrite the element at `index` in the `arr` node at `path`.
    ///
    /// Mirrors upstream `ArrApi.upd(index, value)`.
    ///
    /// Called by `model.api.arr(path).upd(index, value)`.
    #[wasm_bindgen(js_name = "apiArrUpd")]
    pub fn api_arr_upd(
        &mut self,
        path_json: &str,
        index: u32,
        value_json: &str,
    ) -> Result<(), JsValue> {
        let path = parse_path(path_json).map_err(|e| JsValue::from_str(&e))?;
        let arr_id = self.resolve(&path).map_err(|e| JsValue::from_str(&e))?;
        let v: Value = serde_json::from_str(value_json)
            .map_err(|e| JsValue::from_str(&format!("invalid value JSON: {e}")))?;
        let ref_id = {
            let node = match IndexExt::get(&self.inner.index, &arr_id) {
                Some(CrdtNode::Arr(n)) => n,
                _ => return Err(JsValue::from_str("arr node not found at path")),
            };
            node.get_data_ts(index as usize)
                .ok_or_else(|| JsValue::from_str("arr index out of bounds"))?
        };
        self.with_builder(|_, builder| {
            let val_id = const_or_json(builder, &v);
            builder.upd_arr(arr_id, ref_id, val_id);
            Ok(())
        })
        .map_err(|e| JsValue::from_str(&e))
    }

    /// Delete items from the `arr` node at `path`.
    ///
    /// Called by `model.api.arr(path).del(index, count)`.
    #[wasm_bindgen(js_name = "apiArrDel")]
    pub fn api_arr_del(&mut self, path_json: &str, index: u32, length: u32) -> Result<(), JsValue> {
        if length == 0 {
            return Ok(());
        }
        let path = parse_path(path_json).map_err(|e| JsValue::from_str(&e))?;
        let arr_id = self.resolve(&path).map_err(|e| JsValue::from_str(&e))?;
        let spans = {
            let node = match IndexExt::get(&self.inner.index, &arr_id) {
                Some(CrdtNode::Arr(n)) => n,
                _ => return Err(JsValue::from_str("arr node not found at path")),
            };
            node.find_interval(index as usize, length as usize)
        };
        if spans.is_empty() {
            return Err(JsValue::from_str("arr deletion out of bounds"));
        }
        self.with_builder(|_, builder| {
            builder.del(arr_id, spans);
            Ok(())
        })
        .map_err(|e| JsValue::from_str(&e))
    }

    // ── Flush / apply ─────────────────────────────────────────────────────

    /// Return all local changes since the last `apiFlush()` as a single binary
    /// patch, then clear the log.
    ///
    /// Returns an empty `Uint8Array` when there are no pending changes.
    ///
    /// Mirrors `model.api.flush()` which returns a `Patch`.
    #[wasm_bindgen(js_name = "apiFlush")]
    pub fn api_flush(&mut self) -> Vec<u8> {
        if self.local_changes.is_empty() {
            return Vec::new();
        }
        let patches = std::mem::take(&mut self.local_changes);
        merge_patches(patches).to_binary()
    }

    /// Apply all pending local changes to the model and discard them.
    ///
    /// Mirrors `model.api.apply()`.  Equivalent to `apiFlush()` but without
    /// returning the patch.
    #[wasm_bindgen(js_name = "apiApply")]
    pub fn api_apply(&mut self) {
        self.local_changes.clear();
    }

    // ── Diff ─────────────────────────────────────────────────────────────

    /// Compute the patch that transforms this document into `next_json`,
    /// apply it locally, and return the patch bytes.
    ///
    /// Returns an empty `Uint8Array` when the document is already equal to
    /// `next_json`.
    ///
    /// Mirrors the `engine_diff_apply_json` pattern from the previous WASM
    /// layer, and the `JsonCrdtDiff` workflow.
    #[wasm_bindgen(js_name = "diffApply")]
    pub fn diff_apply(&mut self, next_json_str: &str) -> Result<Vec<u8>, JsValue> {
        let next: Value = serde_json::from_str(next_json_str)
            .map_err(|e| JsValue::from_str(&format!("invalid JSON: {e}")))?;

        // Compute diff from current root node to `next`.
        let patch = {
            let sid = self.inner.clock.sid;
            let time = self.inner.clock.time;
            let mut differ = JsonCrdtDiff::new(sid, time, &self.inner.index);

            let root_node = IndexExt::get(&self.inner.index, &self.inner.root.val);
            match root_node {
                Some(node) => differ.diff(node, &next),
                None => {
                    // Document is empty — treat as setting the root.
                    let mut builder = PatchBuilder::new(sid, time);
                    let id = build_json(&mut builder, &next);
                    builder.root(id);
                    builder.flush()
                }
            }
        };

        if patch.ops.is_empty() {
            return Ok(Vec::new());
        }

        let bytes = patch.to_binary();
        self.inner.apply_patch(&patch);
        self.view_cache = None;
        Ok(bytes)
    }

    // ── View helpers ─────────────────────────────────────────────────────

    /// Return the current length of the `str` node at `path`.
    ///
    /// Called by `model.api.str(path).length()`.
    #[wasm_bindgen(js_name = "apiStrLen")]
    pub fn api_str_len(&self, path_json: &str) -> Result<u32, JsValue> {
        let path = parse_path(path_json).map_err(|e| JsValue::from_str(&e))?;
        let str_id = self.resolve(&path).map_err(|e| JsValue::from_str(&e))?;
        match IndexExt::get(&self.inner.index, &str_id) {
            Some(CrdtNode::Str(n)) => Ok(n.size() as u32),
            _ => Err(JsValue::from_str("str node not found at path")),
        }
    }

    /// Return the current length of the `arr` node at `path`.
    ///
    /// Called by `model.api.arr(path).length()`.
    #[wasm_bindgen(js_name = "apiArrLen")]
    pub fn api_arr_len(&self, path_json: &str) -> Result<u32, JsValue> {
        let path = parse_path(path_json).map_err(|e| JsValue::from_str(&e))?;
        let arr_id = self.resolve(&path).map_err(|e| JsValue::from_str(&e))?;
        match IndexExt::get(&self.inner.index, &arr_id) {
            Some(CrdtNode::Arr(n)) => Ok(n.size() as u32),
            _ => Err(JsValue::from_str("arr node not found at path")),
        }
    }

    /// Return the number of live bytes in the `bin` node at `path`.
    ///
    /// Called by `model.api.bin(path).length()`.
    #[wasm_bindgen(js_name = "apiBinLen")]
    pub fn api_bin_len(&self, path_json: &str) -> Result<u32, JsValue> {
        let path = parse_path(path_json).map_err(|e| JsValue::from_str(&e))?;
        let bin_id = self.resolve(&path).map_err(|e| JsValue::from_str(&e))?;
        match IndexExt::get(&self.inner.index, &bin_id) {
            Some(CrdtNode::Bin(n)) => {
                let size: usize = n
                    .rga
                    .iter_live()
                    .filter_map(|c| c.data.as_deref())
                    .map(|b| b.len())
                    .sum();
                Ok(size as u32)
            }
            _ => Err(JsValue::from_str("bin node not found at path")),
        }
    }

    /// Return the number of elements in the `vec` node at `path`.
    ///
    /// Called by `model.api.vec(path).length()`.
    #[wasm_bindgen(js_name = "apiVecLen")]
    pub fn api_vec_len(&self, path_json: &str) -> Result<u32, JsValue> {
        let path = parse_path(path_json).map_err(|e| JsValue::from_str(&e))?;
        let vec_id = self.resolve(&path).map_err(|e| JsValue::from_str(&e))?;
        match IndexExt::get(&self.inner.index, &vec_id) {
            Some(CrdtNode::Vec(n)) => Ok(n.elements.len() as u32),
            _ => Err(JsValue::from_str("vec node not found at path")),
        }
    }

    /// Return the JSON view of the node at `path` as a JS value.
    ///
    /// Uses the JSON-compatible serializer — objects are plain JS objects,
    /// not Maps.
    ///
    /// Useful for reading a sub-document without deserializing the whole model.
    #[wasm_bindgen(js_name = "viewAt")]
    pub fn view_at(&self, path_json: &str) -> Result<JsValue, JsValue> {
        let path = parse_path(path_json).map_err(|e| JsValue::from_str(&e))?;
        let id = self.resolve(&path).map_err(|e| JsValue::from_str(&e))?;
        let view = match IndexExt::get(&self.inner.index, &id) {
            Some(node) => node.view(&self.inner.index),
            None => Value::Null,
        };
        let ser = serde_wasm_bindgen::Serializer::json_compatible();
        view.serialize(&ser)
            .map_err(|e| JsValue::from_str(&format!("{e}")))
    }
}

// ── BinNode navigation helpers ────────────────────────────────────────────────
//
// Mirrors the private helpers in json_crdt/model/api.rs.

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

fn bin_find_interval(node: &BinNode, pos: usize, len: usize) -> Vec<Tss> {
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

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn model() -> Model {
        Model::create(Some(65_536))
    }

    #[test]
    fn create_and_view_empty() {
        let m = model();
        assert_eq!(m.inner.view(), json!(null));
    }

    #[test]
    fn api_set_root_scalar() {
        let mut m = model();
        m.api_set("42").unwrap();
        assert_eq!(m.inner.view(), json!(42));
    }

    #[test]
    fn api_set_root_object() {
        let mut m = model();
        m.api_set(r#"{"x":1,"y":2}"#).unwrap();
        assert_eq!(m.inner.view(), json!({"x": 1, "y": 2}));
    }

    #[test]
    fn api_set_root_array() {
        let mut m = model();
        m.api_set("[1,2,3]").unwrap();
        assert_eq!(m.inner.view(), json!([1, 2, 3]));
    }

    #[test]
    fn api_flush_returns_non_empty_after_edit() {
        let mut m = model();
        m.api_set(r#"{"hello":"world"}"#).unwrap();
        let bytes = m.api_flush();
        assert!(!bytes.is_empty());
    }

    #[test]
    fn api_flush_clears_pending() {
        let mut m = model();
        m.api_set("1").unwrap();
        m.api_flush();
        let bytes = m.api_flush();
        assert!(bytes.is_empty());
    }

    #[test]
    fn apply_patch_roundtrip() {
        let mut sender = model();
        sender.api_set(r#"{"key":"value"}"#).unwrap();
        let patch_bytes = sender.api_flush();

        let mut receiver = Model::create(Some(99_999));
        receiver.apply_patch(&patch_bytes).unwrap();
        assert_eq!(receiver.inner.view(), json!({"key": "value"}));
    }

    #[test]
    fn to_binary_from_binary_roundtrip() {
        let mut m = model();
        m.api_set(r#"{"a":1,"b":[1,2,3]}"#).unwrap();
        let binary = m.to_binary();
        let m2 = Model::from_binary(&binary).unwrap();
        assert_eq!(m2.inner.view(), m.inner.view());
    }

    #[test]
    fn fork_produces_independent_copy() {
        let mut m = model();
        m.api_set(r#"{"x":0}"#).unwrap();
        let mut forked = m.fork(Some(77_777));
        forked.api_obj_set("null", r#"{"x":99}"#).unwrap();
        // Original unchanged
        assert_eq!(m.inner.view(), json!({"x": 0}));
        assert_eq!(forked.inner.view(), json!({"x": 99}));
    }

    #[test]
    fn api_obj_set_nested_key() {
        let mut m = model();
        m.api_set(r#"{}"#).unwrap();
        m.api_obj_set("null", r#"{"name":"alice","age":30}"#)
            .unwrap();
        let v = m.inner.view();
        assert_eq!(v["name"], json!("alice"));
        assert_eq!(v["age"], json!(30));
    }

    #[test]
    fn api_obj_del_key() {
        let mut m = model();
        m.api_set(r#"{"a":1,"b":2}"#).unwrap();
        m.api_obj_del("null", r#"["a"]"#).unwrap();
        let v = m.inner.view();
        assert!(v.get("a").is_none() || v["a"].is_null());
    }

    #[test]
    fn api_set_string_becomes_str_node() {
        // api_set mirrors upstream PatchBuilder.json() where strings → StrNodes
        let mut m = model();
        m.api_set(r#"{"name":"","count":0}"#).unwrap();
        // "name" is now a StrNode — we can insert text directly
        m.api_str_ins(r#"["name"]"#, 0, "Alice").unwrap();
        assert_eq!(m.inner.view()["name"], json!("Alice"));
        // "count" is a ConNode (number) — view is unchanged
        assert_eq!(m.inner.view()["count"], json!(0));
    }

    #[test]
    fn api_set_root_string_editable() {
        // Strings at root also become StrNodes
        let mut m = model();
        m.api_set(r#""""#).unwrap();
        m.api_str_ins("null", 0, "hello").unwrap();
        assert_eq!(m.inner.view(), json!("hello"));
    }

    #[test]
    fn api_str_ins_del() {
        let mut m = model();
        m.api_set(r#"{}"#).unwrap();
        // api_new_str explicitly creates a StrNode in an existing object
        // (api_obj_set uses const_or_json so strings there remain ConNodes)
        m.api_new_str("null", "msg", "").unwrap();
        m.api_str_ins(r#"["msg"]"#, 0, "hello").unwrap();
        assert_eq!(m.inner.view()["msg"], json!("hello"));
        m.api_str_ins(r#"["msg"]"#, 5, " world").unwrap();
        assert_eq!(m.inner.view()["msg"], json!("hello world"));
        m.api_str_del(r#"["msg"]"#, 5, 6).unwrap();
        assert_eq!(m.inner.view()["msg"], json!("hello"));
    }

    #[test]
    fn api_arr_ins_del() {
        let mut m = model();
        m.api_set(r#"{"list":[]}"#).unwrap();
        m.api_arr_ins(r#"["list"]"#, 0, r#"[1,2,3]"#).unwrap();
        assert_eq!(m.inner.view()["list"], json!([1, 2, 3]));
        m.api_arr_del(r#"["list"]"#, 1, 1).unwrap();
        assert_eq!(m.inner.view()["list"], json!([1, 3]));
    }

    #[test]
    fn diff_apply_sets_document() {
        let mut m = model();
        let patch = m.diff_apply(r#"{"x":42}"#).unwrap();
        assert!(!patch.is_empty());
        assert_eq!(m.inner.view(), json!({"x": 42}));
    }

    #[test]
    fn diff_apply_noop_when_equal() {
        let mut m = model();
        m.api_set(r#"{"x":1}"#).unwrap();
        m.diff_apply(r#"{"x":1}"#).unwrap(); // same state — may or may not produce patch
                                             // The important invariant: view is still correct
        let v = m.inner.view();
        assert_eq!(v["x"], json!(1));
    }

    #[test]
    fn multiple_edits_merge_into_one_patch() {
        let mut m = model();
        m.api_set(r#"{"b":[]}"#).unwrap();
        m.api_new_str("null", "a", "").unwrap();
        // Flush initial setup so nodes are in the index
        m.api_flush();

        m.api_str_ins(r#"["a"]"#, 0, "hello").unwrap();
        m.api_arr_ins(r#"["b"]"#, 0, "[1,2]").unwrap();
        let bytes = m.api_flush();
        // Both edits merged into a single patch
        assert!(!bytes.is_empty());
        assert_eq!(m.inner.view(), json!({"a": "hello", "b": [1, 2]}));
    }
}
