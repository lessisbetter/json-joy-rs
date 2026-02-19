//! JSON CRDT document model.
//!
//! Mirrors `packages/json-joy/src/json-crdt/model/Model.ts`.
//!
//! # Overview
//!
//! A [`Model`] is the in-memory representation of a JSON CRDT document.
//! It contains a node index (all known CRDT nodes keyed by their timestamp ID)
//! and a logical clock that tracks which operations have been seen.
//!
//! Operations are applied via [`Model::apply_patch`] or
//! [`Model::apply_operation`].  The resulting JSON view can be obtained with
//! [`Model::view`].

pub mod util;
pub mod api;

pub use api::ModelApi;

use serde_json::Value;

use crate::json_crdt_patch::clock::{ClockVector, Ts};
use crate::json_crdt_patch::enums::SESSION;
use crate::json_crdt_patch::operations::{ConValue, Op};
use crate::json_crdt_patch::patch::Patch;
use super::constants::ORIGIN;
use super::nodes::{
    ArrNode, BinNode, ConNode, CrdtNode, IndexExt, NodeIndex, ObjNode, RootNode,
    StrNode, ValNode, VecNode,
};

/// In-memory JSON CRDT document model.
///
/// Tracks all CRDT nodes in an index and advances a logical clock as patches
/// are applied.
///
/// `tick` mirrors `Model.tick` in the upstream TypeScript: it increments once
/// per `apply_patch` call and is used by callers (e.g. the WASM layer) as a
/// cheap mutation counter to decide when a cached view needs to be rebuilt.
#[derive(Debug, Clone)]
pub struct Model {
    /// Document root — a LWW register pointing at the top-level JSON value.
    pub root: RootNode,
    /// All CRDT nodes keyed by their timestamp ID.
    pub index: NodeIndex,
    /// Logical clock — tracks local time and the times of all peers.
    pub clock: ClockVector,
    /// Mutation counter — incremented once per `apply_patch` call.
    ///
    /// Mirrors `Model.tick` in the upstream TypeScript.
    pub tick: u64,
}

impl Model {
    /// Create a new empty model with the given session ID.
    ///
    /// The clock starts at time `1` so that time `0` (ORIGIN) is permanently
    /// reserved as the "undefined/null" sentinel.
    pub fn new(sid: u64) -> Self {
        Self {
            root: RootNode::new(),
            index: NodeIndex::default(),
            clock: ClockVector::new(sid, 1),
            tick: 0,
        }
    }

    /// Create a model with a randomly-generated session ID.
    pub fn create() -> Self {
        // Use a simple pseudo-random SID (same range as upstream: ≥ 65536).
        let sid = random_sid();
        Self::new(sid)
    }

    /// Return the JSON view of the current document state.
    pub fn view(&self) -> Value {
        self.root.view(&self.index)
    }

    /// Apply all operations in `patch` to this model.
    ///
    /// Increments `self.tick` after all operations are applied, mirroring
    /// `Model.applyPatch` in the upstream TypeScript which does `this.tick++`
    /// at the end of each patch application.
    pub fn apply_patch(&mut self, patch: &Patch) {
        for op in &patch.ops {
            self.apply_operation(op);
        }
        self.tick += 1;
    }

