//! Structural hash over CRDT nodes.
//!
//! Mirrors `packages/json-joy/src/json-hash/structHashCrdt.ts`.
//!
//! Produces a *structural hash* of a JSON CRDT node tree.  Works the same as
//! `struct_hash`, but uses the `CrdtNode` enum / `NodeIndex` from the
//! `json_crdt` module instead of a generic `serde_json::Value`.
//!
//! The hash is guaranteed to contain only printable ASCII characters,
//! excluding the newline character.

use super::hash::hash_str;
use super::struct_hash::struct_hash;
use crate::json_crdt::nodes::{CrdtNode, NodeIndex, TsKey};

/// Compute a structural hash string for a CRDT node.
///
/// The function follows the upstream TypeScript algorithm:
///
/// - `ConNode` → `struct_hash(node.val)` (using the underlying JSON value)
/// - `ValNode` → recursively hash the pointed-to child node
/// - `StrNode` → `hash(node.view()).toString(36)` — hash of the string view
/// - `ObjNode` → `"{k1hash:v1hash,...}"` with keys sorted
/// - `ArrNode` / `VecNode` → `"[c1hash;c2hash;...]"`
/// - `BinNode` → hash of the raw bytes
/// - unknown / missing → `"U"`
pub fn struct_hash_crdt(node: Option<&CrdtNode>, index: &NodeIndex) -> String {
    let Some(node) = node else {
        return "U".to_string();
    };

    match node {
        CrdtNode::Con(con) => {
            // struct_hash on the ConValue (convert to serde_json::Value first)
            let json_val = con.view();
            struct_hash(&json_val)
        }
        CrdtNode::Val(val) => {
            // Resolve the pointed-to node and recurse
            let child_id = val.val;
            let child = index.get(&TsKey::from(child_id));
            struct_hash_crdt(child, index)
        }
        CrdtNode::Str(str_node) => {
            // hash(node.view()).toString(36)
            let view = str_node.view_str();
            let h = hash_str_value(&view);
            radix_36(h as u64)
        }
        CrdtNode::Obj(obj_node) => {
            let mut res = String::from("{");
            let mut sorted_keys: Vec<&String> = obj_node.keys.keys().collect();
            sorted_keys.sort();
            for key in &sorted_keys {
                let child_id = obj_node.keys[key.as_str()];
                let child = index.get(&TsKey::from(child_id));
                // hash(key).toString(36)
                let key_hash = radix_36(hash_str(key) as u64);
                let val_hash = struct_hash_crdt(child, index);
                res.push_str(&key_hash);
                res.push(':');
                res.push_str(&val_hash);
                res.push(',');
            }
            res.push('}');
            res
        }
        CrdtNode::Arr(arr_node) => {
            let mut res = String::from("[");
            // Iterate over live elements: each chunk entry holds Vec<Ts> (node IDs)
            for chunk in arr_node.rga.iter_live() {
                if let Some(ids) = &chunk.data {
                    for id in ids {
                        let child = index.get(&TsKey::from(*id));
                        res.push_str(&struct_hash_crdt(child, index));
                        res.push(';');
                    }
                }
            }
            res.push(']');
            res
        }
        CrdtNode::Vec(vec_node) => {
            let mut res = String::from("[");
            for id in vec_node.elements.iter().flatten() {
                let child = index.get(&TsKey::from(*id));
                res.push_str(&struct_hash_crdt(child, index));
                res.push(';');
            }
            res.push(']');
            res
        }
        CrdtNode::Bin(bin_node) => {
            // hash(node.view()).toString(36) — same as for strings but over bytes
            let bytes = bin_node.view();
            let h = hash_bin_value(&bytes);
            radix_36(h as u64)
        }
    }
}

// ── Internal helpers ───────────────────────────────────────────────────────

/// Hash a string the same way as `hash(str)` in the upstream.
///
/// In TypeScript: `hash(node.view())` where `node.view()` returns a string.
/// The upstream `hash` function dispatches on JS type; for a string it
/// calls `updateStr` (with STRING discriminator prepended twice — once in
/// `updateJson` and once internally in `updateStr`).  We replicate that
/// by using `hash_str` from the `hash` module.
fn hash_str_value(s: &str) -> u32 {
    use super::hash::{update_num, update_str, START_STATE, STRING_CONST};
    // TypeScript: case 'string': state = updateNum(state, STRING); return updateStr(state, s)
    // updateStr itself calls updateNum(state, STRING) internally again.
    let state = update_num(START_STATE, STRING_CONST);
    update_str(state, s) as u32
}

/// Hash binary data the same way as `hash(uint8array)` in the upstream.
fn hash_bin_value(bytes: &[u8]) -> u32 {
    use super::hash::{update_bin, START_STATE};
    update_bin(START_STATE, bytes) as u32
}

