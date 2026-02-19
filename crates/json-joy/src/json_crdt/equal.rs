//! Node equality helpers.
//!
//! Mirrors:
//! - `json-crdt/equal/cmp.ts`      → [`cmp`]
//! - `json-crdt/equal/cmpNode.ts`  → [`cmp_node`]
//!
//! `cmp` checks structural + optional value equality, ignoring CRDT metadata.
//! `cmp_node` checks CRDT metadata (timestamps), ignoring deep values.

use super::nodes::{ArrNode, BinNode, ConNode, CrdtNode, ObjNode, StrNode, ValNode, VecNode};
use super::nodes::{NodeIndex, TsKey};
use crate::json_crdt_patch::clock::{equal as ts_equal, Ts};

/// Resolve a node from the index by `Ts`.
#[inline]
fn get_node<'a>(index: &'a NodeIndex, id: &Ts) -> Option<&'a CrdtNode> {
    index.get(&TsKey::from(*id))
}

// ── cmp ──────────────────────────────────────────────────────────────────

/// Deeply checks if two JSON CRDT nodes have the same schema and optionally
/// the same values.
///
/// When `compare_content` is `false`, only structural type-parity is checked
/// (same node type, same keys/length).  When `true`, leaf values are also
/// compared.
///
/// Mirrors `cmp` in `cmp.ts`.
pub fn cmp(a: &CrdtNode, b: &CrdtNode, compare_content: bool, index: &NodeIndex) -> bool {
    if std::ptr::eq(a as *const _, b as *const _) {
        return true;
    }
    match (a, b) {
        (CrdtNode::Con(na), CrdtNode::Con(nb)) => {
            if !compare_content {
                return true;
            }
            na.val == nb.val
        }
        (CrdtNode::Val(na), CrdtNode::Val(nb)) => {
            // Resolve the values both registers point to and recurse.
            let va = get_node(index, &na.val);
            let vb = get_node(index, &nb.val);
            match (va, vb) {
                (Some(ca), Some(cb)) => cmp(ca, cb, compare_content, index),
                (None, None) => true,
                _ => false,
            }
        }
        (CrdtNode::Str(na), CrdtNode::Str(nb)) => {
            if !compare_content {
                return true;
            }
            let sa = na.view_str();
            let sb = nb.view_str();
            sa.len() == sb.len() && sa == sb
        }
        (CrdtNode::Bin(na), CrdtNode::Bin(nb)) => {
            if !compare_content {
                return true;
            }
            let ba = na.view();
            let bb = nb.view();
            ba.len() == bb.len() && ba == bb
        }
        (CrdtNode::Obj(na), CrdtNode::Obj(nb)) => {
            let len1 = na.keys.len();
            let len2 = nb.keys.len();
            if len1 != len2 {
                return false;
            }
            for (key, id_a) in &na.keys {
                let id_b = match nb.keys.get(key) {
                    Some(id) => id,
                    None => return false,
                };
                let node_a = get_node(index, id_a);
                let node_b = get_node(index, id_b);
                match (node_a, node_b) {
                    (Some(ca), Some(cb)) => {
                        if !cmp(ca, cb, compare_content, index) {
                            return false;
                        }
                    }
                    (None, None) => {}
                    _ => return false,
                }
            }
            true
        }
        (CrdtNode::Vec(na), CrdtNode::Vec(nb)) => {
            let len1 = na.elements.len();
            let len2 = nb.elements.len();
            if len1 != len2 {
                return false;
            }
            for i in 0..len1 {
                let ea = na.elements[i];
                let eb = nb.elements[i];
                match (ea, eb) {
                    (Some(ia), Some(ib)) => {
                        let ca = get_node(index, &ia);
                        let cb = get_node(index, &ib);
                        match (ca, cb) {
                            (Some(na), Some(nb)) => {
                                if !cmp(na, nb, compare_content, index) {
                                    return false;
                                }
                            }
                            (None, None) => {}
                            _ => return false,
                        }
                    }
                    (None, None) => {}
                    _ => return false,
                }
            }
            true
        }
        (CrdtNode::Arr(na), CrdtNode::Arr(nb)) => {
            let va: Vec<Ts> = na
                .rga
                .iter_live()
                .filter_map(|c| c.data.as_ref())
                .flat_map(|v| v.iter().copied())
                .collect();
            let vb: Vec<Ts> = nb
                .rga
                .iter_live()
                .filter_map(|c| c.data.as_ref())
                .flat_map(|v| v.iter().copied())
                .collect();
            if va.len() != vb.len() {
                return false;
            }
            if !compare_content {
                return true;
            }
            for (id_a, id_b) in va.iter().zip(vb.iter()) {
                let ca = get_node(index, id_a);
                let cb = get_node(index, id_b);
                match (ca, cb) {
                    (Some(na), Some(nb)) => {
                        if !cmp(na, nb, compare_content, index) {
                            return false;
                        }
                    }
                    (None, None) => {}
                    _ => return false,
                }
            }
            true
        }
        _ => false, // different types
    }
}

