//! Operational transformation for JSON Patch operations.
//!
//! Mirrors `packages/json-joy/src/json-patch-ot/`.
//!
//! Provides `transform(accepted, proposed)` which transforms a list of
//! *proposed* operations so they can be applied after the *accepted*
//! operations have already been applied.

use crate::json_patch::types::{Op, Path};

// ── Path utilities ────────────────────────────────────────────────────────

fn is_root(path: &[String]) -> bool {
    path.is_empty()
}

/// Returns true if `s` is a valid numeric array index or "-".
fn is_valid_index(s: &str) -> bool {
    s == "-" || s.parse::<usize>().is_ok()
}

/// Returns true if `child` is strictly below `parent` (starts with parent, longer).
fn is_child(parent: &[String], child: &[String]) -> bool {
    child.len() > parent.len() && child.starts_with(parent)
}

/// Returns true if two paths are element-wise equal.
fn path_equal(a: &[String], b: &[String]) -> bool {
    a == b
}

/// Increment the array index in `path2` at the same depth as the last
/// component of `path1`, if `path2` is in the same array and its index
/// is >= `path1`'s index.
fn bump_array_path(path1: &[String], path2: &[String]) -> Option<Vec<String>> {
    let last_idx = path1.len().checked_sub(1)?;
    let step1 = path1.last()?;
    let index1: usize = step1.parse().ok()?;

    // path2 must share the same parent prefix
    if path2.len() <= last_idx {
        return None;
    }
    if path1[..last_idx] != path2[..last_idx] {
        return None;
    }

    let step2 = &path2[last_idx];
    if !is_valid_index(step2) {
        return None;
    }
    let index2: usize = step2.parse().ok()?;

    if index1 <= index2 {
        let mut new_path = path2.to_vec();
        new_path[last_idx] = (index2 + 1).to_string();
        Some(new_path)
    } else {
        None
    }
}

/// Decrement the array index in `path2` at the same depth as the last
/// component of `path1`, if `path2` is in the same array and its index
/// is > `path1`'s index.
fn lower_array_path(path1: &[String], path2: &[String]) -> Option<Vec<String>> {
    let last_idx = path1.len().checked_sub(1)?;
    let step1 = path1.last()?;
    let index1: usize = step1.parse().ok()?;

    if path2.len() <= last_idx {
        return None;
    }
    if path1[..last_idx] != path2[..last_idx] {
        return None;
    }

    let step2 = &path2[last_idx];
    if !is_valid_index(step2) {
        return None;
    }
    let index2: usize = step2.parse().ok()?;

    if index1 < index2 {
        let mut new_path = path2.to_vec();
        new_path[last_idx] = (index2 - 1).to_string();
        Some(new_path)
    } else {
        None
    }
}

// ── Op helpers ────────────────────────────────────────────────────────────

/// Return the effective delete length for a `StrDel` operation.
fn str_del_len(str_val: &Option<String>, len: &Option<usize>) -> usize {
    if let Some(s) = str_val {
        s.chars().count()
    } else {
        len.unwrap_or(0)
    }
}

/// Retrieve the `from` path from ops that have one (Move, Copy).
fn op_from(op: &Op) -> Option<&Path> {
    match op {
        Op::Move { from, .. } | Op::Copy { from, .. } => Some(from),
        _ => None,
    }
}

