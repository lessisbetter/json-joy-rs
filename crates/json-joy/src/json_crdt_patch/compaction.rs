//! Patch compaction utilities.
//!
//! Mirrors `packages/json-joy/src/json-crdt-patch/compaction.ts`.

use crate::json_crdt_patch::clock::{equal, ts};
use crate::json_crdt_patch::operations::Op;
use crate::json_crdt_patch::patch::Patch;

/// Combines two or more patches together.
///
/// The first patch is modified in place. Operations from subsequent patches
/// are appended without cloning. Between patches, a `Nop` is inserted if
/// there is a time gap.
///
/// All patches must share the same session ID. The patches must be ordered
/// by logical time with no overlapping spans.
pub fn combine(patches: &mut Vec<Patch>) {
    if patches.len() < 2 {
        return;
    }
    // Drain everything into a local Vec, then rebuild
    let all: Vec<Patch> = std::mem::take(patches);
    let mut iter = all.into_iter();
    let mut first = iter.next().unwrap();

    for current in iter {
        let first_id = first.get_id();
        let current_id = current.get_id();

        match (first_id, current_id) {
            (None, None) => continue,
            (None, Some(_)) => {
                first.ops.extend(current.ops);
                continue;
            }
            (Some(_), None) => continue,
            (Some(fid), Some(cid)) => {
                if fid.sid != cid.sid {
                    panic!("SID_MISMATCH");
                }
                let next_tick = fid.time + first.span();
                let time_b = cid.time;
                let time_diff = time_b as i64 - next_tick as i64;
                if time_diff < 0 {
                    panic!("TIMESTAMP_CONFLICT");
                }
                if time_diff > 0 {
                    first.ops.push(Op::Nop {
                        id: ts(fid.sid, next_tick),
                        len: time_diff as u64,
                    });
                }
                first.ops.extend(current.ops);
            }
        }
    }
    patches.push(first);
}

/// Compacts operations within a single patch by merging consecutive string
/// inserts (when they are into the same string and are consecutive appends).
///
/// Mutates the patch in place. Clone first if you need the original.
pub fn compact(patch: &mut Patch) {
    if patch.ops.len() < 2 {
        return;
    }
    let ops = std::mem::take(&mut patch.ops);
    let mut new_ops: Vec<Op> = Vec::with_capacity(ops.len());

    for op in ops {
        if let Some(last) = new_ops.last_mut() {
            if let (
                Op::InsStr {
                    id: lid,
                    obj: lobj,
                    after: lafter,
                    data: ldata,
                },
                Op::InsStr {
                    id: cid,
                    obj: cobj,
                    after: cafter,
                    data: cdata,
                },
            ) = (last, &op)
            {
                // Upstream `InsStrOp.span()` uses JS `string.length` (UTF-16 code units).
                let last_next_tick = lid.time + ldata.encode_utf16().count() as u64;
                let is_time_consecutive = last_next_tick == cid.time;
                let is_same_string = equal(*lobj, *cobj);
                // isAppend: the current op's `after` is the last character of the previous op
                let is_append = last_next_tick == cafter.time + 1 && lafter.sid == cafter.sid;
                if is_time_consecutive && is_same_string && is_append {
                    ldata.push_str(cdata);
                    continue;
                }
            }
        }
        new_ops.push(op);
    }
    patch.ops = new_ops;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_crdt_patch::clock::ts;

    #[test]
    fn combine_two_patches() {
        let mut p1 = Patch::new();
        p1.ops.push(Op::NewStr { id: ts(1, 0) });

        let mut p2 = Patch::new();
        p2.ops.push(Op::NewObj { id: ts(1, 1) });

        let mut patches = vec![p1, p2];
        combine(&mut patches);
        assert_eq!(patches.len(), 1);
        assert_eq!(patches[0].ops.len(), 2);
    }

    #[test]
    fn combine_with_gap_inserts_nop() {
        let mut p1 = Patch::new();
        p1.ops.push(Op::NewStr { id: ts(1, 0) });

        let mut p2 = Patch::new();
        p2.ops.push(Op::NewObj { id: ts(1, 5) }); // gap of 4

        let mut patches = vec![p1, p2];
        combine(&mut patches);
        // Should have: NewStr, Nop(4), NewObj
        assert_eq!(patches[0].ops.len(), 3);
        if let Op::Nop { len, .. } = &patches[0].ops[1] {
            assert_eq!(*len, 4);
        } else {
            panic!("expected Nop");
        }
    }

    #[test]
    fn compact_merges_consecutive_ins_str() {
        let mut patch = Patch::new();
        // Two consecutive InsStr ops appending to the same string
        // p[0]: InsStr at time=5, obj=str_id(1,0), after=str_id(1,0), "hel"
        // p[1]: InsStr at time=8, obj=str_id(1,0), after=(1,7), "lo"
        let str_id = ts(1, 0);
        patch.ops.push(Op::InsStr {
            id: ts(1, 5),
            obj: str_id,
            after: str_id,
            data: "hel".into(),
        });
        patch.ops.push(Op::InsStr {
            id: ts(1, 8), // time = 5 + 3 = 8
            obj: str_id,
            after: ts(1, 7), // after = (1, last_char_time) = (1, 7)
            data: "lo".into(),
        });
        compact(&mut patch);
        assert_eq!(patch.ops.len(), 1);
        if let Op::InsStr { data, .. } = &patch.ops[0] {
            assert_eq!(data, "hello");
        }
    }
}