    /// Apply a single operation.
    ///
    /// Mirrors `Model.applyOperation` in the upstream TypeScript.
    pub fn apply_operation(&mut self, op: &Op) {
        // Advance the clock by observing this operation's ID + span.
        self.clock.observe(op.id(), op.span());

        match op {
            // ── Creation operations ────────────────────────────────────────
            Op::NewCon { id, val } => {
                if !self.index.contains_ts(id) {
                    self.index.insert_node(*id, CrdtNode::Con(ConNode::new(*id, val.clone())));
                }
            }
            Op::NewVal { id } => {
                if !self.index.contains_ts(id) {
                    self.index.insert_node(*id, CrdtNode::Val(ValNode::new(*id)));
                }
            }
            Op::NewObj { id } => {
                if !self.index.contains_ts(id) {
                    self.index.insert_node(*id, CrdtNode::Obj(ObjNode::new(*id)));
                }
            }
            Op::NewVec { id } => {
                if !self.index.contains_ts(id) {
                    self.index.insert_node(*id, CrdtNode::Vec(VecNode::new(*id)));
                }
            }
            Op::NewStr { id } => {
                if !self.index.contains_ts(id) {
                    self.index.insert_node(*id, CrdtNode::Str(StrNode::new(*id)));
                }
            }
            Op::NewBin { id } => {
                if !self.index.contains_ts(id) {
                    self.index.insert_node(*id, CrdtNode::Bin(BinNode::new(*id)));
                }
            }
            Op::NewArr { id } => {
                if !self.index.contains_ts(id) {
                    self.index.insert_node(*id, CrdtNode::Arr(ArrNode::new(*id)));
                }
            }

            // ── Mutation operations ────────────────────────────────────────

            /// Set the value of a `val` register (or the document root).
            Op::InsVal { obj, val, .. } => {
                // The root register is addressed by ORIGIN (SESSION::SYSTEM, time 0).
                if obj.sid == SESSION::SYSTEM && obj.time == ORIGIN.time {
                    // Update the document root.
                    self.root.set(*val);
                } else if let Some(CrdtNode::Val(node)) = self.index.get_mut_ts(obj) {
                    node.set(*val);
                }
            }

            /// Set key→value pairs in an `obj` map.
            Op::InsObj { obj, data, .. } => {
                if let Some(CrdtNode::Obj(node)) = self.index.get_mut_ts(obj) {
                    for (key, val_id) in data {
                        // Upstream: skip if node.id.time >= val_id.time
                        if node.id.time >= val_id.time {
                            continue;
                        }
                        node.put(key, *val_id);
                    }
                }
            }

            /// Set index→value pairs in a `vec` vector.
            Op::InsVec { obj, data, .. } => {
                if let Some(CrdtNode::Vec(node)) = self.index.get_mut_ts(obj) {
                    for (idx, val_id) in data {
                        if node.id.time >= val_id.time {
                            continue;
                        }
                        node.put(*idx as usize, *val_id);
                    }
                }
            }

            /// Insert text into a `str` RGA.
            Op::InsStr { id, obj, after, data } => {
                if let Some(CrdtNode::Str(node)) = self.index.get_mut_ts(obj) {
                    node.ins(*after, *id, data.clone());
                }
            }

            /// Insert bytes into a `bin` RGA.
            Op::InsBin { id, obj, after, data } => {
                if let Some(CrdtNode::Bin(node)) = self.index.get_mut_ts(obj) {
                    node.ins(*after, *id, data.clone());
                }
            }

            /// Insert node-ID references into an `arr` RGA.
            Op::InsArr { id, obj, after, data } => {
                if let Some(CrdtNode::Arr(node)) = self.index.get_mut_ts(obj) {
                    // Filter out references older than the array node itself.
                    let filtered: Vec<Ts> = data
                        .iter()
                        .filter(|stamp| node.id.time < stamp.time)
                        .copied()
                        .collect();
                    if !filtered.is_empty() {
                        node.ins(*after, *id, filtered);
                    }
                }
            }

            /// Update (replace) an existing element in an `arr` RGA.
            Op::UpdArr { obj, after, val, .. } => {
                if let Some(CrdtNode::Arr(node)) = self.index.get_mut_ts(obj) {
                    node.upd(*after, *val);
                }
            }

            /// Delete ranges in a `str`, `bin`, or `arr`.
            Op::Del { obj, what, .. } => {
                match self.index.get_mut_ts(obj) {
                    Some(CrdtNode::Str(node)) => node.delete(what),
                    Some(CrdtNode::Bin(node)) => node.delete(what),
                    Some(CrdtNode::Arr(node)) => node.delete(what),
                    _ => {}
                }
            }

            Op::Nop { .. } => {}
        }
    }

    /// Advance the model clock and return the next available timestamp for
    /// this session.  Used when building patches locally.
    pub fn next_ts(&mut self) -> Ts {
        self.clock.tick(1)
    }

    /// Create a model with a server clock at the given time.
    ///
    /// Used by codec decoders to reconstruct a document from a server-clock snapshot.
    pub fn new_server(server_time: u64) -> Self {
        use crate::json_crdt_patch::enums::SESSION;
        Self {
            root: super::nodes::RootNode::new(),
            index: super::nodes::NodeIndex::default(),
            clock: ClockVector::new(SESSION::SERVER, server_time),
            tick: 0,
        }
    }

    /// Create a model from an existing clock vector.
    ///
    /// Used by codec decoders to reconstruct a document from a logical-clock snapshot.
    pub fn new_from_clock(clock: ClockVector) -> Self {
        Self {
            root: super::nodes::RootNode::new(),
            index: super::nodes::NodeIndex::default(),
            clock,
            tick: 0,
        }
    }
}

