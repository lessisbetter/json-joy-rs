//! [`Patch`] — a JSON CRDT patch containing a sequence of operations.
//!
//! Mirrors `packages/json-joy/src/json-crdt-patch/Patch.ts`.

use crate::json_crdt_patch::clock::{ts, Ts};
use crate::json_crdt_patch::operations::{ConValue, Op};
use json_joy_json_pack::PackValue;

/// A JSON CRDT Patch: an ordered list of operations with optional metadata.
///
/// Normally created via [`PatchBuilder`](super::patch_builder::PatchBuilder).
#[derive(Debug, Clone, PartialEq)]
pub struct Patch {
    /// The list of operations in the patch.
    pub ops: Vec<Op>,

    /// Arbitrary metadata (not interpreted by the CRDT library).
    pub meta: Option<PackValue>,
}

impl Default for Patch {
    fn default() -> Self {
        Self::new()
    }
}

impl Patch {
    /// Creates an empty patch with no operations.
    pub fn new() -> Self {
        Self {
            ops: Vec::new(),
            meta: None,
        }
    }

    /// Returns the ID of the first operation, if any.
    pub fn get_id(&self) -> Option<Ts> {
        self.ops.first().map(|op| op.id())
    }

    /// Returns the total logical clock span consumed by all operations.
    pub fn span(&self) -> u64 {
        self.ops.iter().map(|op| op.span()).sum()
    }

    /// Returns the logical time expected for the next operation to be inserted.
    ///
    /// Returns 0 if the patch has no operations.
    pub fn next_time(&self) -> u64 {
        match self.ops.last() {
            None => 0,
            Some(op) => op.id().time + op.span(),
        }
    }

    /// Creates a new patch where every timestamp is transformed by `f`.
    pub fn rewrite_time<F>(&self, f: &F) -> Patch
    where
        F: Fn(Ts) -> Ts,
    {
        let mut new_ops = Vec::with_capacity(self.ops.len());
        for op in &self.ops {
            new_ops.push(rewrite_op(op, f));
        }
        Patch {
            ops: new_ops,
            meta: self.meta.clone(),
        }
    }

    /// Rebases the patch so that the first operation begins at `new_time`.
    ///
    /// Only timestamps belonging to the patch's session ID and at or after
    /// `transform_after` (defaults to the patch start time) are shifted.
    ///
    /// Returns `self` if no shift is needed.
    pub fn rebase(&self, new_time: u64, transform_after: Option<u64>) -> Patch {
        let id = self.get_id().expect("EMPTY_PATCH");
        let sid = id.sid;
        let patch_start_time = id.time;
        let transform_after = transform_after.unwrap_or(patch_start_time);
        if patch_start_time == new_time {
            return self.clone();
        }
        let delta = new_time as i64 - patch_start_time as i64;
        self.rewrite_time(&|id: Ts| -> Ts {
            if id.sid != sid {
                return id;
            }
            if id.time < transform_after {
                return id;
            }
            ts(sid, (id.time as i64 + delta) as u64)
        })
    }

    /// Deep-clones the patch.
    pub fn clone_patch(&self) -> Patch {
        self.rewrite_time(&|id| id)
    }

    /// Encodes the patch to binary (binary codec).
    pub fn to_binary(&self) -> Vec<u8> {
        crate::json_crdt_patch::codec::binary::encode(self)
    }

    /// Decodes a patch from binary (binary codec).
    pub fn from_binary(
        data: &[u8],
    ) -> Result<Patch, crate::json_crdt_patch::codec::binary::DecodeError> {
        crate::json_crdt_patch::codec::binary::decode(data)
    }
}

impl std::fmt::Display for Patch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let id_str = match self.get_id() {
            Some(id) => crate::json_crdt_patch::clock::print_ts(id),
            None => "(nil)".to_owned(),
        };
        write!(f, "Patch {}!{}", id_str, self.span())?;
        for op in &self.ops {
            write!(f, "\n  {}", op)?;
        }
        Ok(())
    }
}