/// Rebuild the op with a different `path`, keeping all other fields intact.
fn with_path(op: &Op, new_path: Path) -> Op {
    match op.clone() {
        Op::Add { value, .. } => Op::Add {
            path: new_path,
            value,
        },
        Op::Remove { old_value, .. } => Op::Remove {
            path: new_path,
            old_value,
        },
        Op::Replace {
            value, old_value, ..
        } => Op::Replace {
            path: new_path,
            value,
            old_value,
        },
        Op::Copy { from, .. } => Op::Copy {
            path: new_path,
            from,
        },
        Op::Move { from, .. } => Op::Move {
            path: new_path,
            from,
        },
        Op::Test { value, not, .. } => Op::Test {
            path: new_path,
            value,
            not,
        },
        Op::StrIns { pos, str_val, .. } => Op::StrIns {
            path: new_path,
            pos,
            str_val,
        },
        Op::StrDel {
            pos, str_val, len, ..
        } => Op::StrDel {
            path: new_path,
            pos,
            str_val,
            len,
        },
        Op::Flip { .. } => Op::Flip { path: new_path },
        Op::Inc { inc, .. } => Op::Inc {
            path: new_path,
            inc,
        },
        Op::Split { pos, props, .. } => Op::Split {
            path: new_path,
            pos,
            props,
        },
        Op::Merge { pos, props, .. } => Op::Merge {
            path: new_path,
            pos,
            props,
        },
        Op::Extend {
            props, delete_null, ..
        } => Op::Extend {
            path: new_path,
            props,
            delete_null,
        },
        other => other,
    }
}

/// Rebuild the op with a different `from`, keeping all other fields intact.
/// Only meaningful for Move and Copy.
fn with_from(op: &Op, new_from: Path) -> Op {
    match op.clone() {
        Op::Copy { path, .. } => Op::Copy {
            path,
            from: new_from,
        },
        Op::Move { path, .. } => Op::Move {
            path,
            from: new_from,
        },
        other => other,
    }
}

// ── Individual transforms ─────────────────────────────────────────────────

/// Transform `op` against an accepted `add` operation.
fn x_add(add_path: &Path, op: &Op) -> Vec<Op> {
    if is_root(add_path) {
        return vec![];
    }
    if is_root(&op.path()) {
        return vec![op.clone()];
    }

    let last_step = match add_path.last() {
        Some(s) => s,
        None => return vec![op.clone()],
    };
    let last_is_index = is_valid_index(last_step);

    // If added a non-array value that op targets inside — op is invalidated
    if is_child(add_path, &op.path()) && !last_is_index {
        return vec![];
    }

    if last_is_index {
        let new_path = bump_array_path(add_path, &op.path());
        let new_from = op_from(op).and_then(|f| bump_array_path(add_path, f));
        if new_path.is_some() || new_from.is_some() {
            let mut result = op.clone();
            if let Some(p) = new_path {
                result = with_path(&result, p);
            }
            if let Some(f) = new_from {
                result = with_from(&result, f);
            }
            return vec![result];
        }
    }

    vec![op.clone()]
}

/// Transform `op` against an accepted `remove` operation.
fn x_remove(rem_path: &Path, op: &Op) -> Vec<Op> {
    if is_root(rem_path) {
        return vec![];
    }
    if is_root(&op.path()) {
        return vec![op.clone()];
    }

    let last_step = match rem_path.last() {
        Some(s) => s,
        None => return vec![op.clone()],
    };
    let last_is_index = is_valid_index(last_step);

    // Concurrent remove at the same numeric index: discard op
    if matches!(op, Op::Remove { .. }) && path_equal(rem_path, &op.path()) && last_is_index {
        return vec![];
    }

    // Op targets a descendant of what was removed — discard it.
    // Also applies to the `from` path for Move/Copy.
    if is_child(rem_path, &op.path()) {
        return vec![];
    }
    if let Some(from) = op_from(op) {
        if is_child(rem_path, from) {
            return vec![];
        }
    }

    if last_is_index {
        let new_path = lower_array_path(rem_path, &op.path());
        let new_from = op_from(op).and_then(|f| lower_array_path(rem_path, f));
        if new_path.is_some() || new_from.is_some() {
            let mut result = op.clone();
            if let Some(p) = new_path {
                result = with_path(&result, p);
            }
            if let Some(f) = new_from {
                result = with_from(&result, f);
            }
            return vec![result];
        }
    }

    vec![op.clone()]
}

/// Transform `op` against an accepted `move` operation.
fn x_move(move_from: &Path, move_to: &Path, op: &Op) -> Vec<Op> {
    if is_root(move_to) {
        return vec![op.clone()];
    }

    if is_child(move_from, &op.path()) {
        // op targets something inside what was moved — update its path.
        // NOTE: The upstream TypeScript erroneously slices at move.path.length
        // instead of move.from.length. We use move_from.len() which is correct:
        // the sub-path within the moved subtree starts at move_from's depth.
        let mut new_path = move_to.to_vec();
        new_path.extend_from_slice(&op.path()[move_from.len()..]);
        return vec![with_path(op, new_path)];
    }

    vec![op.clone()]
}