/// Very simple pseudo-random session ID generator.
/// Produces values in `[65536, u64::MAX]`.
/// Uses both seconds and sub-second nanos to avoid collisions within the same second.
fn random_sid() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let d = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    // Combine seconds (upper bits) and nanos (lower 30 bits) for more entropy.
    let seed = (d.as_secs() << 30) ^ (d.subsec_nanos() as u64);
    seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407) | 65536
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_crdt_patch::clock::ts;
    use crate::json_crdt_patch::operations::ConValue;
    use json_joy_json_pack::PackValue;
    use serde_json::json;

    fn sid() -> u64 { 123456 }

    /// Helper: build a simple one-string-field object.
    ///
    /// Simulates:
    ///   new_obj @1
    ///   new_str @2
    ///   ins_str @3 [obj=2, after=ORIGIN, data="hello"]
    ///   ins_obj @4 [obj=1, data=[("key", 2)]]
    ///   ins_val @5 [obj=ORIGIN, val=1]
    fn make_str_obj_patch(model: &mut Model) -> Value {
        let s = sid();
        // new_obj → id 1
        model.apply_operation(&Op::NewObj { id: ts(s, 1) });
        // new_str → id 2
        model.apply_operation(&Op::NewStr { id: ts(s, 2) });
        // ins_str → id 3, insert "hello" after ORIGIN in str node 2
        model.apply_operation(&Op::InsStr {
            id: ts(s, 3),
            obj: ts(s, 2),
            after: ORIGIN,
            data: "hello".to_string(),
        });
        // ins_obj → id 8 (span=1), put ("key", ts(s,2)) into obj 1
        model.apply_operation(&Op::InsObj {
            id: ts(s, 8),
            obj: ts(s, 1),
            data: vec![("key".to_string(), ts(s, 2))],
        });
        // ins_val → set root to obj 1
        model.apply_operation(&Op::InsVal {
            id: ts(s, 9),
            obj: ORIGIN,
            val: ts(s, 1),
        });
        model.view()
    }

    #[test]
    fn empty_model_view_is_null() {
        let model = Model::new(sid());
        assert_eq!(model.view(), json!(null));
    }

    #[test]
    fn new_con_then_set_root() {
        let mut model = Model::new(sid());
        let s = sid();
        // new_con with value 42
        model.apply_operation(&Op::NewCon {
            id: ts(s, 1),
            val: ConValue::Val(PackValue::Integer(42)),
        });
        // set root to the con node
        model.apply_operation(&Op::InsVal {
            id: ts(s, 2),
            obj: ORIGIN,
            val: ts(s, 1),
        });
        assert_eq!(model.view(), json!(42));
    }

    #[test]
    fn new_str_insert_and_view() {
        let mut model = Model::new(sid());
        let s = sid();
        model.apply_operation(&Op::NewStr { id: ts(s, 1) });
        model.apply_operation(&Op::InsStr {
            id: ts(s, 2),
            obj: ts(s, 1),
            after: ORIGIN,
            data: "hello".to_string(),
        });
        model.apply_operation(&Op::InsVal {
            id: ts(s, 7),
            obj: ORIGIN,
            val: ts(s, 1),
        });
        assert_eq!(model.view(), json!("hello"));
    }

    #[test]
    fn str_delete_chars() {
        let mut model = Model::new(sid());
        let s = sid();
        model.apply_operation(&Op::NewStr { id: ts(s, 1) });
        // insert "hello" at id ts(s,2)..ts(s,6)
        model.apply_operation(&Op::InsStr {
            id: ts(s, 2),
            obj: ts(s, 1),
            after: ORIGIN,
            data: "hello".to_string(),
        });
        model.apply_operation(&Op::InsVal {
            id: ts(s, 7),
            obj: ORIGIN,
            val: ts(s, 1),
        });
        // delete "ell" = ts(s,3)..ts(s,5)
        use crate::json_crdt_patch::clock::tss;
        model.apply_operation(&Op::Del {
            id: ts(s, 8),
            obj: ts(s, 1),
            what: vec![tss(s, 3, 3)],
        });
        assert_eq!(model.view(), json!("ho"));
    }

    #[test]
    fn obj_node_view() {
        let mut model = Model::new(sid());
        let view = make_str_obj_patch(&mut model);
        assert_eq!(view, json!({ "key": "hello" }));
    }

    #[test]
    fn vec_node_view() {
        let mut model = Model::new(sid());
        let s = sid();
        model.apply_operation(&Op::NewVec { id: ts(s, 1) });
        model.apply_operation(&Op::NewCon {
            id: ts(s, 2),
            val: ConValue::Val(PackValue::Bool(true)),
        });
        model.apply_operation(&Op::NewCon {
            id: ts(s, 3),
            val: ConValue::Val(PackValue::Integer(99)),
        });
        model.apply_operation(&Op::InsVec {
            id: ts(s, 4),
            obj: ts(s, 1),
            data: vec![(0u8, ts(s, 2)), (1u8, ts(s, 3))],
        });
        model.apply_operation(&Op::InsVal {
            id: ts(s, 5),
            obj: ORIGIN,
            val: ts(s, 1),
        });
        assert_eq!(model.view(), json!([true, 99]));
    }

    #[test]
    fn arr_node_view() {
        let mut model = Model::new(sid());
        let s = sid();
        model.apply_operation(&Op::NewArr { id: ts(s, 1) });
        model.apply_operation(&Op::NewCon {
            id: ts(s, 2),
            val: ConValue::Val(PackValue::Integer(10)),
        });
        model.apply_operation(&Op::NewCon {
            id: ts(s, 3),
            val: ConValue::Val(PackValue::Integer(20)),
        });
        model.apply_operation(&Op::InsArr {
            id: ts(s, 4),
            obj: ts(s, 1),
            after: ORIGIN,
            data: vec![ts(s, 2), ts(s, 3)],
        });
        model.apply_operation(&Op::InsVal {
            id: ts(s, 6),
            obj: ORIGIN,
            val: ts(s, 1),
        });
        assert_eq!(model.view(), json!([10, 20]));
    }

    #[test]
    fn bin_node_view() {
        let mut model = Model::new(sid());
        let s = sid();
        model.apply_operation(&Op::NewBin { id: ts(s, 1) });
        model.apply_operation(&Op::InsBin {
            id: ts(s, 2),
            obj: ts(s, 1),
            after: ORIGIN,
            data: vec![0xDE, 0xAD],
        });
        model.apply_operation(&Op::InsVal {
            id: ts(s, 4),
            obj: ORIGIN,
            val: ts(s, 1),
        });
        // BinNode view_json returns JSON array of byte values
        assert_eq!(model.view(), json!([0xDE, 0xAD]));
    }

    #[test]
    fn duplicate_ops_are_idempotent() {
        let mut model = Model::new(sid());
        let s = sid();
        let op = Op::NewStr { id: ts(s, 1) };
        model.apply_operation(&op);
        model.apply_operation(&op);
        // Should only have one node for ts(s,1)
        use super::super::nodes::TsKey;
        let key = TsKey { sid: s, time: 1 };
        assert!(model.index.contains_key(&key));
    }

    #[test]
    fn lww_wins_higher_timestamp() {
        let mut model = Model::new(sid());
        let s = sid();
        model.apply_operation(&Op::NewCon {
            id: ts(s, 1),
            val: ConValue::Val(PackValue::Integer(1)),
        });
        model.apply_operation(&Op::NewCon {
            id: ts(s, 2),
            val: ConValue::Val(PackValue::Integer(2)),
        });
        // Two InsVal ops: both target root. Higher timestamp wins.
        model.apply_operation(&Op::InsVal {
            id: ts(s, 3),
            obj: ORIGIN,
            val: ts(s, 2),
        });
        model.apply_operation(&Op::InsVal {
            id: ts(s, 4),
            obj: ORIGIN,
            val: ts(s, 1),
        });
        // ts(s,2) > ts(s,1), so val 2 should win regardless of apply order
        assert_eq!(model.view(), json!(2));
    }

    #[test]
    fn upd_arr_replaces_element() {
        let s = sid();
        let mut model = Model::new(s);

        // Create arr node containing [con(42)]
        model.apply_operation(&Op::NewArr { id: ts(s, 1) });
        model.apply_operation(&Op::NewCon {
            id: ts(s, 2),
            val: ConValue::Val(PackValue::Integer(42)),
        });
        // InsArr slot ts(s,3) → points to ts(s,2)
        model.apply_operation(&Op::InsArr {
            id: ts(s, 3),
            obj: ts(s, 1),
            after: ORIGIN,
            data: vec![ts(s, 2)],
        });
        model.apply_operation(&Op::InsVal {
            id: ts(s, 4),
            obj: ORIGIN,
            val: ts(s, 1),
        });
        assert_eq!(model.view(), json!([42]));

        // Create a new con node with value 99 and update the slot
        model.apply_operation(&Op::NewCon {
            id: ts(s, 5),
            val: ConValue::Val(PackValue::Integer(99)),
        });
        // UpdArr: update slot ts(s,3) to point to ts(s,5)
        model.apply_operation(&Op::UpdArr {
            id: ts(s, 6),
            obj: ts(s, 1),
            after: ts(s, 3),
            val: ts(s, 5),
        });
        assert_eq!(model.view(), json!([99]));
    }
}
