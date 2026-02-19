//! Type class implementations.
//!
//! Each struct here corresponds to a TypeScript class extending AbsType<S>.

use serde_json::Value;
use std::sync::Arc;

use super::abs_type::BaseInfo;
use super::module_type::ModuleType;
use crate::schema::*;

// -------------------------------------------------------------------------
// AnyType

#[derive(Debug, Clone, Default)]
pub struct AnyType {
    pub base: BaseInfo,
}

impl AnyType {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn sys(mut self, system: Option<Arc<ModuleType>>) -> Self {
        self.base.system = system;
        self
    }
    pub fn get_schema(&self) -> Schema {
        Schema::Any(AnySchema {
            base: SchemaBase::default(),
        })
    }
    pub fn kind(&self) -> &'static str {
        "any"
    }
}

// -------------------------------------------------------------------------
// BoolType

#[derive(Debug, Clone, Default)]
pub struct BoolType {
    pub base: BaseInfo,
}

impl BoolType {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn sys(mut self, system: Option<Arc<ModuleType>>) -> Self {
        self.base.system = system;
        self
    }
    pub fn get_schema(&self) -> Schema {
        Schema::Bool(BoolSchema {
            base: SchemaBase::default(),
        })
    }
    pub fn kind(&self) -> &'static str {
        "bool"
    }
}

// -------------------------------------------------------------------------
// NumType

#[derive(Debug, Clone, Default)]
pub struct NumType {
    pub schema: NumSchema,
    pub base: BaseInfo,
}

impl NumType {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn sys(mut self, system: Option<Arc<ModuleType>>) -> Self {
        self.base.system = system;
        self
    }
    pub fn format(mut self, format: NumFormat) -> Self {
        self.schema.format = Some(format);
        self
    }
    pub fn gt(mut self, v: f64) -> Self {
        self.schema.gt = Some(v);
        self
    }
    pub fn gte(mut self, v: f64) -> Self {
        self.schema.gte = Some(v);
        self
    }
    pub fn lt(mut self, v: f64) -> Self {
        self.schema.lt = Some(v);
        self
    }
    pub fn lte(mut self, v: f64) -> Self {
        self.schema.lte = Some(v);
        self
    }
    pub fn get_schema(&self) -> Schema {
        Schema::Num(self.schema.clone())
    }
    pub fn kind(&self) -> &'static str {
        "num"
    }
}

// -------------------------------------------------------------------------
// StrType

#[derive(Debug, Clone, Default)]
pub struct StrType {
    pub schema: StrSchema,
    pub base: BaseInfo,
}

impl StrType {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn sys(mut self, system: Option<Arc<ModuleType>>) -> Self {
        self.base.system = system;
        self
    }
    pub fn format(mut self, format: StrFormat) -> Self {
        self.schema.format = Some(format);
        self
    }
    pub fn min(mut self, v: u64) -> Self {
        self.schema.min = Some(v);
        self
    }
    pub fn max(mut self, v: u64) -> Self {
        self.schema.max = Some(v);
        self
    }
    pub fn ascii(mut self, v: bool) -> Self {
        self.schema.ascii = Some(v);
        self
    }
    pub fn get_schema(&self) -> Schema {
        Schema::Str(self.schema.clone())
    }
    pub fn kind(&self) -> &'static str {
        "str"
    }
}

// -------------------------------------------------------------------------
// BinType

#[derive(Debug, Clone)]
pub struct BinType {
    pub inner_type: Box<super::TypeNode>,
    pub schema: BinSchema,
    pub base: BaseInfo,
}

impl BinType {
    pub fn new(inner_type: super::TypeNode) -> Self {
        let schema = BinSchema {
            base: SchemaBase::default(),
            type_: Box::new(inner_type.get_schema()),
            format: None,
            min: None,
            max: None,
        };
        Self {
            inner_type: Box::new(inner_type),
            schema,
            base: BaseInfo::default(),
        }
    }
    pub fn sys(mut self, system: Option<Arc<ModuleType>>) -> Self {
        self.base.system = system;
        self
    }
    pub fn min(mut self, v: u64) -> Self {
        self.schema.min = Some(v);
        self
    }
    pub fn max(mut self, v: u64) -> Self {
        self.schema.max = Some(v);
        self
    }
    pub fn get_schema(&self) -> Schema {
        Schema::Bin(BinSchema {
            base: SchemaBase::default(),
            type_: Box::new(self.inner_type.get_schema()),
            format: self.schema.format,
            min: self.schema.min,
            max: self.schema.max,
        })
    }
    pub fn kind(&self) -> &'static str {
        "bin"
    }
}

// -------------------------------------------------------------------------
// ConType

#[derive(Debug, Clone)]
pub struct ConType {
    pub value: Value,
    pub base: BaseInfo,
}

impl ConType {
    pub fn new(value: Value) -> Self {
        Self {
            value,
            base: BaseInfo::default(),
        }
    }
    pub fn sys(mut self, system: Option<Arc<ModuleType>>) -> Self {
        self.base.system = system;
        self
    }
    pub fn literal(&self) -> &Value {
        &self.value
    }
    pub fn get_schema(&self) -> Schema {
        Schema::Con(ConSchema {
            base: SchemaBase::default(),
            value: self.value.clone(),
        })
    }
    pub fn kind(&self) -> &'static str {
        "con"
    }
}

