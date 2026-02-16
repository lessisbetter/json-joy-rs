//! Internal runtime model for fixture-driven patch apply/replay testing (M3).
//!
//! Design note:
//! - M3 uses oracle-first replay fixtures whose base model is deterministic.
//! - We intentionally bootstrap from `Model::from_binary` view parity and build
//!   replay semantics in apply logic, instead of re-implementing full model
//!   binary node-id decoding in this milestone.

use crate::crdt_binary::{first_logical_clock_sid_time, LogicalClockBase};
use crate::model::{Model, ModelError};
use crate::patch::Patch;
use serde_json::Value;
use std::collections::HashMap;
use thiserror::Error;

mod apply;
mod cbor;
mod decode;
mod encode;
mod query;
mod rga;
pub(crate) mod types;
mod view;

use decode::{bootstrap_graph_from_view, decode_runtime_graph};
use encode::{encode_logical, encode_server};
use types::{ClockState, Id, RuntimeNode};

#[derive(Debug, Error)]
pub enum ApplyError {
    #[error("unsupported operation for runtime apply")]
    UnsupportedOpForM3,
    #[error("runtime graph invariant violation: {0}")]
    InvariantViolation(String),
}

#[derive(Debug, Clone)]
pub struct RuntimeModel {
    pub(crate) nodes: HashMap<Id, RuntimeNode>,
    pub(crate) root: Option<Id>,
    pub(crate) clock: ClockState,
    pub(crate) fallback_view: Value,
    pub(crate) infer_empty_object_root: bool,
    pub(crate) clock_table: Vec<LogicalClockBase>,
    pub(crate) server_clock_time: Option<u64>,
}

impl RuntimeModel {
    pub fn new_logical_empty(sid: u64) -> Self {
        Self {
            nodes: HashMap::new(),
            root: None,
            clock: ClockState::default(),
            fallback_view: Value::Null,
            infer_empty_object_root: false,
            clock_table: vec![LogicalClockBase { sid, time: 0 }],
            server_clock_time: None,
        }
    }

    pub fn from_model_binary(data: &[u8]) -> Result<Self, ModelError> {
        let model = Model::from_binary(data)?;
        let view = model.view().clone();
        let infer_empty_object_root = matches!(view, Value::Object(ref m) if m.is_empty());

        let decoded = decode_runtime_graph(data);
        let (nodes, root, clock_table, server_clock_time, fallback_view) = match decoded {
            Ok((nodes, root, clock_table, server_clock_time)) => {
                (nodes, root, clock_table, server_clock_time, Value::Null)
            }
            Err(_) => {
                // Reduce fallback-only behavior by materializing a deterministic
                // runtime graph from already-parsed JSON view when structural
                // graph decode is unavailable.
                let sid = if data.first().is_some_and(|b| (b & 0x80) != 0) {
                    1
                } else {
                    first_logical_clock_sid_time(data)
                        .map(|(sid, _)| sid)
                        .unwrap_or(1)
                };
                let (nodes, root, clock_table) = bootstrap_graph_from_view(&view, sid);
                (nodes, root, clock_table, None, Value::Null)
            }
        };

        Ok(Self {
            nodes,
            root,
            clock: ClockState::default(),
            fallback_view,
            infer_empty_object_root,
            clock_table,
            server_clock_time,
        })
    }

    pub fn apply_patch(&mut self, patch: &Patch) -> Result<(), ApplyError> {
        for op in patch.decoded_ops() {
            let id = Id::from(op.id());
            let span = op.span();
            self.clock.observe(id.sid, id.time, span);
            self.apply_op(op)?;
            #[cfg(debug_assertions)]
            self.validate_invariants()
                .map_err(ApplyError::InvariantViolation)?;
        }
        Ok(())
    }

    pub fn view_json(&self) -> Value {
        match self.root {
            Some(id) => self.node_view(id).unwrap_or(Value::Null),
            None => self.fallback_view.clone(),
        }
    }

    pub fn to_model_binary_like(&self) -> Result<Vec<u8>, ModelError> {
        if let Some(server_time) = self.server_clock_time {
            return encode_server(self, server_time);
        }
        encode_logical(self)
    }

