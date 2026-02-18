//! Multi-value register extension (`mval`).
//!
//! Mirrors `packages/json-joy/src/json-crdt-extensions/mval/`.

use serde_json::Value;
use json_joy_json_pack::PackValue;

use crate::json_crdt::constants::ORIGIN;
use crate::json_crdt::model::Model;
use crate::json_crdt::nodes::{CrdtNode, IndexExt, TsKey};
use crate::json_crdt_patch::clock::Ts;
use crate::json_crdt_patch::operations::{ConValue, Op};

// ── MvalNode ──────────────────────────────────────────────────────────────

/// A multi-value register backed by an [`ArrNode`].
#[derive(Debug, Clone, Copy)]
pub struct MvalNode {
    pub arr_id: Ts,
}

impl MvalNode {
    pub fn new(arr_id: Ts) -> Self {
        Self { arr_id }
    }

    /// Return all live values as a `Vec<Value>`.
    pub fn view(&self, model: &Model) -> Vec<Value> {
        let Some(CrdtNode::Arr(arr)) = model.index.get(&TsKey::from(self.arr_id)) else {
            return Vec::new();
        };
        let data_ids: Vec<Ts> = arr
            .rga
            .iter_live()
            .filter_map(|chunk| chunk.data.as_ref())
            .flat_map(|ids| ids.iter().copied())
            .collect();

        data_ids
            .into_iter()
            .filter_map(|id| model.index.get(&TsKey::from(id)))
            .map(|node| node.view(&model.index))
            .collect()
    }

    /// Replace the current value with `value`.
    pub fn set(&self, model: &mut Model, value: Value) {
        let (size, spans) = {
            let Some(CrdtNode::Arr(arr)) = model.index.get(&TsKey::from(self.arr_id)) else {
                return;
            };
            let size = arr.size();
            let spans = if size > 0 { arr.find_interval(0, size) } else { Vec::new() };
            (size, spans)
        };

        if size > 0 {
            let del_id = model.next_ts();
            model.apply_operation(&Op::Del {
                id:   del_id,
                obj:  self.arr_id,
                what: spans,
            });
        }

        let con_id = model.next_ts();
        model.apply_operation(&Op::NewCon {
            id:  con_id,
            val: ConValue::Val(PackValue::from(value)),
        });

        let ins_id = model.next_ts();
        model.apply_operation(&Op::InsArr {
            id:    ins_id,
            obj:   self.arr_id,
            after: ORIGIN,
            data:  vec![con_id],
        });
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_crdt::model::Model;
    use crate::json_crdt_patch::clock::ts;
    use crate::json_crdt_patch::operations::{ConValue, Op};
    use json_joy_json_pack::PackValue;
    use serde_json::json;

    fn sid1() -> u64 { 100 }

    fn setup(sid: u64) -> (Model, MvalNode) {
        let mut model = Model::new(sid);
        let arr_id = ts(sid, 1);
        model.apply_operation(&Op::NewArr { id: arr_id });
        model.clock.observe(arr_id, 1);
        (model, MvalNode::new(arr_id))
    }

    #[test]
    fn set_then_view_returns_single_value() {
        let (mut model, mval) = setup(sid1());
        mval.set(&mut model, json!(42));
        assert_eq!(mval.view(&model), vec![json!(42)]);
    }

    #[test]
    fn set_twice_replaces_value() {
        let (mut model, mval) = setup(sid1());
        mval.set(&mut model, json!(1));
        mval.set(&mut model, json!(2));
        mval.set(&mut model, json!(3));
        assert_eq!(mval.view(&model), vec![json!(3)]);
    }

    #[test]
    fn size_stays_one_after_multiple_sets() {
        let (mut model, mval) = setup(sid1());
        mval.set(&mut model, json!(1));
        mval.set(&mut model, json!(2));
        mval.set(&mut model, json!(3));
        assert_eq!(mval.view(&model).len(), 1);
    }

    #[test]
    fn set_overwrites_after_concurrent_merge() {
        let (mut model, mval) = setup(sid1());
        mval.set(&mut model, json!(1));

        // Inject a second item to simulate a surviving concurrent write.
        let con_extra = model.next_ts();
        model.apply_operation(&Op::NewCon {
            id:  con_extra,
            val: ConValue::Val(PackValue::Integer(3)),
        });
        let ins_extra = model.next_ts();
        model.apply_operation(&Op::InsArr {
            id:    ins_extra,
            obj:   mval.arr_id,
            after: ORIGIN,
            data:  vec![con_extra],
        });
        assert_eq!(mval.view(&model).len(), 2);

        mval.set(&mut model, json!(4));
        assert_eq!(mval.view(&model), vec![json!(4)]);
    }
}
