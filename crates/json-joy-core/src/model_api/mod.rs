//! Native model API slice inspired by upstream `json-crdt/model/api/*`.
//!
//! This module intentionally starts with a compact surface that is already
//! useful for runtime orchestration and test-port mapping:
//! - bootstrap from patches (`Model.fromPatches`-like),
//! - batch apply (`Model.applyBatch`-like),
//! - path lookup (`api.find`-like),
//! - basic mutators (`set`, `obj_put`, `arr_push`, `str_ins`).

use crate::diff_runtime::{diff_model_to_patch_bytes, DiffError};
use crate::model::ModelError;
use crate::model_runtime::{ApplyError, RuntimeModel};
use crate::patch::Patch;
use serde_json::Value;
use std::collections::BTreeMap;
use thiserror::Error;

mod events;
mod path;

pub use events::{BatchChangeEvent, ChangeEvent, ChangeEventOrigin, ScopedChangeEvent};
pub use path::PathStep;
use path::{get_path_mut, parse_json_pointer, split_parent, value_at_path};

#[derive(Debug, Clone, PartialEq)]
pub enum ApiOperation {
    Add { path: Vec<PathStep>, value: Value },
    Replace { path: Vec<PathStep>, value: Value },
    Remove { path: Vec<PathStep>, length: usize },
    Merge { path: Vec<PathStep>, value: Value },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiOperationKind {
    Add,
    Replace,
    Remove,
    Merge,
}

#[derive(Debug, Error)]
pub enum ModelApiError {
    #[error("no patches provided")]
    NoPatches,
    #[error("first patch missing id")]
    MissingPatchId,
    #[error("path not found")]
    PathNotFound,
    #[error("path does not point to object")]
    NotObject,
    #[error("path does not point to array")]
    NotArray,
    #[error("path does not point to string")]
    NotString,
    #[error("invalid path operation")]
    InvalidPathOp,
    #[error("model encode/decode failed: {0}")]
    Model(#[from] ModelError),
    #[error("patch apply failed: {0}")]
    Apply(#[from] ApplyError),
    #[error("diff failed: {0}")]
    Diff(#[from] DiffError),
    #[error("patch decode failed: {0}")]
    PatchDecode(String),
}

pub struct NativeModelApi {
    runtime: RuntimeModel,
    sid: u64,
    next_listener_id: u64,
    listeners: BTreeMap<u64, Box<dyn FnMut(ChangeEvent) + Send + Sync>>,
    next_batch_listener_id: u64,
    batch_listeners: BTreeMap<u64, Box<dyn FnMut(BatchChangeEvent) + Send + Sync>>,
}

pub struct NodeHandle<'a> {
    api: &'a mut NativeModelApi,
    path: Vec<PathStep>,
}

pub struct ObjHandle<'a> {
    inner: NodeHandle<'a>,
}

pub struct ArrHandle<'a> {
    inner: NodeHandle<'a>,
}

pub struct StrHandle<'a> {
    inner: NodeHandle<'a>,
}

pub struct ValHandle<'a> {
    inner: NodeHandle<'a>,
}

pub struct BinHandle<'a> {
    inner: NodeHandle<'a>,
}

pub struct VecHandle<'a> {
    inner: NodeHandle<'a>,
}

pub struct ConHandle<'a> {
    inner: NodeHandle<'a>,
}

impl NativeModelApi {
    pub fn from_model_binary(data: &[u8], sid_hint: Option<u64>) -> Result<Self, ModelApiError> {
        let mut runtime = RuntimeModel::from_model_binary(data)?;
        let sid = sid_hint.unwrap_or(65_536);
        // Match upstream `Model.load(binary, sid)` behavior for logical models:
        // adopt the caller-provided local session ID for subsequent local ops.
        if sid_hint.is_some() && data.first().is_some_and(|b| (b & 0x80) == 0) {
            runtime = runtime.fork_with_sid(sid);
        }
        Ok(Self {
            runtime,
            sid,
            next_listener_id: 1,
            listeners: BTreeMap::new(),
            next_batch_listener_id: 1,
            batch_listeners: BTreeMap::new(),
        })
    }

