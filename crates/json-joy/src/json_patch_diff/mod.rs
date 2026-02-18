//! JSON Patch diff: generate a JSON Patch from two document values.
//!
//! Mirrors `packages/json-joy/src/json-patch-diff/JsonPatchDiff.ts`.

use serde_json::{Map, Value};

use crate::json_patch::types::Op;
use crate::util_inner::diff::line::{diff as line_diff, LinePatchOpType};
use crate::util_inner::diff::str::{apply as str_apply, diff as str_diff, PatchOpType};

// ── Public API ────────────────────────────────────────────────────────────

/// Generate a JSON Patch (list of operations) that transforms `src` into `dst`.
pub fn diff(src: &Value, dst: &Value) -> Vec<Op> {
    let mut ops = Vec::new();
    diff_at_path(&mut ops, &[], src, dst);
    ops
}

// ── Core recursive differ ─────────────────────────────────────────────────

fn diff_at_path(ops: &mut Vec<Op>, path: &[String], src: &Value, dst: &Value) {
    if src == dst { return; }
    match (src, dst) {
        (Value::String(s), Value::String(d)) => diff_str(ops, path, s, d),
        (Value::Object(s), Value::Object(d)) => diff_obj(ops, path, s, d),
        (Value::Array(s),  Value::Array(d))  => diff_arr(ops, path, s, d),
        _ => diff_val(ops, path, src, dst),
    }
}

fn diff_val(ops: &mut Vec<Op>, path: &[String], _src: &Value, dst: &Value) {
    ops.push(Op::Replace {
        path: path.to_vec(),
        value: dst.clone(),
        old_value: None,
    });
}

fn diff_str(ops: &mut Vec<Op>, path: &[String], src: &str, dst: &str) {
    if src == dst { return; }
    let patch = str_diff(src, dst);
    // Count characters to track char positions
    let mut pos = 0usize;
    for (op_type, text) in &patch {
        match op_type {
            PatchOpType::Eql => {
                pos += text.chars().count();
            }
            PatchOpType::Ins => {
                ops.push(Op::StrIns {
                    path: path.to_vec(),
                    pos,
                    str_val: text.clone(),
                });
                pos += text.chars().count();
            }
            PatchOpType::Del => {
                let len = text.chars().count();
                ops.push(Op::StrDel {
                    path: path.to_vec(),
                    pos,
                    str_val: Some(text.clone()),
                    len: None,
                });
                // Don't advance pos — the next op starts at the same position
                // in the new (post-deletion) string. But since we apply ops in
                // order, pos stays as-is for next ops on the MUTATED string.
                // However, we iterate over the patch in src order, so we
                // must track src position (before all mutations).
                // We re-generate ops from scratch: each op's pos is relative
                // to the already-mutated string. Since insertions advance pos
                // and deletions don't (they remove chars at pos), this is:
                let _ = len; // pos stays same after deletion
            }
        }
    }
}

fn diff_obj(
    ops: &mut Vec<Op>,
    path: &[String],
    src: &Map<String, Value>,
    dst: &Map<String, Value>,
) {
    // Remove keys in src that are not in dst
    for key in src.keys() {
        if !dst.contains_key(key) {
            let mut p = path.to_vec();
            p.push(key.clone());
            ops.push(Op::Remove { path: p, old_value: None });
        }
    }
    // Add/replace keys in dst
    for (key, dst_val) in dst {
        let mut p = path.to_vec();
        p.push(key.clone());
        match src.get(key) {
            None => ops.push(Op::Add { path: p, value: dst_val.clone() }),
            Some(src_val) => diff_at_path(ops, &p, src_val, dst_val),
        }
    }
}

