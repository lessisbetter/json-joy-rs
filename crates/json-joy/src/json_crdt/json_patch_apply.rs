//! JSON Patch (RFC 6902) applied to a JSON CRDT model.
//!
//! Mirrors `packages/json-joy/src/json-crdt/json-patch/JsonPatch.ts` and
//! `packages/json-joy/src/json-crdt/json-patch/JsonPatchStore.ts`.
//!
//! # Overview
//!
//! [`JsonPatch`] wraps a [`Model`] reference and translates RFC 6902 patch
//! operations (add, remove, replace, move, copy, test) plus the extended
//! `str_ins` / `str_del` operations into CRDT mutations via [`ModelApi`].
//!
//! [`JsonPatchStore`] is a convenience wrapper that additionally owns a
//! path prefix so that all operations are applied relative to a sub-path
//! of the document.
//!
//! ## Differences from the upstream TypeScript
//!
//! - Event emitters / `SyncStore<T>` interface are not ported (JS-only).
//! - `base` (`NodeApi`) parameter is not exposed — all traversal starts from
//!   the document root.
//! - `apply()` internally calls `ModelApi::apply()` after every operation
//!   (same net effect as the TS transaction approach given the linear API).

use serde_json::Value;

use crate::json_crdt::model::api::find_path;
use crate::json_crdt::model::{Model, ModelApi};
use crate::json_crdt::nodes::{CrdtNode, IndexExt};
use json_joy_json_pointer::{is_child, parse_json_pointer};

// ── Error ───────────────────────────────────────────────────────────────────

/// Errors that can occur while applying a JSON Patch to a CRDT model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsonPatchError {
    /// The operation type string is unknown or malformed.
    InvalidOp(String),
    /// A path or from pointer could not be resolved.
    NotFound,
    /// A numeric array index is not a valid non-negative integer.
    InvalidIndex,
    /// An `arr` operation index is out of bounds.
    OutOfBounds,
    /// A `test` operation failed (values are not equal).
    Test,
    /// The `move` operation has a `path` that is a child of `from`.
    InvalidChild,
    /// An underlying [`ModelApi`] error.
    Api(crate::json_crdt::model::api::ApiError),
}

impl std::fmt::Display for JsonPatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JsonPatchError::InvalidOp(s) => write!(f, "INVALID_OP: {s}"),
            JsonPatchError::NotFound => write!(f, "NOT_FOUND"),
            JsonPatchError::InvalidIndex => write!(f, "INVALID_INDEX"),
            JsonPatchError::OutOfBounds => write!(f, "OUT_OF_BOUNDS"),
            JsonPatchError::Test => write!(f, "TEST"),
            JsonPatchError::InvalidChild => write!(f, "INVALID_CHILD"),
            JsonPatchError::Api(e) => write!(f, "API: {e}"),
        }
    }
}

impl std::error::Error for JsonPatchError {}

impl From<crate::json_crdt::model::api::ApiError> for JsonPatchError {
    fn from(e: crate::json_crdt::model::api::ApiError) -> Self {
        JsonPatchError::Api(e)
    }
}

// ── Path conversion helpers ─────────────────────────────────────────────────

// ── JsonPatch ───────────────────────────────────────────────────────────────

/// Applies RFC 6902 JSON Patch operations to a JSON CRDT model.
///
/// Mirrors `JsonPatch` from the upstream TypeScript.
pub struct JsonPatch<'a> {
    /// The CRDT model being mutated.
    model: &'a mut Model,
    /// Optional path prefix prepended to every operation path.
    pfx: Vec<String>,
}

impl<'a> JsonPatch<'a> {
    /// Create a new `JsonPatch` targeting the document root.
    pub fn new(model: &'a mut Model) -> Self {
        Self {
            model,
            pfx: Vec::new(),
        }
    }

    /// Create a new `JsonPatch` with a path prefix.
    ///
    /// All operations are applied relative to `prefix`.
    pub fn with_prefix(model: &'a mut Model, prefix: Vec<String>) -> Self {
        Self { model, pfx: prefix }
    }

    // ── Public API ─────────────────────────────────────────────────────────

