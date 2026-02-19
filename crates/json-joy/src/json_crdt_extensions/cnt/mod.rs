//! Counter extension (`cnt`).
//!
//! Mirrors `packages/json-joy/src/json-crdt-extensions/cnt/`.

use json_joy_json_pack::PackValue;

use crate::json_crdt::model::Model;
use crate::json_crdt::nodes::{CrdtNode, IndexExt, TsKey};
use crate::json_crdt_patch::clock::Ts;
use crate::json_crdt_patch::operations::{ConValue, Op};

// ── CntNode ───────────────────────────────────────────────────────────────

/// A distributed counter backed by an [`ObjNode`].
#[derive(Debug, Clone, Copy)]
pub struct CntNode {
    pub obj_id: Ts,
}

impl CntNode {
    pub fn new(obj_id: Ts) -> Self {
        Self { obj_id }
    }

    /// Sum of all peer contributions.
    pub fn view(&self, model: &Model) -> i64 {
        let Some(CrdtNode::Obj(obj)) = model.index.get(&TsKey::from(self.obj_id)) else {
            return 0;
        };
        let val_ids: Vec<Ts> = obj.keys.values().copied().collect();
        val_ids
            .into_iter()
            .filter_map(|id| model.index.get(&TsKey::from(id)))
            .filter_map(|node| match node {
                CrdtNode::Con(con) => match &con.val {
                    ConValue::Val(PackValue::Integer(n)) => Some(*n),
                    ConValue::Val(PackValue::UInteger(n)) => Some(*n as i64),
                    _ => None,
                },
                _ => None,
            })
            .sum()
    }

    /// Add `increment` to this peer's contribution.
    ///
    /// Uses `model.clock.sid` converted to base-36 as the contribution key.
    pub fn inc(&self, model: &mut Model, increment: i64) {
        let sid = model.clock.sid;
        let key = to_base36(sid);

        let current: i64 = {
            let Some(CrdtNode::Obj(obj)) = model.index.get(&TsKey::from(self.obj_id)) else {
                return;
            };
            if let Some(&val_id) = obj.keys.get(&key) {
                match model.index.get(&TsKey::from(val_id)) {
                    Some(CrdtNode::Con(con)) => match &con.val {
                        ConValue::Val(PackValue::Integer(n)) => *n,
                        ConValue::Val(PackValue::UInteger(n)) => *n as i64,
                        _ => 0,
                    },
                    _ => 0,
                }
            } else {
                0
            }
        };

        let new_val = current + increment;

        let con_id = model.next_ts();
        model.apply_operation(&Op::NewCon {
            id: con_id,
            val: ConValue::Val(PackValue::Integer(new_val)),
        });

        let ins_id = model.next_ts();
        model.apply_operation(&Op::InsObj {
            id: ins_id,
            obj: self.obj_id,
            data: vec![(key, con_id)],
        });
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────

fn to_base36(mut n: u64) -> String {
    if n == 0 {
        return "0".to_string();
    }
    const DIGITS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut buf = Vec::with_capacity(13);
    while n > 0 {
        buf.push(DIGITS[(n % 36) as usize]);
        n /= 36;
    }
    buf.reverse();
    String::from_utf8(buf).expect("base-36 digits are valid UTF-8")
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_crdt::model::Model;
    use crate::json_crdt_patch::clock::ts;
    use crate::json_crdt_patch::operations::Op;

    fn sid1() -> u64 {
        100
    }
    fn sid2() -> u64 {
        200
    }

    fn setup(sid: u64) -> (Model, CntNode) {
        let mut model = Model::new(sid);
        let obj_id = ts(sid, 1);
        model.apply_operation(&Op::NewObj { id: obj_id });
        model.clock.observe(obj_id, 1);
        (model, CntNode::new(obj_id))
    }

    #[test]
    fn view_of_empty_counter_is_zero() {
        let (model, cnt) = setup(sid1());
        assert_eq!(cnt.view(&model), 0);
    }

    #[test]
    fn single_increment() {
        let (mut model, cnt) = setup(sid1());
        cnt.inc(&mut model, 5);
        assert_eq!(cnt.view(&model), 5);
    }

    #[test]
    fn multiple_increments_accumulate() {
        let (mut model, cnt) = setup(sid1());
        cnt.inc(&mut model, 2);
        cnt.inc(&mut model, 3);
        cnt.inc(&mut model, -1);
        assert_eq!(cnt.view(&model), 4);
    }

    #[test]
    fn two_peers_increment_and_sum() {
        // Peer 1 sets key "sid1" = 2; Peer 2 sets key "sid2" = 3.
        // We merge them into one ObjNode manually.
        let obj_id = ts(sid1(), 1);

        let mut m1 = Model::new(sid1());
        m1.apply_operation(&Op::NewObj { id: obj_id });
        m1.clock.observe(obj_id, 1);
        let cnt1 = CntNode::new(obj_id);
        cnt1.inc(&mut m1, 2); // key = to_base36(sid1()), val = 2

        let mut m2 = Model::new(sid2());
        m2.apply_operation(&Op::NewObj { id: obj_id });
        m2.clock.observe(obj_id, 1);
        let cnt2 = CntNode::new(obj_id);
        cnt2.inc(&mut m2, 3); // key = to_base36(sid2()), val = 3

        // Apply peer 2's contribution into peer 1's model.
        let sid2_key = to_base36(sid2());
        if let Some(CrdtNode::Obj(obj2)) = m2.index.get(&TsKey::from(obj_id)) {
            if let Some(&con_id2) = obj2.keys.get(&sid2_key) {
                if let Some(node2) = m2.index.get(&TsKey::from(con_id2)) {
                    let cloned = node2.clone();
                    m1.index.insert_node(con_id2, cloned);
                    let ins_id = m1.next_ts();
                    m1.apply_operation(&Op::InsObj {
                        id: ins_id,
                        obj: obj_id,
                        data: vec![(sid2_key, con_id2)],
                    });
                }
            }
        }

        assert_eq!(cnt1.view(&m1), 5);
    }

    #[test]
    fn to_base36_examples() {
        assert_eq!(to_base36(0), "0");
        assert_eq!(to_base36(10), "a");
        assert_eq!(to_base36(35), "z");
        assert_eq!(to_base36(36), "10");
        assert_eq!(to_base36(100), "2s");
    }
}
