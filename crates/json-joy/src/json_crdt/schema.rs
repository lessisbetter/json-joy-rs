//! Schema inference from CRDT nodes.
//!
//! Mirrors:
//! - `json-crdt/schema/toSchema.ts`  → [`to_schema`]
//! - `json-crdt/schema/types.ts`      → (type aliases only — no runtime code)
//!
//! `to_schema` converts any JSON CRDT node tree to the corresponding schema
//! representation (a [`NodeBuilder`] box), which can then be used to
//! reproduce the structure in another document.

use super::nodes::{CrdtNode, NodeIndex, TsKey};
use crate::json_crdt_patch::clock::Ts;
use crate::json_crdt_patch::operations::ConValue;
use crate::json_crdt_patch::schema::{
    ArrNode as ArrSchema, BinNode as BinSchema, ConNode as ConSchema, NodeBuilder,
    ObjNode as ObjSchema, StrNode as StrSchema, ValNode as ValSchema, VecNode as VecSchema,
};

/// Resolve a node from the index by `Ts`.
#[inline]
fn get_node<'a>(index: &'a NodeIndex, id: &Ts) -> Option<&'a CrdtNode> {
    index.get(&TsKey::from(*id))
}

/// Convert a JSON CRDT node to its schema representation.
///
/// The returned `Box<dyn NodeBuilder>` captures the structural shape and
/// leaf values of the node tree.  It can be applied to a fresh document to
/// reproduce that structure.
///
/// Mirrors `toSchema` in `toSchema.ts`.
pub fn to_schema(node: &CrdtNode, index: &NodeIndex) -> Box<dyn NodeBuilder> {
    match node {
        CrdtNode::Con(n) => {
            let raw = match &n.val {
                ConValue::Val(pv) => pv.clone(),
                ConValue::Ref(_) => json_joy_json_pack::PackValue::Null,
            };
            Box::new(ConSchema { raw })
        }
        CrdtNode::Val(n) => {
            let inner_node = get_node(index, &n.val);
            let inner_schema: Box<dyn NodeBuilder> = match inner_node {
                Some(child) => to_schema(child, index),
                None => Box::new(ConSchema {
                    raw: json_joy_json_pack::PackValue::Null,
                }),
            };
            Box::new(ValSchema {
                value: inner_schema,
            })
        }
        CrdtNode::Str(n) => Box::new(StrSchema { raw: n.view_str() }),
        CrdtNode::Bin(n) => Box::new(BinSchema { raw: n.view() }),
        CrdtNode::Obj(n) => {
            let mut entries: Vec<(String, Box<dyn NodeBuilder>)> = Vec::new();
            let mut sorted_keys: Vec<&String> = n.keys.keys().collect();
            sorted_keys.sort();
            for key in &sorted_keys {
                let id = n.keys[key.as_str()];
                let child_schema = match get_node(index, &id) {
                    Some(child) => to_schema(child, index),
                    None => Box::new(ConSchema {
                        raw: json_joy_json_pack::PackValue::Null,
                    }),
                };
                entries.push(((*key).clone(), child_schema));
            }
            Box::new(ObjSchema { entries })
        }
        CrdtNode::Vec(n) => {
            let len = n.elements.len();
            let mut slots: Vec<Option<Box<dyn NodeBuilder>>> = Vec::with_capacity(len);
            for opt_id in &n.elements {
                let schema = match opt_id {
                    Some(id) => match get_node(index, id) {
                        Some(child) => Some(to_schema(child, index)),
                        None => None,
                    },
                    None => None,
                };
                slots.push(schema);
            }
            Box::new(VecSchema { value: slots })
        }
        CrdtNode::Arr(n) => {
            let mut items: Vec<Box<dyn NodeBuilder>> = Vec::new();
            for chunk in n.rga.iter_live() {
                if let Some(ids) = &chunk.data {
                    for id in ids {
                        let child_schema = match get_node(index, id) {
                            Some(child) => to_schema(child, index),
                            None => Box::new(ConSchema {
                                raw: json_joy_json_pack::PackValue::Null,
                            }),
                        };
                        items.push(child_schema);
                    }
                }
            }
            Box::new(ArrSchema { items })
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_crdt::constants::ORIGIN;
    use crate::json_crdt::model::Model;
    use crate::json_crdt_patch::clock::ts;
    use crate::json_crdt_patch::operations::{ConValue, Op};
    use crate::json_crdt_patch::patch_builder::PatchBuilder;
    use json_joy_json_pack::PackValue;
    use serde_json::json;

    fn sid() -> u64 {
        77777
    }

    /// Build a schema from a model's root node, apply it to a new model,
    /// and compare their views.
    fn roundtrip_view(model: &Model) -> serde_json::Value {
        let root_id = model.root.val;
        use crate::json_crdt::nodes::TsKey;
        let original_node = model.index.get(&TsKey::from(root_id));
        let schema: Box<dyn NodeBuilder> = match original_node {
            Some(n) => to_schema(n, &model.index),
            None => return serde_json::Value::Null,
        };

        let mut model2 = Model::new(sid());
        let mut builder = PatchBuilder::new(model2.clock.sid, model2.clock.time);
        let root_ts = schema.build(&mut builder);
        builder.set_val(ORIGIN, root_ts);
        model2.apply_patch(&builder.patch);
        model2.view()
    }

    #[test]
    fn to_schema_con_node() {
        let mut model = Model::new(sid());
        let s = sid();
        model.apply_operation(&Op::NewCon {
            id: ts(s, 1),
            val: ConValue::Val(PackValue::Integer(42)),
        });
        model.apply_operation(&Op::InsVal {
            id: ts(s, 2),
            obj: ORIGIN,
            val: ts(s, 1),
        });
        let view2 = roundtrip_view(&model);
        assert_eq!(view2, json!(42));
    }

    #[test]
    fn to_schema_str_node() {
        let mut model = Model::new(sid());
        let s = sid();
        model.apply_operation(&Op::NewStr { id: ts(s, 1) });
        model.apply_operation(&Op::InsStr {
            id: ts(s, 2),
            obj: ts(s, 1),
            after: ORIGIN,
            data: "hello".to_string(),
        });
        model.apply_operation(&Op::InsVal {
            id: ts(s, 7),
            obj: ORIGIN,
            val: ts(s, 1),
        });
        let view2 = roundtrip_view(&model);
        assert_eq!(view2, json!("hello"));
    }

    #[test]
    fn to_schema_obj_node() {
        let mut model = Model::new(sid());
        let s = sid();
        model.apply_operation(&Op::NewObj { id: ts(s, 1) });
        model.apply_operation(&Op::NewCon {
            id: ts(s, 2),
            val: ConValue::Val(PackValue::Integer(99)),
        });
        model.apply_operation(&Op::InsObj {
            id: ts(s, 3),
            obj: ts(s, 1),
            data: vec![("key".to_string(), ts(s, 2))],
        });
        model.apply_operation(&Op::InsVal {
            id: ts(s, 4),
            obj: ORIGIN,
            val: ts(s, 1),
        });
        let view2 = roundtrip_view(&model);
        assert_eq!(view2, json!({ "key": 99 }));
    }

    #[test]
    fn to_schema_bin_node() {
        let mut model = Model::new(sid());
        let s = sid();
        model.apply_operation(&Op::NewBin { id: ts(s, 1) });
        model.apply_operation(&Op::InsBin {
            id: ts(s, 2),
            obj: ts(s, 1),
            after: ORIGIN,
            data: vec![1, 2, 3],
        });
        model.apply_operation(&Op::InsVal {
            id: ts(s, 4),
            obj: ORIGIN,
            val: ts(s, 1),
        });
        let view2 = roundtrip_view(&model);
        assert_eq!(view2, json!([1, 2, 3]));
    }

    #[test]
    fn to_schema_vec_node() {
        let mut model = Model::new(sid());
        let s = sid();
        model.apply_operation(&Op::NewVec { id: ts(s, 1) });
        model.apply_operation(&Op::NewCon {
            id: ts(s, 2),
            val: ConValue::Val(PackValue::Bool(true)),
        });
        model.apply_operation(&Op::InsVec {
            id: ts(s, 3),
            obj: ts(s, 1),
            data: vec![(0u8, ts(s, 2))],
        });
        model.apply_operation(&Op::InsVal {
            id: ts(s, 4),
            obj: ORIGIN,
            val: ts(s, 1),
        });
        let view2 = roundtrip_view(&model);
        assert_eq!(view2, json!([true]));
    }

    #[test]
    fn to_schema_arr_node() {
        let mut model = Model::new(sid());
        let s = sid();
        model.apply_operation(&Op::NewArr { id: ts(s, 1) });
        model.apply_operation(&Op::NewCon {
            id: ts(s, 2),
            val: ConValue::Val(PackValue::Integer(10)),
        });
        model.apply_operation(&Op::NewCon {
            id: ts(s, 3),
            val: ConValue::Val(PackValue::Integer(20)),
        });
        model.apply_operation(&Op::InsArr {
            id: ts(s, 4),
            obj: ts(s, 1),
            after: ORIGIN,
            data: vec![ts(s, 2), ts(s, 3)],
        });
        model.apply_operation(&Op::InsVal {
            id: ts(s, 6),
            obj: ORIGIN,
            val: ts(s, 1),
        });
        let view2 = roundtrip_view(&model);
        assert_eq!(view2, json!([10, 20]));
    }
}