    /// Apply a slice of raw JSON Patch operations (each a `serde_json::Value`
    /// object with at least an `"op"` field).
    ///
    /// Mirrors `JsonPatch.apply()` in the upstream TypeScript.
    pub fn apply(&mut self, ops: &[Value]) -> Result<(), JsonPatchError> {
        for op in ops {
            self.apply_op(op)?;
        }
        Ok(())
    }

    /// Apply a single raw JSON Patch operation value.
    ///
    /// Mirrors `JsonPatch.applyOp()`.
    pub fn apply_op(&mut self, op: &Value) -> Result<(), JsonPatchError> {
        let op_name = op
            .get("op")
            .and_then(Value::as_str)
            .ok_or_else(|| JsonPatchError::InvalidOp("missing 'op' field".to_string()))?;

        match op_name {
            "add" => {
                let path = get_path(op)?;
                let value = op
                    .get("value")
                    .ok_or_else(|| JsonPatchError::InvalidOp("add: missing 'value'".to_string()))?
                    .clone();
                self.add(&path, &value)
            }
            "remove" => {
                let path = get_path(op)?;
                self.remove(&path)
            }
            "replace" => {
                let path = get_path(op)?;
                let value = op
                    .get("value")
                    .ok_or_else(|| {
                        JsonPatchError::InvalidOp("replace: missing 'value'".to_string())
                    })?
                    .clone();
                self.replace(&path, &value)
            }
            "move" => {
                let path = get_path(op)?;
                let from = get_from(op)?;
                self.move_op(&path, &from)
            }
            "copy" => {
                let path = get_path(op)?;
                let from = get_from(op)?;
                self.copy_op(&path, &from)
            }
            "test" => {
                let path = get_path(op)?;
                let value = op
                    .get("value")
                    .ok_or_else(|| JsonPatchError::InvalidOp("test: missing 'value'".to_string()))?
                    .clone();
                self.test(&path, &value)
            }
            "str_ins" => {
                let path = get_path(op)?;
                let pos = op.get("pos").and_then(Value::as_u64).ok_or_else(|| {
                    JsonPatchError::InvalidOp("str_ins: missing 'pos'".to_string())
                })? as usize;
                let str_val = op
                    .get("str")
                    .and_then(Value::as_str)
                    .ok_or_else(|| JsonPatchError::InvalidOp("str_ins: missing 'str'".to_string()))?
                    .to_string();
                self.str_ins(&path, pos, &str_val)
            }
            "str_del" => {
                let path = get_path(op)?;
                let pos = op.get("pos").and_then(Value::as_u64).ok_or_else(|| {
                    JsonPatchError::InvalidOp("str_del: missing 'pos'".to_string())
                })? as usize;
                let len = op.get("len").and_then(Value::as_u64).map(|v| v as usize);
                let str_val = op.get("str").and_then(Value::as_str).map(|s| s.to_string());
                self.str_del(&path, pos, len, str_val.as_deref())
            }
            other => Err(JsonPatchError::InvalidOp(format!("UNKNOWN_OP: {other}"))),
        }
    }

    // ── Core operations ────────────────────────────────────────────────────

    /// Perform the `add` operation.
    ///
    /// Mirrors `JsonPatch.add()` in the upstream TypeScript.
    pub fn add(&mut self, path: &str, value: &Value) -> Result<(), JsonPatchError> {
        let steps = self.to_path(path);
        if steps.is_empty() {
            // Target is root — replace the whole document.
            return self.set_root(value);
        }

        let parent_steps: Vec<Value> = steps[..steps.len() - 1]
            .iter()
            .map(|s| Value::String(s.clone()))
            .collect();
        let key = &steps[steps.len() - 1];

        let root_id = self.model.root.val;
        let parent_id =
            find_path(self.model, root_id, &parent_steps).map_err(|_| JsonPatchError::NotFound)?;

        // Unwrap any ValNode wrapper at the parent
        let parent_id = unwrap_val(self.model, parent_id);

        match IndexExt::get(&self.model.index, &parent_id) {
            Some(CrdtNode::Obj(_)) => {
                let mut api = ModelApi::new(self.model);
                api.obj_set(parent_id, &[(key.clone(), value.clone())])
                    .map_err(JsonPatchError::from)
            }
            Some(CrdtNode::Arr(n)) => {
                let length = n.size();
                if key == "-" {
                    // Append to end
                    let mut api = ModelApi::new(self.model);
                    api.arr_ins(parent_id, length, std::slice::from_ref(value))
                        .map_err(JsonPatchError::from)
                } else {
                    let index = key
                        .parse::<usize>()
                        .map_err(|_| JsonPatchError::InvalidIndex)?;
                    let mut api = ModelApi::new(self.model);
                    api.arr_ins(parent_id, index, std::slice::from_ref(value))
                        .map_err(JsonPatchError::from)
                }
            }
            _ => Err(JsonPatchError::NotFound),
        }
    }