/// Applies the timestamp transform function to a single operation.
fn rewrite_op<F>(op: &Op, f: &F) -> Op
where
    F: Fn(Ts) -> Ts,
{
    match op {
        Op::NewCon { id, val } => Op::NewCon {
            id: f(*id),
            val: match val {
                ConValue::Ref(ts) => ConValue::Ref(f(*ts)),
                ConValue::Val(v) => ConValue::Val(v.clone()),
            },
        },
        Op::NewVal { id } => Op::NewVal { id: f(*id) },
        Op::NewObj { id } => Op::NewObj { id: f(*id) },
        Op::NewVec { id } => Op::NewVec { id: f(*id) },
        Op::NewStr { id } => Op::NewStr { id: f(*id) },
        Op::NewBin { id } => Op::NewBin { id: f(*id) },
        Op::NewArr { id } => Op::NewArr { id: f(*id) },
        Op::InsVal { id, obj, val } => Op::InsVal {
            id: f(*id),
            obj: f(*obj),
            val: f(*val),
        },
        Op::InsObj { id, obj, data } => Op::InsObj {
            id: f(*id),
            obj: f(*obj),
            data: data.iter().map(|(k, v)| (k.clone(), f(*v))).collect(),
        },
        Op::InsVec { id, obj, data } => Op::InsVec {
            id: f(*id),
            obj: f(*obj),
            data: data.iter().map(|(k, v)| (*k, f(*v))).collect(),
        },
        Op::InsStr {
            id,
            obj,
            after,
            data,
        } => Op::InsStr {
            id: f(*id),
            obj: f(*obj),
            after: f(*after),
            data: data.clone(),
        },
        Op::InsBin {
            id,
            obj,
            after,
            data,
        } => Op::InsBin {
            id: f(*id),
            obj: f(*obj),
            after: f(*after),
            data: data.clone(),
        },
        Op::InsArr {
            id,
            obj,
            after,
            data,
        } => Op::InsArr {
            id: f(*id),
            obj: f(*obj),
            after: f(*after),
            data: data.iter().map(|v| f(*v)).collect(),
        },
        Op::UpdArr {
            id,
            obj,
            after,
            val,
        } => Op::UpdArr {
            id: f(*id),
            obj: f(*obj),
            after: f(*after),
            val: f(*val),
        },
        Op::Del { id, obj, what } => Op::Del {
            id: f(*id),
            obj: f(*obj),
            what: what
                .iter()
                .map(|s| {
                    let new_ts = f(s.ts());
                    crate::json_crdt_patch::clock::Tss::new(new_ts.sid, new_ts.time, s.span)
                })
                .collect(),
        },
        Op::Nop { id, len } => Op::Nop {
            id: f(*id),
            len: *len,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_crdt_patch::clock::ts;
    use crate::json_crdt_patch::operations::ConValue;

    #[test]
    fn empty_patch() {
        let p = Patch::new();
        assert_eq!(p.get_id(), None);
        assert_eq!(p.span(), 0);
        assert_eq!(p.next_time(), 0);
    }

    #[test]
    fn patch_with_single_op() {
        let mut p = Patch::new();
        p.ops.push(Op::NewObj { id: ts(1, 100) });
        assert_eq!(p.get_id(), Some(ts(1, 100)));
        assert_eq!(p.span(), 1);
        assert_eq!(p.next_time(), 101);
    }

    #[test]
    fn patch_rebase() {
        let mut p = Patch::new();
        p.ops.push(Op::NewStr { id: ts(1, 10) });
        p.ops.push(Op::InsStr {
            id: ts(1, 11),
            obj: ts(1, 10),
            after: ts(1, 10),
            data: "hi".into(),
        });
        let rebased = p.rebase(20, None);
        assert_eq!(rebased.get_id(), Some(ts(1, 20)));
        assert_eq!(rebased.ops[1].id(), ts(1, 21));
    }

    #[test]
    fn patch_rewrite_time_leaves_foreign_sid_alone() {
        let mut p = Patch::new();
        p.ops.push(Op::InsVal {
            id: ts(1, 5),
            obj: ts(2, 100),
            val: ts(1, 5),
        });
        let rebased = p.rebase(10, None);
        // obj belongs to sid=2, should be untouched
        if let Op::InsVal { obj, .. } = &rebased.ops[0] {
            assert_eq!(*obj, ts(2, 100));
        }
    }
}
