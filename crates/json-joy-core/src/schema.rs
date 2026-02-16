//! Native baseline port of `json-crdt-patch/schema.ts`.

use serde_json::Value;

use crate::patch::{ConValue, DecodedOp, Patch, Timestamp};
use crate::patch_builder::{encode_patch_from_ops, PatchBuildError};

#[derive(Debug, Clone, PartialEq)]
pub enum SchemaNode {
    Con(ConValue),
    Str(String),
    Bin(Vec<u8>),
    Val(Box<SchemaNode>),
    Vec(Vec<Option<SchemaNode>>),
    Obj {
        req: Vec<(String, SchemaNode)>,
        opt: Vec<(String, SchemaNode)>,
    },
    Arr(Vec<SchemaNode>),
    Ext {
        id: u8,
        data: Box<SchemaNode>,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum SchemaError {
    #[error("schema build failed: {0}")]
    Build(#[from] PatchBuildError),
    #[error("schema patch decode failed: {0}")]
    PatchDecode(#[from] crate::patch::PatchError),
}

#[derive(Debug)]
struct SchemaPatchBuilder {
    sid: u64,
    next_time: u64,
    ops: Vec<DecodedOp>,
}

impl SchemaPatchBuilder {
    fn new(sid: u64, time: u64) -> Self {
        Self {
            sid,
            next_time: time,
            ops: Vec::new(),
        }
    }

    fn alloc_op_id(&mut self, span: u64) -> Timestamp {
        let id = Timestamp {
            sid: self.sid,
            time: self.next_time,
        };
        self.next_time = self.next_time.saturating_add(span);
        id
    }

    fn push(&mut self, op: DecodedOp) {
        self.ops.push(op);
    }

    fn build_node(&mut self, node: &SchemaNode) -> Timestamp {
        match node {
            SchemaNode::Con(value) => {
                let id = self.alloc_op_id(1);
                self.push(DecodedOp::NewCon {
                    id,
                    value: value.clone(),
                });
                id
            }
            SchemaNode::Str(raw) => {
                let str_id = self.alloc_op_id(1);
                self.push(DecodedOp::NewStr { id: str_id });
                if !raw.is_empty() {
                    // json-joy string op spans advance by JS string length
                    // (UTF-16 code units), not Unicode scalar count.
                    let ins_id = self.alloc_op_id(raw.encode_utf16().count() as u64);
                    self.push(DecodedOp::InsStr {
                        id: ins_id,
                        obj: str_id,
                        reference: str_id,
                        data: raw.clone(),
                    });
                }
                str_id
            }
            SchemaNode::Bin(raw) => {
                let bin_id = self.alloc_op_id(1);
                self.push(DecodedOp::NewBin { id: bin_id });
                if !raw.is_empty() {
                    let ins_id = self.alloc_op_id(raw.len() as u64);
                    self.push(DecodedOp::InsBin {
                        id: ins_id,
                        obj: bin_id,
                        reference: bin_id,
                        data: raw.clone(),
                    });
                }
                bin_id
            }
            SchemaNode::Val(inner) => {
                let val_id = self.alloc_op_id(1);
                self.push(DecodedOp::NewVal { id: val_id });
                let child = self.build_node(inner);
                let ins_id = self.alloc_op_id(1);
                self.push(DecodedOp::InsVal {
                    id: ins_id,
                    obj: val_id,
                    val: child,
                });
                val_id
            }
            SchemaNode::Vec(items) => {
                let vec_id = self.alloc_op_id(1);
                self.push(DecodedOp::NewVec { id: vec_id });
                let mut data = Vec::new();
                for (i, item) in items.iter().enumerate() {
                    if let Some(item) = item {
                        let id = self.build_node(item);
                        data.push((i as u64, id));
                    }
                }
                if !data.is_empty() {
                    let ins_id = self.alloc_op_id(1);
                    self.push(DecodedOp::InsVec {
                        id: ins_id,
                        obj: vec_id,
                        data,
                    });
                }
                vec_id
            }
            SchemaNode::Obj { req, opt } => {
                let obj_id = self.alloc_op_id(1);
                self.push(DecodedOp::NewObj { id: obj_id });
                let mut data = Vec::new();
                for (k, v) in req.iter().chain(opt.iter()) {
                    data.push((k.clone(), self.build_node(v)));
                }
                if !data.is_empty() {
                    let ins_id = self.alloc_op_id(1);
                    self.push(DecodedOp::InsObj {
                        id: ins_id,
                        obj: obj_id,
                        data,
                    });
                }
                obj_id
            }
            SchemaNode::Arr(items) => {
                let arr_id = self.alloc_op_id(1);
                self.push(DecodedOp::NewArr { id: arr_id });
                if !items.is_empty() {
                    let data = items.iter().map(|v| self.build_node(v)).collect::<Vec<_>>();
                    let ins_id = self.alloc_op_id(data.len() as u64);
                    self.push(DecodedOp::InsArr {
                        id: ins_id,
                        obj: arr_id,
                        reference: arr_id,
                        data,
                    });
                }
                arr_id
            }
            SchemaNode::Ext { id, data } => {
                let tuple_id = self.alloc_op_id(1);
                self.push(DecodedOp::NewVec { id: tuple_id });
                let header = SchemaNode::Con(ConValue::Json(Value::Array(vec![
                    Value::from(*id),
                    Value::from(tuple_id.sid % 256),
                    Value::from(tuple_id.time % 256),
                ])));
                let header_id = self.build_node(&header);
                let data_id = self.build_node(data);
                let ins_id = self.alloc_op_id(1);
                self.push(DecodedOp::InsVec {
                    id: ins_id,
                    obj: tuple_id,
                    data: vec![(0, header_id), (1, data_id)],
                });
                tuple_id
            }
        }
    }
}

impl SchemaNode {
    pub fn to_patch(&self, sid: u64, time: u64) -> Result<Patch, SchemaError> {
        let mut b = SchemaPatchBuilder::new(sid, time);
        let root = b.build_node(self);
        let set_root_id = b.alloc_op_id(1);
        b.push(DecodedOp::InsVal {
            id: set_root_id,
            obj: Timestamp { sid: 0, time: 0 },
            val: root,
        });
        let bytes = encode_patch_from_ops(sid, time, &b.ops)?;
        Ok(Patch::from_binary(&bytes)?)
    }
}

pub fn con_json(value: Value) -> SchemaNode {
    SchemaNode::Con(ConValue::Json(value))
}

pub fn con_undef() -> SchemaNode {
    SchemaNode::Con(ConValue::Undef)
}

pub fn con_ref(ts: Timestamp) -> SchemaNode {
    SchemaNode::Con(ConValue::Ref(ts))
}

pub fn str_node(value: impl Into<String>) -> SchemaNode {
    SchemaNode::Str(value.into())
}

pub fn bin_node(value: impl Into<Vec<u8>>) -> SchemaNode {
    SchemaNode::Bin(value.into())
}

pub fn val_node(value: SchemaNode) -> SchemaNode {
    SchemaNode::Val(Box::new(value))
}

pub fn vec_node(values: Vec<Option<SchemaNode>>) -> SchemaNode {
    SchemaNode::Vec(values)
}

pub fn obj_node(req: Vec<(String, SchemaNode)>, opt: Vec<(String, SchemaNode)>) -> SchemaNode {
    SchemaNode::Obj {
        req,
        opt,
    }
}

pub fn arr_node(values: Vec<SchemaNode>) -> SchemaNode {
    SchemaNode::Arr(values)
}

pub fn ext_node(id: u8, data: SchemaNode) -> SchemaNode {
    SchemaNode::Ext {
        id,
        data: Box::new(data),
    }
}

pub fn json(value: &Value) -> SchemaNode {
    match value {
        Value::Null => val_node(con_json(Value::Null)),
        Value::Bool(b) => val_node(con_json(Value::Bool(*b))),
        Value::Number(n) => val_node(con_json(Value::Number(n.clone()))),
        Value::String(s) => str_node(s.clone()),
        Value::Array(arr) => arr_node(arr.iter().map(json).collect()),
        Value::Object(map) => obj_node(
            map.iter().map(|(k, v)| (k.clone(), json_con(v))).collect::<Vec<_>>(),
            Vec::<(String, SchemaNode)>::new(),
        ),
    }
}

pub fn json_con(value: &Value) -> SchemaNode {
    match value {
        Value::Null | Value::Bool(_) | Value::Number(_) => con_json(value.clone()),
        _ => json(value),
    }
}