/// Transform `op` against an accepted `str_ins` operation.
fn x_str_ins(ins_path: &Path, ins_pos: usize, ins_len: usize, op: &Op) -> Vec<Op> {
    match op {
        Op::StrIns { path, pos, str_val } => {
            if !path_equal(ins_path, path) {
                return vec![op.clone()];
            }
            if ins_pos > *pos {
                return vec![op.clone()];
            }
            // Insertion shifted this op's position right
            vec![Op::StrIns {
                path: path.clone(),
                pos: pos + ins_len,
                str_val: str_val.clone(),
            }]
        }
        Op::StrDel {
            path,
            pos,
            str_val,
            len,
        } => {
            if !path_equal(ins_path, path) {
                return vec![op.clone()];
            }
            let del_len = str_del_len(str_val, len);

            if *pos < ins_pos {
                // Deletion starts before insertion
                if pos + del_len > ins_pos {
                    // Deletion spans the insertion point — split into two
                    let before_len = ins_pos - pos;
                    let after_pos = ins_pos + ins_len;
                    let (del1, del2) = if let Some(s) = str_val {
                        let chars: Vec<char> = s.chars().collect();
                        let s1: String = chars[..before_len].iter().collect();
                        let s2: String = chars[before_len..].iter().collect();
                        (
                            Op::StrDel {
                                path: path.clone(),
                                pos: *pos,
                                str_val: Some(s1),
                                len: None,
                            },
                            Op::StrDel {
                                path: path.clone(),
                                pos: after_pos,
                                str_val: Some(s2),
                                len: None,
                            },
                        )
                    } else {
                        (
                            Op::StrDel {
                                path: path.clone(),
                                pos: *pos,
                                str_val: None,
                                len: Some(before_len),
                            },
                            Op::StrDel {
                                path: path.clone(),
                                pos: after_pos,
                                str_val: None,
                                len: Some(del_len - before_len),
                            },
                        )
                    };
                    // Return second part first (higher pos), then first part
                    return vec![del2, del1];
                }
                // Deletion ends before insertion — no change
                return vec![op.clone()];
            }

            // ins_pos <= pos — insertion shifts deletion right
            if ins_pos <= *pos {
                return vec![Op::StrDel {
                    path: path.clone(),
                    pos: pos + ins_len,
                    str_val: str_val.clone(),
                    len: *len,
                }];
            }

            vec![op.clone()]
        }
        _ => vec![op.clone()],
    }
}