// ── cmp_node ──────────────────────────────────────────────────────────────

/// Performs type and metadata shallow check of two JSON CRDT nodes.
///
/// Compares node type and their timestamps / structural metadata (like the
/// max chunk ID and live length for RGA nodes).  Does not compare values.
///
/// Mirrors `cmpNode` in `cmpNode.ts`.
pub fn cmp_node(a: &CrdtNode, b: &CrdtNode) -> bool {
    if std::ptr::eq(a as *const _, b as *const _) {
        return true;
    }
    match (a, b) {
        (CrdtNode::Con(na), CrdtNode::Con(nb)) => ts_equal(na.id, nb.id),
        (CrdtNode::Val(na), CrdtNode::Val(nb)) => {
            ts_equal(na.id, nb.id) && ts_equal(na.val, nb.val)
        }
        (CrdtNode::Str(na), CrdtNode::Str(nb)) => {
            if !ts_equal(na.id, nb.id) {
                return false;
            }
            cmp_rga_str(na, nb)
        }
        (CrdtNode::Bin(na), CrdtNode::Bin(nb)) => {
            if !ts_equal(na.id, nb.id) {
                return false;
            }
            cmp_rga_bin(na, nb)
        }
        (CrdtNode::Obj(na), CrdtNode::Obj(nb)) => {
            if !ts_equal(na.id, nb.id) {
                return false;
            }
            if na.keys.len() != nb.keys.len() {
                return false;
            }
            for (key, ts_a) in &na.keys {
                match nb.keys.get(key) {
                    Some(ts_b) if ts_equal(*ts_a, *ts_b) => {}
                    _ => return false,
                }
            }
            true
        }
        (CrdtNode::Vec(na), CrdtNode::Vec(nb)) => {
            if !ts_equal(na.id, nb.id) {
                return false;
            }
            let len = na.elements.len();
            if len != nb.elements.len() {
                return false;
            }
            for i in 0..len {
                match (na.elements[i], nb.elements[i]) {
                    (Some(a), Some(b)) if ts_equal(a, b) => {}
                    (None, None) => {}
                    _ => return false,
                }
            }
            true
        }
        (CrdtNode::Arr(na), CrdtNode::Arr(nb)) => {
            if !ts_equal(na.id, nb.id) {
                return false;
            }
            cmp_rga_arr(na, nb)
        }
        _ => false,
    }
}

// ── RGA comparison helpers ────────────────────────────────────────────────

/// Compare two StrNode RGAs by max-chunk-ID and live length.
///
/// Mirrors `cmpRga` in `cmpNode.ts`:
/// - If both have a last chunk, their IDs must match.
/// - `size()` (live char count) and chunk count must match.
fn cmp_rga_str(a: &StrNode, b: &StrNode) -> bool {
    let max_a = a.rga.last_chunk();
    let max_b = b.rga.last_chunk();
    match (max_a, max_b) {
        (Some(ca), Some(cb)) => {
            if !ts_equal(ca.id, cb.id) {
                return false;
            }
        }
        (None, None) => {}
        _ => return false,
    }
    a.size() == b.size() && a.rga.chunk_count() == b.rga.chunk_count()
}

/// Compare two BinNode RGAs by max-chunk-ID and live length.
fn cmp_rga_bin(a: &BinNode, b: &BinNode) -> bool {
    let max_a = a.rga.last_chunk();
    let max_b = b.rga.last_chunk();
    match (max_a, max_b) {
        (Some(ca), Some(cb)) => {
            if !ts_equal(ca.id, cb.id) {
                return false;
            }
        }
        (None, None) => {}
        _ => return false,
    }
    let len_a: usize = a
        .rga
        .iter_live()
        .filter_map(|c| c.data.as_ref())
        .map(|v| v.len())
        .sum();
    let len_b: usize = b
        .rga
        .iter_live()
        .filter_map(|c| c.data.as_ref())
        .map(|v| v.len())
        .sum();
    len_a == len_b && a.rga.chunk_count() == b.rga.chunk_count()
}