    pub fn from_patches(patches: &[Patch]) -> Result<Self, ModelApiError> {
        let first = patches.first().ok_or(ModelApiError::NoPatches)?;
        let (sid, _) = first.id().ok_or(ModelApiError::MissingPatchId)?;
        let mut runtime = RuntimeModel::new_logical_empty(sid);
        for patch in patches {
            runtime.apply_patch(patch)?;
        }
        Ok(Self {
            runtime,
            sid,
            next_listener_id: 1,
            listeners: BTreeMap::new(),
            next_batch_listener_id: 1,
            batch_listeners: BTreeMap::new(),
        })
    }

    pub fn on_change<F>(&mut self, listener: F) -> u64
    where
        F: FnMut(ChangeEvent) + Send + Sync + 'static,
    {
        let id = self.next_listener_id;
        self.next_listener_id = self.next_listener_id.saturating_add(1);
        self.listeners.insert(id, Box::new(listener));
        id
    }

    pub fn off_change(&mut self, listener_id: u64) -> bool {
        self.listeners.remove(&listener_id).is_some()
    }

    pub fn on_changes<F>(&mut self, listener: F) -> u64
    where
        F: FnMut(BatchChangeEvent) + Send + Sync + 'static,
    {
        let id = self.next_batch_listener_id;
        self.next_batch_listener_id = self.next_batch_listener_id.saturating_add(1);
        self.batch_listeners.insert(id, Box::new(listener));
        id
    }

    pub fn off_changes(&mut self, listener_id: u64) -> bool {
        self.batch_listeners.remove(&listener_id).is_some()
    }

    pub fn on_change_at<F>(&mut self, path: Vec<PathStep>, mut listener: F) -> u64
    where
        F: FnMut(ScopedChangeEvent) + Send + Sync + 'static,
    {
        self.on_change(move |ev| {
            let before = value_at_path(&ev.before, &path).cloned();
            let after = value_at_path(&ev.after, &path).cloned();
            if before != after {
                listener(ScopedChangeEvent {
                    path: path.clone(),
                    before,
                    after,
                    patch_id: ev.patch_id,
                    origin: ev.origin,
                });
            }
        })
    }

    pub fn apply_patch(&mut self, patch: &Patch) -> Result<(), ModelApiError> {
        let before = self.runtime.view_json();
        self.runtime.apply_patch(patch)?;
        let after = self.runtime.view_json();
        let origin = match patch.id() {
            Some((sid, _)) if sid == self.sid => ChangeEventOrigin::Local,
            Some(_) => ChangeEventOrigin::Remote,
            None => ChangeEventOrigin::Local,
        };
        self.emit_change(ChangeEvent {
            origin,
            patch_id: patch.id(),
            before,
            after,
        });
        if let Some((sid, _)) = patch.id() {
            self.sid = self.sid.max(sid);
        }
        Ok(())
    }

    pub fn apply_batch(&mut self, patches: &[Patch]) -> Result<(), ModelApiError> {
        let before = self.runtime.view_json();
        let mut patch_ids: Vec<(u64, u64)> = Vec::with_capacity(patches.len());
        for patch in patches {
            if let Some(id) = patch.id() {
                patch_ids.push(id);
            }
            self.apply_patch(patch)?;
        }
        let after = self.runtime.view_json();
        if before != after {
            self.emit_batch_change(BatchChangeEvent {
                patch_ids,
                before,
                after,
            });
        }
        Ok(())
    }

    pub fn view(&self) -> Value {
        self.runtime.view_json()
    }

    pub fn to_model_binary(&self) -> Result<Vec<u8>, ModelApiError> {
        Ok(self.runtime.to_model_binary_like()?)
    }

    pub fn find(&self, path: &[PathStep]) -> Option<Value> {
        let mut node = self.runtime.view_json();
        for step in path {
            node = match (step, node) {
                (PathStep::Key(k), Value::Object(map)) => map.get(k)?.clone(),
                (PathStep::Index(i), Value::Array(arr)) => arr.get(*i)?.clone(),
                (PathStep::Append, _) => return None,
                _ => return None,
            };
        }
        Some(node)
    }

    pub fn read(&self, path: Option<&[PathStep]>) -> Option<Value> {
        match path {
            None => Some(self.runtime.view_json()),
            Some(p) => self.find(p),
        }
    }

    pub fn read_ptr(&self, ptr: Option<&str>) -> Option<Value> {
        match ptr {
            None => self.read(None),
            Some(p) => {
                let steps = parse_json_pointer(p).ok()?;
                self.read(Some(&steps))
            }
        }
    }

    pub fn select(&self, path: Option<&[PathStep]>) -> Option<Value> {
        self.read(path)
    }