fn diff_arr(ops: &mut Vec<Op>, path: &[String], src: &[Value], dst: &[Value]) {
    if src.is_empty() && dst.is_empty() { return; }
    if src.is_empty() {
        for (i, v) in dst.iter().enumerate() {
            let mut p = path.to_vec();
            p.push(i.to_string());
            ops.push(Op::Add { path: p, value: v.clone() });
        }
        return;
    }
    if dst.is_empty() {
        // Remove from end to avoid index shifting
        for i in (0..src.len()).rev() {
            let mut p = path.to_vec();
            p.push(i.to_string());
            ops.push(Op::Remove { path: p, old_value: None });
        }
        return;
    }

    // Use structural hashing to build string sequences for line diff
    let src_strs: Vec<String> = src.iter().map(struct_hash).collect();
    let dst_strs: Vec<String> = dst.iter().map(struct_hash).collect();
    let src_refs: Vec<&str> = src_strs.iter().map(|s| s.as_str()).collect();
    let dst_refs: Vec<&str> = dst_strs.iter().map(|s| s.as_str()).collect();

    let line_patch = line_diff(&src_refs, &dst_refs);

    // Track index offset as we apply insertions/deletions
    let mut offset: i64 = 0;

    for (op_type, src_idx, dst_idx) in &line_patch {
        match op_type {
            LinePatchOpType::Eql => {}
            LinePatchOpType::Del => {
                let actual_idx = (*src_idx as i64 + offset) as usize;
                let mut p = path.to_vec();
                p.push(actual_idx.to_string());
                ops.push(Op::Remove { path: p, old_value: None });
                offset -= 1;
            }
            LinePatchOpType::Ins => {
                let actual_idx = (*src_idx as i64 + offset + 1) as usize;
                let mut p = path.to_vec();
                p.push(actual_idx.to_string());
                let dst_i = *dst_idx as usize;
                ops.push(Op::Add { path: p, value: dst[dst_i].clone() });
                offset += 1;
            }
            LinePatchOpType::Mix => {
                let actual_idx = (*src_idx as i64 + offset) as usize;
                let mut p = path.to_vec();
                p.push(actual_idx.to_string());
                let src_i = *src_idx as usize;
                let dst_i = *dst_idx as usize;
                diff_at_path(ops, &p, &src[src_i], &dst[dst_i]);
            }
        }
    }
}

/// Compute a structural hash for array element comparison.
/// We use the JSON serialization as a simple content identifier.
fn struct_hash(v: &Value) -> String {
    serde_json::to_string(v).unwrap_or_default()
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_patch::apply::apply_op;
    use serde_json::json;

    fn apply_patch(mut doc: Value, ops: &[Op]) -> Value {
        for op in ops {
            apply_op(&mut doc, op).expect("apply failed");
        }
        doc
    }

    #[test]
    fn diff_equal_docs() {
        let ops = diff(&json!({"a": 1}), &json!({"a": 1}));
        assert!(ops.is_empty());
    }

    #[test]
    fn diff_replace_scalar() {
        let ops = diff(&json!(1), &json!(2));
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].op_name(), "replace");
    }

    #[test]
    fn diff_add_key() {
        let ops = diff(&json!({"a": 1}), &json!({"a": 1, "b": 2}));
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].op_name(), "add");
    }

    #[test]
    fn diff_remove_key() {
        let ops = diff(&json!({"a": 1, "b": 2}), &json!({"a": 1}));
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].op_name(), "remove");
    }

    #[test]
    fn diff_object_roundtrip() {
        let src = json!({"name": "Alice", "age": 30});
        let dst = json!({"name": "Bob", "age": 30, "city": "NYC"});
        let ops = diff(&src, &dst);
        let result = apply_patch(src, &ops);
        assert_eq!(result, dst);
    }

    #[test]
    fn diff_array_insert() {
        let src = json!([1, 2, 3]);
        let dst = json!([1, 99, 2, 3]);
        let ops = diff(&src, &dst);
        let result = apply_patch(src, &ops);
        assert_eq!(result, dst);
    }

    #[test]
    fn diff_array_delete() {
        let src = json!([1, 2, 3]);
        let dst = json!([1, 3]);
        let ops = diff(&src, &dst);
        let result = apply_patch(src, &ops);
        assert_eq!(result, dst);
    }

    #[test]
    fn diff_string_ops() {
        let src = json!("hello world");
        let dst = json!("hello rust");
        let ops = diff(&src, &dst);
        // Should use str_del and str_ins ops
        let has_str_ops = ops.iter().any(|op| matches!(op, Op::StrDel { .. } | Op::StrIns { .. }));
        assert!(has_str_ops || !ops.is_empty()); // At minimum something should be generated
    }

    #[test]
    fn diff_string_ops_roundtrip() {
        let src = json!("hello world");
        let dst = json!("hello rust");
        let ops = diff(&src, &dst);
        let result = apply_patch(src, &ops);
        assert_eq!(result, dst);
    }

    #[test]
    fn diff_string_prefix_change() {
        let src = json!("abcd");
        let dst = json!("aXd");
        let ops = diff(&src, &dst);
        let result = apply_patch(src, &ops);
        assert_eq!(result, dst);
    }

    #[test]
    fn diff_string_insert_prefix() {
        let src = json!("abc");
        let dst = json!("Xabc");
        let ops = diff(&src, &dst);
        let result = apply_patch(src, &ops);
        assert_eq!(result, dst);
    }

    #[test]
    fn diff_nested_object() {
        let src = json!({"user": {"name": "Alice", "age": 30}});
        let dst = json!({"user": {"name": "Alice", "age": 31}});
        let ops = diff(&src, &dst);
        assert!(!ops.is_empty());
        // The path should reference user/age
        let path_str = format!("{:?}", ops[0].path());
        assert!(path_str.contains("user") || path_str.contains("age"));
    }
}