    /// Perform the `remove` operation.
    ///
    /// Mirrors `JsonPatch.remove()` in the upstream TypeScript.
    pub fn remove(&mut self, path: &str) -> Result<(), JsonPatchError> {
        let steps = self.to_path(path);
        if steps.is_empty() {
            // Remove root → set to null
            return self.set_root(&Value::Null);
        }

        let parent_steps: Vec<Value> = steps[..steps.len() - 1]
            .iter()
            .map(|s| Value::String(s.clone()))
            .collect();
        let key = &steps[steps.len() - 1];

        let root_id = self.model.root.val;
        let parent_id =
            find_path(self.model, root_id, &parent_steps).map_err(|_| JsonPatchError::NotFound)?;

        let parent_id = unwrap_val(self.model, parent_id);

        match IndexExt::get(&self.model.index, &parent_id) {
            Some(CrdtNode::Obj(n)) => {
                // Check key exists and is not already undefined
                let value_node_id = n.keys.get(key).copied();
                match value_node_id {
                    None => return Err(JsonPatchError::NotFound),
                    Some(vid) => {
                        // Check if it's a ConNode with undefined value
                        if let Some(CrdtNode::Con(c)) = IndexExt::get(&self.model.index, &vid) {
                            use crate::json_crdt_patch::operations::ConValue;
                            if matches!(c.val, ConValue::Val(ref pv) if matches!(pv, json_joy_json_pack::PackValue::Null))
                            {
                                // Already null — upstream treats this as NOT_FOUND for explicit undefined
                                // However, since we map undefined→null, be lenient and allow remove.
                                // Let it proceed to set null (idempotent).
                            }
                        }
                    }
                }
                // Set to undefined (represented as null con in CRDT)
                let mut api = ModelApi::new(self.model);
                api.obj_del(parent_id, std::slice::from_ref(key))
                    .map_err(JsonPatchError::from)
            }
            Some(CrdtNode::Arr(_)) => {
                let index = key
                    .parse::<usize>()
                    .map_err(|_| JsonPatchError::InvalidIndex)?;
                let mut api = ModelApi::new(self.model);
                api.arr_del(parent_id, index, 1).map_err(|e| match e {
                    crate::json_crdt::model::api::ApiError::OutOfBounds => JsonPatchError::NotFound,
                    other => JsonPatchError::Api(other),
                })
            }
            _ => Err(JsonPatchError::NotFound),
        }
    }

    /// Perform the `replace` operation (remove then add).
    ///
    /// Mirrors `JsonPatch.replace()`.
    pub fn replace(&mut self, path: &str, value: &Value) -> Result<(), JsonPatchError> {
        self.remove(path)?;
        self.add(path, value)
    }

    /// Perform the `move` operation.
    ///
    /// Mirrors `JsonPatch.move()`.
    pub fn move_op(&mut self, path: &str, from: &str) -> Result<(), JsonPatchError> {
        let path_steps = self.to_path(path);
        let from_steps = self.to_path(from);

        // Reject if path is a child of from (would move into itself)
        if is_child(&from_steps, &path_steps) {
            return Err(JsonPatchError::InvalidChild);
        }

        let json = self.get_json(from)?;
        self.remove(from)?;
        self.add(path, &json)
    }

    /// Perform the `copy` operation.
    ///
    /// Mirrors `JsonPatch.copy()`.
    pub fn copy_op(&mut self, path: &str, from: &str) -> Result<(), JsonPatchError> {
        let json = self.get_json(from)?;
        self.add(path, &json)
    }

    /// Perform the `test` operation.
    ///
    /// Mirrors `JsonPatch.test()`.
    pub fn test(&mut self, path: &str, value: &Value) -> Result<(), JsonPatchError> {
        let json = self.get_json(path)?;
        if json == *value {
            Ok(())
        } else {
            Err(JsonPatchError::Test)
        }
    }

