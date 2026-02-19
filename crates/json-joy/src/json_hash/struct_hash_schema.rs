//! Structural hash over JSON CRDT schema nodes.
//!
//! Mirrors `packages/json-joy/src/json-hash/structHashSchema.ts`.
//!
//! Provides `struct_hash_schema` which takes any schema node (via the
//! `NodeBuilder` trait) and returns a structural hash string.  The hash
//! format mirrors `struct_hash` / `struct_hash_crdt` for the equivalent
//! value type.

use crate::json_crdt_patch::schema::NodeBuilder;

/// Compute a structural hash string for any schema node.
///
/// Delegates to `NodeBuilder::struct_hash()` which is implemented by all
/// concrete schema node types (`ConNode`, `StrNode`, `BinNode`, `ValNode`,
/// `ObjNode`, `ArrNode`, `VecNode`).
///
/// Returns `"U"` for unknown or unimplemented node types.
pub fn struct_hash_schema(node: &dyn NodeBuilder) -> String {
    node.struct_hash()
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_crdt_patch::schema::{s, NodeBuilder};
    use json_joy_json_pack::PackValue;

    #[test]
    fn hash_con_null() {
        let node = s::con(PackValue::Null);
        assert_eq!(struct_hash_schema(&node), "N");
    }

    #[test]
    fn hash_con_true() {
        let node = s::con(PackValue::Bool(true));
        assert_eq!(struct_hash_schema(&node), "T");
    }

    #[test]
    fn hash_con_false() {
        let node = s::con(PackValue::Bool(false));
        assert_eq!(struct_hash_schema(&node), "F");
    }

    #[test]
    fn hash_str_empty_is_stable() {
        let node = s::str_node("");
        let h1 = struct_hash_schema(&node);
        let h2 = struct_hash_schema(&node);
        assert_eq!(h1, h2);
    }

    #[test]
    fn hash_str_different_values_differ() {
        let n1 = s::str_node("hello");
        let n2 = s::str_node("world");
        assert_ne!(struct_hash_schema(&n1), struct_hash_schema(&n2));
    }

    #[test]
    fn hash_bin_empty_is_stable() {
        let node = s::bin(vec![]);
        let h1 = struct_hash_schema(&node);
        let h2 = struct_hash_schema(&node);
        assert_eq!(h1, h2);
    }

    #[test]
    fn hash_bin_different_bytes_differ() {
        let n1 = s::bin(vec![1, 2, 3]);
        let n2 = s::bin(vec![1, 2, 4]);
        assert_ne!(struct_hash_schema(&n1), struct_hash_schema(&n2));
    }

    #[test]
    fn hash_obj_empty() {
        let node = s::obj(vec![]);
        assert_eq!(struct_hash_schema(&node), "{}");
    }

    #[test]
    fn hash_arr_empty() {
        let node = s::arr(vec![]);
        assert_eq!(struct_hash_schema(&node), "[]");
    }

    #[test]
    fn hash_obj_key_order_independent() {
        let obj1 = s::obj(vec![
            (
                "foo".to_string(),
                Box::new(s::con(PackValue::Integer(1))) as Box<dyn NodeBuilder>,
            ),
            (
                "bar".to_string(),
                Box::new(s::con(PackValue::Integer(2))) as Box<dyn NodeBuilder>,
            ),
        ]);
        let obj2 = s::obj(vec![
            (
                "bar".to_string(),
                Box::new(s::con(PackValue::Integer(2))) as Box<dyn NodeBuilder>,
            ),
            (
                "foo".to_string(),
                Box::new(s::con(PackValue::Integer(1))) as Box<dyn NodeBuilder>,
            ),
        ]);
        assert_eq!(struct_hash_schema(&obj1), struct_hash_schema(&obj2));
    }

    #[test]
    fn hash_arr_with_items() {
        let node = s::arr(vec![
            Box::new(s::con(PackValue::Null)) as Box<dyn NodeBuilder>,
            Box::new(s::con(PackValue::Bool(true))) as Box<dyn NodeBuilder>,
        ]);
        let h = struct_hash_schema(&node);
        assert!(h.starts_with('['));
        assert!(h.ends_with(']'));
        // Two items → two semicolons
        assert_eq!(h.chars().filter(|&c| c == ';').count(), 2);
    }

    #[test]
    fn hash_val_delegates_to_inner() {
        let val_node = s::val(Box::new(s::con(PackValue::Null)));
        let con_node = s::con(PackValue::Null);
        assert_eq!(struct_hash_schema(&val_node), struct_hash_schema(&con_node));
    }

    #[test]
    fn hash_no_newlines_in_output() {
        let obj = s::obj(vec![(
            "key".to_string(),
            Box::new(s::str_node("value")) as Box<dyn NodeBuilder>,
        )]);
        let h = struct_hash_schema(&obj);
        assert!(!h.contains('\n'));
    }
}