// -------------------------------------------------------------------------
// ArrType

#[derive(Debug, Clone, Default)]
pub struct ArrType {
    pub type_: Option<Box<super::TypeNode>>,
    pub head: Vec<super::TypeNode>,
    pub tail: Vec<super::TypeNode>,
    pub schema: ArrSchema,
    pub base: BaseInfo,
}

impl ArrType {
    pub fn new(
        type_: Option<super::TypeNode>,
        head: Vec<super::TypeNode>,
        tail: Vec<super::TypeNode>,
    ) -> Self {
        Self {
            type_: type_.map(Box::new),
            head,
            tail,
            schema: ArrSchema::default(),
            base: BaseInfo::default(),
        }
    }
    pub fn sys(mut self, system: Option<Arc<ModuleType>>) -> Self {
        self.base.system = system;
        self
    }
    pub fn min(mut self, v: u64) -> Self {
        self.schema.min = Some(v);
        self
    }
    pub fn max(mut self, v: u64) -> Self {
        self.schema.max = Some(v);
        self
    }
    pub fn get_schema(&self) -> Schema {
        let mut arr = self.schema.clone();
        if let Some(t) = &self.type_ {
            arr.type_ = Some(Box::new(t.get_schema()));
        }
        if !self.head.is_empty() {
            arr.head = Some(self.head.iter().map(|t| t.get_schema()).collect());
        }
        if !self.tail.is_empty() {
            arr.tail = Some(self.tail.iter().map(|t| t.get_schema()).collect());
        }
        Schema::Arr(arr)
    }
    pub fn kind(&self) -> &'static str {
        "arr"
    }
}

// -------------------------------------------------------------------------
// KeyType / KeyOptType

#[derive(Debug, Clone)]
pub struct KeyType {
    pub key: String,
    pub val: Box<super::TypeNode>,
    pub optional: bool,
    pub base: BaseInfo,
}

impl KeyType {
    pub fn new(key: impl Into<String>, val: super::TypeNode) -> Self {
        Self {
            key: key.into(),
            val: Box::new(val),
            optional: false,
            base: BaseInfo::default(),
        }
    }
    pub fn new_opt(key: impl Into<String>, val: super::TypeNode) -> Self {
        Self {
            key: key.into(),
            val: Box::new(val),
            optional: true,
            base: BaseInfo::default(),
        }
    }
    pub fn sys(mut self, system: Option<Arc<ModuleType>>) -> Self {
        self.base.system = system;
        self
    }
    pub fn get_schema(&self) -> Schema {
        Schema::Key(KeySchema {
            base: SchemaBase::default(),
            key: self.key.clone(),
            value: Box::new(self.val.get_schema()),
            optional: if self.optional { Some(true) } else { None },
        })
    }
    pub fn kind(&self) -> &'static str {
        "key"
    }
}

// -------------------------------------------------------------------------
// ObjType

#[derive(Debug, Clone, Default)]
pub struct ObjType {
    pub keys: Vec<KeyType>,
    pub schema: ObjSchema,
    pub base: BaseInfo,
}

impl ObjType {
    pub fn new(keys: Vec<KeyType>) -> Self {
        Self {
            keys,
            schema: ObjSchema::default(),
            base: BaseInfo::default(),
        }
    }
    pub fn sys(mut self, system: Option<Arc<ModuleType>>) -> Self {
        self.base.system = system;
        self
    }
    pub fn prop(mut self, key: impl Into<String>, val: super::TypeNode) -> Self {
        self.keys.push(KeyType::new(key, val));
        self
    }
    pub fn opt(mut self, key: impl Into<String>, val: super::TypeNode) -> Self {
        self.keys.push(KeyType::new_opt(key, val));
        self
    }
    pub fn extend(mut self, other: ObjType) -> Self {
        self.keys.extend(other.keys);
        self
    }
    pub fn omit(mut self, key: &str) -> Self {
        self.keys.retain(|k| k.key != key);
        self
    }
    pub fn get_field(&self, key: &str) -> Option<&KeyType> {
        self.keys.iter().find(|k| k.key == key)
    }
    pub fn get_schema(&self) -> Schema {
        let mut obj = self.schema.clone();
        obj.keys = self
            .keys
            .iter()
            .map(|k| KeySchema {
                base: SchemaBase::default(),
                key: k.key.clone(),
                value: Box::new(k.val.get_schema()),
                optional: if k.optional { Some(true) } else { None },
            })
            .collect();
        Schema::Obj(obj)
    }
    pub fn kind(&self) -> &'static str {
        "obj"
    }
}

// -------------------------------------------------------------------------
// MapType

#[derive(Debug, Clone)]
pub struct MapType {
    pub value: Box<super::TypeNode>,
    pub key: Option<Box<super::TypeNode>>,
    pub base: BaseInfo,
}