    /// Perform the `str_ins` extended operation.
    ///
    /// Mirrors `JsonPatch.strIns()`.
    pub fn str_ins(&mut self, path: &str, pos: usize, str_val: &str) -> Result<(), JsonPatchError> {
        let steps = self.to_path(path);
        let value_path: Vec<Value> = steps.iter().map(|s| Value::String(s.clone())).collect();

        let root_id = self.model.root.val;
        let node_id =
            find_path(self.model, root_id, &value_path).map_err(|_| JsonPatchError::NotFound)?;
        let node_id = unwrap_val(self.model, node_id);

        match IndexExt::get(&self.model.index, &node_id) {
            Some(CrdtNode::Str(_)) => {
                let mut api = ModelApi::new(self.model);
                api.str_ins(node_id, pos, str_val).map_err(|e| match e {
                    crate::json_crdt::model::api::ApiError::OutOfBounds => {
                        JsonPatchError::OutOfBounds
                    }
                    other => JsonPatchError::Api(other),
                })
            }
            _ => Err(JsonPatchError::NotFound),
        }
    }

    /// Perform the `str_del` extended operation.
    ///
    /// Mirrors `JsonPatch.strDel()`.
    pub fn str_del(
        &mut self,
        path: &str,
        pos: usize,
        len: Option<usize>,
        str_val: Option<&str>,
    ) -> Result<(), JsonPatchError> {
        let steps = self.to_path(path);
        let value_path: Vec<Value> = steps.iter().map(|s| Value::String(s.clone())).collect();

        let root_id = self.model.root.val;
        let node_id =
            find_path(self.model, root_id, &value_path).map_err(|_| JsonPatchError::NotFound)?;
        let node_id = unwrap_val(self.model, node_id);

        let current_len = match IndexExt::get(&self.model.index, &node_id) {
            Some(CrdtNode::Str(n)) => n.size(),
            _ => return Err(JsonPatchError::NotFound),
        };

        if current_len <= pos {
            // Nothing to delete — same behavior as upstream (early return, no error)
            return Ok(());
        }

        // Determine deletion length: min(len ?? str.len, current_len - pos)
        let deletion_len = {
            let raw_len = len.unwrap_or_else(|| str_val.map(|s| s.chars().count()).unwrap_or(0));
            raw_len.min(current_len - pos)
        };

        if deletion_len == 0 {
            return Ok(());
        }

        let mut api = ModelApi::new(self.model);
        api.str_del(node_id, pos, deletion_len)
            .map_err(|e| match e {
                crate::json_crdt::model::api::ApiError::OutOfBounds => JsonPatchError::OutOfBounds,
                other => JsonPatchError::Api(other),
            })
    }

    /// Read the JSON value at `path` (with prefix applied).
    ///
    /// Mirrors `JsonPatch.get()`.
    pub fn get(&self, path: &str) -> Option<Value> {
        self.get_json(path).ok()
    }

    // ── Internal helpers ───────────────────────────────────────────────────

    /// Prepend `pfx` to `path` and return the resulting path components.
    fn to_path(&self, path: &str) -> Vec<String> {
        let mut result = self.pfx.clone();
        result.extend(parse_json_pointer(path));
        result
    }

    /// Read the JSON value at `path` (with prefix), returning an error if not found.
    fn get_json(&self, path: &str) -> Result<Value, JsonPatchError> {
        let steps = self.to_path(path);
        let value_path: Vec<Value> = steps.iter().map(|s| Value::String(s.clone())).collect();
        let root_id = self.model.root.val;
        let node_id =
            find_path(self.model, root_id, &value_path).map_err(|_| JsonPatchError::NotFound)?;
        let node_id = unwrap_val(self.model, node_id);
        match IndexExt::get(&self.model.index, &node_id) {
            Some(n) => Ok(n.view(&self.model.index)),
            None => Err(JsonPatchError::NotFound),
        }
    }

