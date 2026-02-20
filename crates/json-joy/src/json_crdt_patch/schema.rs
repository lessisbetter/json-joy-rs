//! Schema builders for JSON CRDT document construction.
//!
//! Mirrors `packages/json-joy/src/json-crdt-patch/schema.ts`.
//!
//! Provides a composable schema DSL that generates the correct sequence of
//! [`PatchBuilder`] calls to create a structured initial document state.

use crate::json_crdt_patch::clock::Ts;
use crate::json_crdt_patch::patch_builder::PatchBuilder;

// ── Internal hash helpers (used by struct_hash_schema) ─────────────────────

fn hash_pack_value_as_struct_hash(raw: &json_joy_json_pack::PackValue) -> String {
    let json_val: serde_json::Value = serde_json::Value::from(raw.clone());
    struct_hash_json(&json_val)
}

fn struct_hash_json(val: &serde_json::Value) -> String {
    use serde_json::Value;
    match val {
        Value::String(s) => {
            // hash(s).toString(36)
            let h = hash_str_for_schema(s);
            radix36(h as u64)
        }
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                if i < 0 {
                    format!("-{}", radix36((-i) as u64))
                } else {
                    radix36(i as u64)
                }
            } else if let Some(u) = n.as_u64() {
                radix36(u)
            } else {
                let f = n.as_f64().unwrap_or(0.0);
                if f < 0.0 {
                    format!("-{}", radix36((-f) as u64))
                } else {
                    radix36(f as u64)
                }
            }
        }
        Value::Bool(b) => {
            if *b {
                "T".to_string()
            } else {
                "F".to_string()
            }
        }
        Value::Null => "N".to_string(),
        Value::Array(arr) => {
            let mut res = String::from("[");
            for v in arr {
                res.push_str(&struct_hash_json(v));
                res.push(';');
            }
            res.push(']');
            res
        }
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let mut res = String::from("{");
            for key in keys {
                res.push_str(&radix36(hash_str_for_schema(key) as u64));
                res.push(':');
                res.push_str(&struct_hash_json(&map[key]));
                res.push(',');
            }
            res.push('}');
            res
        }
    }
}

fn hash_str_for_schema(s: &str) -> u32 {
    // Matches upstream: hash(str) where str is a JS string.
    // In hash.ts: case 'string': state = updateNum(state, STRING); return updateStr(state, s)
    // updateStr(state, s) internally does: state = updateNum(state, STRING); ...
    const START_STATE: i32 = 5381;
    const STRING_CONST: i32 = 982453601_u32 as i32;
    fn update_num(state: i32, num: i32) -> i32 {
        state.wrapping_shl(5).wrapping_add(state).wrapping_add(num)
    }
    fn update_str(mut state: i32, s: &str) -> i32 {
        let utf16: Vec<u16> = s.encode_utf16().collect();
        let length = utf16.len() as i32;
        state = update_num(state, STRING_CONST);
        state = update_num(state, length);
        for &cu in utf16.iter().rev() {
            state = update_num(state, cu as i32);
        }
        state
    }
    let state = update_num(START_STATE, STRING_CONST);
    update_str(state, s) as u32
}

fn hash_bin_for_schema(bytes: &[u8]) -> u32 {
    const START_STATE: i32 = 5381;
    const BINARY_CONST: i32 = 982454837_u32 as i32;
    fn update_num(state: i32, num: i32) -> i32 {
        state.wrapping_shl(5).wrapping_add(state).wrapping_add(num)
    }
    let mut state = update_num(START_STATE, BINARY_CONST);
    let length = bytes.len() as i32;
    state = update_num(state, length);
    for &b in bytes.iter().rev() {
        state = update_num(state, b as i32);
    }
    state as u32
}

fn radix36(mut n: u64) -> String {
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

// ── NodeBuilder ────────────────────────────────────────────────────────────

/// A composable schema node that knows how to build itself via a [`PatchBuilder`].
pub trait NodeBuilder: std::fmt::Debug {
    fn build(&self, builder: &mut PatchBuilder) -> Ts;

    /// Structural hash string for this schema node.
    ///
    /// Mirrors `structHashSchema` in the upstream TypeScript.
    fn struct_hash(&self) -> String {
        "U".to_string()
    }
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

    fn struct_hash(&self) -> String {
        hash_pack_value_as_struct_hash(&self.raw)
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

    fn struct_hash(&self) -> String {
        let json_val = serde_json::Value::String(self.raw.clone());
        struct_hash_json(&json_val)
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

    fn struct_hash(&self) -> String {
        radix36(hash_bin_for_schema(&self.raw) as u64)
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

    fn struct_hash(&self) -> String {
        self.value.struct_hash()
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

    fn struct_hash(&self) -> String {
        let mut res = String::from("[");
        for child in self.value.iter().flatten() {
            res.push_str(&child.struct_hash());
            res.push(';');
        }
        res.push(']');
        res
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

    fn struct_hash(&self) -> String {
        let mut keys: Vec<&String> = self.entries.iter().map(|(k, _)| k).collect();
        keys.sort();
        let mut res = String::from("{");
        for key in &keys {
            if let Some((_, val_node)) = self.entries.iter().find(|(k, _)| k == *key) {
                let key_hash = radix36(hash_str_for_schema(key) as u64);
                let val_hash = val_node.struct_hash();
                res.push_str(&key_hash);
                res.push(':');
                res.push_str(&val_hash);
                res.push(',');
            }
        }
        res.push('}');
        res
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

    fn struct_hash(&self) -> String {
        let mut res = String::from("[");
        for child in &self.items {
            res.push_str(&child.struct_hash());
            res.push(';');
        }
        res.push(']');
        res
    }
}

// ── Schema factory `s` ─────────────────────────────────────────────────────

/// Convenience factory for building schema nodes. Mirrors the upstream `s` namespace.
pub mod s {
    use super::*;
    use json_joy_json_pack::PackValue;

    pub fn con(raw: PackValue) -> ConNode {
        ConNode { raw }
    }
    pub fn str_node(raw: &str) -> StrNode {
        StrNode {
            raw: raw.to_owned(),
        }
    }
    pub fn bin(raw: Vec<u8>) -> BinNode {
        BinNode { raw }
    }
    pub fn val(value: Box<dyn NodeBuilder>) -> ValNode {
        ValNode { value }
    }
    pub fn vec(slots: Vec<Option<Box<dyn NodeBuilder>>>) -> VecNode {
        VecNode { value: slots }
    }
    pub fn obj(entries: Vec<(String, Box<dyn NodeBuilder>)>) -> ObjNode {
        ObjNode { entries }
    }
    pub fn arr(items: Vec<Box<dyn NodeBuilder>>) -> ArrNode {
        ArrNode { items }
    }
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
        let node = s::obj(vec![(
            "name".into(),
            Box::new(s::str_node("Alice")) as Box<dyn NodeBuilder>,
        )]);
        node.build(&mut builder);
        // NewObj + NewStr + InsStr + InsObj
        assert!(builder.patch.ops.len() >= 3);
    }
}