/// Transform `op` against an accepted `str_del` operation.
fn x_str_del(del_path: &Path, del_pos: usize, del_len: usize, op: &Op) -> Vec<Op> {
    match op {
        Op::StrIns { path, pos, str_val } => {
            if !path_equal(del_path, path) {
                return vec![op.clone()];
            }
            if *pos > del_pos {
                // Insertion was after deletion start — shift left.
                // If the deletion range covers the insertion point, clamp to del_pos
                // (the insertion's chars were deleted, so it lands right at the deletion start).
                let new_pos = if *pos >= del_pos + del_len {
                    pos - del_len
                } else {
                    del_pos
                };
                return vec![Op::StrIns {
                    path: path.clone(),
                    pos: new_pos,
                    str_val: str_val.clone(),
                }];
            }
            vec![op.clone()]
        }
        Op::StrDel {
            path,
            pos,
            str_val,
            len,
        } => {
            if !path_equal(del_path, path) {
                return vec![op.clone()];
            }
            let op_len = str_del_len(str_val, len);

            // How much of del overlaps from left side (del_pos <= pos)
            let overlap1 = (del_pos + del_len).saturating_sub(*pos) as i64;
            // How much of del overlaps from right side (del_pos >= pos)
            let overlap2 = (*pos + op_len).saturating_sub(del_pos) as i64;

            if del_pos <= *pos && overlap1 > 0 {
                // del starts at or before op, overlapping from the left.
                // new_pos = op.pos - (del_len - overlap1), which simplifies to del_pos.
                let new_len = (op_len as i64 - overlap1).max(0) as usize;
                if new_len == 0 {
                    return vec![];
                }
                let new_pos = del_pos; // = pos - (del_len - overlap1) = del_pos
                let new_op = if let Some(s) = str_val {
                    let chars: Vec<char> = s.chars().collect();
                    let skipped = overlap1 as usize;
                    Op::StrDel {
                        path: path.clone(),
                        pos: new_pos,
                        str_val: Some(chars[skipped..].iter().collect()),
                        len: None,
                    }
                } else {
                    Op::StrDel {
                        path: path.clone(),
                        pos: new_pos,
                        str_val: None,
                        len: Some(new_len),
                    }
                };
                return vec![new_op];
            } else if del_pos >= *pos && overlap2 > 0 {
                // del starts at or after op start, overlapping from the right.
                // Surviving length = part before del + any tail of op beyond del's end.
                let before_del = del_pos - *pos; // del_pos >= pos guaranteed
                let after_del = (overlap2 as usize).saturating_sub(del_len);
                let new_len = before_del + after_del;
                if new_len == 0 {
                    return vec![];
                }
                let new_op = if let Some(s) = str_val {
                    let chars: Vec<char> = s.chars().collect();
                    Op::StrDel {
                        path: path.clone(),
                        pos: *pos,
                        str_val: Some(chars[..new_len].iter().collect()),
                        len: None,
                    }
                } else {
                    Op::StrDel {
                        path: path.clone(),
                        pos: *pos,
                        str_val: None,
                        len: Some(new_len),
                    }
                };
                return vec![new_op];
            } else if del_pos < *pos {
                // del is completely before op — shift op left
                let new_op = Op::StrDel {
                    path: path.clone(),
                    pos: pos - del_len,
                    str_val: str_val.clone(),
                    len: *len,
                };
                return vec![new_op];
            }

            vec![op.clone()]
        }
        _ => vec![op.clone()],
    }
}

// ── Main transform ────────────────────────────────────────────────────────

/// Transform `proposed` operations so they apply correctly after `accepted`
/// operations have already been applied.
pub fn transform(accepted: &[Op], proposed: &[Op]) -> Vec<Op> {
    let mut proposed = proposed.to_vec();

    for acc in accepted {
        let mut next: Vec<Op> = Vec::new();
        for prop in &proposed {
            let results = apply_xform(acc, prop);
            next.extend(results);
        }
        proposed = next;
    }

    proposed
}