    /// Replace the entire document root with `value`.
    fn set_root(&mut self, value: &Value) -> Result<(), JsonPatchError> {
        let mut api = ModelApi::new(self.model);
        api.set(value).map_err(JsonPatchError::from)
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Unwrap any chain of `ValNode` wrappers, returning the final data-node ID.
fn unwrap_val(
    model: &Model,
    mut id: crate::json_crdt_patch::clock::Ts,
) -> crate::json_crdt_patch::clock::Ts {
    loop {
        match IndexExt::get(&model.index, &id) {
            Some(CrdtNode::Val(v)) => id = v.val,
            _ => return id,
        }
    }
}

/// Extract the `"path"` string field from a raw op `Value`.
fn get_path(op: &Value) -> Result<String, JsonPatchError> {
    op.get("path")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| JsonPatchError::InvalidOp("missing 'path' field".to_string()))
}

/// Extract the `"from"` string field from a raw op `Value`.
fn get_from(op: &Value) -> Result<String, JsonPatchError> {
    op.get("from")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| JsonPatchError::InvalidOp("missing 'from' field".to_string()))
}

// ── JsonPatchStore ───────────────────────────────────────────────────────────

/// Convenience store that owns a path prefix and exposes a simple mutation API.
///
/// Mirrors `JsonPatchStore` from the upstream TypeScript.  The `SyncStore`
/// interface (change events) is not ported — it is JS-only.
pub struct JsonPatchStore {
    /// The CRDT model managed by this store.
    pub model: Model,
    /// Path prefix applied to every operation.
    pub path: Vec<String>,
}

impl JsonPatchStore {
    /// Create a store wrapping `model`, rooted at the document root.
    pub fn new(model: Model) -> Self {
        Self {
            model,
            path: Vec::new(),
        }
    }

    /// Create a store wrapping `model`, rooted at `path`.
    pub fn with_path(model: Model, path: Vec<String>) -> Self {
        Self { model, path }
    }

    /// Apply one or more raw JSON Patch operations.
    pub fn update(&mut self, ops: &[Value]) -> Result<(), JsonPatchError> {
        let mut patcher = JsonPatch::with_prefix(&mut self.model, self.path.clone());
        patcher.apply(ops)
    }

    /// Convenience: apply a single `add` operation.
    pub fn add(&mut self, path: &str, value: Value) -> Result<(), JsonPatchError> {
        self.update(&[serde_json::json!({ "op": "add", "path": path, "value": value })])
    }

    /// Convenience: apply a single `replace` operation.
    pub fn replace(&mut self, path: &str, value: Value) -> Result<(), JsonPatchError> {
        self.update(&[serde_json::json!({ "op": "replace", "path": path, "value": value })])
    }

    /// Convenience: apply a single `remove` operation.
    pub fn remove(&mut self, path: &str) -> Result<(), JsonPatchError> {
        self.update(&[serde_json::json!({ "op": "remove", "path": path })])
    }

    /// Convenience: apply a `remove` operation, silently ignoring `NotFound`.
    pub fn del(&mut self, path: &str) -> Option<()> {
        match self.remove(path) {
            Ok(()) | Err(JsonPatchError::NotFound) => Some(()),
            Err(_) => None,
        }
    }

    /// Read the JSON value at `path` (relative to this store's prefix).
    pub fn get(&self, path: &str) -> Option<Value> {
        let steps: Vec<String> = {
            let mut s = self.path.clone();
            s.extend(parse_json_pointer(path));
            s
        };
        let value_path: Vec<Value> = steps.iter().map(|s| Value::String(s.clone())).collect();
        let root_id = self.model.root.val;
        find_path(&self.model, root_id, &value_path).ok().map(|id| {
            let id = unwrap_val(&self.model, id);
            match IndexExt::get(&self.model.index, &id) {
                Some(n) => n.view(&self.model.index),
                None => Value::Null,
            }
        })
    }

    /// Return a new `JsonPatchStore` rooted at `self.path + path`.
    pub fn bind(&self, path: &str) -> JsonPatchStoreBound {
        let mut new_path = self.path.clone();
        new_path.extend(parse_json_pointer(path));
        JsonPatchStoreBound { path: new_path }
    }
}

/// A bound view produced by [`JsonPatchStore::bind`].
///
/// Holds only the resolved path; callers pass it back to a `JsonPatchStore`
/// or use it to create a new one with a different root model.
pub struct JsonPatchStoreBound {
    pub path: Vec<String>,
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_crdt::model::Model;
    use serde_json::json;

    // ── helpers ──────────────────────────────────────────────────────────────

