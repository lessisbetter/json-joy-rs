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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathStep {
    Key(String),
    Index(usize),
    Append,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ApiOperation {
    Add { path: Vec<PathStep>, value: Value },
    Replace { path: Vec<PathStep>, value: Value },
    Remove { path: Vec<PathStep>, length: usize },
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeEventOrigin {
    Local,
    Remote,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChangeEvent {
    pub origin: ChangeEventOrigin,
    pub patch_id: Option<(u64, u64)>,
    pub before: Value,
    pub after: Value,
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
        for patch in patches {
            self.apply_patch(patch)?;
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

    pub fn select(&self, path: Option<&[PathStep]>) -> Option<Value> {
        self.read(path)
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
                    arr.remove(*idx);
                }
            }
            (Value::Array(arr), PathStep::Append) => {
                let _ = arr.pop();
            }
            _ => return Err(ModelApiError::InvalidPathOp),
        }
        self.apply_target_view(next)
    }

    // Upstream-compatible tolerant operation helpers: return false on invalid paths/types.
    pub fn try_add(&mut self, path: &[PathStep], value: Value) -> bool {
        self.add(path, value).is_ok()
    }

    pub fn try_replace(&mut self, path: &[PathStep], value: Value) -> bool {
        self.replace(path, value).is_ok()
    }

    pub fn try_remove(&mut self, path: &[PathStep]) -> bool {
        self.remove(path).is_ok()
    }

    pub fn op(&mut self, operation: ApiOperation) -> bool {
        match operation {
            ApiOperation::Add { path, value } => self.try_add(&path, value),
            ApiOperation::Replace { path, value } => self.try_replace(&path, value),
            ApiOperation::Remove { path, .. } => self.try_remove(&path),
        }
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

fn get_path_mut<'a>(value: &'a mut Value, path: &[PathStep]) -> Option<&'a mut Value> {
    let mut cur = value;
    for step in path {
        match (step, cur) {
            (PathStep::Key(key), Value::Object(map)) => {
                cur = map.get_mut(key)?;
            }
            (PathStep::Index(idx), Value::Array(arr)) => {
                cur = arr.get_mut(*idx)?;
            }
            _ => return None,
        }
    }
    Some(cur)
}

fn split_parent(path: &[PathStep]) -> Result<(&[PathStep], &PathStep), ModelApiError> {
    if path.is_empty() {
        return Err(ModelApiError::InvalidPathOp);
    }
    let (parent, leaf) = path.split_at(path.len() - 1);
    Ok((parent, &leaf[0]))
}