/// Apply the appropriate transform function for the accepted operation.
fn apply_xform(accepted: &Op, proposed: &Op) -> Vec<Op> {
    match accepted {
        Op::Add { path, .. } => x_add(path, proposed),
        Op::Remove { path, .. } => x_remove(path, proposed),
        Op::Move { path, from } => x_move(from, path, proposed),
        Op::StrIns { path, pos, str_val } => {
            x_str_ins(path, *pos, str_val.chars().count(), proposed)
        }
        Op::StrDel {
            path,
            pos,
            str_val,
            len,
        } => x_str_del(path, *pos, str_del_len(str_val, len), proposed),
        // Other operations don't have a defined transform — pass through unchanged
        _ => vec![proposed.clone()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn transform_empty() {
        let result = transform(&[], &[]);
        assert!(result.is_empty());
    }

    #[test]
    fn x_add_bumps_array_index() {
        // Accepted: add at [arr, 1]. Proposed: remove at [arr, 2].
        // After accepted, the element at index 2 is now at index 3.
        let accepted = Op::Add {
            path: vec!["arr".to_string(), "1".to_string()],
            value: json!(99),
        };
        let proposed = Op::Remove {
            path: vec!["arr".to_string(), "2".to_string()],
            old_value: None,
        };
        let result = transform(&[accepted], &[proposed]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path().as_slice(), ["arr", "3"]);
    }

    #[test]
    fn x_remove_lowers_array_index() {
        // Accepted: remove at [arr, 1]. Proposed: remove at [arr, 3].
        // After accepted, the element at index 3 is now at index 2.
        let accepted = Op::Remove {
            path: vec!["arr".to_string(), "1".to_string()],
            old_value: None,
        };
        let proposed = Op::Remove {
            path: vec!["arr".to_string(), "3".to_string()],
            old_value: None,
        };
        let result = transform(&[accepted], &[proposed]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path().as_slice(), ["arr", "2"]);
    }

    #[test]
    fn x_remove_concurrent_at_same_index() {
        // Both remove the same element — proposed should be discarded
        let accepted = Op::Remove {
            path: vec!["arr".to_string(), "2".to_string()],
            old_value: None,
        };
        let proposed = Op::Remove {
            path: vec!["arr".to_string(), "2".to_string()],
            old_value: None,
        };
        let result = transform(&[accepted], &[proposed]);
        assert!(result.is_empty());
    }

    #[test]
    fn x_str_ins_shifts_later_ins_right() {
        let path = vec!["text".to_string()];
        let accepted = Op::StrIns {
            path: path.clone(),
            pos: 2,
            str_val: "XY".to_string(),
        };
        let proposed = Op::StrIns {
            path: path.clone(),
            pos: 5,
            str_val: "Z".to_string(),
        };
        let result = transform(&[accepted], &[proposed]);
        assert_eq!(result.len(), 1);
        if let Op::StrIns { pos, .. } = &result[0] {
            assert_eq!(*pos, 7); // 5 + 2
        }
    }

    #[test]
    fn x_str_del_shifts_later_ins_left() {
        let path = vec!["text".to_string()];
        let accepted = Op::StrDel {
            path: path.clone(),
            pos: 2,
            str_val: None,
            len: Some(3),
        };
        let proposed = Op::StrIns {
            path: path.clone(),
            pos: 8,
            str_val: "Z".to_string(),
        };
        let result = transform(&[accepted], &[proposed]);
        assert_eq!(result.len(), 1);
        if let Op::StrIns { pos, .. } = &result[0] {
            assert_eq!(*pos, 5); // 8 - 3
        }
    }

    // ── Comprehensive OT scenarios ─────────────────────────────────────────

    #[test]
    fn x_remove_discards_child_op() {
        // Accepted removes /a; proposed tries to replace /a/b.
        // Since /a no longer exists, the proposed op must be discarded.
        let accepted = Op::Remove {
            path: vec!["a".to_string()],
            old_value: None,
        };
        let proposed = Op::Replace {
            path: vec!["a".to_string(), "b".to_string()],
            value: json!(99),
            old_value: None,
        };
        let result = transform(&[accepted], &[proposed]);
        assert!(
            result.is_empty(),
            "proposed targeting removed subtree must be discarded"
        );
    }

    #[test]
    fn x_remove_discards_grandchild_op() {
        // Accepted removes /a; proposed targets /a/b/c (deeply nested).
        let accepted = Op::Remove {
            path: vec!["a".to_string()],
            old_value: None,
        };
        let proposed = Op::Add {
            path: vec!["a".to_string(), "b".to_string(), "c".to_string()],
            value: json!(1),
        };
        let result = transform(&[accepted], &[proposed]);
        assert!(result.is_empty());
    }

    #[test]
    fn x_remove_preserves_sibling_op() {
        // Accepted removes /a/1; proposed replaces /a/3 (later sibling).
        let accepted = Op::Remove {
            path: vec!["a".to_string(), "1".to_string()],
            old_value: None,
        };
        let proposed = Op::Replace {
            path: vec!["a".to_string(), "3".to_string()],
            value: json!("new"),
            old_value: None,
        };
        let result = transform(&[accepted], &[proposed]);
        assert_eq!(result.len(), 1);
        // Index 3 is now index 2 after removing index 1.
        assert_eq!(result[0].path().as_slice(), ["a", "2"]);
    }

    #[test]
    fn x_add_at_root_discards_proposed() {
        // Accepted add at root (/) replaces the whole document.
        // Proposed is invalidated since the entire document was replaced.
        let accepted = Op::Add {
            path: vec![],
            value: json!({"x": 1}),
        };
        let proposed = Op::Replace {
            path: vec!["foo".to_string()],
            value: json!(99),
            old_value: None,
        };
        let result = transform(&[accepted], &[proposed]);
        assert!(
            result.is_empty(),
            "proposed after root-replace must be discarded"
        );
    }

    #[test]
    fn x_add_at_object_key_preserves_unrelated_op() {
        // Accepted adds /foo (a non-array key); proposed modifies /bar — unrelated.
        let accepted = Op::Add {
            path: vec!["foo".to_string()],
            value: json!(1),
        };
        let proposed = Op::Replace {
            path: vec!["bar".to_string()],
            value: json!(2),
            old_value: None,
        };
        let result = transform(&[accepted], &[proposed]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path().as_slice(), ["bar"]);
    }

    #[test]
    fn x_add_same_array_shifts_proposed_add() {
        // Accepted adds at /arr/0; proposed adds at /arr/0 too.
        // After accepted, the proposed element should be pushed to /arr/1.
        let accepted = Op::Add {
            path: vec!["arr".to_string(), "0".to_string()],
            value: json!("A"),
        };
        let proposed = Op::Add {
            path: vec!["arr".to_string(), "0".to_string()],
            value: json!("B"),
        };
        let result = transform(&[accepted], &[proposed]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path().as_slice(), ["arr", "1"]);
    }

    #[test]
    fn x_move_child_path_redirected_to_destination() {
        // Accepted moves /src → /dst; proposed removes /src/child.
        // After the move, /src/child is now at /dst/child — the proposed op
        // should be redirected to the new location.
        let accepted = Op::Move {
            path: vec!["dst".to_string()],
            from: vec!["src".to_string()],
        };
        let proposed = Op::Remove {
            path: vec!["src".to_string(), "child".to_string()],
            old_value: None,
        };
        let result = transform(&[accepted], &[proposed]);
        assert_eq!(result.len(), 1);
        // /src/child should be redirected to /dst/child.
        assert_eq!(result[0].path().as_slice(), ["dst", "child"]);
    }

    #[test]
    fn x_str_ins_does_not_shift_earlier_pos() {
        // Accepted inserts at pos 5; proposed inserts at pos 3 (before the insert).
        // Earlier position is unaffected.
        let path = vec!["text".to_string()];
        let accepted = Op::StrIns {
            path: path.clone(),
            pos: 5,
            str_val: "XY".to_string(),
        };
        let proposed = Op::StrIns {
            path: path.clone(),
            pos: 3,
            str_val: "Z".to_string(),
        };
        let result = transform(&[accepted], &[proposed]);
        assert_eq!(result.len(), 1);
        if let Op::StrIns { pos, .. } = &result[0] {
            assert_eq!(*pos, 3, "pos before insertion point must not change");
        }
    }

    #[test]
    fn x_str_ins_concurrent_at_same_pos_shifts_right() {
        // Both insert at pos 5. The proposed one should be shifted to pos 6
        // (or greater) to preserve ordering.
        let path = vec!["text".to_string()];
        let accepted = Op::StrIns {
            path: path.clone(),
            pos: 5,
            str_val: "X".to_string(),
        };
        let proposed = Op::StrIns {
            path: path.clone(),
            pos: 5,
            str_val: "Y".to_string(),
        };
        let result = transform(&[accepted], &[proposed]);
        assert_eq!(result.len(), 1);
        if let Op::StrIns { pos, .. } = &result[0] {
            assert!(
                *pos >= 6,
                "concurrent insert at same pos must shift: got {pos}"
            );
        }
    }

    #[test]
    fn x_str_del_removes_overlapping_del_prefix() {
        // Accepted: del pos=2, len=3 (removes chars 2..5).
        // Proposed: del pos=0, len=4 (overlaps: chars 0..4).
        // After accepted, chars 2..5 are gone. Proposed should only remove 0..2 (2 chars).
        let path = vec!["text".to_string()];
        let accepted = Op::StrDel {
            path: path.clone(),
            pos: 2,
            str_val: None,
            len: Some(3),
        };
        let proposed = Op::StrDel {
            path: path.clone(),
            pos: 0,
            str_val: None,
            len: Some(4),
        };
        let result = transform(&[accepted], &[proposed]);
        assert_eq!(result.len(), 1, "should produce exactly one adjusted op");
        if let Op::StrDel { pos, len, .. } = &result[0] {
            // Accepted removed 3 chars from [2..5]. Proposed wanted [0..4].
            // The surviving chars in proposed are [0..2] = 2 chars.
            assert_eq!(*pos, 0);
            assert_eq!(len.unwrap_or(0), 2);
        }
    }

    #[test]
    fn x_str_del_completely_consumed_by_accepted() {
        // Accepted: del pos=0, len=10.
        // Proposed: del pos=2, len=3 — fully within accepted's range.
        // After accepted, those chars are already gone → proposed should be discarded.
        let path = vec!["text".to_string()];
        let accepted = Op::StrDel {
            path: path.clone(),
            pos: 0,
            str_val: None,
            len: Some(10),
        };
        let proposed = Op::StrDel {
            path: path.clone(),
            pos: 2,
            str_val: None,
            len: Some(3),
        };
        let result = transform(&[accepted], &[proposed]);
        assert!(result.is_empty(), "fully consumed del must be discarded");
    }

    #[test]
    fn test_op_passes_through_unchanged() {
        // Test ops are not transformed — they always pass through.
        let accepted = Op::Remove {
            path: vec!["a".to_string(), "0".to_string()],
            old_value: None,
        };
        let proposed = Op::Test {
            path: vec!["b".to_string()],
            value: json!(42),
            not: false,
        };
        let result = transform(&[accepted], &[proposed]);
        assert_eq!(result.len(), 1);
        assert!(matches!(result[0], Op::Test { .. }));
    }

    #[test]
    fn inc_passes_through_unchanged() {
        // Inc ops are not structurally transformed.
        let accepted = Op::Add {
            path: vec!["x".to_string()],
            value: json!(1),
        };
        let proposed = Op::Inc {
            path: vec!["y".to_string()],
            inc: 5.0,
        };
        let result = transform(&[accepted], &[proposed]);
        assert_eq!(result.len(), 1);
        if let Op::Inc { inc, .. } = &result[0] {
            assert_eq!(*inc, 5.0);
        }
    }

    #[test]
    fn multi_op_chain_all_updated() {
        // Accepted adds two array items; proposed has three ops — all should be shifted.
        let accepted = vec![
            Op::Add {
                path: vec!["a".to_string(), "0".to_string()],
                value: json!("x"),
            },
            Op::Add {
                path: vec!["a".to_string(), "1".to_string()],
                value: json!("y"),
            },
        ];
        let proposed = vec![
            Op::Remove {
                path: vec!["a".to_string(), "0".to_string()],
                old_value: None,
            },
            Op::Remove {
                path: vec!["a".to_string(), "1".to_string()],
                old_value: None,
            },
            Op::Remove {
                path: vec!["a".to_string(), "2".to_string()],
                old_value: None,
            },
        ];
        let result = transform(&accepted, &proposed);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].path().as_slice(), ["a", "2"]);
        assert_eq!(result[1].path().as_slice(), ["a", "3"]);
        assert_eq!(result[2].path().as_slice(), ["a", "4"]);
    }

    #[test]
    fn transform_on_different_paths_is_identity() {
        // Accepted modifies /foo; proposed modifies /bar — completely unrelated.
        let accepted = Op::Add {
            path: vec!["foo".to_string()],
            value: json!(1),
        };
        let proposed_ops = vec![
            Op::Remove {
                path: vec!["bar".to_string()],
                old_value: None,
            },
            Op::Replace {
                path: vec!["baz".to_string()],
                value: json!(99),
                old_value: None,
            },
        ];
        let result = transform(&[accepted], &proposed_ops);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].path().as_slice(), ["bar"]);
        assert_eq!(result[1].path().as_slice(), ["baz"]);
    }
}