/// Encode a u64 in base-36 using lowercase letters (matches JS `.toString(36)`).
fn radix_36(mut n: u64) -> String {
    if n == 0 {
        return "0".to_string();
    }
    const DIGITS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut buf = Vec::new();
    while n > 0 {
        buf.push(DIGITS[(n % 36) as usize]);
        n /= 36;
    }
    buf.reverse();
    String::from_utf8(buf).unwrap()
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_crdt::nodes::{
        ArrNode, BinNode, ConNode, NodeIndex, ObjNode, StrNode, TsKey, ValNode,
    };
    use crate::json_crdt_patch::clock::Ts;
    use crate::json_crdt_patch::operations::ConValue;
    use json_joy_json_pack::PackValue;

    fn ts(sid: u64, time: u64) -> Ts {
        Ts::new(sid, time)
    }

    #[test]
    fn hash_missing_node_returns_u() {
        let index: NodeIndex = NodeIndex::new();
        let result = struct_hash_crdt(None, &index);
        assert_eq!(result, "U");
    }

    #[test]
    fn hash_con_null() {
        let index: NodeIndex = NodeIndex::new();
        let node = CrdtNode::Con(ConNode::new(ts(1, 0), ConValue::Val(PackValue::Null)));
        let h = struct_hash_crdt(Some(&node), &index);
        assert_eq!(h, "N");
    }

    #[test]
    fn hash_con_true() {
        let index: NodeIndex = NodeIndex::new();
        let node = CrdtNode::Con(ConNode::new(ts(1, 0), ConValue::Val(PackValue::Bool(true))));
        let h = struct_hash_crdt(Some(&node), &index);
        assert_eq!(h, "T");
    }

    #[test]
    fn hash_con_false() {
        let index: NodeIndex = NodeIndex::new();
        let node = CrdtNode::Con(ConNode::new(
            ts(1, 0),
            ConValue::Val(PackValue::Bool(false)),
        ));
        let h = struct_hash_crdt(Some(&node), &index);
        assert_eq!(h, "F");
    }

    #[test]
    fn hash_str_node_empty() {
        let index: NodeIndex = NodeIndex::new();
        let node = CrdtNode::Str(StrNode::new(ts(1, 0)));
        let h1 = struct_hash_crdt(Some(&node), &index);
        let h2 = struct_hash_crdt(Some(&node), &index);
        assert_eq!(h1, h2);
    }

    #[test]
    fn hash_str_node_with_content() {
        let index: NodeIndex = NodeIndex::new();
        let mut str_node = StrNode::new(ts(1, 0));
        str_node.ins(ts(1, 0), ts(1, 1), "hello".to_string());
        let node = CrdtNode::Str(str_node);
        let h = struct_hash_crdt(Some(&node), &index);
        // Should be a non-empty alphanumeric string
        assert!(!h.is_empty());
        assert!(h.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'));
    }

    #[test]
    fn hash_obj_node_empty() {
        let index: NodeIndex = NodeIndex::new();
        let node = CrdtNode::Obj(ObjNode::new(ts(1, 0)));
        let h = struct_hash_crdt(Some(&node), &index);
        assert_eq!(h, "{}");
    }

    #[test]
    fn hash_arr_node_empty() {
        let index: NodeIndex = NodeIndex::new();
        let node = CrdtNode::Arr(ArrNode::new(ts(1, 0)));
        let h = struct_hash_crdt(Some(&node), &index);
        assert_eq!(h, "[]");
    }

    #[test]
    fn hash_bin_node_empty() {
        let index: NodeIndex = NodeIndex::new();
        let node = CrdtNode::Bin(BinNode::new(ts(1, 0)));
        let h1 = struct_hash_crdt(Some(&node), &index);
        let h2 = struct_hash_crdt(Some(&node), &index);
        assert_eq!(h1, h2);
    }

    #[test]
    fn hash_obj_with_key_is_order_independent() {
        // Two ObjNodes with the same keys produce the same hash.
        let mut index: NodeIndex = NodeIndex::new();
        let con_id_a = ts(1, 1);
        let con_id_b = ts(1, 2);
        index.insert(
            TsKey::from(con_id_a),
            CrdtNode::Con(ConNode::new(con_id_a, ConValue::Val(PackValue::Integer(1)))),
        );
        index.insert(
            TsKey::from(con_id_b),
            CrdtNode::Con(ConNode::new(con_id_b, ConValue::Val(PackValue::Integer(2)))),
        );

        let mut obj1 = ObjNode::new(ts(1, 0));
        obj1.put("foo", con_id_a);
        obj1.put("bar", con_id_b);

        let mut obj2 = ObjNode::new(ts(1, 0));
        obj2.put("bar", con_id_b);
        obj2.put("foo", con_id_a);

        let h1 = struct_hash_crdt(Some(&CrdtNode::Obj(obj1)), &index);
        let h2 = struct_hash_crdt(Some(&CrdtNode::Obj(obj2)), &index);
        assert_eq!(h1, h2);
    }

    #[test]
    fn hash_val_node_resolves_child() {
        let mut index: NodeIndex = NodeIndex::new();
        let child_id = ts(1, 1);
        let child = ConNode::new(child_id, ConValue::Val(PackValue::Null));
        index.insert(TsKey::from(child_id), CrdtNode::Con(child));

        let mut val_node = ValNode::new(ts(1, 0));
        val_node.val = child_id;
        let node = CrdtNode::Val(val_node);

        let h = struct_hash_crdt(Some(&node), &index);
        assert_eq!(h, "N");
    }
}