    pub fn select_ptr(&self, ptr: Option<&str>) -> Option<Value> {
        self.read_ptr(ptr)
    }

    pub fn set(&mut self, path: &[PathStep], value: Value) -> Result<(), ModelApiError> {
        let mut next = self.runtime.view_json();
        if path.is_empty() {
            next = value;
            return self.apply_target_view(next);
        }
        let target = get_path_mut(&mut next, path).ok_or(ModelApiError::PathNotFound)?;
        *target = value;
        self.apply_target_view(next)
    }

    pub fn obj_put(
        &mut self,
        path: &[PathStep],
        key: impl Into<String>,
        value: Value,
    ) -> Result<(), ModelApiError> {
        let mut next = self.runtime.view_json();
        let target = if path.is_empty() {
            &mut next
        } else {
            get_path_mut(&mut next, path).ok_or(ModelApiError::PathNotFound)?
        };
        let map = target.as_object_mut().ok_or(ModelApiError::NotObject)?;
        map.insert(key.into(), value);
        self.apply_target_view(next)
    }

    pub fn arr_push(&mut self, path: &[PathStep], value: Value) -> Result<(), ModelApiError> {
        let mut next = self.runtime.view_json();
        let target = get_path_mut(&mut next, path).ok_or(ModelApiError::PathNotFound)?;
        let arr = target.as_array_mut().ok_or(ModelApiError::NotArray)?;
        arr.push(value);
        self.apply_target_view(next)
    }

    pub fn str_ins(&mut self, path: &[PathStep], pos: usize, text: &str) -> Result<(), ModelApiError> {
        let mut next = self.runtime.view_json();
        let target = get_path_mut(&mut next, path).ok_or(ModelApiError::PathNotFound)?;
        let s = target.as_str().ok_or(ModelApiError::NotString)?;
        let mut chars: Vec<char> = s.chars().collect();
        let p = pos.min(chars.len());
        for (offset, ch) in text.chars().enumerate() {
            chars.insert(p + offset, ch);
        }
        *target = Value::String(chars.into_iter().collect());
        self.apply_target_view(next)
    }

    pub fn add(&mut self, path: &[PathStep], value: Value) -> Result<(), ModelApiError> {
        if path.is_empty() {
            return Err(ModelApiError::InvalidPathOp);
        }
        let mut next = self.runtime.view_json();
        let (parent, leaf) = split_parent(path)?;
        let target = if parent.is_empty() {
            &mut next
        } else {
            get_path_mut(&mut next, parent).ok_or(ModelApiError::PathNotFound)?
        };
        match (target, leaf) {
            (Value::Object(map), PathStep::Key(key)) => {
                map.insert(key.clone(), value);
            }
            (Value::Array(arr), PathStep::Index(idx)) => {
                let i = (*idx).min(arr.len());
                arr.insert(i, value);
            }
            (Value::Array(arr), PathStep::Append) => {
                arr.push(value);
            }
            _ => return Err(ModelApiError::InvalidPathOp),
        }
        self.apply_target_view(next)
    }

    pub fn replace(&mut self, path: &[PathStep], value: Value) -> Result<(), ModelApiError> {
        if path.is_empty() {
            return self.apply_target_view(value);
        }
        let mut next = self.runtime.view_json();
        let target = get_path_mut(&mut next, path).ok_or(ModelApiError::PathNotFound)?;
        *target = value;
        self.apply_target_view(next)
    }

    pub fn remove(&mut self, path: &[PathStep]) -> Result<(), ModelApiError> {
        self.remove_with_length(path, 1)
    }

    pub fn remove_with_length(
        &mut self,
        path: &[PathStep],
        length: usize,
    ) -> Result<(), ModelApiError> {
        if path.is_empty() {
            return Err(ModelApiError::InvalidPathOp);
        }
        let mut next = self.runtime.view_json();
        let (parent, leaf) = split_parent(path)?;
        let target = if parent.is_empty() {
            &mut next
        } else {
            get_path_mut(&mut next, parent).ok_or(ModelApiError::PathNotFound)?
        };
        match (target, leaf) {
            (Value::Object(map), PathStep::Key(key)) => {
                map.remove(key);
            }
            (Value::Array(arr), PathStep::Index(idx)) => {
                if *idx < arr.len() {
                    let end = (*idx + length.max(1)).min(arr.len());
                    arr.drain(*idx..end);
                }
            }
            (Value::Array(arr), PathStep::Append) => {
                let _ = arr.pop();
            }
            (Value::String(s), PathStep::Index(idx)) => {
                let mut chars: Vec<char> = s.chars().collect();
                if *idx < chars.len() {
                    let end = (*idx + length.max(1)).min(chars.len());
                    chars.drain(*idx..end);
                    *s = chars.into_iter().collect();
                }
            }
            _ => return Err(ModelApiError::InvalidPathOp),
        }
        self.apply_target_view(next)
    }