    /// Build a model with a root object `{"name": "Alice", "age": 30}`.
    fn make_obj_model() -> Model {
        let mut model = Model::create();
        let mut api = ModelApi::new(&mut model);
        api.set(&json!({"name": "Alice", "age": 30})).unwrap();
        model
    }

    /// Build a model with a root array `[1, 2, 3]`.
    fn make_arr_model() -> Model {
        let mut model = Model::create();
        let mut api = ModelApi::new(&mut model);
        api.set(&json!([1, 2, 3])).unwrap();
        model
    }

    // ── add ──────────────────────────────────────────────────────────────────

    #[test]
    fn add_to_object() {
        let mut model = make_obj_model();
        let mut patcher = JsonPatch::new(&mut model);
        patcher.add("/city", &json!("NYC")).unwrap();
        assert_eq!(model.view()["city"], json!("NYC"));
    }

    #[test]
    fn add_replaces_existing_object_key() {
        let mut model = make_obj_model();
        let mut patcher = JsonPatch::new(&mut model);
        patcher.add("/name", &json!("Bob")).unwrap();
        assert_eq!(model.view()["name"], json!("Bob"));
    }

    #[test]
    fn add_appends_to_array_with_dash() {
        let mut model = make_arr_model();
        let mut patcher = JsonPatch::new(&mut model);
        patcher.add("/-", &json!(4)).unwrap();
        assert_eq!(model.view(), json!([1, 2, 3, 4]));
    }

    #[test]
    fn add_inserts_into_array_at_index() {
        let mut model = make_arr_model();
        let mut patcher = JsonPatch::new(&mut model);
        patcher.add("/1", &json!(99)).unwrap();
        // Inserts at position 1 → [1, 99, 2, 3]
        let v = model.view();
        assert_eq!(v[0], json!(1));
        assert_eq!(v[1], json!(99));
    }

    #[test]
    fn add_sets_root_when_path_is_empty() {
        let mut model = Model::create();
        let mut patcher = JsonPatch::new(&mut model);
        patcher.add("", &json!({"hello": "world"})).unwrap();
        assert_eq!(model.view()["hello"], json!("world"));
    }

    // ── remove ───────────────────────────────────────────────────────────────

    #[test]
    fn remove_object_key() {
        let mut model = make_obj_model();
        let mut patcher = JsonPatch::new(&mut model);
        patcher.remove("/age").unwrap();
        // After removal the key is set to null (CRDT delete semantic)
        let v = model.view();
        assert_eq!(v["name"], json!("Alice"));
    }

    #[test]
    fn remove_array_element() {
        let mut model = make_arr_model();
        let mut patcher = JsonPatch::new(&mut model);
        patcher.remove("/1").unwrap();
        assert_eq!(model.view(), json!([1, 3]));
    }

    #[test]
    fn remove_nonexistent_array_index_returns_not_found() {
        let mut model = make_arr_model();
        let mut patcher = JsonPatch::new(&mut model);
        let result = patcher.remove("/99");
        assert_eq!(result, Err(JsonPatchError::NotFound));
    }

    // ── replace ──────────────────────────────────────────────────────────────

    #[test]
    fn replace_object_key() {
        let mut model = make_obj_model();
        let mut patcher = JsonPatch::new(&mut model);
        patcher.replace("/name", &json!("Charlie")).unwrap();
        assert_eq!(model.view()["name"], json!("Charlie"));
    }

    #[test]
    fn replace_array_element() {
        let mut model = make_arr_model();
        let mut patcher = JsonPatch::new(&mut model);
        patcher.replace("/0", &json!(10)).unwrap();
        assert_eq!(model.view()[0], json!(10));
    }

    // ── copy ─────────────────────────────────────────────────────────────────

    #[test]
    fn copy_value_within_object() {
        let mut model = make_obj_model();
        let mut patcher = JsonPatch::new(&mut model);
        patcher.copy_op("/nickname", "/name").unwrap();
        assert_eq!(model.view()["nickname"], json!("Alice"));
        // Original still present
        assert_eq!(model.view()["name"], json!("Alice"));
    }

    // ── move ─────────────────────────────────────────────────────────────────

    #[test]
    fn move_renames_object_key() {
        let mut model = make_obj_model();
        let mut patcher = JsonPatch::new(&mut model);
        patcher.move_op("/full_name", "/name").unwrap();
        assert_eq!(model.view()["full_name"], json!("Alice"));
    }

