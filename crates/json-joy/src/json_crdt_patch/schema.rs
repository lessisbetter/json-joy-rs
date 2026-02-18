//! Schema builders for JSON CRDT document construction.
//!
//! Mirrors `packages/json-joy/src/json-crdt-patch/schema.ts`.
//!
//! Provides a composable schema DSL that generates the correct sequence of
//! [`PatchBuilder`] calls to create a structured initial document state.

use crate::json_crdt_patch::clock::Ts;
use crate::json_crdt_patch::patch_builder::PatchBuilder;

// ── NodeBuilder ────────────────────────────────────────────────────────────

/// A composable schema node that knows how to build itself via a [`PatchBuilder`].
pub trait NodeBuilder: std::fmt::Debug {
    fn build(&self, builder: &mut PatchBuilder) -> Ts;
}

// ── Concrete schema nodes ──────────────────────────────────────────────────

/// `con` constant schema node — stores an immutable value.
#[derive(Debug, Clone)]
pub struct ConNode {
    pub raw: json_joy_json_pack::PackValue,
}

impl NodeBuilder for ConNode {
    fn build(&self, builder: &mut PatchBuilder) -> Ts {
        builder.con_val(self.raw.clone())
    }
}

/// `str` schema node — an RGA string.
#[derive(Debug, Clone)]
pub struct StrNode {
    pub raw: String,
}

impl NodeBuilder for StrNode {
    fn build(&self, builder: &mut PatchBuilder) -> Ts {
        let id = builder.str_node();
        if !self.raw.is_empty() {
            builder.ins_str(id, id, self.raw.clone());
        }
        id
    }
}

/// `bin` schema node — RGA binary data.
#[derive(Debug, Clone)]
pub struct BinNode {
    pub raw: Vec<u8>,
}

impl NodeBuilder for BinNode {
    fn build(&self, builder: &mut PatchBuilder) -> Ts {
        let id = builder.bin();
        if !self.raw.is_empty() {
            builder.ins_bin(id, id, self.raw.clone());
        }
        id
    }
}

/// `val` schema node — LWW-Register wrapping another node.
#[derive(Debug)]
pub struct ValNode {
    pub value: Box<dyn NodeBuilder>,
}

impl NodeBuilder for ValNode {
    fn build(&self, builder: &mut PatchBuilder) -> Ts {
        let val_id = builder.val();
        let inner_id = self.value.build(builder);
        builder.set_val(val_id, inner_id);
        val_id
    }
}

/// `vec` schema node — LWW-Vector with indexed slots.
#[derive(Debug)]
pub struct VecNode {
    pub value: Vec<Option<Box<dyn NodeBuilder>>>,
}

impl NodeBuilder for VecNode {
    fn build(&self, builder: &mut PatchBuilder) -> Ts {
        let vec_id = builder.vec();
        let mut pairs: Vec<(u8, Ts)> = Vec::new();
        for (i, slot) in self.value.iter().enumerate() {
            if let Some(node) = slot {
                let elem_id = node.build(builder);
                pairs.push((i as u8, elem_id));
            }
        }
        if !pairs.is_empty() {
            builder.ins_vec(vec_id, pairs);
        }
        vec_id
    }
}

/// `obj` schema node — LWW-Map with named keys.
#[derive(Debug)]
pub struct ObjNode {
    pub entries: Vec<(String, Box<dyn NodeBuilder>)>,
}

impl NodeBuilder for ObjNode {
    fn build(&self, builder: &mut PatchBuilder) -> Ts {
        let obj_id = builder.obj();
        let mut pairs: Vec<(String, Ts)> = Vec::new();
        for (key, node) in &self.entries {
            let val_id = node.build(builder);
            pairs.push((key.clone(), val_id));
        }
        if !pairs.is_empty() {
            builder.ins_obj(obj_id, pairs);
        }
        obj_id
    }
}

/// `arr` schema node — RGA-Array.
#[derive(Debug)]
pub struct ArrNode {
    pub items: Vec<Box<dyn NodeBuilder>>,
}

impl NodeBuilder for ArrNode {
    fn build(&self, builder: &mut PatchBuilder) -> Ts {
        let arr_id = builder.arr();
        if !self.items.is_empty() {
            let ids: Vec<Ts> = self.items.iter().map(|n| n.build(builder)).collect();
            builder.ins_arr(arr_id, arr_id, ids);
        }
        arr_id
    }
}

// ── Schema factory `s` ─────────────────────────────────────────────────────

/// Convenience factory for building schema nodes. Mirrors the upstream `s` namespace.
pub mod s {
    use super::*;
    use json_joy_json_pack::PackValue;

    pub fn con(raw: PackValue) -> ConNode { ConNode { raw } }
    pub fn str_node(raw: &str) -> StrNode { StrNode { raw: raw.to_owned() } }
    pub fn bin(raw: Vec<u8>) -> BinNode { BinNode { raw } }
    pub fn val(value: Box<dyn NodeBuilder>) -> ValNode { ValNode { value } }
    pub fn vec(slots: Vec<Option<Box<dyn NodeBuilder>>>) -> VecNode { VecNode { value: slots } }
    pub fn obj(entries: Vec<(String, Box<dyn NodeBuilder>)>) -> ObjNode { ObjNode { entries } }
    pub fn arr(items: Vec<Box<dyn NodeBuilder>>) -> ArrNode { ArrNode { items } }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_crdt_patch::patch_builder::PatchBuilder;
    use json_joy_json_pack::PackValue;

    #[test]
    fn con_node_builds() {
        let mut builder = PatchBuilder::new(1, 0);
        let node = s::con(PackValue::Integer(42));
        node.build(&mut builder);
        assert_eq!(builder.patch.ops.len(), 1);
    }

    #[test]
    fn str_node_builds_with_content() {
        let mut builder = PatchBuilder::new(1, 0);
        let node = s::str_node("hello");
        node.build(&mut builder);
        // NewStr + InsStr
        assert_eq!(builder.patch.ops.len(), 2);
    }

    #[test]
    fn obj_node_builds() {
        let mut builder = PatchBuilder::new(1, 0);
        let node = s::obj(vec![
            ("name".into(), Box::new(s::str_node("Alice")) as Box<dyn NodeBuilder>),
        ]);
        node.build(&mut builder);
        // NewObj + NewStr + InsStr + InsObj
        assert!(builder.patch.ops.len() >= 3);
    }
}
