//! TypeBuilder — factory for constructing TypeNode instances.
//!
//! Upstream reference: json-type/src/type/TypeBuilder.ts

use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

use super::classes::*;
use super::module_type::ModuleType;
use super::TypeNode;
use crate::schema::Schema;

/// Factory for constructing TypeNode instances.
///
/// Mirrors the TypeScript `TypeBuilder` class.
#[derive(Debug, Clone, Default)]
pub struct TypeBuilder {
    pub system: Option<Arc<ModuleType>>,
}

#[allow(non_snake_case)]
impl TypeBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_system(system: Arc<ModuleType>) -> Self {
        Self {
            system: Some(system),
        }
    }

    fn sys(&self) -> Option<Arc<ModuleType>> {
        self.system.clone()
    }

    // ------------------------------------------------------------------
    // Shorthand getters

    pub fn any(&self) -> TypeNode {
        self.Any(None)
    }

    pub fn bool(&self) -> TypeNode {
        self.Boolean(None)
    }

    pub fn num(&self) -> TypeNode {
        self.Number(None)
    }

    pub fn str(&self) -> TypeNode {
        self.String(None)
    }

    pub fn bin(&self) -> TypeNode {
        self.Binary(self.any(), None)
    }

    pub fn arr(&self) -> TypeNode {
        self.Array(self.any(), None)
    }

    pub fn obj(&self) -> TypeNode {
        self.Object(vec![])
    }

    pub fn map(&self) -> TypeNode {
        self.Map(self.any(), None, None)
    }

    pub fn undef(&self) -> TypeNode {
        self.Const(Value::Null, None)
    }

    pub fn nil(&self) -> TypeNode {
        self.Const(Value::Null, None)
    }

    pub fn fn_(&self) -> TypeNode {
        self.Function(self.undef(), self.undef(), None)
    }

    pub fn fn_rx(&self) -> TypeNode {
        self.function_streaming(self.undef(), self.undef(), None)
    }

    // ------------------------------------------------------------------
    // Factory methods

    pub fn Any(&self, _opts: Option<()>) -> TypeNode {
        TypeNode::Any(AnyType::new().sys(self.sys()))
    }

    pub fn Boolean(&self, _opts: Option<()>) -> TypeNode {
        TypeNode::Bool(BoolType::new().sys(self.sys()))
    }

    pub fn Number(&self, _opts: Option<()>) -> TypeNode {
        TypeNode::Num(NumType::new().sys(self.sys()))
    }

    pub fn String(&self, _opts: Option<()>) -> TypeNode {
        TypeNode::Str(StrType::new().sys(self.sys()))
    }

    pub fn Binary(&self, type_: TypeNode, _opts: Option<()>) -> TypeNode {
        TypeNode::Bin(BinType::new(type_).sys(self.sys()))
    }

    pub fn Array(&self, type_: TypeNode, _opts: Option<()>) -> TypeNode {
        TypeNode::Arr(ArrType::new(Some(type_), vec![], vec![]).sys(self.sys()))
    }

    pub fn Tuple(
        &self,
        head: Vec<TypeNode>,
        type_: Option<TypeNode>,
        tail: Option<Vec<TypeNode>>,
    ) -> TypeNode {
        TypeNode::Arr(ArrType::new(type_, head, tail.unwrap_or_default()).sys(self.sys()))
    }

    pub fn Object(&self, keys: Vec<KeyType>) -> TypeNode {
        TypeNode::Obj(ObjType::new(keys).sys(self.sys()))
    }

    pub fn Key(&self, key: impl Into<String>, value: TypeNode) -> TypeNode {
        TypeNode::Key(KeyType::new(key, value).sys(self.sys()))
    }

    pub fn KeyOpt(&self, key: impl Into<String>, value: TypeNode) -> TypeNode {
        TypeNode::Key(KeyType::new_opt(key, value).sys(self.sys()))
    }

    pub fn Map(&self, val: TypeNode, key: Option<TypeNode>, _opts: Option<()>) -> TypeNode {
        TypeNode::Map(MapType::new(val, key).sys(self.sys()))
    }

    pub fn Or(&self, types: Vec<TypeNode>) -> TypeNode {
        TypeNode::Or(OrType::new(types).sys(self.sys()))
    }

    pub fn Ref(&self, ref_: impl Into<String>) -> TypeNode {
        TypeNode::Ref(RefType::new(ref_).sys(self.sys()))
    }

    pub fn Const(&self, value: Value, _opts: Option<()>) -> TypeNode {
        TypeNode::Con(ConType::new(value).sys(self.sys()))
    }

    pub fn Function(&self, req: TypeNode, res: TypeNode, _opts: Option<()>) -> TypeNode {
        TypeNode::Fn(FnType::new(req, res).sys(self.sys()))
    }

    pub fn function_streaming(&self, req: TypeNode, res: TypeNode, _opts: Option<()>) -> TypeNode {
        TypeNode::FnRx(FnRxType::new(req, res).sys(self.sys()))
    }

    // ------------------------------------------------------------------
    // Higher-level helpers

    /// Create a union type from a list of const values.
    pub fn enum_<T: Into<Value> + Clone>(&self, values: Vec<T>) -> TypeNode {
        let types = values
            .into_iter()
            .map(|v| self.Const(v.into(), None))
            .collect();
        self.Or(types)
    }

    /// Create an "optional" union (T | undefined).
    pub fn maybe(&self, type_: TypeNode) -> TypeNode {
        self.Or(vec![type_, self.undef()])
    }

    /// Create an object type from a `key → TypeNode` map.
    pub fn object(&self, record: HashMap<String, TypeNode>) -> TypeNode {
        let mut keys: Vec<_> = record.into_iter().collect();
        keys.sort_by(|a, b| a.0.cmp(&b.0));
        let key_types: Vec<KeyType> = keys.into_iter().map(|(k, v)| KeyType::new(k, v)).collect();
        self.Object(key_types)
    }

    /// Create a tuple type from a list of element types.
    pub fn tuple(&self, types: Vec<TypeNode>) -> TypeNode {
        self.Tuple(types, None, None)
    }

    /// Import a Schema into a TypeNode.
    pub fn import(&self, schema: &Schema) -> TypeNode {
        match schema {
            Schema::Any(_) => self.Any(None),
            Schema::Bool(_) => self.Boolean(None),
            Schema::Num(s) => {
                let mut n = NumType::new().sys(self.sys());
                n.schema = s.clone();
                TypeNode::Num(n)
            }
            Schema::Str(s) => {
                let mut st = StrType::new().sys(self.sys());
                st.schema = s.clone();
                TypeNode::Str(st)
            }
            Schema::Bin(s) => {
                let inner = self.import(&s.type_);
                TypeNode::Bin(BinType::new(inner).sys(self.sys()))
            }
            Schema::Con(s) => self.Const(s.value.clone(), None),
            Schema::Arr(s) => {
                let head: Vec<TypeNode> = s
                    .head
                    .as_deref()
                    .unwrap_or(&[])
                    .iter()
                    .map(|h| self.import(h))
                    .collect();
                let type_ = s.type_.as_deref().map(|t| self.import(t));
                let tail: Vec<TypeNode> = s
                    .tail
                    .as_deref()
                    .unwrap_or(&[])
                    .iter()
                    .map(|t| self.import(t))
                    .collect();
                let mut arr = ArrType::new(type_, head, tail);
                arr.schema.min = s.min;
                arr.schema.max = s.max;
                TypeNode::Arr(arr.sys(self.sys()))
            }
            Schema::Obj(s) => {
                let keys: Vec<KeyType> = s
                    .keys
                    .iter()
                    .map(|k| {
                        let val = self.import(&k.value);
                        if k.optional == Some(true) {
                            KeyType::new_opt(k.key.clone(), val)
                        } else {
                            KeyType::new(k.key.clone(), val)
                        }
                    })
                    .collect();
                let mut obj = ObjType::new(keys).sys(self.sys());
                obj.schema.decode_unknown_keys = s.decode_unknown_keys;
                obj.schema.encode_unknown_keys = s.encode_unknown_keys;
                TypeNode::Obj(obj)
            }
            Schema::Key(s) => {
                let val = self.import(&s.value);
                if s.optional == Some(true) {
                    TypeNode::Key(KeyType::new_opt(s.key.clone(), val).sys(self.sys()))
                } else {
                    TypeNode::Key(KeyType::new(s.key.clone(), val).sys(self.sys()))
                }
            }
            Schema::Map(s) => {
                let val = self.import(&s.value);
                let key = s.key.as_deref().map(|k| self.import(k));
                self.Map(val, key, None)
            }
            Schema::Ref(s) => self.Ref(s.ref_.clone()),
            Schema::Or(s) => {
                let types: Vec<TypeNode> = s.types.iter().map(|t| self.import(t)).collect();
                TypeNode::Or(OrType::new(types).sys(self.sys()))
            }
            Schema::Fn(s) => {
                let req = self.import(&s.req);
                let res = self.import(&s.res);
                self.Function(req, res, None)
            }
            Schema::FnRx(s) => {
                let req = self.import(&s.req);
                let res = self.import(&s.res);
                self.function_streaming(req, res, None)
            }
            Schema::Alias(s) => self.import(&s.value),
            Schema::Module(_) => {
                // Modules are not directly representable as a TypeNode
                self.Any(None)
            }
        }
    }

    /// Infer a TypeNode from a JSON value.
    pub fn from_value(&self, value: &Value) -> TypeNode {
        match value {
            Value::Null => self.nil(),
            Value::Bool(_) => self.bool(),
            Value::Number(_) => self.num(),
            Value::String(_) => self.str(),
            Value::Array(arr) => {
                if arr.is_empty() {
                    return self.arr();
                }
                let first_type = self.from_value(&arr[0]);
                let first_kind = first_type.kind().to_string();
                let all_same = arr.iter().all(|v| self.from_value(v).kind() == first_kind);
                if all_same {
                    self.Array(first_type, None)
                } else {
                    let types = arr.iter().map(|v| self.from_value(v)).collect();
                    self.tuple(types)
                }
            }
            Value::Object(map) => {
                let keys: Vec<KeyType> = map
                    .iter()
                    .map(|(k, v)| KeyType::new(k.clone(), self.from_value(v)))
                    .collect();
                self.Object(keys)
            }
        }
    }
}