    // Upstream-compatible tolerant operation helpers: return false on invalid paths/types.
    pub fn try_add(&mut self, path: &[PathStep], value: Value) -> bool {
        self.add(path, value).is_ok()
    }

    pub fn try_add_ptr(&mut self, ptr: &str, value: Value) -> bool {
        let Ok(steps) = parse_json_pointer(ptr) else {
            return false;
        };
        self.try_add(&steps, value)
    }

    pub fn try_replace(&mut self, path: &[PathStep], value: Value) -> bool {
        self.replace(path, value).is_ok()
    }

    pub fn try_replace_ptr(&mut self, ptr: &str, value: Value) -> bool {
        let Ok(steps) = parse_json_pointer(ptr) else {
            return false;
        };
        self.try_replace(&steps, value)
    }

    pub fn try_remove(&mut self, path: &[PathStep]) -> bool {
        self.remove(path).is_ok()
    }

    pub fn try_remove_with_length(&mut self, path: &[PathStep], length: usize) -> bool {
        self.remove_with_length(path, length).is_ok()
    }

    pub fn try_remove_ptr(&mut self, ptr: &str) -> bool {
        let Ok(steps) = parse_json_pointer(ptr) else {
            return false;
        };
        self.try_remove(&steps)
    }

    pub fn merge_ptr(&mut self, ptr: Option<&str>, value: Value) -> bool {
        match ptr {
            None => self.merge(None, value),
            Some(p) => match parse_json_pointer(p) {
                Ok(steps) => self.merge(Some(&steps), value),
                Err(_) => false,
            },
        }
    }

    pub fn op(&mut self, operation: ApiOperation) -> bool {
        match operation {
            ApiOperation::Add { path, value } => self.try_add(&path, value),
            ApiOperation::Replace { path, value } => self.try_replace(&path, value),
            ApiOperation::Remove { path, length } => self.try_remove_with_length(&path, length),
            ApiOperation::Merge { path, value } => self.merge(Some(&path), value),
        }
    }

    pub fn op_tuple(
        &mut self,
        kind: ApiOperationKind,
        path: &[PathStep],
        value: Option<Value>,
        length: Option<usize>,
    ) -> bool {
        match kind {
            ApiOperationKind::Add => value.map(|v| self.try_add(path, v)).unwrap_or(false),
            ApiOperationKind::Replace => value.map(|v| self.try_replace(path, v)).unwrap_or(false),
            ApiOperationKind::Remove => self.try_remove_with_length(path, length.unwrap_or(1)),
            ApiOperationKind::Merge => value.map(|v| self.merge(Some(path), v)).unwrap_or(false),
        }
    }

    pub fn op_ptr_tuple(
        &mut self,
        kind: ApiOperationKind,
        ptr: &str,
        value: Option<Value>,
        length: Option<usize>,
    ) -> bool {
        let Ok(path) = parse_json_pointer(ptr) else {
            return false;
        };
        self.op_tuple(kind, &path, value, length)
    }

    pub fn diff(&self, next: &Value) -> Result<Option<Patch>, ModelApiError> {
        let base = self.runtime.to_model_binary_like()?;
        let patch = diff_model_to_patch_bytes(&base, next, self.sid)?;
        match patch {
            Some(bytes) => {
                let decoded = Patch::from_binary(&bytes)
                    .map_err(|e| ModelApiError::PatchDecode(e.to_string()))?;
                Ok(Some(decoded))
            }
            None => Ok(None),
        }
    }

    pub fn merge(&mut self, path: Option<&[PathStep]>, value: Value) -> bool {
        let mut next = self.runtime.view_json();
        match path {
            None => next = value,
            Some(p) if p.is_empty() => next = value,
            Some(p) => {
                let Some(target) = get_path_mut(&mut next, p) else {
                    return false;
                };
                *target = value;
            }
        }
        self.apply_target_view(next).is_ok()
    }