impl MapType {
    pub fn new(value: super::TypeNode, key: Option<super::TypeNode>) -> Self {
        Self {
            value: Box::new(value),
            key: key.map(Box::new),
            base: BaseInfo::default(),
        }
    }
    pub fn sys(mut self, system: Option<Arc<ModuleType>>) -> Self {
        self.base.system = system;
        self
    }
    pub fn get_schema(&self) -> Schema {
        Schema::Map(MapSchema {
            base: SchemaBase::default(),
            key: self.key.as_ref().map(|k| Box::new(k.get_schema())),
            value: Box::new(self.value.get_schema()),
        })
    }
    pub fn kind(&self) -> &'static str {
        "map"
    }
}

// -------------------------------------------------------------------------
// RefType

#[derive(Debug, Clone)]
pub struct RefType {
    pub ref_: String,
    pub base: BaseInfo,
}

impl RefType {
    pub fn new(ref_: impl Into<String>) -> Self {
        Self {
            ref_: ref_.into(),
            base: BaseInfo::default(),
        }
    }
    pub fn sys(mut self, system: Option<Arc<ModuleType>>) -> Self {
        self.base.system = system;
        self
    }
    pub fn ref_name(&self) -> &str {
        &self.ref_
    }
    pub fn get_schema(&self) -> Schema {
        Schema::Ref(RefSchema {
            base: SchemaBase::default(),
            ref_: self.ref_.clone(),
        })
    }
    pub fn kind(&self) -> &'static str {
        "ref"
    }
}

// -------------------------------------------------------------------------
// OrType

#[derive(Debug, Clone)]
pub struct OrType {
    pub types: Vec<super::TypeNode>,
    pub discriminator: Value,
    pub base: BaseInfo,
}

impl OrType {
    pub fn new(types: Vec<super::TypeNode>) -> Self {
        let discriminator = compute_discriminator(&types);
        Self {
            types,
            discriminator,
            base: BaseInfo::default(),
        }
    }
    pub fn sys(mut self, system: Option<Arc<ModuleType>>) -> Self {
        self.base.system = system;
        self
    }
    pub fn get_schema(&self) -> Schema {
        Schema::Or(OrSchema {
            base: SchemaBase::default(),
            types: self.types.iter().map(|t| t.get_schema()).collect(),
            discriminator: self.discriminator.clone(),
        })
    }
    pub fn kind(&self) -> &'static str {
        "or"
    }
}

fn compute_discriminator(_types: &[super::TypeNode]) -> Value {
    // Default: unresolved discriminator (will be computed at runtime if needed)
    serde_json::json!(["num", -1])
}

// -------------------------------------------------------------------------
// FnType / FnRxType

#[derive(Debug, Clone)]
pub struct FnType {
    pub req: Box<super::TypeNode>,
    pub res: Box<super::TypeNode>,
    pub base: BaseInfo,
}

impl FnType {
    pub fn new(req: super::TypeNode, res: super::TypeNode) -> Self {
        Self {
            req: Box::new(req),
            res: Box::new(res),
            base: BaseInfo::default(),
        }
    }
    pub fn sys(mut self, system: Option<Arc<ModuleType>>) -> Self {
        self.base.system = system;
        self
    }
    pub fn get_schema(&self) -> Schema {
        Schema::Fn(FnSchema {
            base: SchemaBase::default(),
            req: Box::new(self.req.get_schema()),
            res: Box::new(self.res.get_schema()),
        })
    }
    pub fn kind(&self) -> &'static str {
        "fn"
    }
}

#[derive(Debug, Clone)]
pub struct FnRxType {
    pub req: Box<super::TypeNode>,
    pub res: Box<super::TypeNode>,
    pub base: BaseInfo,
}

impl FnRxType {
    pub fn new(req: super::TypeNode, res: super::TypeNode) -> Self {
        Self {
            req: Box::new(req),
            res: Box::new(res),
            base: BaseInfo::default(),
        }
    }
    pub fn sys(mut self, system: Option<Arc<ModuleType>>) -> Self {
        self.base.system = system;
        self
    }
    pub fn get_schema(&self) -> Schema {
        Schema::FnRx(FnRxSchema {
            base: SchemaBase::default(),
            req: Box::new(self.req.get_schema()),
            res: Box::new(self.res.get_schema()),
        })
    }
    pub fn kind(&self) -> &'static str {
        "fn$"
    }
}

// -------------------------------------------------------------------------
// AliasType

#[derive(Debug, Clone)]
pub struct AliasType {
    pub id: String,
    pub type_: Box<super::TypeNode>,
    pub system: Arc<ModuleType>,
    pub base: BaseInfo,
}

impl AliasType {
    pub fn new(system: Arc<ModuleType>, id: impl Into<String>, type_: super::TypeNode) -> Self {
        Self {
            id: id.into(),
            type_: Box::new(type_),
            system,
            base: BaseInfo::default(),
        }
    }
    pub fn get_type(&self) -> &super::TypeNode {
        &self.type_
    }
    pub fn get_schema(&self) -> Schema {
        self.type_.get_schema()
    }
    pub fn kind(&self) -> &'static str {
        "alias"
    }
}