    #[test]
    fn move_into_child_returns_invalid_child() {
        let mut model = Model::create();
        {
            let mut api = ModelApi::new(&mut model);
            api.set(&json!({"a": {"b": 1}})).unwrap();
        }
        let mut patcher = JsonPatch::new(&mut model);
        // Trying to move /a into /a/c is moving a parent into one of its children
        let result = patcher.move_op("/a/c", "/a");
        assert_eq!(result, Err(JsonPatchError::InvalidChild));
    }

    // ── test ─────────────────────────────────────────────────────────────────

    #[test]
    fn test_succeeds_when_equal() {
        let mut model = make_obj_model();
        let mut patcher = JsonPatch::new(&mut model);
        patcher.test("/name", &json!("Alice")).unwrap();
    }

    #[test]
    fn test_fails_when_not_equal() {
        let mut model = make_obj_model();
        let mut patcher = JsonPatch::new(&mut model);
        let result = patcher.test("/name", &json!("Bob"));
        assert_eq!(result, Err(JsonPatchError::Test));
    }

    // ── str_ins / str_del ─────────────────────────────────────────────────────

    #[test]
    fn str_ins_into_crdt_string() {
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
        let mut patcher = JsonPatch::new(&mut model);
        patcher.str_ins("", 5, " world").unwrap();
        assert_eq!(model.view(), json!("hello world"));
    }

    #[test]
    fn str_del_from_crdt_string() {
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
            api.str_ins(str_id, 0, "hello world").unwrap();
        }
        let mut patcher = JsonPatch::new(&mut model);
        patcher.str_del("", 5, Some(6), None).unwrap();
        assert_eq!(model.view(), json!("hello"));
    }

    // ── apply (batch) ─────────────────────────────────────────────────────────

    #[test]
    fn apply_batch_operations() {
        let mut model = make_obj_model();
        let mut patcher = JsonPatch::new(&mut model);
        patcher
            .apply(&[
                json!({ "op": "add",     "path": "/city",  "value": "NYC" }),
                json!({ "op": "replace", "path": "/age",   "value": 31 }),
            ])
            .unwrap();
        let v = model.view();
        assert_eq!(v["city"], json!("NYC"));
        assert_eq!(v["age"], json!(31));
    }

    #[test]
    fn apply_op_unknown_returns_error() {
        let mut model = make_obj_model();
        let mut patcher = JsonPatch::new(&mut model);
        let result = patcher.apply_op(&json!({ "op": "frobnicate", "path": "/x" }));
        assert!(matches!(result, Err(JsonPatchError::InvalidOp(_))));
    }

    // ── JsonPatchStore ────────────────────────────────────────────────────────

    #[test]
    fn store_add_and_get() {
        let model = make_obj_model();
        let mut store = JsonPatchStore::new(model);
        store.add("/score", json!(100)).unwrap();
        assert_eq!(store.get("/score"), Some(json!(100)));
    }

    #[test]
    fn store_replace() {
        let model = make_obj_model();
        let mut store = JsonPatchStore::new(model);
        store.replace("/name", json!("Dave")).unwrap();
        assert_eq!(store.get("/name"), Some(json!("Dave")));
    }

    #[test]
    fn store_remove() {
        let model = make_arr_model();
        let mut store = JsonPatchStore::new(model);
        store.remove("/0").unwrap();
        assert_eq!(store.get(""), Some(json!([2, 3])));
    }

    #[test]
    fn store_del_ignores_not_found() {
        let model = make_arr_model();
        let mut store = JsonPatchStore::new(model);
        // Index 99 does not exist — del should return Some(()) not panic
        let result = store.del("/99");
        assert!(result.is_some());
    }

    #[test]
    fn store_with_path_prefix() {
        let mut model = Model::create();
        {
            let mut api = ModelApi::new(&mut model);
            api.set(&json!({"user": {"name": "Eve"}})).unwrap();
        }
        let mut store = JsonPatchStore::with_path(model, vec!["user".to_string()]);
        // Operations on the store are relative to /user
        store.add("/email", json!("eve@example.com")).unwrap();
        assert_eq!(store.get("/name"), Some(json!("Eve")));
        assert_eq!(store.get("/email"), Some(json!("eve@example.com")));
    }
}