    pub fn node(&mut self) -> NodeHandle<'_> {
        NodeHandle {
            api: self,
            path: Vec::new(),
        }
    }

    fn apply_target_view(&mut self, next: Value) -> Result<(), ModelApiError> {
        let base = self.runtime.to_model_binary_like()?;
        let patch = diff_model_to_patch_bytes(&base, &next, self.sid)?;
        if let Some(bytes) = patch {
            let decoded =
                Patch::from_binary(&bytes).map_err(|e| ModelApiError::PatchDecode(e.to_string()))?;
            self.apply_patch(&decoded)?;
        }
        Ok(())
    }

    fn emit_change(&mut self, event: ChangeEvent) {
        for listener in self.listeners.values_mut() {
            listener(event.clone());
        }
    }

    fn emit_batch_change(&mut self, event: BatchChangeEvent) {
        for listener in self.batch_listeners.values_mut() {
            listener(event.clone());
        }
    }
}

impl<'a> NodeHandle<'a> {
    pub fn at_key(mut self, key: impl Into<String>) -> Self {
        self.path.push(PathStep::Key(key.into()));
        self
    }

    pub fn at_index(mut self, index: usize) -> Self {
        self.path.push(PathStep::Index(index));
        self
    }

    pub fn at_append(mut self) -> Self {
        self.path.push(PathStep::Append);
        self
    }

    pub fn path(&self) -> &[PathStep] {
        &self.path
    }

    pub fn read(&self) -> Option<Value> {
        self.api.read(Some(&self.path))
    }

    pub fn set(&mut self, value: Value) -> Result<(), ModelApiError> {
        self.api.set(&self.path, value)
    }

    pub fn add(&mut self, value: Value) -> Result<(), ModelApiError> {
        self.api.add(&self.path, value)
    }

    pub fn replace(&mut self, value: Value) -> Result<(), ModelApiError> {
        self.api.replace(&self.path, value)
    }

    pub fn remove(&mut self) -> Result<(), ModelApiError> {
        self.api.remove(&self.path)
    }

    pub fn obj_put(
        &mut self,
        key: impl Into<String>,
        value: Value,
    ) -> Result<(), ModelApiError> {
        self.api.obj_put(&self.path, key, value)
    }

    pub fn arr_push(&mut self, value: Value) -> Result<(), ModelApiError> {
        self.api.arr_push(&self.path, value)
    }

    pub fn str_ins(&mut self, pos: usize, text: &str) -> Result<(), ModelApiError> {
        self.api.str_ins(&self.path, pos, text)
    }

    pub fn as_obj(self) -> Result<ObjHandle<'a>, ModelApiError> {
        match self.read() {
            Some(Value::Object(_)) => Ok(ObjHandle { inner: self }),
            _ => Err(ModelApiError::NotObject),
        }
    }

    pub fn as_arr(self) -> Result<ArrHandle<'a>, ModelApiError> {
        match self.read() {
            Some(Value::Array(_)) => Ok(ArrHandle { inner: self }),
            _ => Err(ModelApiError::NotArray),
        }
    }

    pub fn as_str(self) -> Result<StrHandle<'a>, ModelApiError> {
        match self.read() {
            Some(Value::String(_)) => Ok(StrHandle { inner: self }),
            _ => Err(ModelApiError::NotString),
        }
    }

    pub fn as_val(self) -> Result<ValHandle<'a>, ModelApiError> {
        Ok(ValHandle { inner: self })
    }

    pub fn as_bin(self) -> Result<BinHandle<'a>, ModelApiError> {
        match self.read() {
            Some(Value::Array(arr))
                if arr
                    .iter()
                    .all(|v| v.as_u64().is_some_and(|n| n <= 255)) =>
            {
                Ok(BinHandle { inner: self })
            }
            _ => Err(ModelApiError::NotArray),
        }
    }

    pub fn as_vec(self) -> Result<VecHandle<'a>, ModelApiError> {
        match self.read() {
            Some(Value::Array(_)) => Ok(VecHandle { inner: self }),
            _ => Err(ModelApiError::NotArray),
        }
    }

    pub fn as_con(self) -> Result<ConHandle<'a>, ModelApiError> {
        Ok(ConHandle { inner: self })
    }
}

impl<'a> ObjHandle<'a> {
    pub fn has(&self, key: &str) -> bool {
        self.inner
            .read()
            .and_then(|v| v.as_object().map(|m| m.contains_key(key)))
            .unwrap_or(false)
    }

