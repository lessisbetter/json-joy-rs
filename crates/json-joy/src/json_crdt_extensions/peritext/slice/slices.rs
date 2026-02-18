//! `Slices` — the CRDT-ordered collection of [`Slice`]s for a Peritext field.
//!
//! Mirrors `packages/json-joy/src/json-crdt-extensions/peritext/slice/Slices.ts`.

use serde_json::Value;
use json_joy_json_pack::PackValue;

use crate::json_crdt::constants::ORIGIN;
use crate::json_crdt::model::Model;
use crate::json_crdt::nodes::{CrdtNode, IndexExt, TsKey};
use crate::json_crdt_patch::clock::{Ts, Tss};
use crate::json_crdt_patch::operations::{ConValue, Op};
use crate::json_crdt_extensions::peritext::rga::{Anchor, Point, Range};
use super::{
    constants::{
        HEADER_X1_ANCHOR_BIT, HEADER_X2_ANCHOR_BIT, HEADER_STACKING_SHIFT,
        tuple_index,
    },
    Slice, SliceStacking, SliceType,
};

const HEADER_STACKING_MASK_LOCAL: u64 = 0b111 << HEADER_STACKING_SHIFT;

// ── Slices ────────────────────────────────────────────────────────────────

/// CRDT-ordered collection of [`Slice`]s backed by an [`ArrNode`].
#[derive(Debug, Clone, Copy)]
pub struct Slices {
    pub arr_id: Ts,
}

impl Slices {
    pub fn new(arr_id: Ts) -> Self {
        Self { arr_id }
    }

    // ── Insertion ─────────────────────────────────────────────────────────

    /// Insert a new slice and return its ID (the `VecNode` ID).
    pub fn ins(
        &self,
        model: &mut Model,
        range: &Range,
        stacking: SliceStacking,
        slice_type: impl Into<SliceType>,
        data: Option<Value>,
    ) -> Ts {
        let slice_type = slice_type.into();

        let x1_anchor_bit: u64 = match range.start.anchor {
            Anchor::Before => 0,
            Anchor::After  => HEADER_X1_ANCHOR_BIT,
        };
        let x2_anchor_bit: u64 = match range.end.anchor {
            Anchor::Before => 0,
            Anchor::After  => HEADER_X2_ANCHOR_BIT,
        };
        let header_bits = ((stacking as u64) << HEADER_STACKING_SHIFT)
            | x2_anchor_bit
            | x1_anchor_bit;

        let same_point = range.start.id == range.end.id;

        // Allocate vec_id FIRST so its timestamp is lower than the con node IDs.
        // The InsVec guard skips elements whose time <= vec_id.time, so con IDs
        // must come after (higher timestamps) for them to be accepted.
        let vec_id    = model.next_ts();
        let header_id = model.next_ts();
        let x1_id     = model.next_ts();
        let x2_id     = model.next_ts();
        let type_id   = model.next_ts();

        // Create the VecNode container first (so its ID is lowest).
        model.apply_operation(&Op::NewVec { id: vec_id });

        model.apply_operation(&Op::NewCon {
            id:  header_id,
            val: ConValue::Val(PackValue::UInteger(header_bits)),
        });
        model.apply_operation(&Op::NewCon {
            id:  x1_id,
            val: ConValue::Val(ts_to_pack(range.start.id)),
        });
        model.apply_operation(&Op::NewCon {
            id:  x2_id,
            val: ConValue::Val(if same_point {
                PackValue::Integer(0)
            } else {
                ts_to_pack(range.end.id)
            }),
        });
        model.apply_operation(&Op::NewCon {
            id:  type_id,
            val: ConValue::Val(slice_type.to_pack()),
        });

        let data_id = data.map(|d| {
            let id = model.next_ts();
            model.apply_operation(&Op::NewCon {
                id,
                val: ConValue::Val(PackValue::from(d)),
            });
            id
        });

        let mut vec_data: Vec<(u8, Ts)> = vec![
            (tuple_index::HEADER as u8, header_id),
            (tuple_index::X1     as u8, x1_id),
            (tuple_index::X2     as u8, x2_id),
            (tuple_index::TYPE_  as u8, type_id),
        ];
        if let Some(d_id) = data_id {
            vec_data.push((tuple_index::DATA as u8, d_id));
        }
        let ins_vec_id = model.next_ts();
        model.apply_operation(&Op::InsVec {
            id:   ins_vec_id,
            obj:  vec_id,
            data: vec_data,
        });

        let ins_arr_id = model.next_ts();
        model.apply_operation(&Op::InsArr {
            id:    ins_arr_id,
            obj:   self.arr_id,
            after: ORIGIN,
            data:  vec![vec_id],
        });

        vec_id
    }