/// Compare two ArrNode RGAs by max-chunk-ID and live length.
fn cmp_rga_arr(a: &ArrNode, b: &ArrNode) -> bool {
    let max_a = a.rga.last_chunk();
    let max_b = b.rga.last_chunk();
    match (max_a, max_b) {
        (Some(ca), Some(cb)) => {
            if !ts_equal(ca.id, cb.id) {
                return false;
            }
        }
        (None, None) => {}
        _ => return false,
    }
    a.size() == b.size() && a.rga.chunk_count() == b.rga.chunk_count()
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_crdt_patch::clock::ts;
    use crate::json_crdt_patch::operations::ConValue;
    use json_joy_json_pack::PackValue;
    use std::collections::HashMap;

    fn sid() -> u64 {
        999
    }

    // ── cmp tests ────────────────────────────────────────────────────────

    #[test]
    fn cmp_con_same_value() {
        let index = HashMap::default();
        let a = CrdtNode::Con(ConNode::new(
            ts(sid(), 1),
            ConValue::Val(PackValue::Integer(42)),
        ));
        let b = CrdtNode::Con(ConNode::new(
            ts(sid(), 2),
            ConValue::Val(PackValue::Integer(42)),
        ));
        assert!(cmp(&a, &b, true, &index));
    }

    #[test]
    fn cmp_con_different_value() {
        let index = HashMap::default();
        let a = CrdtNode::Con(ConNode::new(
            ts(sid(), 1),
            ConValue::Val(PackValue::Integer(1)),
        ));
        let b = CrdtNode::Con(ConNode::new(
            ts(sid(), 2),
            ConValue::Val(PackValue::Integer(2)),
        ));
        assert!(!cmp(&a, &b, true, &index));
    }

    #[test]
    fn cmp_con_no_content() {
        // With compareContent=false, different values should be "equal".
        let index = HashMap::default();
        let a = CrdtNode::Con(ConNode::new(
            ts(sid(), 1),
            ConValue::Val(PackValue::Integer(1)),
        ));
        let b = CrdtNode::Con(ConNode::new(
            ts(sid(), 2),
            ConValue::Val(PackValue::Integer(2)),
        ));
        assert!(cmp(&a, &b, false, &index));
    }

    #[test]
    fn cmp_different_types_false() {
        let index = HashMap::default();
        let a = CrdtNode::Con(ConNode::new(
            ts(sid(), 1),
            ConValue::Val(PackValue::Integer(1)),
        ));
        let b = CrdtNode::Str(StrNode::new(ts(sid(), 1)));
        assert!(!cmp(&a, &b, false, &index));
    }

    // ── cmp_node tests ───────────────────────────────────────────────────

    #[test]
    fn cmp_node_same_con_id() {
        let a = CrdtNode::Con(ConNode::new(
            ts(sid(), 5),
            ConValue::Val(PackValue::Integer(1)),
        ));
        let b = CrdtNode::Con(ConNode::new(
            ts(sid(), 5),
            ConValue::Val(PackValue::Integer(2)),
        ));
        // Same ID → true (cmpNode ignores values)
        assert!(cmp_node(&a, &b));
    }

    #[test]
    fn cmp_node_different_con_id() {
        let a = CrdtNode::Con(ConNode::new(
            ts(sid(), 5),
            ConValue::Val(PackValue::Integer(1)),
        ));
        let b = CrdtNode::Con(ConNode::new(
            ts(sid(), 6),
            ConValue::Val(PackValue::Integer(1)),
        ));
        assert!(!cmp_node(&a, &b));
    }

    #[test]
    fn cmp_node_different_types() {
        let a = CrdtNode::Con(ConNode::new(
            ts(sid(), 5),
            ConValue::Val(PackValue::Integer(1)),
        ));
        let b = CrdtNode::Str(StrNode::new(ts(sid(), 5)));
        assert!(!cmp_node(&a, &b));
    }

    #[test]
    fn cmp_node_identical_str_nodes() {
        // Two empty StrNodes with same ID should be equal.
        let a = CrdtNode::Str(StrNode::new(ts(sid(), 1)));
        let b = CrdtNode::Str(StrNode::new(ts(sid(), 1)));
        assert!(cmp_node(&a, &b));
    }

    #[test]
    fn cmp_node_obj_same_keys() {
        let mut na = ObjNode::new(ts(sid(), 1));
        na.put("x", ts(sid(), 10));
        let mut nb = ObjNode::new(ts(sid(), 1));
        nb.put("x", ts(sid(), 10));
        let a = CrdtNode::Obj(na);
        let b = CrdtNode::Obj(nb);
        assert!(cmp_node(&a, &b));
    }

    #[test]
    fn cmp_node_obj_different_keys() {
        let mut na = ObjNode::new(ts(sid(), 1));
        na.put("x", ts(sid(), 10));
        let mut nb = ObjNode::new(ts(sid(), 1));
        nb.put("x", ts(sid(), 11));
        let a = CrdtNode::Obj(na);
        let b = CrdtNode::Obj(nb);
        assert!(!cmp_node(&a, &b));
    }
}