    pub fn set(&mut self, key: impl Into<String>, value: Value) -> Result<(), ModelApiError> {
        self.inner.obj_put(key, value)
    }

    pub fn del(&mut self, key: &str) -> Result<(), ModelApiError> {
        let mut path = self.inner.path.clone();
        path.push(PathStep::Key(key.to_owned()));
        self.inner.api.remove(&path)
    }
}

impl<'a> ArrHandle<'a> {
    pub fn length(&self) -> usize {
        self.inner
            .read()
            .and_then(|v| v.as_array().map(|a| a.len()))
            .unwrap_or(0)
    }

    pub fn ins(&mut self, index: usize, value: Value) -> Result<(), ModelApiError> {
        let mut path = self.inner.path.clone();
        path.push(PathStep::Index(index));
        self.inner.api.add(&path, value)
    }

    pub fn upd(&mut self, index: usize, value: Value) -> Result<(), ModelApiError> {
        let mut path = self.inner.path.clone();
        path.push(PathStep::Index(index));
        self.inner.api.replace(&path, value)
    }

    pub fn del(&mut self, index: usize) -> Result<(), ModelApiError> {
        let mut path = self.inner.path.clone();
        path.push(PathStep::Index(index));
        self.inner.api.remove(&path)
    }
}

impl<'a> StrHandle<'a> {
    pub fn length(&self) -> usize {
        self.inner
            .read()
            .and_then(|v| v.as_str().map(|s| s.chars().count()))
            .unwrap_or(0)
    }

    pub fn ins(&mut self, index: usize, text: &str) -> Result<(), ModelApiError> {
        self.inner.str_ins(index, text)
    }

    pub fn del(&mut self, index: usize, length: usize) -> Result<(), ModelApiError> {
        let current = self.inner.read().ok_or(ModelApiError::PathNotFound)?;
        let s = current.as_str().ok_or(ModelApiError::NotString)?;
        let mut chars: Vec<char> = s.chars().collect();
        if index < chars.len() {
            let end = (index + length).min(chars.len());
            chars.drain(index..end);
            self.inner
                .api
                .replace(&self.inner.path, Value::String(chars.into_iter().collect()))?;
        }
        Ok(())
    }
}

impl<'a> ValHandle<'a> {
    pub fn view(&self) -> Option<Value> {
        self.inner.read()
    }

    pub fn set(&mut self, value: Value) -> Result<(), ModelApiError> {
        self.inner.replace(value)
    }
}

impl<'a> BinHandle<'a> {
    pub fn length(&self) -> usize {
        self.inner
            .read()
            .and_then(|v| v.as_array().map(|a| a.len()))
            .unwrap_or(0)
    }

    pub fn ins(&mut self, index: usize, bytes: &[u8]) -> Result<(), ModelApiError> {
        let mut current = self.inner.read().ok_or(ModelApiError::PathNotFound)?;
        let arr = current.as_array_mut().ok_or(ModelApiError::NotArray)?;
        let mut i = index.min(arr.len());
        for b in bytes {
            arr.insert(i, Value::from(*b));
            i += 1;
        }
        self.inner.api.replace(&self.inner.path, current)
    }

    pub fn del(&mut self, index: usize, length: usize) -> Result<(), ModelApiError> {
        let mut current = self.inner.read().ok_or(ModelApiError::PathNotFound)?;
        let arr = current.as_array_mut().ok_or(ModelApiError::NotArray)?;
        if index < arr.len() {
            let end = (index + length).min(arr.len());
            arr.drain(index..end);
        }
        self.inner.api.replace(&self.inner.path, current)
    }
}

impl<'a> VecHandle<'a> {
    pub fn set(&mut self, index: usize, value: Option<Value>) -> Result<(), ModelApiError> {
        let mut current = self.inner.read().ok_or(ModelApiError::PathNotFound)?;
        let arr = current.as_array_mut().ok_or(ModelApiError::NotArray)?;
        if index >= arr.len() {
            arr.resize(index + 1, Value::Null);
        }
        match value {
            Some(v) => arr[index] = v,
            None => arr[index] = Value::Null,
        }
        self.inner.api.replace(&self.inner.path, current)
    }
}

impl<'a> ConHandle<'a> {
    pub fn view(&self) -> Option<Value> {
        self.inner.read()
    }

    pub fn set(&mut self, value: Value) -> Result<(), ModelApiError> {
        self.inner.replace(value)
    }
}