    /// Insert a `Many`-stacking (stackable) slice.
    pub fn ins_stack(
        &self,
        model: &mut Model,
        range: &Range,
        slice_type: impl Into<SliceType>,
        data: Option<Value>,
    ) -> Ts {
        self.ins(model, range, SliceStacking::Many, slice_type, data)
    }

    /// Insert a `One`-stacking (exclusive) slice.
    pub fn ins_one(
        &self,
        model: &mut Model,
        range: &Range,
        slice_type: impl Into<SliceType>,
        data: Option<Value>,
    ) -> Ts {
        self.ins(model, range, SliceStacking::One, slice_type, data)
    }

    /// Insert a block-split `Marker`.
    pub fn ins_marker(
        &self,
        model: &mut Model,
        range: &Range,
        slice_type: impl Into<SliceType>,
        data: Option<Value>,
    ) -> Ts {
        self.ins(model, range, SliceStacking::Marker, slice_type, data)
    }

    // ── Deletion ──────────────────────────────────────────────────────────

    /// Soft-delete the slice with the given `vec_id` from the ArrNode.
    pub fn del(&self, model: &mut Model, vec_id: Ts) {
        let slot_tss: Option<Tss> = {
            let Some(CrdtNode::Arr(arr)) = model.index.get(&TsKey::from(self.arr_id)) else {
                return;
            };
            let mut found = None;
            'outer: for chunk in &arr.rga.chunks {
                if chunk.deleted {
                    continue;
                }
                if let Some(data) = &chunk.data {
                    for (offset, &data_id) in data.iter().enumerate() {
                        if data_id == vec_id {
                            found = Some(Tss::new(
                                chunk.id.sid,
                                chunk.id.time + offset as u64,
                                1,
                            ));
                            break 'outer;
                        }
                    }
                }
            }
            found
        };