    pub fn fork_with_sid(&self, sid: u64) -> Self {
        let mut cloned = self.clone();
        if cloned.server_clock_time.is_some() {
            return cloned;
        }
        if cloned.clock_table.is_empty() {
            cloned.clock_table = vec![LogicalClockBase { sid, time: 0 }];
            cloned.clock = ClockState::default();
            return cloned;
        }

        let old_local = cloned.clock_table[0];
        let mut peers: HashMap<u64, u64> = HashMap::new();
        for c in cloned.clock_table.iter().skip(1) {
            peers
                .entry(c.sid)
                .and_modify(|t| *t = (*t).max(c.time))
                .or_insert(c.time);
        }
        if sid != old_local.sid {
            peers
                .entry(old_local.sid)
                .and_modify(|t| *t = (*t).max(old_local.time))
                .or_insert(old_local.time);
        }

        let mut next = vec![LogicalClockBase {
            sid,
            time: old_local.time,
        }];
        for (peer_sid, peer_time) in peers {
            if peer_sid != sid {
                next.push(LogicalClockBase {
                    sid: peer_sid,
                    time: peer_time,
                });
            }
        }
        next.sort_by_key(|c| (c.sid != sid, c.sid));
        cloned.clock_table = next;
        cloned.clock = ClockState::default();
        cloned
    }

    pub fn validate_invariants(&self) -> Result<(), String> {
        if let Some(root) = self.root {
            if !self.nodes.contains_key(&root) {
                return Err("root points to missing node".to_string());
            }
        }

        for (id, node) in &self.nodes {
            match node {
                RuntimeNode::Con(_) => {}
                RuntimeNode::Val(child) => {
                    if (child.sid != 0 || child.time != 0) && !self.nodes.contains_key(child) {
                        return Err(format!(
                            "val node {}.{} points to missing child {}.{}",
                            id.sid, id.time, child.sid, child.time
                        ));
                    }
                }
                RuntimeNode::Obj(entries) => {
                    let mut seen = std::collections::HashSet::new();
                    for (k, v) in entries {
                        if !seen.insert(k) {
                            return Err(format!(
                                "obj node {}.{} has duplicate key {}",
                                id.sid, id.time, k
                            ));
                        }
                        if !self.nodes.contains_key(v) {
                            return Err(format!(
                                "obj node {}.{} key {} points to missing child {}.{}",
                                id.sid, id.time, k, v.sid, v.time
                            ));
                        }
                    }
                }
                RuntimeNode::Vec(map) => {
                    for (idx, v) in map {
                        if !self.nodes.contains_key(v) {
                            return Err(format!(
                                "vec node {}.{} index {} points to missing child {}.{}",
                                id.sid, id.time, idx, v.sid, v.time
                            ));
                        }
                    }
                }
                RuntimeNode::Str(atoms) => {
                    let mut seen = std::collections::HashSet::new();
                    for atom in atoms {
                        if !seen.insert(atom.slot) {
                            return Err(format!(
                                "str node {}.{} has duplicate slot {}.{}",
                                id.sid, id.time, atom.slot.sid, atom.slot.time
                            ));
                        }
                    }
                }
                RuntimeNode::Bin(atoms) => {
                    let mut seen = std::collections::HashSet::new();
                    for atom in atoms {
                        if !seen.insert(atom.slot) {
                            return Err(format!(
                                "bin node {}.{} has duplicate slot {}.{}",
                                id.sid, id.time, atom.slot.sid, atom.slot.time
                            ));
                        }
                    }
                }
                RuntimeNode::Arr(atoms) => {
                    let mut seen = std::collections::HashSet::new();
                    for atom in atoms {
                        if !seen.insert(atom.slot) {
                            return Err(format!(
                                "arr node {}.{} has duplicate slot {}.{}",
                                id.sid, id.time, atom.slot.sid, atom.slot.time
                            ));
                        }
                        if let Some(value_id) = atom.value {
                            if !self.nodes.contains_key(&value_id) {
                                return Err(format!(
                                    "arr node {}.{} slot {}.{} points to missing child {}.{}",
                                    id.sid,
                                    id.time,
                                    atom.slot.sid,
                                    atom.slot.time,
                                    value_id.sid,
                                    value_id.time
                                ));
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