        if let Some(tss) = slot_tss {
            let del_id = model.next_ts();
            model.apply_operation(&Op::Del {
                id:   del_id,
                obj:  self.arr_id,
                what: vec![tss],
            });
        }
    }

    // ── Querying ──────────────────────────────────────────────────────────

    /// Number of live slice entries in the `ArrNode`.
    pub fn size(&self, model: &Model) -> usize {
        match model.index.get(&TsKey::from(self.arr_id)) {
            Some(CrdtNode::Arr(arr)) => arr.size(),
            _ => 0,
        }
    }

    /// Return all live slices, deserialised from the model.
    pub fn iter_slices(&self, model: &Model) -> Vec<Slice> {
        let vec_ids: Vec<Ts> = {
            let Some(CrdtNode::Arr(arr)) = model.index.get(&TsKey::from(self.arr_id)) else {
                return Vec::new();
            };
            arr.rga
                .iter_live()
                .filter_map(|chunk| chunk.data.as_ref())
                .flat_map(|ids| ids.iter().copied())
                .collect()
        };

        vec_ids
            .into_iter()
            .filter_map(|vid| deserialize_slice(model, vid))
            .collect()
    }

    /// Look up a single slice by its `vec_id`.
    pub fn get(&self, model: &Model, vec_id: Ts) -> Option<Slice> {
        let present = {
            let Some(CrdtNode::Arr(arr)) = model.index.get(&TsKey::from(self.arr_id)) else {
                return None;
            };
            arr.rga
                .iter_live()
                .filter_map(|chunk| chunk.data.as_ref())
                .flat_map(|ids| ids.iter())
                .any(|&id| id == vec_id)
        };
        if present { deserialize_slice(model, vec_id) } else { None }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────

fn ts_to_pack(ts: Ts) -> PackValue {
    PackValue::Array(vec![
        PackValue::UInteger(ts.sid),
        PackValue::UInteger(ts.time),
    ])
}

fn pack_to_ts(pv: &PackValue) -> Option<Ts> {
    let PackValue::Array(arr) = pv else { return None; };
    if arr.len() < 2 { return None; }
    let sid = match &arr[0] {
        PackValue::UInteger(n) => *n,
        PackValue::Integer(n) if *n >= 0 => *n as u64,
        _ => return None,
    };
    let time = match &arr[1] {
        PackValue::UInteger(n) => *n,
        PackValue::Integer(n) if *n >= 0 => *n as u64,
        _ => return None,
    };
    Some(Ts::new(sid, time))
}

/// Deserialise a [`Slice`] from a `VecNode` in the model.
fn deserialize_slice(model: &Model, vec_id: Ts) -> Option<Slice> {
    // VecNode lookup.
    let Some(CrdtNode::Vec(vec)) = model.index.get(&TsKey::from(vec_id)) else {
        return None;
    };

    // Element 0: header con.
    let header_id: Ts = vec.elements.get(tuple_index::HEADER).copied().flatten()?;
    let header: u64 = match model.index.get(&TsKey::from(header_id))? {
        CrdtNode::Con(c) => match &c.val {
            ConValue::Val(PackValue::UInteger(n)) => *n,
            ConValue::Val(PackValue::Integer(n))  => *n as u64,
            _ => return None,
        },
        _ => return None,
    };
    let stacking_bits = ((header & HEADER_STACKING_MASK_LOCAL) >> HEADER_STACKING_SHIFT) as u8;
    let stacking  = SliceStacking::try_from(stacking_bits).ok()?;
    let x1_anchor = if header & HEADER_X1_ANCHOR_BIT != 0 { Anchor::After } else { Anchor::Before };
    let x2_anchor = if header & HEADER_X2_ANCHOR_BIT != 0 { Anchor::After } else { Anchor::Before };

    // Element 1: x1 con.
    let x1_id: Ts = vec.elements.get(tuple_index::X1).copied().flatten()?;
    let x1_ts: Ts = match model.index.get(&TsKey::from(x1_id))? {
        CrdtNode::Con(c) => match &c.val {
            ConValue::Val(pv) => pack_to_ts(pv)?,
            _ => return None,
        },
        _ => return None,
    };

    // Element 2: x2 con (Integer(0) means same as x1).
    let x2_id: Ts = vec.elements.get(tuple_index::X2).copied().flatten()?;
    let x2_ts: Ts = match model.index.get(&TsKey::from(x2_id))? {
        CrdtNode::Con(c) => match &c.val {
            ConValue::Val(PackValue::Integer(0)) => x1_ts,
            ConValue::Val(pv) => pack_to_ts(pv)?,
            _ => return None,
        },
        _ => return None,
    };

    // Element 3: type con.
    let type_id: Ts = vec.elements.get(tuple_index::TYPE_).copied().flatten()?;
    let slice_type: SliceType = match model.index.get(&TsKey::from(type_id))? {
        CrdtNode::Con(c) => match &c.val {
            ConValue::Val(pv) => SliceType::from_pack(pv)?,
            _ => return None,
        },
        _ => return None,
    };

    // Element 4 (optional): data con.
    let data: Option<serde_json::Value> =
        if let Some(Some(data_id)) = vec.elements.get(tuple_index::DATA).copied() {
            match model.index.get(&TsKey::from(data_id)) {
                Some(CrdtNode::Con(c)) => match &c.val {
                    ConValue::Val(pv) => Some(serde_json::Value::from(pv.clone())),
                    _ => None,
                },
                _ => None,
            }
        } else {
            None
        };

    Some(Slice::new(
        vec_id,
        stacking,
        slice_type,
        Point::new(x1_ts, x1_anchor),
        Point::new(x2_ts, x2_anchor),
        data,
    ))
}
